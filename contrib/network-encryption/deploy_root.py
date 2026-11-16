#!/usr/bin/python3
"""Coordinate the qrab native MACsec deployment from the physical gateway."""
import argparse
import base64
import copy
import ctypes
import datetime
import fcntl
import hashlib
import io
import ipaddress
import json
import os
from pathlib import Path
import pwd
import re
import secrets
import shlex
import shutil
import signal
import socket
import stat
import struct
import subprocess
import sys
import tarfile
import tempfile
import threading
import time

BASE = Path("/var/lib/qrab-native-link")
UPPER = "macsec0"
MTU = 1500
LOWER_MTU = 1532
PROBE_PORT = 41377
ROLLBACK_SECONDS = 720
GUARD_TABLE = "qrab_native_link"
SEARCH_PATH = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
DIAGNOSTIC_STATE = None
HOSTS = {
    "gateway": {"ip": "10.203.0.1", "aux": "192.0.2.1", "lower": "br-kube", "priority": 0},
    **{f"kube0{n}": {"ip": f"10.203.0.{10+n}", "aux": f"192.0.2.{10+n}",
                    "lower": "enp0s2", "priority": n * 16} for n in range(1, 4)},
}
NETWORK_DIR = Path("/etc/systemd/network")
LOWER_FILE = NETWORK_DIR / "05-qrab-macsec-lower.network"
UPPER_FILE = NETWORK_DIR / "06-qrab-macsec-upper.network"
SYSCTL_FILE = Path("/etc/sysctl.d/80-qrab-macsec.conf")
UNIT_DIR = Path("/etc/systemd/system")


class DeployError(RuntimeError):
    pass


def require(condition, message):
    if not condition:
        raise DeployError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def run(argv, *, data=None, check=True, timeout=40, cwd=None):
    argv = [str(v) for v in argv]
    executable = shutil.which(argv[0], path=SEARCH_PATH)
    require(executable is not None, f"required command is missing: {argv[0]}")
    argv[0] = executable
    try:
        result = subprocess.run(argv, input=data, stdout=subprocess.PIPE,
                                stderr=subprocess.PIPE, timeout=timeout, cwd=cwd)
    except subprocess.TimeoutExpired:
        raise DeployError(f"{Path(argv[0]).name} exceeded its {timeout}s deadline") from None
    if check and result.returncode:
        # Never echo command input or configuration diagnostics containing keys.
        detail = ""
        if DIAGNOSTIC_STATE is not None:
            save(DIAGNOSTIC_STATE / "last-command.json",
                 {"argv": argv, "exit": result.returncode,
                  "stderr": result.stderr.decode(errors="replace")[-65536:]})
            detail = f"; private diagnostics: {DIAGNOSTIC_STATE}/last-command.json"
        raise DeployError(f"{Path(argv[0]).name} failed (exit {result.returncode})" + detail)
    return result


def jrun(argv, **kwargs):
    return json.loads(run(argv, **kwargs).stdout)


def atomic(path, data, mode=0o600):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    require(not path.is_symlink(), f"refusing symlink: {path}")
    fd, name = tempfile.mkstemp(prefix="." + path.name + ".", dir=path.parent)
    temp = Path(name)
    try:
        os.fchmod(fd, mode)
        with os.fdopen(fd, "wb") as output:
            output.write(data)
            output.flush()
            os.fsync(output.fileno())
        os.replace(temp, path)
        directory = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    finally:
        temp.unlink(missing_ok=True)


def save(path, value):
    atomic(path, (json.dumps(value, indent=2, sort_keys=True) + "\n").encode())


def load(path):
    return json.loads(Path(path).read_text())


def state_path(value):
    path = Path(value)
    require(path.parent == BASE and re.fullmatch(r"[0-9a-f]{24}", path.name),
            "invalid deployment state path")
    require(path.is_dir() and not path.is_symlink(), "deployment state is missing or a symlink")
    require(path.stat().st_uid == 0 and stat.S_IMODE(path.stat().st_mode) == 0o700,
            "deployment state must be root-owned mode 0700")
    return path


def phase(state, value):
    atomic(state / "phase", (value + "\n").encode())


def current_phase(state):
    return (state / "phase").read_text().strip() if (state / "phase").exists() else "new"


def snapshot_file(path):
    path = Path(path)
    require(not path.is_symlink(), f"managed path is a symlink: {path}")
    if not path.exists():
        return None
    require(path.is_file(), f"managed path is not a regular file: {path}")
    info = path.stat()
    return {"data": base64.b64encode(path.read_bytes()).decode(),
            "mode": stat.S_IMODE(info.st_mode), "uid": info.st_uid, "gid": info.st_gid}


def remember_write(state, path, data, mode=0o644):
    path = Path(path)
    ledger_path = state / "files.json"
    ledger = load(ledger_path) if ledger_path.exists() else {}
    name = str(path)
    if name not in ledger:
        ledger[name] = {"before": snapshot_file(path)}
    else:
        require(not path.is_symlink(), f"managed path became a symlink: {path}")
        actual = digest(path.read_bytes()) if path.exists() else None
        require(actual in ledger[name].get("owned", [ledger[name]["after"]]),
                f"managed file changed outside deployment: {path}")
    ledger[name]["after"] = digest(data)
    ledger[name]["owned"] = list(dict.fromkeys(ledger[name].get("owned", [])
                                              + [ledger[name]["after"]]))
    save(ledger_path, ledger)  # Record ownership before the write for crash recovery.
    atomic(path, data, mode)


def restore_files(state):
    errors = []
    ledger = load(state / "files.json") if (state / "files.json").exists() else {}
    for name, item in reversed(list(ledger.items())):
        path = Path(name)
        before = item["before"]
        if path.is_symlink() or (path.exists() and not path.is_file()):
            errors.append(f"managed path changed type: {name}")
            continue
        actual = digest(path.read_bytes()) if path.is_file() and not path.is_symlink() else None
        previous = digest(base64.b64decode(before["data"])) if before else None
        if actual == previous:
            continue
        if actual not in item.get("owned", [item["after"]]) or path.is_symlink():
            errors.append(f"external change prevents automatic restore: {name}")
            continue
        if before is None:
            path.unlink()
        else:
            atomic(path, base64.b64decode(before["data"]), before["mode"])
            os.chown(path, before["uid"], before["gid"])
    return errors


def link(name):
    result = run(["ip", "-j", "-d", "link", "show", "dev", name], check=False)
    return json.loads(result.stdout)[0] if result.returncode == 0 else None


def addresses(name):
    return jrun(["ip", "-j", "address", "show", "dev", name])[0]["addr_info"]


def sysctl_value(name):
    return run(["sysctl", "-n", name]).stdout.decode().strip()


def bgp_routes():
    def hops(route):
        return sorted([route["gateway"]] if "gateway" in route else
                      [v["gateway"] for v in route.get("nexthops", []) if "gateway" in v])
    return {v["dst"]: hops(v) for v in jrun(["ip", "-j", "-4", "route", "show", "proto", "bgp"])}


def tcp_peers(port):
    result = run(["ss", "-Hnt", "state", "established",
                  f"( sport = :{port} or dport = :{port} )"])
    peers = []
    for line in result.stdout.decode().splitlines():
        fields = line.split()
        if len(fields) >= 4:
            peers.append(fields[3])
    return sorted(peers)


def cilium(state, *args):
    containers = jrun(["crictl", "ps", "--name", "cilium-agent", "-o", "json"])["containers"]
    require(len(containers) == 1, "exactly one running Cilium agent is required")
    item = containers[0]
    expected = load(state / "release.json")["cilium_digest"]
    image = json.dumps(item.get("image", {}))
    require(expected in image, "the prepared Cilium release is not running on this node")
    return run(["crictl", "exec", item["id"], *args], timeout=30)


def cilium_devices(state):
    result = cilium(state, "cilium-dbg", "status", "--output", "json")
    firewall = json.loads(result.stdout)["host-firewall"]
    require(firewall["mode"] == "Enabled", "Cilium host firewall must remain enabled")
    return firewall["devices"]


def nft(state, *args, data=None):
    return run([state / "tools/run-nft", *args], data=data)


