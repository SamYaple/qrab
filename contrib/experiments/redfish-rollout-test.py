#!/usr/bin/env python3
"""Read-only live Redfish rollout acceptance for an explicitly selected VM.

Runs typed manager inspect RPCs through kubectl exec (no QMP/VNC TCP), validates
VM/Pod/PVC identity, separate service budgets, TPM public EK continuity, and the
HTTP authentication rejection matrix. It never mutates Kubernetes/VM state.
An optional private --token-file enables authenticated inventory GETs using a
real issuer token; credentials and raw guest serial/config data are never printed.

Example (port-forwarding is managed separately):
  python3 contrib/experiments/redfish-rollout-test.py --vm qrab-redfish-poc \
      --origin http://127.0.0.1:18081 --expect-secure-boot disabled \
      --report target/redfish-poc-before.json
Repeat after a separately authorized rollout/power cycle with --baseline REPORT
and --require-new-pod. Existing identity PVCs must never be reused for a new CR.
"""
import argparse
import json
import os
from pathlib import Path
import re
import socket
import ssl
import stat
import subprocess
import urllib.error
import urllib.parse
import urllib.request

SYSTEM = "/redfish/v1/Systems/1"
GETS = ["/redfish", "/redfish/v1/", SYSTEM, SYSTEM + "/Processors", SYSTEM + "/Memory",
        SYSTEM + "/EthernetInterfaces", SYSTEM + "/Storage", SYSTEM + "/BootOptions",
        SYSTEM + "/VirtualMedia", SYSTEM + "/SecureBoot", SYSTEM + "/SecureBoot/SecureBootDatabases",
        "/redfish/v1/Chassis", "/redfish/v1/Chassis/1", "/redfish/v1/Chassis/1/TrustedComponents",
        "/redfish/v1/UpdateService", "/redfish/v1/UpdateService/FirmwareInventory",
        "/redfish/v1/TelemetryService", "/redfish/v1/TelemetryService/MetricDefinitions",
        "/redfish/v1/TelemetryService/MetricReportDefinitions", "/redfish/v1/TelemetryService/MetricReports",
        "/redfish/v1/TelemetryService/MetricReports/VM", "/redfish/v1/TelemetryService/Triggers",
        "/redfish/v1/EventService", "/redfish/v1/EventService/Subscriptions",
        "/redfish/v1/TaskService", "/redfish/v1/TaskService/Tasks",
        "/redfish/v1/Managers/1/LogServices/Diagnostics", "/redfish/v1/Managers/1/LogServices/Serial"]
# Deliberately invalid mutation bodies remain harmless even if a supplied
# "viewer" token unexpectedly resolves to admin. Admission still must return403.
DENIED = [("POST", SYSTEM + "/Actions/ComputerSystem.Reset", {"ResetType": "QrabInvalid"}),
          ("PATCH", SYSTEM, {"Boot": {"QrabInvalid": True}}),
          ("PATCH", SYSTEM + "/SecureBoot", {"QrabInvalid": True}),
          ("POST", SYSTEM + "/VirtualMedia/CD/Actions/VirtualMedia.InsertMedia", {"QrabInvalid": True}),
          ("POST", SYSTEM + "/SecureBoot/Actions/Oem/QrabSecureBoot.UpdateAuthenticatedDatabase", {"QrabInvalid": True})]


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def kubectl(args, *command, json_output=False):
    call = ["kubectl", "-n", args.namespace, *command]
    result = subprocess.run(call, capture_output=True, text=True, timeout=60)
    # CLI stderr or serial output can hold sensitive operational detail. Retain
    # it only in memory and report the safe command category and exit code.
    require(result.returncode == 0, f"kubectl {command[0]} failed (exit {result.returncode}); inspect it locally")
    return json.loads(result.stdout) if json_output else result.stdout


def inspect(args, pod, operation):
    output = kubectl(args, "exec", pod, "-c", "manager", "--", "qrab-manager", "inspect", operation,
                     "--socket", args.socket)
    return json.loads(output)


