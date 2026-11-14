#!/usr/bin/env python3
"""Independent Redfish clients and DMTF validators; no secrets in reports.

Install redfish-interop-requirements.txt in an ignored target/ virtualenv.
Supply REDFISH_VALIDATION_PASSWORD in the environment, never in argv.
Basic/session clients create and delete sessions but do not mutate VM state.
--protocol-disposable explicitly enables the upstream protocol suite, which
does perform mutations: use only a disposable isolated fixture.
"""

import argparse
import contextlib
import functools
import importlib.metadata
import io
import json
import logging
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
from unittest.mock import patch
from urllib.parse import urlsplit


def ca_bundle(args):
    import requests
    return str(args.ca_bundle or os.environ.get("REQUESTS_CA_BUNDLE") or requests.certs.where())


def clients(args):
    import redfish
    import sushy
    from sushy import auth

    results = []
    for mode in ("basic", "session"):
        client = None
        row = {"client": "DMTF Python redfish", "auth": mode}
        try:
            client = redfish.redfish_client(
                base_url=args.base_url, username=args.username,
                password=os.environ["REDFISH_VALIDATION_PASSWORD"],
                timeout=5, max_retry=0, cafile=ca_bundle(args),
            )
            client.login(auth=mode)
            paths = ("/redfish/v1/", "/redfish/v1/Systems", "/redfish/v1/Systems/1",
                     "/redfish/v1/Managers", "/redfish/v1/Managers/1")
            row["statuses"] = {path: client.get(path).status for path in paths}
            row["passed"] = all(status == 200 for status in row["statuses"].values())
        except Exception as exc:
            row.update(passed=False, exception=type(exc).__name__)
        finally:
            if client is not None:
                try:
                    client.logout()
                except Exception as exc:
                    row["logout_exception"] = type(exc).__name__
                    row["passed"] = False
        results.append(row)

    for mode, mechanism in (("basic", auth.BasicAuth), ("session", auth.SessionAuth)):
        time.sleep(2)  # Respect the service's bounded expensive-password budget.
        authentication = mechanism(
            username=args.username, password=os.environ["REDFISH_VALIDATION_PASSWORD"],
        )
        client = None
        row = {"client": "OpenStack Sushy", "auth": mode}
        try:
            client = sushy.Sushy(
                args.base_url, auth=authentication, verify=ca_bundle(args), server_side_retries=0,
                read_timeout=5, connect_timeout=5,
            )
            systems = client.get_system_collection().get_members()
            managers = client.get_manager_collection().get_members()
            row.update(
                passed=bool(systems) and bool(managers),
                redfish_version=client.redfish_version,
                systems=[{"identity": system.identity,
                          "power_state": str(system.power_state),
                          "reset_types": sorted(str(x) for x in system.get_allowed_reset_system_values())}
                         for system in systems],
                manager_count=len(managers),
            )
        except Exception as exc:
            row.update(passed=False, exception=type(exc).__name__)
        finally:
            try:
                authentication.close()
                if client is not None:
                    client._conn.close()
            except Exception as exc:
                row["logout_exception"] = type(exc).__name__
                row["passed"] = False
        results.append(row)
    return results


def service_validator(args, auth_only=False):
    """Use unmodified DMTF validation logic with a pinned local CSDL cache.

    Select only assertion metadata for the report: never raw payload values,
    credentials, diagnostic attachments, console text or configuration.
    """
    from redfish_service_validator import metadata
    from redfish_service_validator.system_under_test import SystemUnderTest

    metadata.parse_schema_files(str(args.csdl_directory))
    # Upstream ServiceValidator does not expose the client's cafile option,
    # and redfish3.4.2 defaults to verify=False. Configure only its transport
    # constructor; resource/authentication/validation behavior is unchanged.
    import redfish
    configured_client = functools.partial(redfish.redfish_client, cafile=ca_bundle(args))
    with patch("redfish.redfish_client", configured_client):
        sut = SystemUnderTest(
            args.base_url, args.username, os.environ["REDFISH_VALIDATION_PASSWORD"],
            5, "Session", None, None, None, ["LogEntry", "5"], False,
        )
    try:
        metadata_response = sut.session.get("/redfish/v1/$metadata")
        if auth_only:
            paths = ["/redfish/v1/SessionService", "/redfish/v1/SessionService/Sessions",
                     "/redfish/v1/AccountService", "/redfish/v1/AccountService/Roles"]
            paths += ["/redfish/v1/AccountService/Roles/" + role
                      for role in ("Administrator", "Operator", "ReadOnly")]
            sessions = sut.session.get("/redfish/v1/SessionService/Sessions")
            paths += [member["@odata.id"] for member in sessions.dict.get("Members", [])]
            for path in paths:
                sut.validate("Single", path, path)
        else:
            sut.validate("Tree", "/redfish/v1/", "/redfish/v1/")
        failures = []
        warnings = []
        unavailable = []
        for uri, resource in sut._resources.items():
            if resource.get("StatusCode") != 200:
                unavailable.append({"uri": uri, "status": resource.get("StatusCode")})
            for prop, result in resource.get("Results", {}).items():
                entry = {"uri": uri, "property": prop, "message": result["Message"]}
                if result["Result"] == "FAIL":
                    failures.append(entry)
                elif result["Result"] == "WARN":
                    warnings.append(entry)
        return {"passed": sut.fail_count == 0, "counts": {
            "pass": sut.pass_count, "warn": sut.warn_count,
            "fail": sut.fail_count, "skip": sut.skip_count,
        }, "scope": "authentication resources only" if auth_only else "full resource tree",
            "metadata_status": metadata_response.status,
            "unavailable_resources": unavailable, "failures": failures, "warnings": warnings}
    finally:
        sut.logout()