def legacy_programs():
    result = []
    for program in ("iptables", "ip6tables"):
        if shutil.which(program, path=SEARCH_PATH) and shutil.which(program+"-save", path=SEARCH_PATH):
            version = run([program, "--version"]).stdout
            if b"(legacy)" in version:
                result.append((program, program+"-save"))
    return result


def legacy_rules(text):
    table = None
    positions = {}
    rules = []
    for line in text.splitlines():
        if line.startswith("*"):
            table = line[1:]
        elif line.startswith("-A "):
            tokens = shlex.split(line)
            chain = tokens[1]
            key = (table, chain)
            positions[key] = positions.get(key, 0) + 1
            rules.append({"table": table, "chain": chain, "position": positions[key],
                          "body": tokens[2:]})
    return rules


def legacy_marker(body):
    for index, value in enumerate(body[:-1]):
        if value == "--comment" and body[index+1].startswith("qrab-native:"):
            return body[index+1]
    return None


def legacy_plans(text, lower, deployment_id):
    plans = []
    for rule in legacy_rules(text):
        body = rule["body"]
        if legacy_marker(body):
            continue
        changed = list(body)
        found = False
        for index, token in enumerate(body[:-1]):
            if token in ("-i", "--in-interface", "-o", "--out-interface"):
                value = body[index+1]
                matches = value == lower or (value.endswith("+") and lower.startswith(value[:-1]))
                if matches:
                    require(value == lower and not (index and body[index-1] == "!"),
                            "legacy wildcard/negated native interface rule needs explicit migration")
                    changed[index+1] = UPPER
                    found = True
            elif token in ("--physdev-in", "--physdev-out") and body[index+1] == lower:
                raise DeployError("legacy physical bridge match needs explicit migration")
        if found:
            identity = json.dumps([rule["table"], rule["chain"], body]).encode()
            marker = "qrab-native:" + deployment_id + ":" + digest(identity)[:12]
            jump = next((i for i,v in enumerate(changed)
                         if v in ("-j", "--jump", "-g", "--goto")), len(changed))
            changed[jump:jump] = ["-m", "comment", "--comment", marker]
            plans.append(dict(rule, clone=changed, marker=marker))
    return plans


def sync_legacy(state):
    profile = load(state / "profile.json")
    ledger_path = state / "legacy-owned.json"
    ledger = load(ledger_path) if ledger_path.exists() else {}
    for program, exporter in legacy_programs():
        text = run([exporter]).stdout.decode()
        existing = {legacy_marker(v["body"]) for v in legacy_rules(text)}
        # Reverse order keeps original per-chain indices valid while adding
        # each equivalent upper-interface rule directly after its original.
        for plan in reversed(legacy_plans(text, profile["lower"], state.name)):
            if plan["marker"] in existing:
                continue
            record = {"program": program, "exporter": exporter, "table": plan["table"],
                      "chain": plan["chain"], "body": plan["clone"]}
            ledger[plan["marker"]] = record
            save(ledger_path, ledger)
            run([program, "-w", "5", "-t", plan["table"], "-I", plan["chain"],
                 str(plan["position"]+1), *plan["clone"]])
            current = legacy_rules(run([exporter]).stdout.decode())
            actual = [v for v in current if legacy_marker(v["body"]) == plan["marker"]]
            require(len(actual) == 1, "legacy upper rule was not installed exactly once")
            ledger[plan["marker"]]["body"] = actual[0]["body"]  # Kernel canonical form.
            save(ledger_path, ledger)


def restore_legacy(state):
    ledger_path = state / "legacy-owned.json"
    if not ledger_path.exists():
        return []
    ledger = load(ledger_path)
    errors = []
    for program, exporter in legacy_programs():
        current = legacy_rules(run([exporter]).stdout.decode())
        for rule in reversed(current):
            marker = legacy_marker(rule["body"])
            if marker not in ledger:
                continue
            if rule["body"] != ledger[marker]["body"]:
                errors.append("external legacy firewall edit prevents removal of an owned clone")
                continue
            run([program, "-w", "5", "-t", rule["table"], "-D",
                 rule["chain"], str(rule["position"])])
    return errors


def string_present(value, needle):
    if isinstance(value, str):
        return value == needle
    if isinstance(value, list):
        return any(string_present(v, needle) for v in value)
    if isinstance(value, dict):
        return any(string_present(v, needle) for k, v in value.items() if k != "comment")
    return False


def rebind_expr(value, old_name, new_name, old_index, new_index, both=False):
    """Change only explicit interface matches, preserving all other predicates."""
    value = copy.deepcopy(value)
    if isinstance(value, list):
        return [rebind_expr(v, old_name, new_name, old_index, new_index, both) for v in value]
    if not isinstance(value, dict):
        return value
    match = value.get("match")
    if isinstance(match, dict):
        key = match.get("left", {}).get("meta", {}).get("key")
        old, new = ((old_name, new_name) if key in ("iifname", "oifname")
                    else (old_index, new_index) if key in ("iif", "oif") else (None, None))
        if old is not None:
            right = match.get("right")
            if right == old:
                match["right"] = {"set": [old, new]} if both else new
            elif isinstance(right, dict) and "set" in right:
                items = right["set"]
                require(isinstance(items, list), "unsupported interface set expression")
                if old in items:
                    result = list(dict.fromkeys(
                        items + [new] if both else [new if item == old else item for item in items]))
                    match["right"] = result[0] if len(result) == 1 else {"set": result}
    if string_present(value, old_name) and not both:
        raise DeployError("unsupported named-interface firewall expression")
    return value


def firewall_changes(ruleset, old_name, new_name, old_index, new_index, both=False):
    commands = []
    before = []
    for item in ruleset["nftables"]:
        objects = list(item.values())
        if any(isinstance(v, dict) and v.get("family") == "netdev"
               and v.get("table", v.get("name")) == GUARD_TABLE for v in objects):
            continue
        if "rule" in item:
            rule = item["rule"]
            expr = rebind_expr(rule["expr"], old_name, new_name, old_index, new_index, both)
            if expr != rule["expr"]:
                require(not any("xt" in e for e in rule["expr"]),
                        "interface rule contains an opaque xtables extension")
                replacement = {k: rule[k] for k in ("family", "table", "chain", "handle", "comment")
                               if k in rule}
                replacement["expr"] = expr
                commands.append({"replace": {"rule": replacement}})
                before.append(copy.deepcopy(rule))
        elif string_present(item, old_name):
            raise DeployError("interface-bound nft set/chain requires an explicit migration")
    return {"nftables": commands}, before


def guard_commands(profile, deployment_id, exists=False):
    commands = []
    if exists:
        commands.append({"delete": {"table": {"family": "netdev", "name": GUARD_TABLE}}})
    commands.append({"add": {"table": {"family": "netdev", "name": GUARD_TABLE,
                                      "comment": "qrab " + deployment_id}}})
    for direction in ("ingress", "egress"):
        commands.append({"add": {"chain": {"family": "netdev", "table": GUARD_TABLE,
                                           "name": direction, "type": "filter",
                                           "hook": direction, "prio": -500,
                                           "dev": profile["lower"], "policy": "drop"}}})
        commands.append({"add": {"rule": {"family": "netdev", "table": GUARD_TABLE,
                                          "chain": direction,
                                          "expr": [{"match": {
                                              "left": {"payload": {"protocol": "ether", "field": "type"}},
                                              "op": "==", "right": {"set": [0x88E5, 0x888E]}}},
                                              {"counter": None}, {"accept": None}]}}})
        commands.append({"add": {"rule": {"family": "netdev", "table": GUARD_TABLE,
                                          "chain": direction, "expr": [
                                              {"counter": None}, {"drop": None}]}}})
    return {"nftables": commands}


def firewall_semantics(value):
    if isinstance(value, list):
        return [firewall_semantics(v) for v in value if not (isinstance(v, dict) and "metainfo" in v)]
    if isinstance(value, dict):
        if set(value) == {"set"} and isinstance(value["set"], list):
            items = [firewall_semantics(v) for v in value["set"]]
            if len(items) == 1:
                return items[0]
            return {"set": sorted(items, key=lambda v: json.dumps(v, sort_keys=True))}
        return {k: (None if k == "counter" else firewall_semantics(v))
                for k, v in value.items() if k not in ("handle", "index")}
    return value