def cluster(args):
    vm = kubectl(args, "get", "virtualmachine", args.vm, "-o", "json", json_output=True)
    uid = vm["metadata"]["uid"]
    require(vm.get("status", {}).get("ready") and vm["status"].get("observedGeneration") == vm["metadata"].get("generation"), "VM CR is not reconciled and ready")
    pods = kubectl(args, "get", "pods", "-l", f"app.kubernetes.io/instance={args.vm}", "-o", "json", json_output=True)
    pods = [p for p in pods["items"] if not p["metadata"].get("deletionTimestamp")
            and p["status"].get("phase") == "Running"
            and any(o.get("uid") == uid and o.get("controller") for o in p["metadata"].get("ownerReferences", []))
            and any(c["name"] == "manager" for c in p["spec"].get("containers", []))]
    require(len(pods) == 1, "expected exactly one running manager Pod owned by this VM CR")
    pod = pods[0]
    require(any(c["type"] == "Ready" and c["status"] == "True" for c in pod["status"].get("conditions", [])), "VM Pod is not ready")
    claim_name = next(v["persistentVolumeClaim"]["claimName"] for v in pod["spec"]["volumes"] if v["name"] == "state")
    claim = kubectl(args, "get", "pvc", claim_name, "-o", "json", json_output=True)
    require(claim["status"]["phase"] == "Bound", "identity PVC is not Bound")
    require(claim["metadata"]["uid"] == vm["status"].get("stateVolumeUid"), "operator identity PVC UID binding differs from the attached claim")
    require(vm["status"].get("podUid") == pod["metadata"]["uid"], "operator status points at a different Pod")
    require(not claim["metadata"].get("ownerReferences"), "identity PVC must survive VM CR deletion")
    config_name = next(v["configMap"]["name"] for v in pod["spec"]["volumes"] if v["name"] == "config")
    # Read only vm.json, never cloud-init user-data or Secrets. Print none of it.
    config = json.loads(kubectl(args, "get", "configmap", config_name, "-o", "jsonpath={.data.vm\\.json}"))
    require(config.get("state_id") == uid, "manager state_id is not the immutable VM CR UID")
    require(config.get("name") == args.vm, "manager configuration belongs to another VM")
    containers = {c["name"]: c for c in pod["spec"].get("containers", []) + pod["spec"].get("initContainers", [])}
    require("redfish" in containers, "POC VM has no Redfish sidecar")
    budgets = {}
    for name in ["manager", "bmc", "redfish"]:
        if name not in containers:
            continue
        resource = containers[name].get("resources", {})
        require(all(resource.get(kind, {}).get(key) for kind in ["requests", "limits"] for key in ["cpu", "memory"]), f"{name} lacks separate CPU/memory requests/limits")
        budgets[name] = resource
    for name in ["bmc", "redfish"]:
        if name in containers:
            require(containers[name].get("restartPolicy") == "Always", f"{name} is not a native sidecar")
            require(budgets[name]["requests"] == {"cpu": "20m", "memory": "32Mi"}, f"{name} request budget changed")
            require(budgets[name]["limits"] == {"cpu": "500m", "memory": "128Mi"}, f"{name} limit budget changed")
    image = containers["manager"]["image"]
    require("@sha256:" in image, "manager image is not digest-pinned")
    image_id = next(c.get("imageID", "") for c in pod["status"].get("containerStatuses", []) if c["name"] == "manager")
    require(image_id.endswith(image.split("@", 1)[1]), "running manager imageID differs from its desired digest")
    if args.expected_image:
        require(image == args.expected_image, "manager image differs from the requested published digest")
    report = {"namespace": args.namespace, "vm": args.vm, "vm_uid": uid, "pod": pod["metadata"]["name"],
              "pod_uid": pod["metadata"]["uid"], "state_id": config["state_id"],
              "state_claim": claim_name, "state_claim_uid": claim["metadata"]["uid"],
              "manager_image": image, "manager_image_id": image_id, "node": pod["spec"].get("nodeName"), "service_budgets": budgets,
              "secure_boot_capable": config.get("uefi", {}).get("secure_boot_capable", False)}
    return report


