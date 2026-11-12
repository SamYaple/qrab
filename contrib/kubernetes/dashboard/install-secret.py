#!/usr/bin/env python3
"""Install or rotate Secret qrab/qrab-dashboard through hidden prompts and kubectl stdin.

Credentials are never placed in process arguments, files or output.
"""
import argparse
import getpass
import json
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--namespace', default='qrab')
args = parser.parse_args()
assert args.namespace == 'qrab', 'This installer is scoped to the qrab namespace'
client_secret = getpass.getpass('OIDC client secret of qrab-dashboard: ').strip()
token = getpass.getpass('Kanidm API token of service account qrab-dashboard-groups: ').strip()
assert client_secret and token and client_secret != token, 'Two distinct credentials are required'
secret = {
    'apiVersion': 'v1',
    'kind': 'Secret',
    'metadata': {'name': 'qrab-dashboard', 'namespace': args.namespace},
    'type': 'Opaque',
    'stringData': {'oidc-client-secret': client_secret, 'kanidm-token': token},
}
result = subprocess.run(['kubectl', 'apply', '-f', '-'], input=json.dumps(secret), text=True, capture_output=True)
if result.returncode:
    raise SystemExit('Secret installation failed; check the namespace and Kubernetes permissions. Credential values suppressed.')
print(f'Installed {args.namespace}/qrab-dashboard; credential values suppressed. Restart the Deployment to read them.')
