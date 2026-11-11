#!/usr/bin/env bash
# Read-only verification of a CR-managed UEFI/TPM VM and its OIDC console.
set -euo pipefail
cd "$(dirname "$0")/../.."
python3 - "${1:-qrab}" "${2:-qrab-demo}" <<'PY'
import json,re,subprocess,sys,time,urllib.request,urllib.error,urllib.parse
from http.client import HTTPSConnection
from pathlib import Path
namespace,pod=sys.argv[1:]
def kubectl(*args):
    return subprocess.check_output(['kubectl','-n',namespace,*args],text=True)
lock=json.loads(Path('contrib/images/images.lock.json').read_text())
before=json.loads(kubectl('get','pod',pod,'-o','json'))
vm=json.loads(kubectl('get','virtualmachine',pod,'-o','json'))
assert vm['status']['ready'] and vm['status']['observedGeneration']==vm['metadata']['generation'], 'VM CR is not reconciled/ready'
assert vm['status']['podUid']==before['metadata']['uid'], 'CR points at a different Pod'
assert any(o['uid']==vm['metadata']['uid'] and o.get('controller') for o in before['metadata'].get('ownerReferences',[])), 'Pod is not owned by this VM CR'
claim=json.loads(kubectl('get','pvc',vm['spec']['stateVolumeClaim'],'-o','json'))
assert claim['metadata']['uid']==vm['status']['stateVolumeUid'] and claim['status']['phase']=='Bound', 'persistent identity PVC binding changed'
assert not claim['metadata'].get('ownerReferences'), 'identity PVC must survive CR deletion'
operator=json.loads(kubectl('get','deployment','qrab-operator','-o','json'))
assert operator['spec']['replicas']==1 and operator['spec']['strategy']['type']=='Recreate', 'operator must have one active reconciler'
assert operator['spec']['template']['spec']['containers'][0]['image']==lock['operator']['image'], 'operator image differs from pin'
assert operator['status'].get('availableReplicas')==1, 'operator is not available'
assert before['status']['phase']=='Running', 'pod is not running'
assert any(c['type']=='Ready' and c['status']=='True' for c in before['status']['conditions']), 'pod is not ready'
container=before['status']['containerStatuses'][0]
assert container['imageID']==lock['manager']['image'], 'running image differs from the published pin'
assert not before['spec'].get('imagePullSecrets'), 'unexpected static image pull credentials'
assert before['spec'].get('automountServiceAccountToken') is False, 'unexpected API token mount'
required=['QRAB_GUEST_BOOTED','QRAB_UEFI_BOOTED','QRAB_TPM2_READY','QRAB_TPM_EVENTLOG_PRESENT','QRAB_GUEST_NO_NIC_OK']
for _ in range(60):
    logs=kubectl('logs',pod)
    if all(marker in logs for marker in required) and re.search(r'QRAB_TPM_EK_SHA256=[a-f0-9]{64}',logs):break
    time.sleep(1)
else:raise AssertionError('UEFI/TPM2/eventlog/EK/no-NIC guest evidence is missing')
ek=re.search(r'QRAB_TPM_EK_SHA256=([a-f0-9]{64})',logs).group(1)
for pcr in [0,4,7,11]:
    match=re.search(rf'(?m)^\s*{pcr}\s*:\s*0x([a-fA-F0-9]{{64}})',logs)
    assert match and int(match.group(1),16), f'PCR{pcr} is missing or unmeasured'
