#!/usr/bin/env bash
# Live benchmark and isolation check of the guest NIC datapath
# (docs/networking.md, "Benchmark"). Run on the VM host as an admin kubectl
# user.
#
# One throwaway CirrOS VM, <vm>-tap (2 vCPUs, KVM, one NIC in security group
# <vm>), is measured on kube03. Until 2026-11-15 the same script also
# measured the removed passt datapath; its numbers are in the docs. A tiny agent in its cloud-init
# user-data polls this script for shell commands, because CirrOS has only
# busybox (no iperf3). The matrix per mode:
#   - ICMP RTT from the VM host and from a Pod on another node (50 pings each);
#   - TCP guest -> VM host, 1 and 4 streams (dd | nc into a timed sink here);
#   - TCP VM host -> guest, 1 stream (into busybox nc -l).
# Probes; a failure exits non-zero:
#   - allowed ingress (TCP 5201 from the VM host) works;
#   - denied ingress (TCP 23 from a Pod) is dropped, with a Hubble
#     "Policy denied" record;
#   - denied egress (the guest to the API server's ClusterIP) fails;
#   - the guest cannot reach the VM Pod's BMC (8080) or Redfish (8081);
#   - the VM Pod's network namespace holds no tap and no guest address.
# It prints one JSON object and deletes everything on exit except the
# VM's root disk: the storage service never destroys a disk, so every run keeps
# one 1 GiB CoW clone against the block-storage budget (contrib/storage/README.md).
#
#   bash contrib/check-nic-datapath.sh [vm-name]
set -euo pipefail
exec python3 -I - "${1:-netbench}" tap <<'PY'
import http.server, json, os, re, socket, socketserver, subprocess, sys, threading, time

VM, MODES = sys.argv[1], sys.argv[2].split(',')
NS, HOST = 'qrab', os.environ.get('HOST_IP', '10.203.0.1')
CTRL, SINK, GUEST_PORT = 18181, 18182, 5201
MIB = int(os.environ.get('BENCH_MIB', '256'))
failures = []

def log(*a):
    print(time.strftime('%H:%M:%S'), *a, file=sys.stderr, flush=True)

def kubectl(*args, check=True, input=None):
    r = subprocess.run(['kubectl', *args], capture_output=True, text=True, input=input)
    if check and r.returncode:
        raise SystemExit(f'kubectl {" ".join(args)}: {r.stderr.strip()}')
    return r.stdout

def jsonpath(kind, name, path, ns=NS):
    return kubectl('-n', ns, 'get', kind, name, '-o', f'jsonpath={path}', check=False).strip()

def expect(ok, what):
    log(('ok: ' if ok else 'FAIL: ') + what)
    if not ok:
        failures.append(what)

# --- the guest's command channel and the TCP sink -------------------------
commands, results, sink = [], [], []
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

class Sink(socketserver.BaseRequestHandler):
    def handle(self):
        start, total, want = time.monotonic(), 0, MIB << 20
        while total < want:
            chunk = self.request.recv(1 << 20)
            if not chunk:
                break
            total += len(chunk)
        with lock:
            sink.append((start, time.monotonic(), total)); lock.notify_all()

for server in (http.server.ThreadingHTTPServer((HOST, CTRL), Control),
               socketserver.ThreadingTCPServer((HOST, SINK), Sink)):
    server.daemon_threads = True
    threading.Thread(target=server.serve_forever, daemon=True).start()

def guest(cmd, timeout=180):
    """Run a shell command in the guest; returns its output (with rc=N)."""
    with lock:
        seen = len(results)
        commands.append(cmd)
        if not lock.wait_for(lambda: len(results) > seen, timeout):
            raise SystemExit(f'guest did not answer {cmd!r} within {timeout}s')
        return results[seen]

