#!/usr/bin/env python3
"""Authenticated live check of a VM's Redfish API with a real Kanidm ID token.

Run by a person whose Kanidm subject is in the VM's Redfish allowlist:

    python3 contrib/kubernetes/redfish-live.py [https://qrab-demo.yapplesauce.com] [--expect admin|viewer]

It prints a Kanidm authorization URL (public client qrab-redfish, PKCE S256,
loopback redirect http://localhost:8400/callback). Open it, log in, and either
let the browser reach the local listener or paste the final redirected URL
(it fails to load if the browser is on another machine). The ID token stays in
memory only and is never printed or written.

admin: reads Systems/1, then Reset Pause and Resume (PowerState Paused -> On).
viewer: reads Systems/1 with a redacted configuration and gets 403 for Reset.
"""
import base64, hashlib, http.server, json, secrets, sys, threading, urllib.error, urllib.parse, urllib.request, time

ISSUER = 'https://idm.yaple.net/oauth2/openid/qrab-redfish'
CLIENT = 'qrab-redfish'
REDIRECT = 'http://localhost:8400/callback'


def main():
    args = [a for a in sys.argv[1:] if not a.startswith('--')]
    origin = (args[0] if args else 'https://qrab-demo.yapplesauce.com').rstrip('/')
    expect = 'viewer' if '--expect' in sys.argv and sys.argv[sys.argv.index('--expect') + 1] == 'viewer' else 'admin'
    discovery = json.load(urllib.request.urlopen(ISSUER + '/.well-known/openid-configuration', timeout=15))
    verifier = secrets.token_urlsafe(64)
    challenge = base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest()).rstrip(b'=').decode()
    state = secrets.token_urlsafe(24)
    print(discovery['authorization_endpoint'] + '?' + urllib.parse.urlencode({
        'response_type': 'code', 'client_id': CLIENT, 'redirect_uri': REDIRECT, 'scope': 'openid profile groups_uuid',
        'state': state, 'code_challenge': challenge, 'code_challenge_method': 'S256'}))
    print('Open the URL above. Waiting on localhost:8400, or paste the redirected URL here:', flush=True)
    code = wait_for_code(state)
    token = json.load(urllib.request.urlopen(urllib.request.Request(
        discovery['token_endpoint'], method='POST',
        data=urllib.parse.urlencode({'grant_type': 'authorization_code', 'code': code, 'redirect_uri': REDIRECT,
                                     'client_id': CLIENT, 'code_verifier': verifier}).encode(),
        headers={'Content-Type': 'application/x-www-form-urlencoded'}), timeout=15))['id_token']

    def call(path, body=None):
        request = urllib.request.Request(origin + path, data=None if body is None else json.dumps(body).encode(),
                                         headers={'Authorization': 'Bearer ' + token, 'Content-Type': 'application/json'})
        try:
            with urllib.request.urlopen(request, timeout=20) as reply:
                raw = reply.read()
                return reply.status, json.loads(raw) if raw else None
        except urllib.error.HTTPError as error:
            return error.code, None

    status, system = call('/redfish/v1/Systems/1')
    assert status == 200, f'GET Systems/1 returned {status}'
    print(f"ok: Systems/1 PowerState={system['PowerState']} Health={system['Status']['Health']}")
    reset = '/redfish/v1/Systems/1/Actions/ComputerSystem.Reset'
    if expect == 'viewer':
        assert call(reset, {'ResetType': 'Pause'})[0] == 403, 'viewer Reset was not refused'
        print('PASS viewer: read allowed, Reset refused with 403')
        return
    assert system['PowerState'] == 'On', 'VM must be On before the Pause/Resume cycle'
    for reset_type, want in [('Pause', 'Paused'), ('Resume', 'On')]:
        status, _ = call(reset, {'ResetType': reset_type})
        assert status == 204, f'Reset {reset_type} returned {status}'
        for _ in range(30):
            if call('/redfish/v1/Systems/1')[1]['PowerState'] == want:
                break
            time.sleep(1)
        else:
            raise AssertionError(f'PowerState did not reach {want}')
        print(f'ok: Reset {reset_type} -> {want}')
    print('PASS admin: read and Reset Pause/Resume through HTTPS with a Kanidm ID token')


def wait_for_code(state):
    found = {}
    done = threading.Event()

    def accept(url):
        query = urllib.parse.parse_qs(urllib.parse.urlsplit(url.strip()).query)
        if query.get('state') == [state] and query.get('code'):
            found['code'] = query['code'][0]
            done.set()
            return True
        return False

    class Callback(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            ok = accept('http://localhost' + self.path)
            self.send_response(200 if ok else 400)
            self.end_headers()
            self.wfile.write(b'qrab Redfish login received; return to the terminal.\n' if ok else b'unexpected callback\n')

        def log_message(self, *args):
            pass

    try:
        server = http.server.HTTPServer(('127.0.0.1', 8400), Callback)
        threading.Thread(target=server.serve_forever, daemon=True).start()
    except OSError:
        server = None

    def read_stdin():
        for line in sys.stdin:
            if accept(line):
                return
            print('That URL has no matching state/code; paste the full redirected URL.', flush=True)

    threading.Thread(target=read_stdin, daemon=True).start()
    if not done.wait(600):
        sys.exit('no authorization code within 10 minutes')
    if server:
        server.shutdown()
    return found['code']


if __name__ == '__main__':
    main()