def install_guard(state):
    profile = load(state / "profile.json")
    tables = json.loads(nft(state, "-j", "list", "tables").stdout)["nftables"]
    exists = any(v.get("table", {}).get("family") == "netdev"
                 and v["table"]["name"] == GUARD_TABLE for v in tables)
    if exists:
        table = json.loads(nft(state, "-j", "list", "table", "netdev", GUARD_TABLE).stdout)
        own = [v["table"] for v in table["nftables"] if "table" in v]
        require(len(own) == 1 and own[0].get("comment") == "qrab " + state.name,
                "native guard table is owned by another deployment")
    commands = json.dumps(guard_commands(profile, state.name, exists)).encode()
    nft(state, "-j", "-c", "-f", "-", data=commands)
    nft(state, "-j", "-f", "-", data=commands)
    actual = json.loads(nft(state, "-j", "list", "table", "netdev", GUARD_TABLE).stdout)
    save(state / "guard-semantics.json", firewall_semantics(actual))
    atomic(state / "guard-installed", b"owned\n")


def remove_guard(state):
    # The kernel transaction may have committed before its marker was saved.
    result = run([state / "tools/run-nft", "-j", "list", "table", "netdev", GUARD_TABLE],
                 check=False)
    if result.returncode == 0:
        table = json.loads(result.stdout)
        own = [v["table"] for v in table["nftables"] if "table" in v]
        require(len(own) == 1 and own[0].get("comment") == "qrab " + state.name,
                "refusing to remove a foreign native guard table")
        nft(state, "delete", "table", "netdev", GUARD_TABLE)
    (state / "guard-installed").unlink(missing_ok=True)


def sync_firewall(state, both=False):
    profile = load(state / "profile.json")
    lower = link(profile["lower"])
    upper = link(UPPER)
    require(lower is not None and upper is not None, "firewall interfaces are missing")
    rules = json.loads(nft(state, "-j", "list", "ruleset").stdout)
    changes, before = firewall_changes(rules, profile["lower"], UPPER,
                                      lower["ifindex"], upper["ifindex"], both)
    if before:
        record = state / "firewall-before.json"
        saved = load(record) if record.exists() else []
        identities = {(v["family"], v["table"], v["chain"], v["handle"]) for v in saved}
        saved.extend(v for v in before
                     if (v["family"], v["table"], v["chain"], v["handle"]) not in identities)
        save(record, saved)
        ownership = load(state / "firewall-owned.json") if (state / "firewall-owned.json").exists() else {}
        for command in changes["nftables"]:
            rule = command["replace"]["rule"]
            key = json.dumps([rule[k] for k in ("family", "table", "chain", "handle")])
            version = firewall_semantics(rule["expr"])
            versions = ownership.get(key, [])
            if version not in versions:
                versions.append(version)
            ownership[key] = versions
        save(state / "firewall-owned.json", ownership)
        body = json.dumps(changes).encode()
        nft(state, "-j", "-c", "-f", "-", data=body)
        nft(state, "-j", "-f", "-", data=body)
    sync_legacy(state)


def restore_firewall(state):
    record = state / "firewall-before.json"
    if not record.exists():
        return []
    current = json.loads(nft(state, "-j", "list", "ruleset").stdout)
    rules = {(v["rule"]["family"], v["rule"]["table"], v["rule"]["chain"],
              v["rule"]["handle"]): v["rule"] for v in current["nftables"] if "rule" in v}
    commands = []
    errors = []
    ownership = load(state / "firewall-owned.json")
    for old in load(record):
        key = (old["family"], old["table"], old["chain"], old["handle"])
        original = firewall_semantics(old["expr"])
        if key not in rules:
            if any(v["family"] == old["family"] and v["table"] == old["table"]
                   and v["chain"] == old["chain"] and firewall_semantics(v["expr"]) == original
                   for v in rules.values()):
                continue  # The native service already reloaded the original rule.
            errors.append("firewall rule was removed/reloaded; original config must be checked")
            continue
        actual = firewall_semantics(rules[key]["expr"])
        if actual == original:
            continue
        if actual not in ownership[json.dumps(list(key))]:
            errors.append("external firewall rule change prevents automatic restore")
            continue
        replacement = {k: old[k] for k in ("family", "table", "chain", "handle", "expr", "comment")
                       if k in old}
        commands.append({"replace": {"rule": replacement}})
    if commands:
        nft(state, "-j", "-f", "-", data=json.dumps({"nftables": commands}).encode())
    return errors


def inspect_host(state):
    profile = load(state / "profile.json")
    lower = link(profile["lower"])
    require(lower is not None, "expected lower interface is missing")
    own = {v["local"] for v in addresses(profile["lower"]) if v["scope"] == "global"}
    require(profile["ip"] in own, "expected production address is not on the lower interface")
    allowed = {profile["ip"]} | ({"10.203.0.10"} if profile["role"] != "gateway" else set())
    require(own <= allowed, "unexpected address on the native lower")
    require(link(UPPER) is None, "a MACsec upper already exists; use status/rollback")
    require(lower["mtu"] == 1500 and lower.get("max_mtu", 0) >= LOWER_MTU,
            "native lower must have MTU 1500 and support 1532")
    require(not any(v.get("dst") == "192.0.2.0/24" for v in jrun(["ip", "-j", "-4", "route"])),
            "the temporary 192.0.2.0/24 staging subnet is already used")
    require(run(["systemctl", "is-active", "systemd-networkd"], check=False).returncode == 0,
            "systemd-networkd must be active")
    require(run(["modprobe", "--dry-run", "macsec"], check=False).returncode == 0,
            "the MACsec kernel module is unavailable")
    require(not LOWER_FILE.exists() and not UPPER_FILE.exists(),
            "managed networkd overrides already exist")
    require(not (UNIT_DIR / "qrab-macsec.service").exists(),
            "a native MACsec service is already installed")
    snapshot = {"lower": lower, "routes": bgp_routes(), "bgp": tcp_peers(179),
                "iscsi": tcp_peers(3260),
                "ipv6_disabled": sysctl_value(f"net.ipv6.conf.{profile['lower']}.disable_ipv6")}
    snapshot["legacy"] = {}
    for program, exporter in legacy_programs():
        text = run([exporter]).stdout.decode()
        require(not any(legacy_marker(v["body"]) for v in legacy_rules(text)),
                "legacy rules from an older native deployment remain")
        legacy_plans(text, profile["lower"], state.name)
        snapshot["legacy"][program] = text
    if profile["role"] == "gateway":
        require(lower.get("linkinfo", {}).get("info_kind") == "bridge",
                "the gateway lower is not a bridge")
        snapshot["bridge_network"] = snapshot_file(NETWORK_DIR / "10-kube-bridge.network")
        require(snapshot["bridge_network"] is not None, "gateway networkd config is missing")
        snapshot["taps"] = {}
        for name in ("kube-tap1", "kube-tap2", "kube-tap3"):
            item = link(name)
            require(item and item.get("master") == profile["lower"]
                    and item.get("max_mtu", 0) >= LOWER_MTU, "unexpected gateway tap topology")
            snapshot["taps"][name] = item["mtu"]
    else:
        snapshot["keepalived"] = snapshot_file("/etc/keepalived/keepalived.conf")
        require(snapshot["keepalived"] is not None, "keepalived config is missing")
        render_keepalived(base64.b64decode(snapshot["keepalived"]["data"]).decode())
        require(run(["systemctl", "is-active", "keepalived"], check=False).returncode == 0,
                "keepalived must be active before deployment")
        require(run(["systemctl", "is-active", "kubelet"], check=False).returncode == 0,
                "kubelet must be active before deployment")
        unit = run(["systemctl", "cat", "kubelet.service"]).stdout.decode()
        starts = [v.removeprefix("ExecStart=") for v in unit.splitlines()
                  if v.startswith("ExecStart=")]
        # Follow systemd's reset semantics for the effective single command.
        effective = []
        for command in starts:
            if command:
                effective.append(command)
            else:
                effective.clear()
        require(len(effective) == 1
                and effective[0].split()[0] in ("/usr/bin/kubelet", "/usr/local/bin/kubelet"),
                "kubelet does not match the reviewed direct-executable layout")
        snapshot["kubelet_start"] = effective[0]
        # A bound kernel initiator cannot silently keep using the raw interface.
        for path in Path("/etc/iscsi/ifaces").glob("*"):
            if path.is_file():
                require(not re.search(r"iface.net_ifacename\s*=\s*enp0s2\b", path.read_text()),
                        "a kernel iSCSI initiator is bound to the raw interface")
        require(profile["lower"] in cilium_devices(state), "Cilium does not select the lower")
        configured = cilium(state, "cat", "/tmp/cilium/config-map/devices").stdout.decode()
        require(UPPER in re.split(r"[\s,]+", configured.strip()), "Cilium does not select macsec0")
        allowed = cilium(state, "cat", "/tmp/cilium/config-map/macsec-underlay-devices").stdout.decode()
        require(allowed.strip() == profile["lower"], "Cilium lower exception is not configured")
    # Read/validate the firewall before any interface or service changes.
    rules = json.loads(nft(state, "-j", "list", "ruleset").stdout)
    require(not any(v.get("table", {}).get("name") == GUARD_TABLE
                    and v["table"].get("family") == "netdev" for v in rules["nftables"]),
            "a native MACsec guard already exists")
    firewall_changes(rules, profile["lower"], UPPER, lower["ifindex"], 65530)
    nft(state, "-j", "-c", "-f", "-",
        data=json.dumps(guard_commands(profile, state.name)).encode())
    save(state / "before.json", snapshot)
    phase(state, "prepared")
    return {"role": profile["role"], "ip": profile["ip"], "clock": time.time(),
            "bgp_peers": len(snapshot["bgp"]),
            "iscsi_connections": len(snapshot["iscsi"]), "phase": "prepared"}


