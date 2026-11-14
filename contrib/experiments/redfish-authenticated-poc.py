#!/usr/bin/env python3
"""Interactive real-OIDC Redfish POC. ID tokens exist only in process memory.

Single stdin reader accepts the PKCE callback URL and subsequent commands.
--connect-ip overrides DNS only for the Redfish origin, preserving TLS Host/SNI;
IdP DNS stays normal. No proxy, insecure TLS, token files or auth bypass exist.
Mutations occur once per explicit command; polling retries only GET requests.
"""
import argparse
import base64
import datetime
import hashlib
import http.server
import ipaddress
import json
from pathlib import Path
import secrets
import select
import shlex
import socket
import ssl
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.parse
import urllib.request

SYSTEM = "/redfish/v1/Systems/1"
SECURE_BOOT = SYSTEM + "/SecureBoot"
AUTH_UPDATE = SECURE_BOOT + "/Actions/Oem/QrabSecureBoot.UpdateAuthenticatedDatabase"
DIAGNOSTICS = "/redfish/v1/Managers/1/LogServices/Diagnostics/Actions/LogService.CollectDiagnosticData"
HELP = "commands: admin/check, viewer, enroll [PROFILE_DIR], stage-on, stage-off, powercycle, diagnostics, login, help, quit"


class Failure(Exception):
    pass


def need(condition, message):
    if not condition:
        raise Failure(message)


def emit(event, **fields):
    print(json.dumps({"event": event, **fields}, separators=(",", ":")), flush=True)


