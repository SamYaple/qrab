"""Deployment safety contracts. No root, manager, systemd or network required."""
import base64
import copy
import importlib.util
import io
import json
import os
from pathlib import Path
import tarfile
import tempfile
import subprocess
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location(
    "deploy_root", Path(__file__).resolve().parents[1] / "deploy_root.py")
D = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(D)


def rule(expr, handle=7):
    return {"family": "inet", "table": "filter", "chain": "forward",
            "handle": handle, "expr": expr, "comment": "preserve user policy"}


def match(key, value):
    return {"match": {"left": {"meta": {"key": key}}, "op": "==", "right": value}}


class FirewallContracts(unittest.TestCase):
    def test_legacy_clones_preserve_original_policy_and_order(self):
        text='*filter\n:FORWARD DROP [0:0]\n-A FORWARD -i br-kube -o br0 -p tcp -m comment --comment "keep me" -j ACCEPT\nCOMMIT\n'
        plan=D.legacy_plans(text,"br-kube","a"*24)[0]
        self.assertEqual(plan["position"],1)
        self.assertEqual(plan["body"][1],"br-kube")
        self.assertEqual(plan["clone"][1],"macsec0")
        self.assertIn("keep me",plan["clone"])
        self.assertEqual(plan["clone"][-2:],["-j","ACCEPT"])
        self.assertEqual(D.legacy_marker(plan["clone"]),plan["marker"])

    def test_legacy_negative_and_wildcard_interface_refuse(self):
        for body in ("! -i br-kube","-i br+","-m physdev --physdev-in br-kube"):
            with self.subTest(body=body),self.assertRaises(D.DeployError):
                D.legacy_plans("*filter\n-A INPUT "+body+" -j DROP\nCOMMIT\n","br-kube","x")

    def test_both_phases_preserve_other_predicates_and_counters(self):
        expression = [match("iifname", "br-kube"), match("oifname", "br0"),
                      {"counter": {"packets": 99, "bytes": 1234}}, {"accept": None}]
        old = rule(expression)
        source = {"nftables": [{"rule": old}]}
        staged, before = D.firewall_changes(source, "br-kube", "macsec0", 38, 40, both=True)
        staged_rule = staged["nftables"][0]["replace"]["rule"]
        self.assertEqual(staged_rule["expr"][0], match("iifname", {"set": ["br-kube", "macsec0"]}))
        self.assertEqual(staged_rule["expr"][1:], expression[1:])
        active, _ = D.firewall_changes({"nftables": [{"rule": staged_rule}]},
                                      "br-kube", "macsec0", 38, 40)
        final = active["nftables"][0]["replace"]["rule"]
        self.assertEqual(final["expr"][0], match("iifname", "macsec0"))
        self.assertEqual(final["expr"][1:], expression[1:])
        self.assertEqual(before, [old])
        self.assertEqual(source["nftables"][0]["rule"]["expr"], expression)

    def test_numeric_interface_and_unrelated_numeric_values(self):
        expr = [match("iif", 38), {"match": {"left": {"payload": {"protocol": "tcp", "field": "dport"}},
                                            "op": "==", "right": 38}}, {"accept": None}]
        changed, _ = D.firewall_changes({"nftables": [{"rule": rule(expr)}]},
                                        "br-kube", "macsec0", 38, 55)
        value = changed["nftables"][0]["replace"]["rule"]["expr"]
        self.assertEqual(value[0], match("iif", 55))
        self.assertEqual(value[1], expr[1])

    def test_negated_interface_retains_operator(self):
        expr = match("oifname", "br-kube")
        expr["match"]["op"] = "!="
        value = D.rebind_expr(expr, "br-kube", "macsec0", 2, 3)
        self.assertEqual(value["match"]["op"], "!=")
        self.assertEqual(value["match"]["right"], "macsec0")

    def test_opaque_xt_interface_rule_is_refused(self):
        source = {"nftables": [{"rule": rule([match("iifname", "br-kube"), {"xt": {}}])}]}
        with self.assertRaises(D.DeployError):
            D.firewall_changes(source, "br-kube", "macsec0", 38, 40)

    def test_named_set_or_bound_chain_requires_explicit_migration(self):
        for item in ({"chain": {"dev": "br-kube"}},
                     {"element": {"elem": ["br-kube"]}},
                     {"rule": rule([{"lookup": {"name": "br-kube"}}])}):
            with self.subTest(item=item), self.assertRaises(D.DeployError):
                D.firewall_changes({"nftables": [item]}, "br-kube", "macsec0", 38, 40)

    def test_guard_stays_on_raw_lower(self):
        generated = D.guard_commands({"lower": "br-kube"}, "abc")
        chains = [v["add"]["chain"] for v in generated["nftables"] if "chain" in v.get("add", {})]
        self.assertEqual({v["hook"] for v in chains}, {"ingress", "egress"})
        self.assertTrue(all(v["dev"] == "br-kube" and v["policy"] == "drop" for v in chains))
        allowed = [v["add"]["rule"] for v in generated["nftables"]
                   if "rule" in v.get("add", {}) and "match" in v["add"]["rule"]["expr"][0]]
        self.assertEqual(len(allowed), 2)
        for value in allowed:
            self.assertEqual(value["expr"][0]["match"]["right"]["set"], [0x88E5, 0x888E])
        listed = {"nftables": [v["add"] for v in generated["nftables"]]}
        changes, before = D.firewall_changes(listed, "br-kube", "macsec0", 38, 40)
        self.assertEqual(changes["nftables"], [])
        self.assertEqual(before, [])

    def test_mutable_counters_do_not_hide_semantic_changes(self):
        one = {"nftables": [{"metainfo": {"generation": 1}},
                           {"rule": rule([{"counter": {"packets": 7}}, {"drop": None}])}]}
        two = copy.deepcopy(one)
        two["nftables"][0]["metainfo"]["generation"] = 900
        two["nftables"][1]["rule"]["handle"] = 20
        two["nftables"][1]["rule"]["expr"][0]["counter"]["packets"] = 9999
        self.assertEqual(D.firewall_semantics(one), D.firewall_semantics(two))
        two["nftables"][1]["rule"]["expr"][1] = {"accept": None}
        self.assertNotEqual(D.firewall_semantics(one), D.firewall_semantics(two))