def protocol_validator(args):
    """Run upstream tests unchanged, retaining only safe assertion metadata."""
    from redfish_protocol_validator import console_scripts, utils
    from redfish_protocol_validator.constants import Result

    captured = {}
    original_summary = utils.print_summary

    def summary(sut):
        captured["counts"] = {result.name.lower(): sut.summary_count(result) for result in Result}
        captured["failed_assertions"] = [
            {"assertion": assertion.name,
             "requests": [{key: entry[key] for key in ("method", "status", "uri")}
                          for entry in entries if entry["result"] == Result.FAIL]}
            for assertion, entries in sut.results.items()
            if any(entry["result"] == Result.FAIL for entry in entries)
        ]
        original_summary(sut)

    utils.print_summary = summary
    with tempfile.TemporaryDirectory(prefix="qrab-protocol-") as directory:
        # Credential exists only in this process's Python argv, never the OS
        # command line or a configuration file. Raw upstream reports are deleted.
        sys.argv = ["rf_protocol_validator", "--rhost", args.base_url,
                    "--user", args.username, "--password", os.environ["REDFISH_VALIDATION_PASSWORD"],
                    "--report-dir", directory, "--report-type", "tsv",
                    "--avoid-http-redirect", "--ca-bundle", ca_bundle(args)]
        try:
            console_scripts.main()
            captured["exit_code"] = 0
        except SystemExit as exc:
            captured["exit_code"] = exc.code
    captured["passed"] = captured.get("exit_code") == 0 and bool(captured.get("counts"))
    return captured


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base-url", required=True)
    parser.add_argument("--username", default="validator")
    parser.add_argument("--csdl-directory", type=Path)
    parser.add_argument("--ca-bundle", type=Path)
    parser.add_argument("--protocol-disposable", action="store_true")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--worker", choices=("clients", "service", "service_auth", "protocol"), help=argparse.SUPPRESS)
    args = parser.parse_args()
    if args.worker == "protocol" and not args.protocol_disposable:
        parser.error("protocol tests require --protocol-disposable")
    url = urlsplit(args.base_url)
    if (url.username or url.password or url.query or url.fragment
            or url.path not in ("", "/") or not url.hostname):
        parser.error("base URL must contain only scheme, hostname and optional port")
    if url.scheme != "https" and not (url.scheme == "http" and url.hostname in ("127.0.0.1", "::1", "localhost")):
        parser.error("HTTP is permitted only for an explicit loopback fixture")
    if not os.environ.get("REDFISH_VALIDATION_PASSWORD"):
        parser.error("set REDFISH_VALIDATION_PASSWORD; credentials must not be passed in argv")
    if args.worker:
        # Upstream clients/validators can log payloads. Suppress all raw output
        # and logging; emit only the selected structured fields below.
        logging.disable(logging.CRITICAL)
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            try:
                result = {"clients": clients, "service": service_validator,
                          "service_auth": lambda options: service_validator(options, auth_only=True),
                          "protocol": protocol_validator}[args.worker](args)
            except Exception as exc:
                result = {"passed": False, "exception": type(exc).__name__}
        print(json.dumps(result))
        return
    results = {"base_url": args.base_url, "tls_certificate_verification": url.scheme == "https", "versions": {
        package: importlib.metadata.version(package) for package in
        ("redfish", "sushy", "redfish_service_validator", "redfish_protocol_validator")},
        "fixture_limit": "The local test fixture has no QEMU; its live-query 503s are not live hardware acceptance."}
    if args.csdl_directory:
        revision = subprocess.run(["git", "-C", str(args.csdl_directory.resolve()), "rev-parse", "HEAD"],
                                  capture_output=True, text=True, check=False)
        results["schema_revision"] = revision.stdout.strip() if revision.returncode == 0 else None
    workers = ["clients"]
    if args.csdl_directory:
        workers += ["service", "service_auth"]
    if args.protocol_disposable:
        workers.append("protocol")
    for worker in workers:
        command = [sys.executable, str(Path(__file__).resolve()), "--base-url", args.base_url,
                   "--username", args.username, "--worker", worker]
        if args.csdl_directory:
            command += ["--csdl-directory", str(args.csdl_directory.resolve())]
        if args.ca_bundle:
            command += ["--ca-bundle", str(args.ca_bundle.resolve())]
        if worker == "protocol":
            command.append("--protocol-disposable")
        try:
            completed = subprocess.run(command, capture_output=True, text=True, timeout=180, check=False)
            results[worker] = json.loads(completed.stdout)
        except subprocess.TimeoutExpired:
            results[worker] = {"passed": False, "exception": "TimeoutExpired"}
        except (ValueError, OSError):
            results[worker] = {"passed": False, "exception": "WorkerFailed"}
    output = json.dumps(results, indent=2) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(output)
    print(output, end="")
    checks = results["clients"]
    if not isinstance(checks, list) or not all(check.get("passed") for check in checks):
        raise SystemExit(1)
    if any(not results[worker].get("passed") for worker in workers if worker != "clients"):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
