#!/usr/bin/env python3
"""Real MKA association recovery in private namespaces; no production changes."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import secrets
import subprocess
import tempfile
import tarfile
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("agent", type=Path)
    parser.add_argument("tools", type=Path)
    parser.add_argument("--previous-tools", type=Path,
                        help="also exercise the public agent's tool upgrade against this old bundle")
    args = parser.parse_args()
    assert os.geteuid() == 0
    os.umask(0o077)
    spec = importlib.util.spec_from_file_location("native", args.agent)
    native = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(native)
    # No OS boot is simulated here. Keep the production failure debounce,
    # priority staggering and persistent restart cooldown.
    native.HEALTH_STARTUP_SECONDS = 0
    names = ["gateway", "node1", "node2", "node3", "wrong"]
    trusted = names[:4]
    prefix = "qh" + str(os.getpid())
    namespaces = {name: prefix + str(i) for i, name in enumerate(names)}
    lowers = {name: "br0" if name == "gateway" else "eth0" for name in names}
    ips = {name: "192.0.2." + str(i + 1) for i, name in enumerate(names)}
    owned = []
    processes = []
    outputs = []
    supplicants = {}
    configs = {}
    pending_fresh = {}
    started = time.monotonic()
    health = {name: {"bad": None, "previous": {}} for name in trusted}
    recoveries = []
    results = []
    health_paths = {}
    guards = {}

    def run(*argv, check=True, data=None):
        result = subprocess.run([str(a) for a in argv], input=data, capture_output=True, timeout=12)
        if check and result.returncode:
            raise RuntimeError("fixture command failed: " + result.stderr.decode(errors="replace")[-400:])
        return result

    def ip(name, *argv, **kwargs):
        return run("ip", "-n", namespaces[name], *argv, **kwargs)

    def inside(name, *argv, **kwargs):
        return run("ip", "netns", "exec", namespaces[name], *argv, **kwargs)

    def launch(name, label, *argv):
        output = (work / (label + ".log")).open("wb")
        outputs.append(output)
        process = subprocess.Popen(["ip", "netns", "exec", namespaces[name],
                                    *[str(a) for a in argv]], stdout=output, stderr=output)
        processes.append(process)
        return process

    def state(name):
        rows = json.loads(ip(name, "-j", "-s", "macsec", "show").stdout)
        return next((row for row in rows if row["ifname"] == "macsec0"), None)

    def encoding_key(row):
        return next((sa.get("key") for sa in row.get("sa_list", [])
                     if sa.get("an") == row.get("encoding_sa") and sa.get("active")), None)

    def address(name):
        for _ in range(100):
            if ip(name, "link", "show", "macsec0", check=False).returncode == 0:
                break
            time.sleep(.1)
        else:
            raise RuntimeError("supplicant did not create its upper")
        ip(name, "addr", "replace", ips[name] + "/24", "dev", "macsec0")
        ip(name, "link", "set", "macsec0", "mtu", "1500", "up")

    def restart(name, offline=0):
        row = state(name)
        if row and encoding_key(row):
            pending_fresh[name] = encoding_key(row)
        supplicants[name].terminate()
        supplicants[name].wait(timeout=5)
        if offline:
            ip(name, "link", "set", lowers[name], "down")
            time.sleep(offline)
            ip(name, "link", "set", lowers[name], "up")
        supplicants[name] = launch(name, name + "-restart-" + str(len(processes)),
                                   args.tools / "run-wpa", "-Dmacsec_linux",
                                   "-i" + lowers[name], "-c", configs[name])
        address(name)

    probe = """import socket,sys
s=socket.socket(socket.AF_INET,socket.SOCK_DGRAM);s.settimeout(.4)
s.setsockopt(socket.SOL_SOCKET,socket.SO_BINDTODEVICE,b'macsec0\\0')
s.bind((sys.argv[1],0))
for target in sys.argv[2:]:
 s.sendto(b'qrab-health',(target,43435));data,peer=s.recvfrom(128)
 assert data==b'qrab-health' and peer[0]==target
