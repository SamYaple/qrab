#!/usr/bin/env bash
# Read-only end-to-end verification of the manually launched VM pod.
set -euo pipefail
cd "$(dirname "$0")/../.."
python3 - "${1:-qrab}" "${2:-qrab-demo}" <<'PY'
import json,subprocess,sys,urllib.request,urllib.error,urllib.parse
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
arguments=kubectl('exec',pod,'--','cat',f"/proc/{status['qemu_pid']}/cmdline").strip('\n').split('\0')
assert any(arguments[i:i+2]==['-nic','none'] for i in range(len(arguments))), 'guest NIC is not explicitly disabled'
assert '-netdev' not in arguments, 'unexpected guest network backend'
vnc=arguments[arguments.index('-vnc')+1]
assert vnc.startswith('unix:/run/qrab/session-') and ',websocket' not in vnc, 'VNC is not private Unix-only'
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
assert response('/api/console',headers={'Connection':'Upgrade','Upgrade':'websocket','Sec-WebSocket-Version':'13','Sec-WebSocket-Key':'dGhlIHNhbXBsZSBub25jZQ==','Origin':origin}).status==401, 'console upgrade is not protected'
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
print(f"PASS {namespace}/{pod}: Linux boot/QMP {status['qemu_version']}, published digest, no guest NIC, private VNC, HTTPS/OIDC PKCE and protected VM APIs/console; no static pull credentials")
PY
