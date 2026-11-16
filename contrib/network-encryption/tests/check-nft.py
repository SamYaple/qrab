#!/usr/bin/python3
"""Privileged firewall journal checks in an owned network namespace."""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


def main():
    if len(sys.argv) == 3 and sys.argv[1] == "--inside":
        state = Path(sys.argv[2])
        spec = importlib.util.spec_from_file_location("deploy", state / "deploy_root.py")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        module.save(state / "profile.json", {"lower": "eth0"})
        module.run(["ip", "link", "add", "eth0", "type", "dummy"])
        module.run(["ip", "link", "add", "link", "eth0", "name", "macsec0", "type", "macsec"])
        source = {"nftables": [
            {"add": {"table": {"family": "inet", "name": "fixture"}}},
            {"add": {"chain": {"family": "inet", "table": "fixture", "name": "forward"}}},
            {"add": {"rule": {"family": "inet", "table": "fixture", "chain": "forward",
                              "expr": [{"match": {"left": {"meta": {"key": "iifname"}},
                                                  "op": "==", "right": "eth0"}},
                                       {"match": {"left": {"payload": {"protocol": "tcp", "field": "dport"}},
                                                  "op": "==", "right": 443}},
                                       {"counter": None}, {"accept": None}]}}}]}
        module.nft(state, "-j", "-f", "-", data=json.dumps(source).encode())
        def rules():
            value = json.loads(module.nft(state, "-j", "list", "table", "inet", "fixture").stdout)
            return [v["rule"] for v in value["nftables"] if "rule" in v]
        original = module.firewall_semantics(rules()[0]["expr"])
        module.install_guard(state)
        module.sync_firewall(state, both=True)
        staged = rules()[0]
        assert staged["expr"][0]["match"]["right"] == {"set": ["eth0", "macsec0"]}
        module.sync_firewall(state)
        active = rules()[0]
        right = active["expr"][0]["match"]["right"]
        assert right == "macsec0" or right == {"set": ["macsec0"]}
        errors=module.restore_firewall(state)
        assert not errors, errors
        assert module.firewall_semantics(rules()[0]["expr"]) == original
        # A crash after kernel creation but before saving the marker must not
        # prevent removal of our own guard on rollback.
        (state / "guard-installed").unlink()
        module.remove_guard(state)
        assert module.run([state / "tools/run-nft", "list", "table", "netdev", module.GUARD_TABLE],
                          check=False).returncode != 0
        # Restore must preserve a subsequent user edit to an owned rule.
        module.sync_firewall(state)
        changed = rules()[0]
        changed["expr"][1]["match"]["right"] = 8443
        replacement = {k: changed[k] for k in ("family", "table", "chain", "handle", "expr")}
        module.nft(state, "-j", "-f", "-",
                   data=json.dumps({"nftables": [{"replace": {"rule": replacement}}]}).encode())
        assert module.restore_firewall(state)
        assert rules()[0]["expr"][1]["match"]["right"] == 8443
        if shutil.which("iptables-legacy", path=module.SEARCH_PATH):
            module.legacy_programs = lambda: [("iptables-legacy", "iptables-legacy-save")]
            module.run(["iptables-legacy", "-N", "QRABLEGACY"])
            module.run(["iptables-legacy", "-A", "QRABLEGACY", "-i", "eth0", "-p", "tcp",
                        "--dport", "443", "-m", "comment", "--comment", "retain user comment",
                        "-j", "ACCEPT"])
            def legacy():
                return [v for v in module.legacy_rules(module.run(["iptables-legacy-save"]).stdout.decode())
                        if v["chain"] == "QRABLEGACY"]
            original_legacy = legacy()[0]["body"]
            module.sync_legacy(state)
            assert len(legacy()) == 2
            assert legacy()[0]["body"] == original_legacy
            module.sync_legacy(state)
            assert len(legacy()) == 2, "legacy sync is not idempotent"
            assert not module.restore_legacy(state)
            assert [v["body"] for v in legacy()] == [original_legacy]
            legacy_result = True
        else:
            legacy_result = "not available"
        module.run(["ip", "link", "set", "eth0", "up"])
        module.run(["ip", "link", "set", "macsec0", "up"])
        module.run(["ip", "address", "add", "192.0.2.11/24", "dev", "macsec0"])
        profile = {"role":"kube01", "ip":"10.203.0.11", "aux":"192.0.2.11"}
        module.production_addresses(profile)
        own = [v["local"] for v in module.addresses("macsec0") if v["family"]=="inet"]
        assert own == ["10.203.0.11","192.0.2.11"]
        default = module.jrun(["ip","-j","-4","route","show","default"])
        assert default[0]["prefsrc"]=="10.203.0.11"
        print(json.dumps({"firewallRoundtrip": True, "guardMarkerCrashRecovered": True,
                          "externalEditPreserved": True, "legacyRoundtrip": legacy_result,
                          "productionSourcePreserved":True}))
        return
    if os.geteuid() != 0 or len(sys.argv) != 3:
        raise SystemExit("usage: sudo python3 check-nft.py DEPLOY_ROOT_PY EXPORTED_TOOLS")
    program, tools = map(Path, sys.argv[1:])
    namespace = "qrabnft" + str(os.getpid())
    with tempfile.TemporaryDirectory(prefix="qrab-nft-state-", dir="/run") as temporary:
        state = Path(temporary)
        (state / "tools").symlink_to(tools.resolve())
        shutil.copyfile(program, state / "deploy_root.py")
        subprocess.run(["ip", "netns", "add", namespace], check=True)
        try:
            subprocess.run(["ip", "netns", "exec", namespace, sys.executable,
                            str(Path(__file__).resolve()), "--inside", str(state)], check=True)
        finally:
            subprocess.run(["ip", "netns", "del", namespace], check=True)


if __name__ == "__main__":
    main()