def render_keepalived(text):
    text, interface_count = re.subn(r"(?m)^(\s*interface\s+)enp0s2(\s*)$",
                                    r"\g<1>macsec0\2", text)
    text, vip_count = re.subn(r"(10\.203\.0\.10/24\s+dev\s+)enp0s2\b",
                              r"\g<1>macsec0", text)
    require(interface_count == 1 and vip_count == 1,
            "keepalived does not match the reviewed single API VIP layout")
    return text


def render_network(profile, before):
    lower = f"""[Match]
Name={profile['lower']}
[Link]
MTUBytes={LOWER_MTU}
RequiredForOnline=no
[Network]
DHCP=no
LinkLocalAddressing=no
IPv6AcceptRA=no
ConfigureWithoutCarrier=yes
KeepConfiguration=no
"""
    if profile["role"] == "gateway":
        original = base64.b64decode(before["bridge_network"]["data"]).decode()
        upper, count = re.subn(r"(?m)^Name=br-kube$", "Name=macsec0", original)
        require(count == 1 and "Address=10.203.0.1/24" in upper,
                "gateway network file changed shape")
        upper += f"\n[Link]\nMTUBytes={MTU}\nRequiredForOnline=no\n"
    else:
        upper = f"""[Match]
Name={UPPER}
[Link]
MTUBytes={MTU}
RequiredForOnline=no
[Network]
Address={profile['ip']}/24
DNS=1.1.1.1
DNS=9.9.9.9
DHCP=no
LinkLocalAddressing=no
IPv6AcceptRA=no
ConfigureWithoutCarrier=yes
KeepConfiguration=no
[Route]
Destination=0.0.0.0/0
Gateway=10.203.0.1
GatewayOnLink=yes
PreferredSource={profile['ip']}
"""
    upper += f"\n[Address]\nAddress={profile['aux']}/24\n"
    return lower, upper


def systemd_units(state, profile):
    program = f"/usr/bin/python3 -I {state}/deploy_root.py"
    agent = f"{program} agent --state {state}"
    guarded = current_phase(state) in ("activating", "activated", "verified", "committed")
    dependency = "Requires=qrab-macsec-guard.service\nAfter=qrab-macsec-guard.service\n" if guarded else ""
    wpa = f"""[Unit]
Description=qrab native MACsec/MKA on {profile['lower']}
{dependency}Wants=systemd-networkd.service
After=systemd-networkd.service nftables.service iptables.service ip6tables.service netfilter-persistent.service
Before=network-online.target kubelet.service keepalived.service
[Service]
Type=simple
UMask=0077
RuntimeDirectory=qrab-macsec
RuntimeDirectoryMode=0700
ExecStartPre={agent} wait-lower
ExecStart={state}/tools/run-wpa -Dmacsec_linux -i{profile['lower']} -c{state}/mka.conf
ExecStartPost={agent} init
Restart=always
RestartSec=1
TimeoutStartSec=60
CapabilityBoundingSet=CAP_NET_ADMIN CAP_NET_RAW
NoNewPrivileges=yes
ProtectHome=yes
ProtectSystem=strict
ReadWritePaths={state} /run/qrab-macsec
PrivateTmp=yes
[Install]
WantedBy=network-online.target
"""
    probe = f"""[Unit]
Description=qrab encrypted native-link verification endpoint
After=qrab-macsec.service
Requires=qrab-macsec.service
[Service]
ExecStart={agent} probe-server
Restart=on-failure
RestartSec=1
UMask=0077
NoNewPrivileges=yes
ProtectHome=yes
ProtectSystem=strict
[Install]
WantedBy=multi-user.target
"""
    rollback = f"""[Unit]
Description=Rollback uncommitted qrab native link deployment
ConditionPathExists={state}/uncommitted
[Service]
Type=oneshot
ExecStart={agent} timeout-rollback
TimeoutStartSec=120
"""
    guard = f"""[Unit]
Description=Block unencrypted frames on the qrab native lower
Wants=systemd-networkd.service
After=systemd-networkd.service
Before=qrab-macsec.service network-online.target kubelet.service keepalived.service
[Service]
Type=oneshot
RemainAfterExit=yes
ExecStartPre={agent} wait-lower
ExecStart={agent} guard
TimeoutStartSec=45
[Install]
WantedBy=network-online.target
"""
    deadline = datetime.datetime.fromtimestamp(profile["deadline"], datetime.timezone.utc)
    timer = f"""[Unit]
Description=qrab native link deployment rollback deadline
[Timer]
OnCalendar={deadline.strftime('%Y-%m-%d %H:%M:%S')} UTC
Persistent=yes
AccuracySec=1s
Unit=qrab-macsec-rollback.service
[Install]
WantedBy=timers.target
"""
    return {"qrab-macsec.service": wpa, "qrab-macsec-probe.service": probe,
            "qrab-macsec-rollback.service": rollback, "qrab-macsec-rollback.timer": timer,
            "qrab-macsec-guard.service": guard}


def mka_config(profile):
    return f"""ctrl_interface=/run/qrab-macsec
eapol_version=3
ap_scan=0
network={{
 key_mgmt=NONE
 eapol_flags=0
 macsec_policy=1
 macsec_integ_only=0
 macsec_replay_protect=1
 macsec_replay_window=0
 mka_cak={profile['cak']}
 mka_ckn={profile['ckn']}
 mka_priority={profile['priority']}
}}
"""


def production_addresses(profile):
    run(["ip", "address", "replace", profile["ip"] + "/24", "dev", UPPER])
    # Keep the production address first for interface-address consumers such
    # as masquerade discovery. Existing auxiliary SSH sockets survive this
    # brief remove/re-add; production data already has its unchanged address.
    if profile["aux"] in {v["local"] for v in addresses(UPPER)}:
        run(["ip", "address", "del", profile["aux"] + "/24", "dev", UPPER])
    run(["ip", "address", "replace", profile["aux"] + "/24", "dev", UPPER])
    if profile["role"] != "gateway":
        run(["ip", "route", "replace", "default", "via", "10.203.0.1",
             "dev", UPPER, "onlink", "src", profile["ip"]])