class Session:
    def __init__(self, args):
        self.args, self.token, self.expiry = args, None, 0
        origin = urllib.parse.urlsplit(args.origin)
        need(origin.scheme == "https" and origin.hostname and not origin.username and not origin.password
             and not origin.query and not origin.fragment and origin.path in ["", "/"], "origin must be a credential-free HTTPS origin")
        if args.connect_ip:
            ipaddress.ip_address(args.connect_ip)
            original = socket.getaddrinfo
            hostname = origin.hostname.lower().rstrip(".")
            def resolve(host, port, *rest, **kwargs):
                if isinstance(host, str) and host.lower().rstrip(".") == hostname:
                    return original(args.connect_ip, port, *rest, **kwargs)
                return original(host, port, *rest, **kwargs)
            socket.getaddrinfo = resolve
        class NoRedirect(urllib.request.HTTPRedirectHandler):
            def redirect_request(self, req, fp, code, msg, headers, newurl):
                return None
        self.opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect,
            urllib.request.HTTPSHandler(context=ssl.create_default_context(cafile=args.ca_file)))
        prior = {"__name__": "redfish_real_oidc_constants"}
        path = Path(__file__).resolve().parents[1] / "kubernetes/redfish-live.py"
        exec(compile(path.read_text(), str(path), "exec"), prior)
        self.issuer, self.client, self.redirect = prior["ISSUER"], prior["CLIENT"], prior["REDIRECT"]

    def external_json(self, url, data=None):
        request = urllib.request.Request(url, data=data,
            headers={"Content-Type": "application/x-www-form-urlencoded"} if data else {})
        try:
            with self.opener.open(request, timeout=30) as reply:
                raw = reply.read(1024 * 1024 + 1)
                need(len(raw) <= 1024 * 1024, "IdP response exceeds bound")
                return json.loads(raw)
        except urllib.error.HTTPError as error:
            raise Failure(f"IdP request failed with HTTP{error.code}; response body suppressed") from None

    def login(self):
        self.token = None
        discovery = self.external_json(self.issuer + "/.well-known/openid-configuration")
        for endpoint in [discovery["authorization_endpoint"], discovery["token_endpoint"]]:
            need(urllib.parse.urlsplit(endpoint).scheme == "https", "IdP discovery returned a non-HTTPS endpoint")
        verifier, state, nonce = secrets.token_urlsafe(64), secrets.token_urlsafe(24), secrets.token_urlsafe(24)
        challenge = base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest()).rstrip(b"=").decode()
        url = discovery["authorization_endpoint"] + "?" + urllib.parse.urlencode({
            "response_type": "code", "client_id": self.client, "redirect_uri": self.redirect,
            "scope": "openid profile groups_uuid", "state": state, "nonce": nonce,
            "code_challenge": challenge, "code_challenge_method": "S256"})
        found, event = {}, threading.Event()
        def accept(url):
            query = urllib.parse.parse_qs(urllib.parse.urlsplit(url.strip()).query)
            if query.get("state") == [state] and len(query.get("code", [])) == 1:
                found["code"] = query["code"][0]
                event.set()
                return True
            return False
        class Callback(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                accepted = accept("http://localhost" + self.path)
                self.send_response(200 if accepted else 400)
                self.end_headers()
                self.wfile.write(b"Redfish login received; return to the terminal.\n" if accepted else b"Unexpected callback.\n")
            def log_message(self, *_):
                pass
        server = None
        try:
            server = http.server.HTTPServer(("127.0.0.1", 8400), Callback)
            threading.Thread(target=server.serve_forever, daemon=True).start()
        except OSError:
            pass
        emit("LOGIN_URL", url=url)
        emit("WAITING_CALLBACK", redirect=self.redirect, stdin="paste complete redirect URL")
        # Only this main thread ever reads stdin. The callback HTTP thread never
        # reads terminal input and cannot consume the later admin commands.
        deadline = time.monotonic() + 600
        terminal = None
        try:
            if sys.stdin.isatty():
                import termios
                terminal = termios.tcgetattr(sys.stdin)
                quiet = list(terminal)
                quiet[3] &= ~termios.ECHO
                termios.tcsetattr(sys.stdin, termios.TCSANOW, quiet)
            while not event.is_set() and time.monotonic() < deadline:
                if select.select([sys.stdin], [], [], 0.25)[0]:
                    line = sys.stdin.readline()
                    need(line != "", "stdin closed while waiting for login")
                    if not accept(line):
                        emit("CALLBACK_REJECTED", reason="state/code mismatch")
        finally:
            if terminal is not None:
                import termios
                termios.tcsetattr(sys.stdin, termios.TCSANOW, terminal)
            if server:
                server.shutdown()
                server.server_close()
        need(event.is_set(), "login callback timed out")
        result = self.external_json(discovery["token_endpoint"], urllib.parse.urlencode({
            "grant_type": "authorization_code", "code": found.pop("code"), "redirect_uri": self.redirect,
            "client_id": self.client, "code_verifier": verifier}).encode())
        token = result.get("id_token")
        need(isinstance(token, str) and len(token) <= 16 * 1024, "IdP returned no bounded ID token")
        payload = token.split(".")[1]
        claims = json.loads(base64.urlsafe_b64decode(payload + "=" * (-len(payload) % 4)))
        need(claims.get("nonce") == nonce, "ID token nonce does not match the login")
        self.expiry = int(claims["exp"])
        self.token = token
        del result, token, claims
        emit("READY", expiry_claim_utc=datetime.datetime.fromtimestamp(self.expiry, datetime.timezone.utc).isoformat(),
             remaining_seconds=self.expiry-int(time.time()), origin=self.args.origin, help=HELP)

    def call(self, method, path, body=None, raw=False):
        need(self.token is not None and self.expiry > time.time(), "token expired or absent; use login")
        need(path.startswith("/redfish/") and not path.startswith("//") and "?" not in path and "#" not in path, "unsafe resource URI")
        request = urllib.request.Request(self.args.origin.rstrip("/") + path, method=method,
            headers={"Authorization": "Bearer " + self.token, "Content-Type": "application/json"},
            data=None if body is None else json.dumps(body).encode())
        try:
            response = self.opener.open(request, timeout=30)
        except urllib.error.HTTPError as error:
            response = error
        except (OSError, urllib.error.URLError) as error:
            raise Failure(f"transport failed ({type(error).__name__}); mutation outcome may be unknown; no retry issued") from None
        with response:
            data = response.read(2 * 1024 * 1024 + 1)
            need(len(data) <= 2 * 1024 * 1024, "Redfish response exceeds bound")
            return response.status, response.headers, data if raw else (json.loads(data) if data else None)

    def get(self, path):
        status, _, body = self.call("GET", path)
        need(status == 200, f"GET {path} returned HTTP{status}; body suppressed")
        return body

    def identity(self):
        result = subprocess.run(["kubectl", "-n", self.args.namespace, "get", "virtualmachine", self.args.vm,
                                 "-o", "json"], capture_output=True, text=True, timeout=20)
        need(result.returncode == 0, "could not read POC VM identity; Kubernetes output suppressed")
        vm = json.loads(result.stdout)
        return {"vm_uid": vm["metadata"]["uid"], "pod_uid": vm.get("status", {}).get("podUid"),
                "ready": vm.get("status", {}).get("ready")}

    def target(self):
        identity = self.identity()
        need(self.get(SYSTEM).get("UUID") == identity["vm_uid"], "origin is not the selected POC VM or token is not admin; mutation refused")
        return identity

    def check(self, viewer=False):
        source = {"__name__": "redfish_readonly_paths"}
        path = Path(__file__).with_name("redfish-rollout-test.py")
        exec(compile(path.read_text(), str(path), "exec"), source)
        urls = list(source["GETS"]) + ["/redfish/v1/Managers/1/Oem/Qrab/Resources", SYSTEM + "/ResetActionInfo",
                                      SECURE_BOOT + "/Settings", "/redfish/v1/UpdateService/Oem/Qrab/Artifacts",
                                      "/redfish/v1/UpdateService/Oem/Qrab/StagedFirmware"]
        urls = [url for url in urls if url != "/redfish"]
        seen, count, system = set(), 0, None
        while urls and count < 180:
            url = urls.pop(0)
            if url in seen:
                continue
            seen.add(url)
            status, _, body = self.call("GET", url)
            if viewer and "/LogServices/Serial" in url:
                need(status == 403, "viewer could read guest serial metadata")
                continue
            need(status == 200, f"GET {url} returned HTTP{status}; body suppressed")
            need(isinstance(body, dict) and "@odata.type" in body, f"GET {url} lacks native resource type")
            if url == SYSTEM:
                system = body
            def links(value):
                if isinstance(value, dict):
                    target = value.get("@odata.id")
                    if isinstance(target, str) and target.startswith("/redfish/") and "/attachment" not in target:
                        urls.append(target)
                    for nested in value.values():
                        links(nested)
                elif isinstance(value, list):
                    for nested in value:
                        links(nested)
            links(body)
            count += 1
        need(not urls, "resource traversal exceeded bound")
        invalid = list(source["DENIED"]) + [
            ("POST", DIAGNOSTICS, {}),
            ("POST", "/redfish/v1/UpdateService/Actions/Oem/QrabUpdateService.StageFirmware", {})]
        for method, path, body in invalid:
            status, _, _ = self.call(method, path, body)
            need(status == (403 if viewer else 400), f"{method} harmless validation returned HTTP{status} instead of {'403' if viewer else '400'}")
        if viewer:
            need(system["Oem"]["Qrab"]["Configuration"].get("redacted") and "UUID" not in system,
                 "viewer configuration/identity is not redacted")
        secure = self.get(SECURE_BOOT)
        emit("VIEWER_PASS" if viewer else "ADMIN_PASS", resource_gets=count, harmless_validation_checks=len(invalid),
             power_state=system.get("PowerState"), secure_boot_current=secure.get("SecureBootCurrentBoot"))

    def wait_task(self, status, headers, body):
        need(status == 202, f"operation returned HTTP{status}, expected202; body suppressed")
        path = body.get("@odata.id") if isinstance(body, dict) else None
        if not (isinstance(path, str) and path.startswith("/redfish/v1/TaskService/Tasks/")):
            location = headers.get("Location", "")
            need(location.startswith("/redfish/v1/TaskService/TaskMonitors/"), "operation supplied no safe task URI")
            path = "/redfish/v1/TaskService/Tasks/" + location.rsplit("/", 1)[1]
        deadline = time.monotonic() + 180
        while time.monotonic() < deadline:
            task = self.get(path)
            phase = task.get("TaskState")
            if phase == "Completed":
                emit("TASK_COMPLETED", task=path)
                return task
            need(phase not in ["Exception", "Killed", "Cancelled"], f"task ended with {phase}; task body suppressed")
            time.sleep(1)
        raise Failure("task wait timed out; operation was not reissued")

    def enroll(self, profile):
        self.target()
        need(profile is not None, "enroll needs --profile DIR or enroll DIR")
        profile = Path(profile).resolve()
        for database in ["db", "KEK", "PK"]:
            file = profile / f"enroll-{database}.request.json"
            need(file.is_file() and file.stat().st_size <= 200 * 1024, "missing or oversized signed enrollment request")
            request = json.loads(file.read_text())
            need(set(request).issubset({"DatabaseId", "PayloadBase64", "Append"}) and request.get("DatabaseId") == database
                 and isinstance(request.get("PayloadBase64"), str), "invalid signed enrollment request shape")
            status, _, body = self.call("POST", AUTH_UPDATE, request)
            need(status == 200, f"enrollment {database} returned HTTP{status}; no mutation retry issued")
            emit("ENROLLED", database=database, database_count=body.get("DatabaseCount"))
        emit("ENROLL_COMPLETE", secure_boot_current=self.get(SECURE_BOOT).get("SecureBootCurrentBoot"))

    def stage(self, enabled):
        self.target()
        before = self.get(SECURE_BOOT)
        path = before.get("@Redfish.Settings", {}).get("SettingsObject", {}).get("@odata.id", SECURE_BOOT)
        self.wait_task(*self.call("PATCH", path, {"SecureBootEnable": enabled}))
        settings = self.get(SECURE_BOOT + "/Settings")
        current = self.get(SECURE_BOOT)
        emit("SETTINGS_STAGED", requested=enabled, pending_cold_boot=settings.get("Oem", {}).get("Qrab", {}).get("PendingColdBoot"),
             current_boot_before=before.get("SecureBootCurrentBoot"), current_boot_after=current.get("SecureBootCurrentBoot"))

    def powercycle(self):
        before = self.target()
        try:
            status, _, _ = self.call("POST", SYSTEM + "/Actions/ComputerSystem.Reset", {"ResetType": "PowerCycle"})
            need(status == 204, f"PowerCycle returned HTTP{status}; no retry issued")
            emit("POWERCYCLE_REQUESTED", accepted=True)
        except Failure as error:
            if "transport failed" not in str(error):
                raise
            emit("POWERCYCLE_REQUESTED", accepted="unknown_response_lost", retry=False)
        deadline = time.monotonic() + 240
        while time.monotonic() < deadline:
            try:
                current = self.identity()
                if current["vm_uid"] == before["vm_uid"] and current["pod_uid"] and current["pod_uid"] != before["pod_uid"] and current["ready"]:
                    system = self.get(SYSTEM)
                    secure = self.get(SECURE_BOOT)
                    if system.get("PowerState") == "On" and secure.get("Oem", {}).get("Qrab", {}).get("CurrentBootStateAvailable"):
                        emit("POWERCYCLE_COMPLETED", vm_uid_unchanged=True, pod_uid_changed=True,
                             secure_boot_current=secure.get("SecureBootCurrentBoot"))
                        return
            except Failure:
                pass
            time.sleep(2)
        raise Failure("fresh ready Pod/guest resource was not observed; PowerCycle was not reissued")

    def diagnostics(self):
        self.target()
        task = self.wait_task(*self.call("POST", DIAGNOSTICS, {"DiagnosticDataType": "Manager"}))
        created = task.get("Links", {}).get("CreatedResources", [])
        need(len(created) == 1, "diagnostic task has no created resource")
        entry = self.get(created[0]["@odata.id"])
        status, _, data = self.call("GET", entry["AdditionalDataURI"], raw=True)
        need(status == 200 and len(data) <= 128 * 1024, "diagnostic attachment failed or exceeded bound")
        need(json.loads(data).get("redacted") is True, "diagnostic attachment is not marked redacted")
        emit("DIAGNOSTICS_PASS", bytes=len(data), sha256=hashlib.sha256(data).hexdigest(), redacted=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--origin", default="https://qrab-demo.yapplesauce.com")
    parser.add_argument("--connect-ip")
    parser.add_argument("--ca-file")
    parser.add_argument("--profile", type=Path)
    parser.add_argument("--namespace", default="qrab")
    parser.add_argument("--vm", default="qrab-redfish-poc")
    args = parser.parse_args()
    session = Session(args)
    session.login()
    for line in sys.stdin:
        try:
            command = shlex.split(line)
            if not command:
                continue
            name = command[0]
            if name in ["quit", "exit"]:
                break
            if name == "help":
                emit("HELP", help=HELP)
            elif name in ["admin", "check"]:
                session.check()
            elif name == "viewer":
                session.check(viewer=True)
            elif name == "enroll":
                session.enroll(Path(command[1]) if len(command) == 2 else args.profile)
            elif name == "stage-on":
                session.stage(True)
            elif name == "stage-off":
                session.stage(False)
            elif name == "powercycle":
                session.powercycle()
            elif name == "diagnostics":
                session.diagnostics()
            elif name == "login":
                session.login()
            else:
                emit("ERROR", message="unknown command", help=HELP)
        except Failure as error:
            emit("ERROR", message=str(error))
        except Exception as error:
            emit("ERROR", message=type(error).__name__ + "; details suppressed to protect credentials")
    session.token = None
    emit("EXIT", token_discarded=True)


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        emit("EXIT", interrupted=True)
        raise SystemExit(130) from None
    except Failure as error:
        emit("ERROR", message=str(error))
        raise SystemExit(1) from None
    except Exception as error:
        emit("ERROR", message=type(error).__name__ + "; details suppressed to protect credentials")
        raise SystemExit(1) from None
