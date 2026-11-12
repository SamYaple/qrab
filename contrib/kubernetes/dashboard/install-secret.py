#!/usr/bin/env python3
"""Install or rotate Secret qrab/qrab-dashboard from hidden prompts.

Both credentials reach `kubectl create secret --dry-run=client -o yaml` through
inherited pipes (/dev/fd), never through process arguments, files or output.
The rendered Secret is applied server-side from stdin
(`kubectl apply --server-side`), so no kubectl.kubernetes.io/last-applied-configuration
annotation is written: a client-side apply would copy both credentials into it.
"""
import argparse
import getpass
import os
import subprocess

parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
parser.add_argument('--namespace', default='qrab')
args = parser.parse_args()
assert args.namespace == 'qrab', 'This installer is scoped to the qrab namespace'
client_secret = getpass.getpass('OIDC client secret of qrab-dashboard: ').strip()
token = getpass.getpass('Kanidm API token of service account qrab-dashboard-groups: ').strip()
assert client_secret and token and client_secret != token, 'Two distinct credentials are required'


def pipe(value):
    """A readable fd holding `value`; credentials are small enough for the pipe buffer."""
    read, write = os.pipe()
    os.write(write, value.encode())
    os.close(write)
    return read


fds = [pipe(client_secret), pipe(token)]
try:
    rendered = subprocess.run(
        ['kubectl', 'create', 'secret', 'generic', 'qrab-dashboard', '--namespace', args.namespace,
         f'--from-file=oidc-client-secret=/dev/fd/{fds[0]}', f'--from-file=kanidm-token=/dev/fd/{fds[1]}',
         '--dry-run=client', '-o', 'yaml'],
        pass_fds=fds, text=True, capture_output=True)
finally:
    for fd in fds:
        os.close(fd)
if rendered.returncode:
    raise SystemExit('Rendering the Secret failed. Credential values suppressed.')
result = subprocess.run(
    ['kubectl', 'apply', '--server-side', '--field-manager=qrab-dashboard-install', '--force-conflicts', '-f', '-'],
    input=rendered.stdout, text=True, capture_output=True)
if result.returncode:
    raise SystemExit('Secret installation failed; check the namespace and Kubernetes permissions. Credential values suppressed.')
print(f'Installed {args.namespace}/qrab-dashboard server-side; credential values suppressed. Restart the Deployment to read them.')