def control(args, report):
    pod = report["pod"]
    status = json.loads(kubectl(args, "exec", pod, "-c", "manager", "--", "qrab-manager", "status", "--config", "/etc/qrab/vm.json"))
    require(status.get("phase") == "running" and status.get("run_state") == "running", "QEMU is not running")
    require(tuple(status.get("qemu_version", [])) >= (11, 1, 0), "POC is not running the pinned QEMU11.1+ baseline")
    require(status.get("swtpm_pid"), "manager is not supervising swtpm")
    inventory = inspect(args, pod, "inventory")
    metrics = inspect(args, pod, "metrics")
    require(sum(p["total_threads"] for p in inventory["processors"]) == metrics["logical_processors"], "live inventory/metric vCPU counts disagree")
    require(metrics["memory_bytes"] > 0, "live QMP configured memory is unavailable")
    resources = inspect(args, pod, "resources")
    require(resources.get("container") is not None, "manager container accounting is unavailable")
    config = inspect(args, pod, "config")
    require(config.get("redacted"), "inspect config must use the redacted view")
    network = inspect(args, pod, "network")
    boot = inspect(args, pod, "boot")
    firmware = inspect(args, pod, "firmware")
    databases = inspect(args, pod, "secure-boot")
    settings = inspect(args, pod, "settings")
    require(firmware["state"]["efi-runtime-available"], "guest EFI runtime is unavailable")
    if report["secure_boot_capable"]:
        require(settings["supported"], "Secure Boot capable VM did not expose cold settings")
    current = firmware["state"].get("secure-boot")
    wanted = {"enabled": True, "disabled": False, "unknown": None}.get(args.expect_secure_boot)
    if args.expect_secure_boot:
        require(current == wanted, "observed guest Secure Boot does not match expectation")
    # Guest serial can contain secrets: capture bounded logs privately, extract
    # only public evidence markers, and never print/write the original text.
    text = kubectl(args, "logs", pod, "-c", "manager", "--tail=400", "--limit-bytes=131072")
    match = re.search(r"QRAB_TPM_EK_SHA256=([a-f0-9]{64})", text)
    require(match is not None, "guest TPM public EK evidence is missing from retained serial tail")
    for marker in ["QRAB_UEFI_BOOTED", "QRAB_TPM2_READY", "QRAB_TPM_EVENTLOG_PRESENT"]:
        require(marker in text, f"guest evidence marker {marker} is missing")
    if args.expect_no_nic:
        require(not config.get("guest_network"), "POC unexpectedly has configured guest NICs")
        require("QRAB_GUEST_NO_NIC_OK" in text, "guest no-NIC evidence is missing")
    report["control"] = {"qemu_version": status["qemu_version"], "processor_sockets": len(inventory["processors"]),
                         "logical_processors": metrics["logical_processors"], "memory_bytes": metrics["memory_bytes"],
                         "drives": len(inventory["drives"]), "guest_interfaces": len(network["interfaces"]),
                         "boot_options": len(boot["state"]["options"]), "secure_boot_current": current,
                         "secure_boot_databases": len(databases["databases"]), "secure_boot_settings": settings,
                         "tpm_ek_sha256": match.group(1), "manager_resources": resources}
    # None of the raw serial/config/database response bodies enter the report.


def http_client(args):
    origin = urllib.parse.urlsplit(args.origin)
    require(origin.hostname is not None and origin.username is None and origin.password is None
            and not origin.query and not origin.fragment and origin.path in ["", "/"], "origin must be a credential-free HTTP(S) origin")
    require(origin.scheme == "https" or (origin.scheme == "http" and origin.hostname in ["127.0.0.1", "localhost", "::1"]), "HTTP is permitted only for an explicit loopback port-forward")
    if args.connect_ip:
        original=socket.getaddrinfo
        def resolve(host,port,*rest,**kwargs):
            return original(args.connect_ip if host==origin.hostname else host,port,*rest,**kwargs)
        socket.getaddrinfo=resolve
    context = ssl.create_default_context(cafile=args.ca_file)
    class NoRedirect(urllib.request.HTTPRedirectHandler):
        def redirect_request(self, req, fp, code, msg, headers, newurl):
            return None
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect, urllib.request.HTTPSHandler(context=context))
    def call(method, path, token=None, body=None):
        require(path.startswith("/redfish") and not path.startswith("//"), "unexpected Redfish URI")
        headers = {"Content-Type": "application/json"}
        if token is not None:
            headers["Authorization"] = "Bearer " + token
        request = urllib.request.Request(args.origin.rstrip("/") + path, method=method, headers=headers,
                                         data=None if body is None else json.dumps(body).encode())
        try:
            response = opener.open(request, timeout=30)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            data = response.read(2 * 1024 * 1024 + 1)
            require(len(data) <= 2 * 1024 * 1024, "Redfish response exceeds test bound")
            return response.status, response.headers, json.loads(data) if data else None
    return call