class FileTransactions(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.state = self.root / "state"
        self.state.mkdir()

    def tearDown(self):
        self.temp.cleanup()

    def test_restore_original_bytes_mode_and_created_file(self):
        existing = self.root / "original"
        existing.write_bytes(b"original\n")
        existing.chmod(0o640)
        created = self.root / "new"
        D.remember_write(self.state, existing, b"owned\n")
        D.remember_write(self.state, existing, b"owned-second-phase\n")
        D.remember_write(self.state, created, b"new\n")
        self.assertEqual(D.restore_files(self.state), [])
        self.assertEqual(existing.read_bytes(), b"original\n")
        self.assertEqual(existing.stat().st_mode & 0o777, 0o640)
        self.assertFalse(created.exists())

    def test_external_edits_are_not_overwritten(self):
        path = self.root / "config"
        D.remember_write(self.state, path, b"owned")
        path.write_bytes(b"user edit")
        self.assertTrue(D.restore_files(self.state))
        self.assertEqual(path.read_bytes(), b"user edit")

    def test_symlink_is_never_followed_or_silently_accepted(self):
        path, victim = self.root / "config", self.root / "victim"
        victim.write_bytes(b"leave alone")
        D.remember_write(self.state, path, b"owned")
        path.unlink()
        path.symlink_to(victim)
        self.assertTrue(D.restore_files(self.state))
        self.assertEqual(victim.read_bytes(), b"leave alone")
        with self.assertRaises(D.DeployError):
            D.remember_write(self.state, path, b"wrong")

    def test_failed_atomic_write_does_not_replace_original(self):
        path = self.root / "config"
        path.write_bytes(b"original")
        with patch.object(D.os, "replace", side_effect=OSError("injected")):
            with self.assertRaises(OSError):
                D.atomic(path, b"changed")
        self.assertEqual(path.read_bytes(), b"original")
        self.assertEqual(sorted(v.name for v in self.root.iterdir()), ["config", "state"])

    def test_crash_between_second_journal_and_file_write_restores_first_version(self):
        path = self.root / "config"
        path.write_bytes(b"original")
        D.remember_write(self.state, path, b"stage")
        real = D.atomic
        def interrupted(target, data, mode=0o600):
            if Path(target)==path:
                raise OSError("power loss before second replace")
            return real(target,data,mode)
        with patch.object(D,"atomic",side_effect=interrupted),self.assertRaises(OSError):
            D.remember_write(self.state,path,b"activation")
        self.assertEqual(path.read_bytes(),b"stage")
        self.assertEqual(D.restore_files(self.state),[])
        self.assertEqual(path.read_bytes(),b"original")


class BootAndIdentity(unittest.TestCase):
    def test_keepalived_preserves_ips_and_only_moves_devices(self):
        source = """global_defs { router_id kube01 }\n vrrp_instance KUBERNETES_API {\n    interface enp0s2\n    unicast_src_ip 10.203.0.11\n    virtual_ipaddress {\n        10.203.0.10/24 dev enp0s2\n    }\n}\n"""
        changed = D.render_keepalived(source)
        self.assertIn("unicast_src_ip 10.203.0.11", changed)
        self.assertIn("10.203.0.10/24 dev macsec0", changed)
        self.assertNotIn("enp0s2", changed)
        with self.assertRaises(D.DeployError):
            D.render_keepalived("interface arbitrary\n")

    def test_gateway_preserves_dhcp_reservations(self):
        text = "[Match]\nName=br-kube\n[Network]\nAddress=10.203.0.1/24\nDHCPServer=yes\n[DHCPServerStaticLease]\nMACAddress=52:54:00:00:00:01\nAddress=10.203.0.11\n"
        profile = dict(D.HOSTS["gateway"], role="gateway")
        before = {"bridge_network": {"data": base64.b64encode(text.encode()).decode()}}
        lower, upper = D.render_network(profile, before)
        self.assertNotIn("Address=", lower)
        self.assertIn("DHCP=no", lower)
        self.assertIn("Name=macsec0", upper)
        self.assertIn("Address=10.203.0.11", upper)
        self.assertIn("Address=192.0.2.1/24", upper)
        self.assertIn("RequiredForOnline=no", upper)

    def test_standard_mka_not_static_transmit_keys(self):
        profile = dict(D.HOSTS["kube01"], role="kube01", cak="a"*32, ckn="b"*32)
        config = D.mka_config(profile)
        self.assertIn("macsec_integ_only=0", config)
        self.assertIn("macsec_replay_protect=1", config)
        self.assertIn("mka_cak=" + "a"*32, config)
        self.assertNotIn("tx sa", config)

    def test_boot_requires_guard_and_does_not_wait_for_kubernetes(self):
        with tempfile.TemporaryDirectory() as root:
            state = Path(root)
            D.phase(state, "committed")
            profile = dict(D.HOSTS["kube01"], role="kube01", deadline=1800000000)
            units = D.systemd_units(state, profile)
            wpa = units["qrab-macsec.service"]
            self.assertIn("Requires=qrab-macsec-guard.service", wpa)
            self.assertIn("Before=network-online.target kubelet.service", wpa)
            self.assertNotIn("After=kubelet", wpa)
            self.assertIn("Persistent=yes", units["qrab-macsec-rollback.timer"])
            self.assertIn("ConditionPathExists=", units["qrab-macsec-rollback.service"])
            self.assertNotIn("cak=", wpa)

    def test_initial_stage_does_not_enable_lower_guard(self):
        with tempfile.TemporaryDirectory() as root:
            state = Path(root)
            D.phase(state, "staging")
            units = D.systemd_units(state, dict(D.HOSTS["gateway"], deadline=1800000000))
            self.assertNotIn("Requires=qrab-macsec-guard", units["qrab-macsec.service"])

    def test_ssh_aux_uses_original_host_configuration_and_pinned_host_key(self):
        command = D.ssh_command("root", D.HOSTS["kube01"], "public command", aux=True)
        self.assertIn("automation@10.203.0.11", command)
        self.assertIn("HostName=192.0.2.11", command)
        self.assertIn("HostKeyAlias=10.203.0.11", command)
        self.assertIn("StrictHostKeyChecking=yes", command)
        self.assertIn("ForwardAgent=no", command)

    def test_boot_init_never_needs_the_cilium_container(self):
        with tempfile.TemporaryDirectory() as root:
            state = Path(root)
            profile = dict(D.HOSTS["kube01"], role="kube01")
            D.save(state / "profile.json", profile)
            D.phase(state, "committed")
            upper = {"ifindex": 99, "link": "enp0s2", "mtu": 1500,
                     "linkinfo": {"info_kind": "macsec", "info_data": {
                         "encrypt": True, "protect": True, "validation": "strict",
                         "replay_protect": True, "window": 0}}}
            calls = []
            def execute(argv, **kwargs):
                calls.append([str(v) for v in argv])
                return subprocess.CompletedProcess(argv, 0, b"", b"")
            with patch.object(D, "link", side_effect=lambda name: upper if name=="macsec0" else {"ifindex":2}), \
                 patch.object(D, "run", side_effect=execute), \
                 patch.object(D, "addresses", return_value=[]), \
                 patch.object(D, "install_guard", side_effect=lambda _: calls.append(["guard"])), \
                 patch.object(D, "sync_firewall"), \
                 patch.object(D, "cilium", side_effect=AssertionError("boot must not require Kubernetes")):
                D.init_upper(state)
            primary = next(i for i,v in enumerate(calls) if profile["ip"]+"/24" in v)
            self.assertLess(calls.index(["guard"]), primary)

    def test_auxiliary_address_does_not_become_default_source(self):
        profile=dict(D.HOSTS["kube01"],role="kube01")
        calls=[]
        with patch.object(D,"addresses",return_value=[{"local":profile["aux"]}]), \
             patch.object(D,"run",side_effect=lambda argv: calls.append(argv)):
            D.production_addresses(profile)
        self.assertIn(["ip","address","del",profile["aux"]+"/24","dev","macsec0"],calls)
        self.assertEqual(calls[-1][-2:],["src","10.203.0.11"])
        config=D.render_network(profile,{})[1]
        self.assertIn("PreferredSource=10.203.0.11",config)

    def test_expired_timer_cannot_undo_a_completed_commit(self):
        with tempfile.TemporaryDirectory() as root:
            state=Path(root)
            D.phase(state,"committed")
            with patch.object(D,"rollback",side_effect=AssertionError("committed timer race")):
                self.assertEqual(D.agent(state,"timeout-rollback"),{"phase":"committed"})


class ClusterTransactions(unittest.TestCase):
    def exercise(self, failure=None):
        with tempfile.TemporaryDirectory() as root:
            state=Path(root)
            calls=[]
            failed=False
            states={role:"prepared" for role in D.HOSTS}
            def request(st,role,action,user,aux=False):
                nonlocal failed
                calls.append((role,action))
                if failure == (role,action) and not failed:
                    failed=True
                    raise D.DeployError("injected failure")
                if action=="verify" and failure=="always_verify":
                    raise D.DeployError("injected verification failure")
                values={"stage":"staged","activate":"activated","verify":"verified",
                        "commit":"committed","rollback":"rolled_back"}
                if action in values:states[role]=values[action]
                return {"phase":states[role]}
            with patch.object(D,"rpc",side_effect=request),patch.object(D.time,"sleep"):
                if failure:
                    with self.assertRaises(D.DeployError):
                        D.deploy_cluster(state,"root")
                else:
                    result=D.deploy_cluster(state,"root")
                    self.assertEqual(result["deployment"],"committed")
            return calls,states

    def test_all_peers_verified_and_captured_before_any_commit(self):
        calls,states=self.exercise()
        first_commit=next(i for i,v in enumerate(calls) if v[1]=="commit")
        self.assertEqual({r for r,a in calls[:first_commit] if a=="capture"},set(D.HOSTS))
        first_activate=next(i for i,v in enumerate(calls) if v[1]=="activate")
        self.assertEqual({r for r,a in calls[:first_activate] if a=="peers"},set(D.HOSTS))
        self.assertTrue(all(v=="committed" for v in states.values()))

    def test_stage_failure_rolls_back_every_participant(self):
        calls,states=self.exercise(("kube02","stage"))
        self.assertNotIn("activate",[a for _,a in calls])
        self.assertTrue(all(v=="rolled_back" for v in states.values()))

    def test_unconverged_verification_never_commits(self):
        calls,states=self.exercise("always_verify")
        self.assertNotIn("commit",[a for _,a in calls])
        self.assertEqual(sum(a=="verify" for _,a in calls),24)
        self.assertTrue(all(v=="rolled_back" for v in states.values()))

    def test_partial_commit_failure_rolls_back_committed_nodes_too(self):
        calls,states=self.exercise(("kube02","commit"))
        self.assertIn(("kube01","commit"),calls)
        self.assertTrue(all(v=="rolled_back" for v in states.values()))


if __name__ == "__main__":
    unittest.main()