"""

    def ready(label):
        deadline = time.monotonic() + 240
        while time.monotonic() < deadline:
            rows = {name: state(name) for name in trusted}
            changed = False
            for ordinal, name in enumerate(trusted):
                status = native.macsec_health([rows[name]] if rows[name] else [])
                record = health[name]
                now = time.monotonic()
                record["bad"], due = native.health_recovery_due(
                    status, now, started, record["bad"], record["previous"], "fixture",
                    failure_seconds=native.HEALTH_FAILURE_SECONDS + ordinal * native.HEALTH_PRIORITY_STEP_SECONDS)
                if due:
                    record["previous"] = {"boot_id": "fixture", "monotonic": now}
                    recoveries.append({"peer": name, "status": status})
                    print(json.dumps({"recovering": name, "phase": label}), flush=True)
                    for peer in trusted:
                        key = encoding_key(rows[peer]) if rows[peer] else None
                        if key:pending_fresh[peer] = key
                    assert native.wpa_control(health_paths[name], "MKA_REKEY",
                                              work / ("ctrl-" + name)) == b"OK\n"
                    changed = True
                    break
            complete = not changed and all(
                row and native.macsec_health([row])["healthy"] and len(row.get("rx_sc", [])) == 3
                for row in rows.values())
            if complete:
                for name, before in list(pending_fresh.items()):
                    after = encoding_key(rows[name])
                    if after and after != before:
                        del pending_fresh[name]
                complete = not pending_fresh
            if complete and all(inside(name, "python3", "-c", probe, ips[name],
                                      *[ips[p] for p in trusted if p != name],
                                      check=False).returncode == 0 for name in trusted):
                results.append(label)
                print(json.dumps({"all12Paths": label}), flush=True)
                return
            time.sleep(1)
        raise RuntimeError("association recovery did not restore all twelve paths: " + label)

    with tempfile.TemporaryDirectory(prefix="qrab-association-health-", dir="/run") as tmp:
        work = Path(tmp)
        try:
            for name in names:
                run("ip", "netns", "add", namespaces[name]); owned.append(name)
                ip(name, "link", "set", "lo", "up")
            ip("gateway", "link", "add", "br0", "type", "bridge", "group_fwd_mask", "8")
            ip("gateway", "link", "set", "br0", "address", "02:00:00:00:00:01", "mtu", "1532", "up")
            for i, name in enumerate(names[1:], 2):
                port = "p" + str(i)
                ip("gateway", "link", "add", port, "type", "veth", "peer", "name", "e" + str(i))
                ip("gateway", "link", "set", "e" + str(i), "netns", namespaces[name])
                ip(name, "link", "set", "e" + str(i), "name", "eth0")
                ip(name, "link", "set", "eth0", "address", "02:00:00:00:00:" + f"{i:02x}", "mtu", "1532", "up")
                ip("gateway", "link", "set", port, "mtu", "1532", "master", "br0", "up")
            cak, ckn = secrets.token_hex(16), secrets.token_hex(16)
            for i, name in enumerate(names):
                inside(name, "sysctl", "-q", "-w", "net.ipv6.conf." + lowers[name] + ".disable_ipv6=1")
                guard = native.guard_commands({"lower": lowers[name]}, prefix)
                inside(name, args.tools / "run-nft", "-j", "-f", "-", data=json.dumps(guard).encode())
                guards[name] = native.firewall_semantics(json.loads(inside(
                    name, args.tools / "run-nft", "-j", "list", "table", "netdev", native.GUARD_TABLE).stdout))
                config = work / (name + ".conf")
                key = cak if name != "wrong" else secrets.token_hex(16)
                config.write_text(f"ctrl_interface={work / ('ctrl-' + name)}\n"
                                  "eapol_version=3\nap_scan=0\nnetwork={\nkey_mgmt=NONE\n"
                                  "eapol_flags=0\nmacsec_policy=1\nmacsec_integ_only=0\n"
                                  "macsec_replay_protect=1\nmacsec_replay_window=0\n"
                                  f"mka_cak={key}\nmka_ckn={ckn}\nmka_priority={i*16}\n}}\n")
                configs[name] = config
                health_paths[name] = work / ("health-" + name)
                health_paths[name].mkdir()
                (health_paths[name] / "profile.json").write_text(json.dumps({"lower": lowers[name]}))
                supplicants[name] = launch(name, name, args.tools / "run-wpa", "-Dmacsec_linux",
                                           "-i" + lowers[name], "-c", config)
                address(name)
                launch(name, name + "-echo", "python3", "-u", "-c",
                       "import socket\ns=socket.socket(socket.AF_INET,socket.SOCK_DGRAM);s.bind(('0.0.0.0',43435))\n"
                       "while True:\n d,p=s.recvfrom(128);s.sendto(d,p)\n")
            ready("initial group")
            time.sleep(10)
            ready("initial group after old association retirement")
            # Force the exact observed kernel symptom without exposing keys.
            row = state("gateway"); channel = row["rx_sc"][0]
            sci = int(channel["sci"], 16)
            mac = (sci >> 16).to_bytes(6, "big").hex(":")
            for sa in channel["sa_list"]:
                ip("gateway", "macsec", "set", "macsec0", "rx", "port", str(sci & 65535),
                   "address", mac, "sa", str(sa["an"]), "off")
                ip("gateway", "macsec", "del", "macsec0", "rx", "port", str(sci & 65535),
                   "address", mac, "sa", str(sa["an"]))
            assert not native.macsec_health([state("gateway")])["healthy"]
            ready("empty receive association recovered")
            row = state("node3"); channel = row["rx_sc"][0]
            sci = int(channel["sci"], 16)
            mac = (sci >> 16).to_bytes(6, "big").hex(":")
            for sa in channel["sa_list"]:
                ip("node3", "macsec", "set", "macsec0", "rx", "port", str(sci & 65535),
                   "address", mac, "sa", str(sa["an"]), "off")
                ip("node3", "macsec", "del", "macsec0", "rx", "port", str(sci & 65535),
                   "address", mac, "sa", str(sa["an"]))
            assert not native.macsec_health([state("node3")])["healthy"]
            ready("client receive association recovered")
            for name in ["node2", "gateway", "node1"]:
                restart(name, offline=8)
                ready(name + " rejoined")
                time.sleep(10)
                ready(name + " after old association retirement")
            if args.previous_tools:
                # Only the systemd adapter is replaced. Tool verification,
                # extraction, file journaling, Unix control, actual MKA and
                # kernel health checks run unchanged in this private topology.
                upgrade = work / "upgrade"; upgrade.mkdir()
                (upgrade / "phase").write_text("committed\n")
                profile = {"role":"gateway","lower":"br0","priority":0,"cak":cak,"ckn":ckn}
                (upgrade / "profile.json").write_text(json.dumps(profile))
                (upgrade / "mka.conf").write_bytes(configs["gateway"].read_bytes())
                (upgrade / "deploy_root.py").write_bytes(b"retained original agent")
                archive = upgrade / "health-tools.tar.gz"
                with tarfile.open(archive, "w:gz") as packed:
                    for path in sorted(args.tools.rglob("*")):
                        packed.add(path, arcname=str(path.relative_to(args.tools)), recursive=False)
                native.save(upgrade / "health-release.json", {
                    "tools_archive_sha256":native.digest(archive.read_bytes()),
                    "wpa_sha256":native.digest((args.tools / "wpa_supplicant").read_bytes())})
                original_profile = (upgrade / "profile.json").read_bytes()
                original_config = (upgrade / "mka.conf").read_bytes()
                supplicants["gateway"].terminate();supplicants["gateway"].wait(timeout=5)
                configs["gateway"] = upgrade / "mka.conf"
                supplicants["gateway"] = launch("gateway", "old-tool-upgrade-baseline",
                    args.previous_tools / "run-wpa", "-Dmacsec_linux", "-ibr0", "-c", configs["gateway"])
                address("gateway")
                saved_run, saved_units, saved_control = native.run, native.UNIT_DIR, native.wpa_control
                active = {"command":str(args.previous_tools / "run-wpa") + " -c" + str(upgrade / "mka.conf")}
                restarts = []
                def adapter(argv, **kwargs):
                    if argv[0] == "systemctl":
                        if argv[1] == "show":
                            if "MainPID" in argv:
                                return subprocess.CompletedProcess(argv,0,str(supplicants["gateway"].pid).encode(),b"")
                            return subprocess.CompletedProcess(argv,0,active["command"].encode(),b"")
                        if argv[1] == "restart" and argv[2] == "qrab-macsec.service":
                            restarts.append(argv[2])
                            supplicants["gateway"].terminate();supplicants["gateway"].wait(timeout=5)
                            text=(native.UNIT_DIR / "qrab-macsec.service.d/85-qrab-health-tools.conf").read_text()
                            executable=next(line[10:].split()[0] for line in text.splitlines()
                                            if line.startswith("ExecStart=") and line != "ExecStart=")
                            active["command"]=executable + " -c" + str(upgrade / "mka.conf")
                            supplicants["gateway"]=launch("gateway","upgraded-tool",executable,
                                "-Dmacsec_linux","-ibr0","-c",configs["gateway"])
                            address("gateway")
                        elif argv[1] not in ("show","daemon-reload","restart"):
                            raise AssertionError("unexpected systemd operation")
                        return subprocess.CompletedProcess(argv,0,b"",b"")
                    if argv[0] == "ip":
                        return saved_run(["ip","netns","exec",namespaces["gateway"],*argv],**kwargs)
                    return saved_run(argv,**kwargs)
                try:
                    native.run=adapter;native.UNIT_DIR=work/"units"
                    native.wpa_control=lambda state,command,control_dir=None: saved_control(
                        state,command,work/"ctrl-gateway")
                    first=native.install_native_health(upgrade)
                    second=native.install_native_health(upgrade)
                    assert first["health"]["healthy"] and second["health"]["healthy"]
                    assert restarts == ["qrab-macsec.service"], "idempotent upgrade restarted MKA again"
                    assert (upgrade/"profile.json").read_bytes()==original_profile
                    assert (upgrade/"mka.conf").read_bytes()==original_config
                    assert (upgrade/"deploy_root.py").read_bytes()==b"retained original agent"
                    assert (upgrade/"phase").read_text().strip()=="committed"
                finally:
                    native.run=saved_run;native.UNIT_DIR=saved_units;native.wpa_control=saved_control
                ready("old bundle upgraded without replacing profile or original agent")
            wrong = inside("wrong", "python3", "-c", probe, ips["wrong"], ips["gateway"], check=False)
            assert wrong.returncode != 0, "wrong CAK was admitted"
            for name in trusted:
                assert not json.loads(ip(name, "-j", "addr", "show", "dev", lowers[name]).stdout)[0].get("addr_info")
                assert native.macsec_health([state(name)])["healthy"]
                actual = json.loads(inside(name, args.tools / "run-nft", "-j", "list", "table",
                                           "netdev", native.GUARD_TABLE).stdout)
                assert native.firewall_semantics(actual) == guards[name], "raw guard changed"
            print(json.dumps({"passed": results, "healthRecoveries": recoveries,
                              "freshTransmitKeyIdentifiers": True, "wrongCAKRefused": True,
                              "rawGuardsRetained": True, "productionTouched": False}), flush=True)
        except BaseException:
            print(json.dumps({"failureHealth": {name: native.macsec_health([state(name)] if state(name) else [])
                                                for name in trusted}}), flush=True)
            raise
        finally:
            for process in reversed(processes):
                if process.poll() is None:
                    process.terminate()
                    try: process.wait(timeout=3)
                    except subprocess.TimeoutExpired: process.kill(); process.wait(timeout=3)
            for output in outputs: output.close()
            for name in reversed(owned):run("ip", "netns", "del", namespaces[name], check=False)


if __name__ == "__main__":
    main()