def http(args, report):
    call = http_client(args)
    rejected = 0
    for method, path, body in [("GET", p, None) for p in GETS] + DENIED:
        for token in [None, "not.a.valid.jwt"]:
            status, headers, _ = call(method, path, token, body)
            require(status == 401 and headers.get("WWW-Authenticate", "").startswith("Bearer"), f"{method} {path} did not reject unauthenticated/malformed tokens")
            require(headers.get("OData-Version") == "4.0", f"{path} was not answered by native Redfish")
            rejected += 1
    summary = {"unauthenticated_rejections": rejected, "authenticated_gets": 0}
    if args.token_file:
        file = Path(args.token_file)
        require(not file.is_symlink(), "token file must not be a symlink")
        meta = file.stat()
        require(stat.S_ISREG(meta.st_mode) and meta.st_mode & 0o077 == 0 and meta.st_size <= 16 * 1024, "token file must be a private regular file of at most16KiB")
        token = file.read_text().strip()
        require(token and token.isascii() and not any(c.isspace() for c in token), "token file has invalid credential formatting")
        for path in GETS:
            status, _, data = call("GET", path, token)
            require(status == 200, f"authenticated GET {path} returned {status}")
            if path != "/redfish":
                require(isinstance(data, dict) and "@odata.type" in data, f"{path} lacks a native Redfish resource type")
            summary["authenticated_gets"] += 1
        if args.expect_role == "viewer":
            for method, path, body in DENIED:
                require(call(method, path, token, body)[0] == 403, f"viewer privilege check failed for {method} {path}")
            summary["viewer_mutations_denied"] = len(DENIED)
        del token
    report["http"] = summary


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--namespace", default="qrab")
    parser.add_argument("--vm", required=True)
    parser.add_argument("--socket", default="/run/qrab-control/manager/control.sock")
    parser.add_argument("--origin", help="HTTPS origin or loopback port-forward origin")
    parser.add_argument("--ca-file")
    parser.add_argument("--connect-ip", help="connect to this IP while preserving HTTPS Host/SNI")
    parser.add_argument("--token-file", help="private existing real-OIDC ID token file; never printed")
    parser.add_argument("--expect-role", choices=["admin", "viewer"], default="admin")
    parser.add_argument("--expect-secure-boot", choices=["enabled", "disabled", "unknown"])
    parser.add_argument("--expect-no-nic", action="store_true")
    parser.add_argument("--expected-image")
    parser.add_argument("--skip-control", action="store_true")
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--require-new-pod", action="store_true")
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    report = cluster(args)
    if not args.skip_control:
        control(args, report)
    if args.origin:
        http(args, report)
    if args.baseline:
        baseline = json.loads(args.baseline.read_text())
        for key in ["vm_uid", "state_id", "state_claim_uid"]:
            require(report[key] == baseline[key], f"{key} changed; retained identity continuity was lost")
        if "control" in baseline and "control" in report:
            require(report["control"]["tpm_ek_sha256"] == baseline["control"]["tpm_ek_sha256"], "TPM public EK changed across rollout/power cycle")
        if args.require_new_pod:
            require(report["pod_uid"] != baseline["pod_uid"], "expected a new Pod after the separately authorized cold cycle")
        report["identity_continuity"] = True
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        fd = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(fd, "w") as output:
            json.dump(report, output, indent=2)
            output.write("\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, subprocess.SubprocessError, OSError, ValueError, KeyError) as error:
        raise SystemExit(f"FAIL: {error}") from None