def init_upper(state):
    profile = load(state / "profile.json")
    for _ in range(100):
        upper = link(UPPER)
        if upper:
            break
        time.sleep(.1)
    else:
        raise DeployError("MKA did not create macsec0")
    info = upper.get("linkinfo", {}).get("info_data", {})
    require(upper.get("linkinfo", {}).get("info_kind") == "macsec"
            and (upper.get("link") == profile["lower"]
                 or upper.get("link_index") == link(profile["lower"])["ifindex"]),
            "macsec0 is not attached to the expected lower")
    run(["ip", "link", "set", UPPER, "type", "macsec", "encrypt", "on",
         "protect", "on", "validate", "strict", "replay", "on", "window", "0"])
    run(["sysctl", "-q", "-w", f"net.ipv6.conf.{UPPER}.disable_ipv6=1"])
    run(["ip", "link", "set", UPPER, "up"])
    # Creating the upper first keeps Cilium's selected MTU from briefly rising
    # above 1500 while the lower MTU is increased.
    run(["ip", "link", "set", profile["lower"], "mtu", str(LOWER_MTU)])
    run(["ip", "link", "set", UPPER, "mtu", str(MTU)])
    if current_phase(state) in ("activating", "activated", "verified", "committed"):
        install_guard(state)
        production_addresses(profile)
        sync_firewall(state)
    else:
        run(["ip", "address", "replace", profile["aux"] + "/24", "dev", UPPER])
    secure_state(state, require_cilium=False)


def secure_state(state, require_cilium=True):
    profile = load(state / "profile.json")
    upper = link(UPPER)
    require(upper and upper["mtu"] == MTU, "MACsec upper MTU is incorrect")
    info = upper["linkinfo"]["info_data"]
    require(info.get("encrypt") is True and info.get("protect") is True
            and info.get("validation") == "strict" and info.get("replay_protect") is True
            and info.get("window") == 0, "MACsec protection flags are incorrect")
    if profile["role"] != "gateway" and require_cilium:
        require(UPPER in cilium_devices(state), "Cilium host policy is missing on macsec0")
        attached = json.loads(cilium(state, "bpftool", "net", "-j").stdout)
        entries = [v for net in attached for v in net.get("tc", [])]
        for name, direction in (("cil_from_netdev", "ingress"), ("cil_to_netdev", "egress")):
            require(any(v.get("devname") == UPPER and v.get("ifindex") == upper["ifindex"]
                        and v.get("name") == name and v.get("kind", "").endswith("/" + direction)
                        for v in entries), "Cilium's kernel hook is missing on macsec0")
        wg = link("cilium_wg0")
        require(wg and wg["mtu"] <= 1420, "WireGuard MTU exceeds the encrypted upper budget")
    return upper


def stage(state):
    profile = load(state / "profile.json")
    require(current_phase(state) == "prepared", "host has not passed preflight")
    atomic(state / "uncommitted", b"armed\n")
    phase(state, "staging")
    for name, content in systemd_units(state, profile).items():
        remember_write(state, UNIT_DIR / name, content.encode())
    atomic(state / "mka.conf", mka_config(profile).encode())
    remember_write(state, Path("/etc/modules-load.d/qrab-macsec.conf"), b"macsec\n")
    run(["modprobe", "macsec"])
    run(["systemctl", "daemon-reload"])
    run(["systemctl", "enable", "--now", "qrab-macsec-rollback.timer"])
    if profile["role"] == "gateway":
        before = load(state / "before.json")
        old = int(before["lower"]["linkinfo"]["info_data"]["group_fwd_mask"], 0)
        run(["ip", "link", "set", profile["lower"], "type", "bridge",
             "group_fwd_mask", str(old | 8)])
        for name in before["taps"]:
            run(["ip", "link", "set", name, "mtu", str(LOWER_MTU)])
    run(["systemctl", "start", "qrab-macsec.service"], timeout=70)
    run(["systemctl", "start", "qrab-macsec-probe.service"])
    for _ in range(30):
        try:
            secure_state(state)
            break
        except DeployError:
            time.sleep(1)
    else:
        raise DeployError("MACsec protection or upper policy is not ready")
    sync_firewall(state, both=True)
    phase(state, "staged")
    return {"phase": "staged"}


def probe_server(state):
    profile = load(state / "profile.json")
    allowed = {v["aux"] for v in HOSTS.values()}
    listener = socket.socket()
    listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    listener.setsockopt(socket.SOL_SOCKET, socket.SO_BINDTODEVICE, UPPER.encode() + b"\0")
    listener.bind((profile["aux"], PROBE_PORT))
    listener.listen(16)
    while True:
        connection, peer = listener.accept()
        try:
            connection.settimeout(3)
            if peer[0] not in allowed:
                continue
            data = b""
            while len(data) < 32768:
                part = connection.recv(32768 - len(data))
                if not part:
                    break
                data += part
            connection.sendall(json.dumps({"peer": peer[0], "bytes": len(data),
                                           "sha256": digest(data)}).encode())
        except (OSError, TimeoutError):
            pass
        finally:
            connection.close()


def peer_checks(state):
    profile = load(state / "profile.json")
    secure_state(state)
    payload = secrets.token_bytes(32768)
    for role, peer in HOSTS.items():
        if role == profile["role"]:
            continue
        step = "connect"
        try:
            with socket.socket() as client:
                client.settimeout(5)
                client.setsockopt(socket.SOL_SOCKET, socket.SO_BINDTODEVICE, UPPER.encode() + b"\0")
                client.bind((profile["aux"], 0))
                client.connect((peer["aux"], PROBE_PORT))
                step = "send"
                client.sendall(payload)
                client.shutdown(socket.SHUT_WR)
                step = "receive"
                answer = b""
                while True:
                    part = client.recv(4096)
                    if not part:
                        break
                    answer += part
                    require(len(answer) <= 4096, "probe response exceeds limit")
            step = "validate reply"
            value = json.loads(answer)
            require(value == {"peer": profile["aux"], "bytes": len(payload), "sha256": digest(payload)},
                    "encrypted peer probe failed or changed source address")
        except (OSError, ValueError, DeployError) as error:
            record = {"source": profile["role"], "source_ip": profile["aux"],
                      "target": role, "target_ip": peer["aux"], "step": step,
                      "error": str(error), "at": time.time()}
            try:
                save(state / "peer-error.json", record)
            except OSError:
                pass  # Diagnostics must not replace the actual probe error.
            raise DeployError(f"encrypted peer {profile['role']} ({profile['aux']}) -> "
                              f"{role} ({peer['aux']}:{PROBE_PORT}) failed during {step}: {error}") from None
    return {"encrypted_peers": 3, "probe_bytes_per_peer": len(payload)}


def activate(state):
    profile = load(state / "profile.json")
    require(current_phase(state) == "staged", "host has not passed staging")
    require(time.time() < profile["deadline"] - 120, "rollback deadline is too close")
    before = load(state / "before.json")
    phase(state, "activating")
    # At boot, the lower guard must start before MKA can create an addressed
    # upper. Merely removing lower IPs is insufficient on Linux's weak host
    # model, and rp_filter is not a substitute for link authentication.
    remember_write(state, UNIT_DIR / "qrab-macsec.service",
                   systemd_units(state, profile)["qrab-macsec.service"].encode())
    lower_config, upper_config = render_network(profile, before)
    remember_write(state, LOWER_FILE, lower_config.encode())
    remember_write(state, UPPER_FILE, upper_config.encode())
    remember_write(state, SYSCTL_FILE,
                   f"net.ipv6.conf.{profile['lower']}.disable_ipv6 = 1\n".encode())
    run(["sysctl", "-q", "-w", f"net.ipv6.conf.{profile['lower']}.disable_ipv6=1"])
    if profile["role"] == "gateway":
        mask = int(before["lower"]["linkinfo"]["info_data"]["group_fwd_mask"], 0) | 8
        remember_write(state, NETWORK_DIR / "10-kube-bridge.netdev.d/80-qrab-macsec.conf",
                       f"[NetDev]\nMTUBytes={LOWER_MTU}\n[Bridge]\nGroupForwardMask={mask}\n".encode())
        remember_write(state, NETWORK_DIR / "10-kube-taps.network.d/80-qrab-macsec.conf",
                       f"[Link]\nMTUBytes={LOWER_MTU}\n".encode())
    else:
        original = base64.b64decode(before["keepalived"]["data"]).decode()
        new = render_keepalived(original).encode()
        candidate = state / "keepalived-candidate.conf"
        atomic(candidate, new)
        run(["keepalived", "--config-test", "-f", candidate])
        remember_write(state, Path("/etc/keepalived/keepalived.conf"), new,
                       before["keepalived"]["mode"])
        # A temporary auxiliary IP must never become Kubernetes' NodeInternalIP.
        # Retain the existing ExecStart variables/flags and append the explicit
        # production address. The coordinator stops kubelet during the move.
        kubelet = ("[Service]\nExecStart=\nExecStart=" + before["kubelet_start"]
                   + " --node-ip=" + profile["ip"] + "\n")
        remember_write(state, UNIT_DIR / "kubelet.service.d/80-qrab-macsec.conf", kubelet.encode())
    # Preserve the local IP continuously, then remove its clear lower binding.
    production_addresses(profile)
    run(["networkctl", "reload"])
    run(["networkctl", "reconfigure", UPPER, profile["lower"]], timeout=45)
    run(["ip", "-4", "address", "flush", "dev", profile["lower"], "scope", "global"])
    production_addresses(profile)  # Reassert ordering after networkd applies both addresses.
    sync_firewall(state)
    install_guard(state)
    # Repeat the interface rebinding after a native firewall service reload.
    for service in ("nftables", "iptables", "ip6tables", "netfilter-persistent"):
        loaded = run(["systemctl", "show", "-p", "LoadState", "--value", service + ".service"]).stdout
        if loaded.strip() == b"loaded":
            hook = f"[Service]\nExecStartPost=/usr/bin/python3 -I {state}/deploy_root.py agent --state {state} firewall-sync\n"
            remember_write(state, UNIT_DIR / f"{service}.service.d/80-qrab-macsec.conf", hook.encode())
    run(["systemctl", "daemon-reload"])
    run(["systemctl", "enable", "qrab-macsec.service", "qrab-macsec-guard.service"])
    run(["systemctl", "start", "qrab-macsec-guard.service"])
    phase(state, "activated")
    return {"phase": "activated"}