# --- objects --------------------------------------------------------------
agent = f'''#!/bin/sh
while true; do
  c=$(wget -q -T 5 -O - http://{HOST}:{CTRL}/cmd)
  if [ -n "$c" ]; then
    sh -c "$c" > /tmp/out 2>&1; echo "rc=$?" >> /tmp/out
    wget -q -T 5 -O /dev/null --post-file=/tmp/out http://{HOST}:{CTRL}/result
  fi
  sleep 1
done &
'''
policy = {
    'apiVersion': 'cilium.io/v2', 'kind': 'CiliumNetworkPolicy',
    'metadata': {'name': f'sg-{VM}', 'namespace': NS},
    'spec': {
        'description': f'Security group {VM} (check-nic-datapath.sh)',
        'endpointSelector': {'matchLabels': {f'sg.qrab.yapplesauce.com/{VM}': ''}},
        'ingress': [
            {'fromEntities': ['world'], 'toPorts': [{'ports': [{'port': str(GUEST_PORT), 'protocol': 'TCP'}]}]},
            {'fromEntities': ['world', 'cluster'], 'icmps': [{'fields': [{'type': 8, 'family': 'IPv4'}]}]},
        ],
        'egress': [
            {'toEntities': ['world']},
            {'toEndpoints': [{'matchLabels': {'k8s:io.kubernetes.pod.namespace': 'kube-system', 'k8s-app': 'kube-dns'}}],
             'toPorts': [{'ports': [{'port': '53', 'protocol': 'UDP'}, {'port': '53', 'protocol': 'TCP'}]}]},
        ],
    },
}
def vm_object(mode):
    return {'apiVersion': 'qrab.yapplesauce.com/v1alpha1', 'kind': 'VirtualMachine',
            'metadata': {'name': f'{VM}-{mode}', 'namespace': NS},
            'spec': {'stateVolumeClaim': f'{VM}-{mode}-state', 'accelerator': 'kvm', 'cpus': 2, 'memoryMiB': 512,
                     'storageNode': 'kube03', 'components': {'bmc': {'enabled': False}},
                     'disks': [{'id': 'root', 'sizeBytes': 1 << 30, 'bootIndex': 0,
                                'baseImage': {'name': 'cirros', 'generation': 1}}],
                     'nics': [{'name': 'eth0', 'securityGroups': [VM]}],
                     'cloudInit': {'hostname': VM, 'userData': agent}}}
def pvc(mode):
    return {'apiVersion': 'v1', 'kind': 'PersistentVolumeClaim', 'metadata': {'name': f'{VM}-{mode}-state', 'namespace': NS},
            'spec': {'accessModes': ['ReadWriteOncePod'], 'storageClassName': 'zfs-block', 'volumeMode': 'Filesystem',
                     'resources': {'requests': {'storage': '1Gi'}}}}

def apply(obj):
    kubectl('apply', '-f', '-', input=json.dumps(obj))

def cleanup():
    for pod in (f'{VM}-ping', f'{VM}-deny'):
        kubectl('-n', 'default', 'delete', 'pod', pod, '--ignore-not-found', '--wait=false', check=False)
    for mode in MODES:
        kubectl('-n', NS, 'delete', 'virtualmachine', f'{VM}-{mode}', '--ignore-not-found', '--wait=true', '--timeout=180s', check=False)
        kubectl('-n', NS, 'delete', 'pvc', f'{VM}-{mode}-state', '--ignore-not-found', check=False)
    kubectl('-n', NS, 'delete', 'ciliumnetworkpolicy', f'sg-{VM}', '--ignore-not-found', check=False)

def wait(what, predicate, limit):
    deadline = time.monotonic() + limit
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(2)
    raise SystemExit(f'timed out: {what}')

def pod_run(name, node, *command, timeout=180):
    kubectl('-n', 'default', 'delete', 'pod', name, '--ignore-not-found', '--wait=true', check=False)
    overrides = {'spec': {'nodeName': node, 'terminationGracePeriodSeconds': 1,
                          'tolerations': [{'operator': 'Exists'}]}}
    kubectl('-n', 'default', 'run', name, '--image=docker.io/library/busybox:1.37', '--restart=Never',
            f'--overrides={json.dumps(overrides)}', '--', *command)
    wait(f'pod {name} finished', lambda: jsonpath('pod', name, '{.status.phase}', 'default') in ('Succeeded', 'Failed'), timeout)
    return jsonpath('pod', name, '{.status.phase}', 'default'), kubectl('-n', 'default', 'logs', name, check=False)

def rtt(output):
    m = re.search(r'= [\d.]+/([\d.]+)/([\d.]+)', output)
    loss = re.search(r'([\d.]+)% packet loss', output)
    return {'avg_ms': float(m.group(1)) if m else None, 'max_ms': float(m.group(2)) if m else None,
            'loss_pct': float(loss.group(1)) if loss else None}

def mbit(nbytes, seconds):
    return round(nbytes * 8 / seconds / 1e6, 1)

