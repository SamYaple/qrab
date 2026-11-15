#!/usr/bin/env bash
# Live end-to-end check of a ProtonVPN NIC (docs/networking.md, "ProtonVPN").
# Run on the VM host as an admin kubectl user, after `qrab-proton login`.
#
# It creates VpnProfile <vm> with port forwarding and one CirrOS VM with two
# NICs: `vpn` (the profile's Secret; the guest's default route) and `ctl`
# (security group <vm>-ctl: only this script's command channel on the VM
# host, through a /32 route). A tiny agent in the user-data polls the
# channel for shell commands, because the tunnel cannot reach the VM host's
# private address. The script requires:
#   - the profile becomes Ready with a server, a certificate and the NIC in
#     status.attached;
#   - the guest's egress IP, as a public echo service sees it, is not the VM
#     host's;
#   - status.forwardedPort is set, and a TCP connection from the VM host's
#     own internet uplink to <guest egress IP>:<port> reaches a listener in
#     the guest;
#   - the private key is absent from the VM Pod and qrab-netd's logs;
#   - a manual switch to a server in another country changes the guest's
#     egress, and the new forwarded port reaches the guest;
#   - after a live migration the guest (same boot) still egresses through
#     Proton and its forwarded port reaches it.
# It deletes everything it made on exit except the VM's root disk
# (contrib/storage/README.md). The Proton session and keys stay.
#
#   bash contrib/check-proton-vpn.sh [vm-name]
set -euo pipefail
exec python3 -I - "${1:-pvpn}" <<'PY'
import base64, gzip, http.server, json, os, re, socket, subprocess, sys, threading, time

VM = sys.argv[1]
NS, HOST = 'qrab', os.environ.get('HOST_IP', '10.203.0.1')
CTRL = int(os.environ.get('HOST_PORT', '18191'))
SWITCH_TO = os.environ.get('SWITCH_COUNTRY', 'CH')
summary = []

def log(*a):
    print(time.strftime('%H:%M:%S'), *a, file=sys.stderr, flush=True)

def fail(what):
    log('FAIL: ' + what)
    pod = get('virtualmachine', VM).get('status', {}).get('podName')
    if pod:
        tail = kubectl('-n', NS, 'logs', pod, '--all-containers', '--tail=60', check=False)
        log('the VM Pod log (with the guest console):\n' + tail)
    raise SystemExit(1)

def kubectl(*args, check=True, input=None):
    r = subprocess.run(['kubectl', *args], capture_output=True, text=True, input=input)
    if check and r.returncode:
        raise SystemExit(f'kubectl {" ".join(args)}: {r.stderr.strip()}')
    return r.stdout

def get(kind, name):
    out = kubectl('-n', NS, 'get', kind, name, '-o', 'json', check=False)
    return json.loads(out) if out else {}

def apply(obj):
    kubectl('apply', '-f', '-', input=json.dumps(obj))

def wait(what, predicate, limit, every=2):
    deadline = time.monotonic() + limit
    while time.monotonic() < deadline:
        value = predicate()
        if value:
            return value
        time.sleep(every)
    fail(f'{what} within {limit}s')

# --- the guest's command channel -------------------------------------------
commands, results = [], []
lock = threading.Condition()