def verify(state):
    require(current_phase(state) in ("activated", "verified"), "host is not activated")
    profile = load(state / "profile.json")
    before = load(state / "before.json")
    peer_checks(state)
    guarded = json.loads(nft(state, "-j", "list", "table", "netdev", GUARD_TABLE).stdout)
    require(firewall_semantics(guarded) == load(state / "guard-semantics.json"),
            "the lower plaintext guard is missing or changed")
    require(not any(v["scope"] == "global" for v in addresses(profile["lower"])),
            "a clear-IP address remains on the lower interface")
    require(profile["ip"] in {v["local"] for v in addresses(UPPER)},
            "production address is missing from MACsec")
    routes = bgp_routes()
    require(all(routes.get(k) == v for k, v in before["routes"].items()),
            "original BGP next hops have not reconverged")
    require(len(tcp_peers(179)) >= len(before["bgp"]), "BGP sessions have not recovered")
    require(len(tcp_peers(3260)) >= len(before["iscsi"]), "iSCSI connections have not recovered")
    if profile["role"] != "gateway":
        run(["curl", "--cacert", "/etc/kubernetes/pki/ca.crt", "-fsS", "--max-time", "5",
             "https://10.203.0.10:8443/readyz"])
        # Exercise retained filesystem transports without reading data into logs.
        devices = set()
        for path in Path("/sys/class/iscsi_session").glob("session*/device/target*/*/block/*"):
            devices.add("/dev/" + path.name)
        for device in sorted(devices):
            run(["dd", f"if={device}", "of=/dev/null", "bs=4096", "count=1",
                 "iflag=direct", "status=none"], timeout=15)
    phase(state, "verified")
    return {"phase": "verified", "bgp_routes": len(routes),
            "iscsi_connections": len(tcp_peers(3260))}


def capture_clear(state):
    profile = load(state / "profile.json")
    frames = {"ipv4": 0, "ipv6": 0}
    capture = socket.socket(socket.AF_PACKET, socket.SOCK_RAW, socket.htons(3))
    # Filter in the kernel so busy encrypted migration/storage traffic cannot
    # consume the capture budget. Tagged frames are treated as inconclusive.
    class Filter(ctypes.Structure):
        _fields_ = [("code", ctypes.c_ushort), ("jt", ctypes.c_ubyte),
                    ("jf", ctypes.c_ubyte), ("k", ctypes.c_uint32)]
    class Program(ctypes.Structure):
        _fields_ = [("length", ctypes.c_ushort), ("instructions", ctypes.POINTER(Filter))]
    instructions = (Filter * 7)(
        Filter(0x28, 0, 0, 12), Filter(0x15, 4, 0, 0x800),
        Filter(0x15, 3, 0, 0x86DD), Filter(0x15, 2, 0, 0x8100),
        Filter(0x15, 1, 0, 0x88A8), Filter(0x06, 0, 0, 0), Filter(0x06, 0, 0, 64))
    program = Program(7, instructions)
    capture.setsockopt(socket.SOL_SOCKET, 26, bytes(program))
    capture.setsockopt(socket.SOL_SOCKET, socket.SO_RCVBUF, 8 * 1024 * 1024)
    capture.bind((profile["lower"], 0))
    capture.settimeout(.1)
    end = time.monotonic() + 5
    while time.monotonic() < end:
        try:
            packet = capture.recv(64)  # Payload is discarded and never written.
        except socket.timeout:
            continue
        if len(packet) >= 14:
            protocol = int.from_bytes(packet[12:14], "big")
            if protocol == 0x800:
                frames["ipv4"] += 1
            elif protocol == 0x86DD:
                frames["ipv6"] += 1
            elif protocol in (0x8100, 0x88A8):
                frames["unexpected_vlan"] = frames.get("unexpected_vlan", 0) + 1
    received, dropped = struct.unpack("II", capture.getsockopt(263, 6, 8))
    capture.close()
    require(dropped == 0, "lower-link capture dropped packets; verification is inconclusive")
    require(not any(frames.values()), "plaintext IP was observed on the native lower")
    save(state / "capture.json", {"duration_seconds": 5, "received": received,
                                  "dropped": dropped, "clear": frames})
    return {"capture_drops": dropped, "clear": frames}


def commit(state):
    require(current_phase(state) == "verified", "host verification is incomplete")
    require(time.time() < load(state / "profile.json")["deadline"] - 10,
            "rollback deadline is too close to commit safely")
    run(["systemctl", "stop", "qrab-macsec-probe.service"])
    (state / "uncommitted").unlink()
    run(["systemctl", "disable", "--now", "qrab-macsec-rollback.timer"])
    phase(state, "committed")
    return {"phase": "committed"}


def rollback(state):
    before_path = state / "before.json"
    if not before_path.exists() or current_phase(state) in ("prepared", "rolled_back"):
        return {"phase": current_phase(state)}
    profile = load(state / "profile.json")
    before = load(before_path)
    errors = []
    if profile["role"] != "gateway":
        run(["systemctl", "stop", "keepalived", "kubelet"], check=False)
    run(["systemctl", "disable", "--now", "qrab-macsec-rollback.timer"], check=False)
    for unit in ("qrab-macsec-probe.service", "qrab-macsec.service"):
        run(["systemctl", "disable", "--now", unit], check=False)
    run(["systemctl", "disable", "--now", "qrab-macsec-guard.service"], check=False)
    remove_guard(state)
    errors.extend(restore_legacy(state))
    errors.extend(restore_firewall(state))
    errors.extend(restore_files(state))
    run(["systemctl", "daemon-reload"])
    if link(UPPER):
        run(["ip", "link", "del", UPPER])
    run(["ip", "link", "set", profile["lower"], "mtu", str(before["lower"]["mtu"])])
    run(["sysctl", "-q", "-w",
         f"net.ipv6.conf.{profile['lower']}.disable_ipv6={before['ipv6_disabled']}"])
    if profile["role"] == "gateway":
        old = before["lower"]["linkinfo"]["info_data"]["group_fwd_mask"]
        run(["ip", "link", "set", profile["lower"], "type", "bridge", "group_fwd_mask", old])
        for name, mtu in before["taps"].items():
            run(["ip", "link", "set", name, "mtu", str(mtu)])
    run(["networkctl", "reload"])
    run(["networkctl", "reconfigure", profile["lower"]], timeout=45)
    # Reacquire DHCP asynchronously on nodes, but restore their original static
    # lease immediately so rollback does not depend on the gateway's timing.
    run(["ip", "address", "replace", profile["ip"] + "/24", "dev", profile["lower"]])
    if profile["role"] != "gateway":
        run(["ip", "route", "replace", "default", "via", "10.203.0.1",
             "dev", profile["lower"], "onlink"])
        run(["systemctl", "start", "kubelet", "keepalived"])
    (state / "uncommitted").unlink(missing_ok=True)
    run(["systemctl", "disable", "--now", "qrab-macsec-rollback.timer"], check=False)
    phase(state, "rolled_back" if not errors else "rollback_requires_attention")
    save(state / "rollback.json", {"errors": errors, "at": time.time()})
    require(not errors, "; ".join(errors))
    return {"phase": "rolled_back"}