kubectl('exec',pod,'--','qrab-manager','ready','--config','/etc/qrab/vm.json')
status=json.loads(kubectl('exec',pod,'--','qrab-manager','status','--config','/etc/qrab/vm.json'))
assert status['ready'] and status['qemu_version']==list(map(int,lock['qemu']['version'].split('.'))), 'unexpected QMP state/version'
assert status.get('swtpm_pid'), 'swtpm child is not supervised'
arguments=kubectl('exec',pod,'--','cat',f"/proc/{status['qemu_pid']}/cmdline").strip('\n').split('\0')
assert any(arguments[i:i+2]==['-nic','none'] for i in range(len(arguments))), 'guest NIC is not explicitly disabled'
assert '-netdev' not in arguments, 'unexpected guest network backend'
assert '-kernel' not in arguments and '-initrd' not in arguments, 'UEFI must boot the ESP/UKI rather than direct kernel arguments'
assert any('pflash0=pflash-code,pflash1=pflash-vars' in arg for arg in arguments), 'UEFI pflash is not configured'
devices=[]
for index,arg in enumerate(arguments[:-1]):
    if arg=='-device':
        raw=arguments[index+1]
        devices.append(json.loads(raw).get('driver') if raw.startswith('{') else raw.split(',',1)[0])
assert '-tpmdev' in arguments and {'tpm-tis','tpm-crb'}.intersection(devices), 'QEMU TPM2 device is missing'
vnc=arguments[arguments.index('-vnc')+1]
assert vnc.startswith('unix:/run/qrab/session-') and ',websocket' not in vnc, 'VNC is not private Unix-only'
socket_dir=vnc.split(',',1)[0].removeprefix('unix:').rsplit('/',1)[0]
mode=int(kubectl('exec',pod,'--','stat','-c','%a',socket_dir).strip(),8)
assert mode & 0o777 == 0o700, 'another UID can traverse the QMP/VNC session directory'
assert int(kubectl('exec',pod,'--','stat','-c','%a','/var/lib/qrab/identity').strip(),8)&0o777==0o700, 'persistent identity directory is not private'
env={v['name']:v.get('value') for v in before['spec']['containers'][0]['env']}
origin=env['BMC_PUBLIC_URL']
class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self,*args):return None
http=urllib.request.build_opener(NoRedirect)
def response(path,**kwargs):
    try:return http.open(urllib.request.Request(origin+path,**kwargs),timeout=15)
    except urllib.error.HTTPError as error:return error
assert response('/').status==200, 'HTTPS console is unavailable'
for path in ['/api/session','/api/status']:
    assert response(path).status==401, f'{path} is not protected'
assert response('/api/action',data=b'"pause"',headers={'Content-Type':'application/json'}).status==401, 'VM control is not protected'
# urllib forces Connection: close, so it cannot exercise a WebSocket upgrade.
connection=HTTPSConnection(urllib.parse.urlsplit(origin).netloc,timeout=15)
try:
    connection.request('GET','/api/console',headers={'Connection':'Upgrade','Upgrade':'websocket','Sec-WebSocket-Version':'13','Sec-WebSocket-Key':'dGhlIHNhbXBsZSBub25jZQ==','Origin':origin})
    upgrade=connection.getresponse()
    assert upgrade.status==401, f'unexpected unauthenticated console HTTP status {upgrade.status}'
finally:
    connection.close()
login=response('/auth/login')
assert login.status==307, 'OIDC login does not redirect'
url=urllib.parse.urlsplit(login.headers['Location'])
query=urllib.parse.parse_qs(url.query)
assert url.scheme=='https' and url.hostname=='idm.yaple.net', 'wrong OIDC authority'
assert query['client_id']==[env['BMC_OIDC_CLIENT_ID']] and query['redirect_uri']==[origin+'/auth/callback'] and query['code_challenge_method']==['S256'], 'incorrect OIDC redirect/PKCE'
cookie=login.headers['Set-Cookie']
assert cookie.startswith('__Host-qrab-session=') and all(flag in cookie for flag in ['Secure','HttpOnly','SameSite=Lax','Path=/']) and 'Domain=' not in cookie, 'session cookie is not confined to this HTTPS origin'
after=json.loads(kubectl('get','pod',pod,'-o','json'))
assert before['metadata']['uid']==after['metadata']['uid'], 'pod changed during verification; retry'
print(f"PASS {namespace}/{pod}: CR-owned UEFI/TPM2 VM, EK={ek}, QMP {status['qemu_version']}, published VM/operator digests, retained private state, no guest NIC, HTTPS/OIDC and protected console")
PY
