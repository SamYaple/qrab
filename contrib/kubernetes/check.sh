#!/usr/bin/env bash
# Read-only end-to-end verification of the manually launched VM pod.
set -euo pipefail
cd "$(dirname "$0")/../.."
python3 - "${1:-qrab}" "${2:-qrab-demo}" <<'PY'
import json,subprocess,sys
from pathlib import Path
namespace,pod=sys.argv[1:]
def kubectl(*args):
    return subprocess.check_output(['kubectl','-n',namespace,*args],text=True)
lock=json.loads(Path('contrib/images/images.lock.json').read_text())
before=json.loads(kubectl('get','pod',pod,'-o','json'))
assert before['status']['phase']=='Running', 'pod is not running'
assert any(c['type']=='Ready' and c['status']=='True' for c in before['status']['conditions']), 'pod is not ready'
container=before['status']['containerStatuses'][0]
assert container['imageID']==lock['manager']['image'], 'running image differs from the published pin'
assert not before['spec'].get('imagePullSecrets'), 'unexpected static image pull credentials'
assert before['spec'].get('automountServiceAccountToken') is False, 'unexpected API token mount'
assert 'QRAB_GUEST_BOOTED' in kubectl('logs',pod), 'guest boot marker is missing'
kubectl('exec',pod,'--','qrab-manager','ready','--config','/etc/qrab/vm.json')
status=json.loads(kubectl('exec',pod,'--','qrab-manager','status','--config','/etc/qrab/vm.json'))
assert status['ready'] and status['qemu_version']==list(map(int,lock['qemu']['version'].split('.'))), 'unexpected QMP state/version'
after=json.loads(kubectl('get','pod',pod,'-o','json'))
assert before['metadata']['uid']==after['metadata']['uid'], 'pod changed during verification; retry'
print(f"PASS {namespace}/{pod}: Linux guest booted, QMP {status['qemu_version']} ready, published digest verified, no static pull credentials")
PY