def agent(state, action):
    if action == "guard":
        install_guard(state)
        return {"guard": "installed"}
    if action == "wait-lower":
        profile = load(state / "profile.json")
        for _ in range(200):
            if link(profile["lower"]):
                return {"lower": profile["lower"]}
            time.sleep(.1)
        raise DeployError("native lower was not created by networkd")
    if action == "firewall-sync":
        if current_phase(state) in ("activated", "verified", "committed"):
            sync_firewall(state)
            install_guard(state)
        return {"firewall": "synchronized"}
    if action == "init":
        init_upper(state)
        return {"upper": UPPER}
    if action == "probe-server":
        probe_server(state)
        return {}
    if action == "status":
        return {"phase": current_phase(state), "role": load(state / "profile.json")["role"]}
    if action == "stop-control":
        run(["systemctl", "stop", "keepalived", "kubelet"])
        return {"control": "stopped"}
    if action == "start-control":
        run(["systemctl", "start", "kubelet", "keepalived"])
        return {"control": "started"}
    def timeout_rollback(target):
        if not (target / "uncommitted").exists():
            return {"phase": current_phase(target)}
        return rollback(target)
    actions = {"inspect": inspect_host, "stage": stage, "peers": peer_checks,
               "activate": activate, "verify": verify, "capture": capture_clear,
               "commit": commit, "rollback": rollback, "timeout-rollback": timeout_rollback}
    require(action in actions, "unknown agent action")
    lock = os.open(state / "agent.lock", os.O_CREAT | os.O_RDWR, 0o600)
    try:
        fcntl.flock(lock, fcntl.LOCK_EX)
        return actions[action](state)
    finally:
        os.close(lock)


def ssh_command(user, host, command, *, aux=False):
    account = pwd.getpwnam(user)
    target = host["aux"] if aux else host["ip"]
    ssh = shutil.which("ssh", path=SEARCH_PATH)
    require(ssh is not None, "OpenSSH client is missing")
    return ["runuser", "-u", account.pw_name, "--", ssh, "-T",
            "-o", "BatchMode=yes", "-o", "StrictHostKeyChecking=yes",
            "-o", "ConnectTimeout=5", "-o", "ServerAliveInterval=5",
            "-o", "ServerAliveCountMax=2", "-o", "HostKeyAlias=" + host["ip"],
            "-o", "HostName=" + target, "-o", "ForwardAgent=no",
            "automation@" + host["ip"], command]


def ssh_run(user, host, command, *, aux=False, data=None, check=True, timeout=40):
    # A root login may have an inaccessible working directory. Use the
    # intended account's real home without inheriting caller identity defaults.
    account = pwd.getpwnam(user)
    require(Path(account.pw_dir).is_dir(), "operator home directory is missing")
    return run(ssh_command(user, host, command, aux=aux), data=data,
               check=check, timeout=timeout, cwd=account.pw_dir)


def ssh_failure_reason(stderr):
    message = stderr.decode(errors="replace").lower()
    for phrase, description in (
        ("host key verification failed", "strict host-key verification failed"),
        ("remote host identification has changed", "the recorded host key does not match"),
        ("permission denied", "SSH authentication or remote sudo was denied"),
        ("connection refused", "the node refused the SSH connection"),
        ("timed out", "the SSH connection timed out"),
        ("no route to host", "the node is unreachable"),
        ("could not resolve", "SSH could not resolve the configured host"),
    ):
        if phrase in message:
            return description
    return "SSH or remote sudo failed"


def ssh_preflight(user):
    account = pwd.getpwnam(user)
    require(account.pw_uid != 0, "select the unprivileged operator account with --operator-user")
    code = "import json,os,socket;print(json.dumps({'uid':os.geteuid(),'node':socket.gethostname()}))"
    command = shlex.join(["sudo", "-n", "/usr/bin/python3", "-I", "-c", code])
    results = {}
    for role in ("kube01", "kube02", "kube03"):
        host = HOSTS[role]
        result = ssh_run(user, host, command, check=False, timeout=20)
        if result.returncode:
            detail = ""
            if DIAGNOSTIC_STATE is not None:
                save(DIAGNOSTIC_STATE / "last-command.json",
                     {"argv": ssh_command(user, host, command), "exit": result.returncode,
                      "stderr": result.stderr.decode(errors="replace")[-65536:]})
                detail = f"; private diagnostics: {DIAGNOSTIC_STATE}/last-command.json"
            raise DeployError(f"SSH preflight for {role} as operator {user}: "
                              + ssh_failure_reason(result.stderr) + detail)
        try:
            value = json.loads(result.stdout)
        except ValueError:
            raise DeployError(f"SSH preflight for {role} returned unexpected output") from None
        require(value.get("uid") == 0 and value.get("node", "").split(".")[0] == role,
                f"SSH preflight did not reach root on the expected node {role}")
        results[role] = {"ssh": "verified", "sudo": "verified"}
    return results


def rpc(state, role, action, user, aux=False):
    if role == "gateway":
        return agent(state, action)
    command = shlex.join(["sudo", "-n", "/usr/bin/python3", "-I",
                          str(state / "deploy_root.py"), "agent", "--state", str(state), action])
    timeout = 100
    if action not in ("rollback", "status"):
        remaining = load(state / "profile.json")["deadline"] - time.time() - 15
        require(remaining > 0, "deployment rollback deadline expired")
        timeout = min(timeout, max(1, int(remaining)))
    result=ssh_run(user, HOSTS[role], command, aux=aux, timeout=timeout, check=False)
    if result.returncode:
        stderr=result.stderr.decode(errors="replace")
        if DIAGNOSTIC_STATE is not None:
            save(DIAGNOSTIC_STATE/"last-command.json",
                 {"argv":ssh_command(user,HOSTS[role],command,aux=aux),
                  "exit":result.returncode,"stderr":stderr[-65536:]})
        prefix="qrab native deployment failed: "
        messages=[line[len(prefix):] for line in stderr.splitlines() if line.startswith(prefix)]
        reason=messages[-1] if messages else ssh_failure_reason(result.stderr)
        raise DeployError(f"{role} {action} failed: {reason}")
    try:
        return json.loads(result.stdout)
    except ValueError:
        raise DeployError(f"{role} {action} returned an invalid agent reply") from None


def package(state, role, profile, source, tools, release):
    output = io.BytesIO()
    files = {"deploy_root.py": source, "tools.tar.gz": tools,
             "release.json": json.dumps(release).encode(),
             "profile.json": json.dumps(profile).encode()}
    with tarfile.open(fileobj=output, mode="w:gz") as archive:
        for name, body in files.items():
            entry = tarfile.TarInfo(name)
            entry.mode = 0o600
            entry.size = len(body)
            archive.addfile(entry, io.BytesIO(body))
    return output.getvalue()


def extract_tools(state):
    release = load(state / "release.json")
    archive = state / "tools.tar.gz"
    require(digest(archive.read_bytes()) == release["tools_archive_sha256"],
            "tool archive hash does not match the release pin")
    target = state / "tools"
    target.mkdir(mode=0o700)
    with tarfile.open(archive) as contents:
        for item in contents:
            path = Path(item.name)
            require(not path.is_absolute() and ".." not in path.parts,
                    "unsafe archive member")
            require(item.isfile() or item.isdir(), "links and special files are not permitted in tools")
        contents.extractall(target, filter="data")
    require(digest((target / "wpa_supplicant").read_bytes()) == release["wpa_sha256"],
            "supplicant hash does not match the tested binary")
    run([target / "run-wpa", "-v"])
    run([target / "run-nft", "--version"])