# --- one mode -------------------------------------------------------------
def measure(mode):
    log(f'--- {mode} mode')
    vm = f'{VM}-{mode}'
    apply(pvc(mode))
    apply(vm_object(mode))
    wait('VM Running in ' + mode, lambda: jsonpath('virtualmachine', vm, '{.status.phase}') == 'Running'
         and jsonpath('virtualmachine', vm, '{.status.nics[0].ready}') == 'true', 420)
    address = jsonpath('virtualmachine', vm, '{.status.nics[0].address}')
    node = jsonpath('pod', vm, '{.spec.nodeName}')
    other = next(n for n in kubectl('get', 'nodes', '-o', 'jsonpath={.items[*].metadata.name}').split() if n != node)
    log(f'{vm} on {node}, guest {address}; probes from {other}')
    with lock:
        results.clear(); commands.clear()
    print(guest('uname -a; cat /proc/cpuinfo | grep -c processor', timeout=300), file=sys.stderr)
    out = {'mode': mode, 'node': node, 'guest': address}

    ping = subprocess.run(['ping', '-c', '50', '-i', '0.2', '-q', address], capture_output=True, text=True).stdout
    out['rtt_vm_host'] = rtt(ping)
    _, logs = pod_run(f'{VM}-ping', other, 'ping', '-c', '50', '-i', '0.2', '-q', address)
    out['rtt_pod_other_node'] = rtt(logs)

    count = MIB * 16  # 64 KiB blocks
    for streams in (1, 4):
        with lock:
            sink.clear()
        cmd = ' & '.join([f'dd if=/dev/zero bs=65536 count={count} 2>/dev/null | nc {HOST} {SINK}'] * streams) + '; wait'
        guest(cmd, timeout=600)
        with lock:
            lock.wait_for(lambda: len(sink) >= streams, 30)
            done = list(sink)
        total = sum(t for _, _, t in done)
        expect(total == streams * (MIB << 20), f'{mode}: guest->host {streams} stream(s) delivered {total} bytes')
        span = max(e for _, e, _ in done) - min(s for s, _, _ in done)
        out[f'tx_guest_to_host_{streams}_mbit'] = mbit(total, span)

    guest(f'nc -l -p {GUEST_PORT} > /dev/null & sleep 1')
    data = b'\0' * (1 << 20)
    start = time.monotonic()
    with socket.create_connection((address, GUEST_PORT), timeout=10) as conn:
        for _ in range(MIB):
            conn.sendall(data)
        conn.shutdown(socket.SHUT_WR)
        conn.settimeout(30)
        try:
            while conn.recv(65536):
                pass
        except OSError:
            pass
    out['rx_host_to_guest_1_mbit'] = mbit(MIB << 20, time.monotonic() - start)
    expect(True, f'{mode}: allowed ingress TCP {GUEST_PORT} from the VM host carried {MIB} MiB')

    phase, _ = pod_run(f'{VM}-deny', other, 'nc', '-w', '3', address, '23')
    expect(phase == 'Failed', f'{mode}: denied ingress TCP 23 from a Pod failed ({phase})')
    agent_pod = kubectl('-n', 'kube-system', 'get', 'pods', '-l', 'k8s-app=cilium', '--field-selector', f'spec.nodeName={node}',
                        '-o', 'jsonpath={.items[0].metadata.name}')
    hubble = kubectl('-n', 'kube-system', 'exec', agent_pod, '-c', 'cilium-agent', '--', 'hubble', 'observe', '--since', '2m',
                     '--to-ip', address, '--to-port', '23', '--verdict', 'DROPPED', '-o', 'compact', check=False)
    denied = [l for l in hubble.splitlines() if 'Policy denied' in l]
    expect(bool(denied), f'{mode}: Hubble shows the denied ingress')
    out['hubble_denied_ingress'] = denied[-1] if denied else None

    egress = guest('wget -q -T 3 -O - http://10.96.0.1:443/')
    expect('rc=0' not in egress, f'{mode}: denied egress to the API server ClusterIP failed')
    vm_ip = jsonpath('pod', vm, '{.status.podIP}')
    for port in (8080, 8081):
        reply = guest(f'wget -q -T 3 -O - http://{vm_ip}:{port}/')
        expect('rc=0' not in reply, f'{mode}: the guest cannot reach the VM Pod {vm_ip}:{port}')
    devices = kubectl('-n', NS, 'exec', vm, '-c', 'manager', '--', 'cat', '/proc/net/dev', '/proc/net/fib_trie', check=False)
    expect(' qt' not in devices and address not in devices, 'tap: the VM Pod netns holds no tap and no guest address')
    return out

try:
    apply(policy)
    for mode in MODES:
        # Printed as each mode finishes, so a later failure keeps it.
        print(json.dumps(measure(mode)), flush=True)
        kubectl('-n', NS, 'delete', 'virtualmachine', f'{VM}-{mode}', '--wait=false', check=False)
finally:
    cleanup()
if failures:
    log(f'FAIL: {len(failures)} probe(s): {failures}')
    sys.exit(1)
log('PASS')
PY