class Control(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        with lock:
            body = commands.pop(0) if commands else ''
        self.send_response(200); self.end_headers(); self.wfile.write(body.encode())
    def do_POST(self):
        data = self.rfile.read(int(self.headers.get('Content-Length', 0))).decode(errors='replace')
        with lock:
            results.append(data); lock.notify_all()
        self.send_response(200); self.end_headers()
    def log_message(self, *a):
        pass

server = http.server.ThreadingHTTPServer((HOST, CTRL), Control)
server.daemon_threads = True
threading.Thread(target=server.serve_forever, daemon=True).start()

def guest(cmd, timeout=120):
    """Run a shell command in the guest; returns (rc, output)."""
    with lock:
        seen = len(results)
        commands.append(cmd)
        if not lock.wait_for(lambda: len(results) > seen, timeout):
            fail(f'the guest answering {cmd!r}')
        text = results[seen]
    # The agent appends rc=N, on the output's last line when it has no newline.
    m = re.search(r'rc=(\d+)\s*$', text)
    return (int(m.group(1)), text[:m.start()].rstrip('\n')) if m else (-1, text)

def egress():
    rc, out = guest('wget -q -T 10 -O - http://api.ipify.org || wget -q -T 10 -O - http://ifconfig.me/ip')
    out = out.strip()
    return out if rc == 0 and out.count('.') == 3 else None

def listen(port, tag):
    rc, out = guest(f'killall nc 2>/dev/null; kill $(cat /tmp/listener 2>/dev/null) 2>/dev/null; '
                    f'(while true; do echo {tag} | nc -l -p {port}; done) >/dev/null 2>&1 & echo $! > /tmp/listener')
    rc == 0 or fail(f'starting the guest listener: {out}')

def reach(ip, port, tag, limit=90):
    """Connect from the VM host's own uplink to the forwarded port."""
    def attempt():
        try:
            with socket.create_connection((ip, port), timeout=5) as s:
                s.settimeout(5)
                return tag.encode() in s.recv(256)
        except OSError:
            return False
    start = time.monotonic()
    wait(f'{ip}:{port} reaching the guest from outside', attempt, limit, every=3)
    return time.monotonic() - start

# --- objects ----------------------------------------------------------------
# CirrOS leases every NIC with dhcpcd, which gives each a default route. The
# agent keeps the leases and pins the routes: default through the NIC that
# holds the tunnel address, the VM host through the other, and Proton's DNS.
agent = f'''#!/bin/sh
exec >/dev/ttyS0 2>&1
routes() {{
  vpn=; ctl=
  for d in /sys/class/net/eth*; do
    n=${{d##*/}}
    a=$(ip -4 -o addr show dev $n)
    case "$a" in *" 10.2.0.2/"*) vpn=$n;; *inet*) ctl=$n;; esac
  done
  [ -n "$vpn" ] && [ -n "$ctl" ] || return 1
  gw=$(ip route show dev $vpn scope link | awk '{{print $1; exit}}')
  ip route replace default via $gw dev $vpn onlink
  ip route del default dev $ctl 2>/dev/null
  ip route replace {HOST}/32 via $(ip route show dev $ctl scope link | awk '{{print $1; exit}}') dev $ctl onlink
  [ "$(cat /etc/resolv.conf)" = "nameserver 10.2.0.1" ] || echo "nameserver 10.2.0.1" > /etc/resolv.conf
}}
until routes; do sleep 1; done
echo "check-proton-vpn: tunnel on $vpn, control on $ctl"; ip route
while true; do
  routes
  c=$(wget -q -T 5 -O - http://{HOST}:{CTRL}/cmd 2>/dev/null)
  if [ -n "$c" ]; then
    sh -c "$c" > /tmp/out 2>&1; echo "rc=$?" >> /tmp/out
    wget -q -T 5 -O /dev/null --post-file=/tmp/out http://{HOST}:{CTRL}/result
  fi
  sleep 1
done &
'''
def policy(name, description, egress):
    return {'apiVersion': 'cilium.io/v2', 'kind': 'CiliumNetworkPolicy',
            'metadata': {'name': f'sg-{name}', 'namespace': NS},
            'spec': {'description': description,
                     'endpointSelector': {'matchLabels': {f'sg.qrab.yapplesauce.com/{name}': ''}},
                     'egress': egress}}
objects = [
    policy(f'{VM}-vpn', f'Security group {VM}-vpn (check-proton-vpn.sh): WireGuard to any server',
           [{'toEntities': ['world'], 'toPorts': [{'ports': [{'port': '51820', 'protocol': 'UDP'}]}]}]),
    policy(f'{VM}-ctl', f'Security group {VM}-ctl (check-proton-vpn.sh): the command channel only',
           [{'toCIDR': [f'{HOST}/32'], 'toPorts': [{'ports': [{'port': str(CTRL), 'protocol': 'TCP'}]}]}]),
    {'apiVersion': 'qrab.yapplesauce.com/v1alpha1', 'kind': 'VpnProfile',
     'metadata': {'name': VM, 'namespace': NS}, 'spec': {'proton': {'portForwarding': True}}},
    {'apiVersion': 'v1', 'kind': 'PersistentVolumeClaim', 'metadata': {'name': f'{VM}-state', 'namespace': NS},
     'spec': {'accessModes': ['ReadWriteOncePod'], 'storageClassName': 'zfs-block', 'volumeMode': 'Filesystem',
              'resources': {'requests': {'storage': '1Gi'}}}},
    {'apiVersion': 'qrab.yapplesauce.com/v1alpha1', 'kind': 'VirtualMachine',
     'metadata': {'name': VM, 'namespace': NS},
     'spec': {'stateVolumeClaim': f'{VM}-state', 'accelerator': 'kvm', 'cpus': 1, 'memoryMiB': 512,
              'storageNode': 'kube03', 'components': {'bmc': {'enabled': False}},
              'disks': [{'id': 'root', 'sizeBytes': 1 << 30, 'bootIndex': 0,
                         'baseImage': {'name': 'cirros', 'generation': 1}}],
              'nics': [{'name': 'vpn', 'securityGroups': [f'{VM}-vpn'], 'vpn': {'secretName': f'vpn-{VM}'}},
                       {'name': 'ctl', 'securityGroups': [f'{VM}-ctl']}],
              'cloudInit': {'hostname': VM, 'userData': agent}}},
]

def cleanup():
    kubectl('-n', NS, 'delete', 'virtualmachinemigration', '-l', f'qrab-check={VM}', '--ignore-not-found', check=False)
    kubectl('-n', NS, 'delete', 'virtualmachine', VM, '--ignore-not-found', '--wait=true', '--timeout=180s', check=False)
    kubectl('-n', NS, 'delete', 'pvc', f'{VM}-state', '--ignore-not-found', check=False)
    kubectl('-n', NS, 'delete', 'vpnprofile', VM, '--ignore-not-found', '--wait=true', '--timeout=60s', check=False)
    for sg in ('vpn', 'ctl'):
        kubectl('-n', NS, 'delete', 'ciliumnetworkpolicy', f'sg-{VM}-{sg}', '--ignore-not-found', check=False)

def profile():
    return get('vpnprofile', VM).get('status', {})

def ready(status):
    return any(c.get('type') == 'Ready' and c.get('status') == 'True' for c in status.get('conditions', []))

def forwarded(server=None):
    """The forwarded port, once the profile reports one (for `server`)."""
    status = profile()
    if server and status.get('server') != server:
        return None
    return status.get('forwardedPort')

def catalog():
    cm = get('configmap', 'proton-catalog')
    return json.loads(gzip.decompress(base64.b64decode(cm['binaryData']['catalog.json.gz'])))

try:
    host_egress = None
    try:
        host_egress = subprocess.run(['curl', '-s', '-m', '10', 'http://api.ipify.org'],
                                     capture_output=True, text=True).stdout.strip() or None
    except OSError:
        pass
    log(f'VM host egress {host_egress}; creating profile and VM {VM}')
    for obj in objects:
        apply(obj)
    status = wait('profile Ready', lambda: (lambda s: s if ready(s) else None)(profile()), 120)
    server1 = status['server']
    log(f'profile Ready: {server1} ({status.get("city")}, {status.get("exitCountry")}), '
        f'load {status.get("load")}, certificate until {time.strftime("%Y-%m-%d %H:%MZ", time.gmtime(status["certificateExpires"]))}')
    summary.append(f'profile: auto-selected {server1} ({status.get("exitCountry")}, load {status.get("load")}), '
                   f'certificate valid {round((status["certificateExpires"] - time.time()) / 86400, 1)} days')

    wait('VM Running', lambda: get('virtualmachine', VM).get('status', {}).get('phase') == 'Running', 480)
    node = get('virtualmachine', VM)['status']['nodeName']
    rc, out = guest('ip -4 -o addr show; ip route; cat /etc/resolv.conf', timeout=300)
    log(f'guest on {node}:\n{out}')
    '10.2.0.2/' in out or fail('the guest has no tunnel address')

    exit1 = wait('the guest egressing through the tunnel', egress, 120, every=3)
    exit1 != host_egress or fail(f'the guest egresses as the VM host ({exit1})')
    log(f'guest egress {exit1} (VM host {host_egress})')

    port1 = wait('status.forwardedPort', forwarded, 120)
    attached = profile().get('attached', [])
    any(a.get('vm') == VM and a.get('nic') == 'vpn' for a in attached) or fail(f'status.attached is {attached}')
    boot = guest('cat /proc/sys/kernel/random/boot_id')[1].strip()
    listen(port1, f'qrab-{boot}')
    took = reach(exit1, port1, f'qrab-{boot}')
    summary.append(f'tunnel on {node}: guest egress {exit1} (VM host {host_egress}); '
                   f'forwarded port {port1} reached the guest from outside ({took:.0f}s)')

    key = base64.b64decode(kubectl('-n', NS, 'get', 'secret', f'vpn-{VM}', '-o', 'jsonpath={.data.wg\\.conf}')).decode()
    prefix = next(l.split('=', 1)[1].strip() for l in key.splitlines() if l.startswith('PrivateKey'))[:20]
    vm_pod = get('virtualmachine', VM)['status']['podName']
    r = subprocess.run(['kubectl', '-n', NS, 'exec', vm_pod, '-c', 'manager', '--', 'sh', '-c',
                        f"grep -rqsF '{prefix}' /etc/qrab /run /var/lib/qrab /tmp"], capture_output=True)
    r.returncode != 0 or fail('the private key is visible in the VM Pod')
    for pod in kubectl('-n', 'qrab-node', 'get', 'pods', '-l', 'app.kubernetes.io/name=qrab-netd', '-o', 'name').split():
        prefix not in kubectl('-n', 'qrab-node', 'logs', pod, check=False) or fail(f'{pod} logged the private key')
    summary.append('key: absent from the VM Pod and every qrab-netd log')

    # Manual server switch to another country.
    servers = [s for s in catalog()['servers']
               if s['x'] == SWITCH_TO and s['f'] & 4 and not s['f'] & 3 and s['t'] <= 2 and s.get('s', 1)]
    servers or fail(f'no P2P server in {SWITCH_TO}')
    server2 = min(servers, key=lambda s: s['r'])['n']
    start = time.monotonic()
    kubectl('-n', NS, 'patch', 'vpnprofile', VM, '--type', 'merge', '-p',
            json.dumps({'spec': {'proton': {'server': server2}}}))
    wait(f'the profile on {server2}', lambda: profile().get('server') == server2 and ready(profile()), 60)
    exit2 = wait('the guest egress changing', lambda: (lambda e: e if e and e != exit1 else None)(egress()), 120, every=3)
    switched = time.monotonic() - start
    port2 = wait('a new forwarded port', lambda: (lambda p: p if p and p != port1 else None)(forwarded(server2)), 120)
    listen(port2, f'qrab-{boot}')
    took = reach(exit2, port2, f'qrab-{boot}')
    summary.append(f'switch {server1} -> {server2}: egress {exit1} -> {exit2} after {switched:.0f}s; '
                   f'forwarded port {port2} reached the guest ({took:.0f}s)')

    # Live migration.
    target = 'kube01' if node != 'kube01' else 'kube02'
    apply({'apiVersion': 'qrab.yapplesauce.com/v1alpha1', 'kind': 'VirtualMachineMigration',
           'metadata': {'name': f'{VM}-move', 'namespace': NS, 'labels': {'qrab-check': VM}},
           'spec': {'vmName': VM, 'targetNode': target}})
    start = time.monotonic()
    def moved():
        phase = get('virtualmachinemigration', f'{VM}-move').get('status', {}).get('phase')
        if phase == 'Failed':
            fail('migration failed: ' + get('virtualmachinemigration', f'{VM}-move')['status'].get('message', ''))
        return phase == 'Succeeded'
    wait('the migration', moved, 600)
    log(f'migrated {node} -> {target} in {time.monotonic() - start:.0f}s')
    exit3 = wait('the guest egressing after the move', egress, 120, every=3)
    exit3 == exit2 or fail(f'egress after the move is {exit3}, not {exit2}')
    guest('cat /proc/sys/kernel/random/boot_id')[1].strip() == boot or fail('the guest rebooted')
    def fresh_port():
        status = profile()
        here = [a for a in status.get('attached', []) if a.get('nic') == 'vpn' and a.get('node') == target]
        return here and status.get('forwardedPort')
    port3 = wait('the forwarded port from the new node', fresh_port, 120)
    if port3 != port2:
        listen(port3, f'qrab-{boot}')
    took = reach(exit3, port3, f'qrab-{boot}')
    summary.append(f'migration {node} -> {target}: same boot, egress {exit3}, '
                   f'forwarded port {port3} reached the guest ({took:.0f}s)')
    log('summary:')
    for line in summary:
        print('  ' + line, file=sys.stderr)
    log('PASS: ProtonVPN NIC')
finally:
    if os.environ.get('KEEP') and sys.exc_info()[0]:
        log(f'KEEP is set: leaving {VM} for inspection; rerun without KEEP to clean up')
    else:
        cleanup()
PY