def prepare(state, user, kit):
    global DIAGNOSTIC_STATE
    DIAGNOSTIC_STATE = state
    # Check the actual root-to-operator SSH context before keys, tool copies,
    # node staging directories or any native configuration are created.
    save(state / "ssh-preflight.json", ssh_preflight(user))
    release = load(kit / "deployment-release.json")
    source = (kit / "deploy_root.py").read_bytes()
    tools = (kit / "tools.tar.gz").read_bytes()
    require(digest(tools) == release["tools_archive_sha256"], "tool archive hash mismatch")
    cak, ckn = secrets.token_hex(16), secrets.token_hex(16)
    deadline = int(time.time()) + ROLLBACK_SECONDS
    result = {}
    for role, host in HOSTS.items():
        profile = dict(host, role=role, cak=cak, ckn=ckn, deadline=deadline)
        body = package(state, role, profile, source, tools, release)
        if role == "gateway":
            with tarfile.open(fileobj=io.BytesIO(body)) as archive:
                archive.extractall(state, filter="data")
            extract_tools(state)
        else:
            command = ("sudo -n sh -c " + shlex.quote(
                f"set -eu; umask 077; test ! -e {state}; mkdir -p {BASE}; "
                f"mkdir -m 0700 {state}; tar -xzf - -C {state}"))
            ssh_run(user, host, command, data=body, timeout=100)
            command = shlex.join(["sudo", "-n", "/usr/bin/python3", "-I",
                                  str(state / "deploy_root.py"), "agent", "--state", str(state), "unpack"])
            ssh_run(user, host, command, timeout=40)
        result[role] = rpc(state, role, "inspect", user)
        require(abs(time.time() - result[role]["clock"]) < 5,
                "host clocks differ by more than five seconds")
    save(state / "preflight.json", result)
    return result


def rollback_all(state, user):
    results = {}
    # Keep the gateway MKA/auxiliary path until remote rollback has been sent.
    for role in ("kube03", "kube02", "kube01"):
        errors = []
        for aux in (True, False):
            try:
                results[role] = rpc(state, role, "rollback", user, aux)
                break
            except Exception as error:
                errors.append(str(error))
        else:
            results[role] = {"error": errors[-1], "local_timer": "still armed if uncommitted"}
    try:
        results["gateway"] = rpc(state, "gateway", "rollback", user)
    except Exception as error:
        results["gateway"] = {"error": str(error)}
    # An SSH reply can disappear when a node removes its auxiliary interface.
    # Confirm such nodes again over the restored original network.
    for role in ("kube01", "kube02", "kube03"):
        if "error" in results.get(role, {}):
            try:
                status = rpc(state, role, "status", user)
                if status["phase"] in ("prepared", "rolled_back"):
                    results[role] = status
            except Exception:
                pass
    save(state / "cluster-rollback.json", results)
    return results


def staging_peers(state, user, seconds=60):
    # New members and a newly elected key server may take several MKA rounds
    # to install every receive SC. Never move production addresses until a
    # complete round of all twelve directed TCP paths succeeds.
    end=time.monotonic()+seconds
    attempts=[]
    while True:
        failures={}
        for role in HOSTS:
            try:
                rpc(state,role,"peers",user,aux=role!="gateway")
            except (DeployError,OSError) as error:
                failures[role]=str(error)
        attempts.append({"at":time.time(),"failures":failures})
        save(state/"staging-peer-attempts.json",attempts)
        if not failures:
            return
        if time.monotonic()>=end:
            raise DeployError("encrypted staging peers did not converge: "
                              + "; ".join(role+": "+reason for role,reason in failures.items()))
        time.sleep(1)


def deploy_cluster(state, user):
    receipts = {}
    try:
        # Start all node daemons without depending on the gateway daemon's
        # readiness. All four peers use the same freshly generated CAK.
        for role in ("kube01", "kube02", "kube03", "gateway"):
            receipts[role] = rpc(state, role, "stage", user)
        staging_peers(state,user)
        for role in ("kube01", "kube02", "kube03"):
            rpc(state, role, "stop-control", user, aux=True)
        for role in HOSTS:
            rpc(state, role, "activate", user, aux=role != "gateway")
        for role in ("kube01", "kube02", "kube03"):
            rpc(state, role, "start-control", user, aux=True)
        for _ in range(24):
            try:
                for role in HOSTS:
                    receipts[role] = rpc(state, role, "verify", user, aux=role != "gateway")
                break
            except (DeployError, OSError):
                time.sleep(3)
        else:
            raise DeployError("API/BGP/storage verification did not converge")
        for role in HOSTS:
            receipts[role]["capture"] = rpc(state, role, "capture", user, aux=role != "gateway")
        # Native rollback timers stay armed until all four have passed.
        for role in ("kube01", "kube02", "kube03", "gateway"):
            rpc(state, role, "commit", user, aux=role != "gateway")
        save(state / "cluster-receipt.json", receipts)
        return {"state": str(state), "deployment": "committed", "hosts": receipts}
    except BaseException as error:
        # Cleanup has its own SSH failures as auxiliary addresses disappear.
        # Preserve the triggering error before rollback can replace diagnostics.
        record = {"error": str(error), "at": time.time()}
        if (state / "last-command.json").exists():
            record["command"] = load(state / "last-command.json")
        try:
            save(state / "primary-error.json", record)
        except OSError:
            pass  # Native recovery must still run if diagnostic storage fails.
        try:
            rollback_all(state, user)
        except BaseException as cleanup_error:
            try:
                save(state / "rollback-controller-error.json", {"error": str(cleanup_error)})
            except OSError:
                pass
        raise


def coordinator(action, user, kit, selected):
    require(os.geteuid() == 0, "run this script as root on the physical gateway")
    BASE.mkdir(mode=0o700, exist_ok=True)
    require(BASE.stat().st_uid == 0 and not BASE.is_symlink(), "state directory is not trusted")
    require(stat.S_IMODE(BASE.stat().st_mode) == 0o700, "state directory must have mode 0700")
    lock = os.open(BASE / "coordinator.lock", os.O_CREAT | os.O_RDWR, 0o600)
    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    try:
        index = BASE / "current.json"
        if not selected and index.exists():
            selected = load(index)["state"]
        if action in ("status", "rollback"):
            require(selected, "no current deployment; specify --state")
            state = state_path(selected)
            if action == "rollback":
                results = rollback_all(state, user)
                require(not any("error" in value for value in results.values()),
                        f"rollback needs attention; inspect {state}/cluster-rollback.json")
                return results
            return {"state": str(state), "phase": current_phase(state)}
        require(action in ("preflight", "deploy"), "unknown coordinator action")
        if action == "deploy" and selected:
            previous = state_path(selected)
            status = current_phase(previous)
            if status == "committed":
                return {"state": str(previous), "deployment": "already committed",
                        "key_action": "none"}
            require(status in ("rolled_back", "prepared"),
                    "a previous deployment is incomplete; run status/rollback first")
        state = BASE / secrets.token_hex(12)
        state.mkdir(mode=0o700)
        print(json.dumps({"state": str(state), "action": action, "operator_user": user}), flush=True)
        prepare(state, user, kit)
        if action == "preflight":
            return {"state": str(state), "preflight": "passed", "network_changes": False}
        save(index, {"state": str(state), "operator_user": user})
        return deploy_cluster(state, user)
    finally:
        os.close(lock)


def arguments(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("preflight", "deploy", "status", "rollback", "agent"))
    parser.add_argument("--operator-user", default="bob",
                        help="local account whose SSH configuration/keys reach the nodes (default: bob)")
    parser.add_argument("--state")
    parser.add_argument("agent_action", nargs="?")
    return parser.parse_intermixed_args(argv)


def main():
    args = arguments()
    os.umask(0o077)
    signal.signal(signal.SIGTERM, lambda *_: (_ for _ in ()).throw(DeployError("terminated")))
    if args.action == "agent":
        require(os.geteuid() == 0, "agent requires root")
        state = state_path(args.state)
        global DIAGNOSTIC_STATE
        DIAGNOSTIC_STATE = state
        if args.agent_action == "unpack":
            extract_tools(state)
            result = {"tools": "verified"}
        else:
            result = agent(state, args.agent_action)
    else:
        result = coordinator(args.action, args.operator_user, Path(__file__).resolve().parent,
                             args.state)
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    try:
        main()
    except (DeployError, OSError, ValueError) as error:
        print(f"qrab native deployment failed: {error}", file=sys.stderr)
        raise SystemExit(1)
