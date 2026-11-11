#!/usr/bin/env python3
"""Run an isolated loopback-only userspace iSCSI test target.

This uses a disposable regular file only for protocol tests, not VM production
storage. Build Containerfile.iscsi-test first. Connection metadata and the CHAP
password file are private; SIGTERM stops the target and removes all test state.
"""
import argparse
import json
import os
from pathlib import Path
import secrets
import signal
import socket
import subprocess
import tempfile
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--image', default='localhost/qrab-iscsi-target-test:latest')
args = parser.parse_args()
os.umask(0o077)
suffix = secrets.token_hex(4)
container = 'qrab-iscsi-test-' + suffix
password = secrets.token_urlsafe(12)
username = 'qrab-test-' + suffix
initiator = 'iqn.2026-11.com.yapplesauce.qrab:test-' + suffix
target = 'iqn.2026-11.com.yapplesauce.qrab:fixture-' + suffix
stop = False

def finish(_signal, _frame):
    global stop
    stop = True

for sig in [signal.SIGTERM, signal.SIGINT, signal.SIGHUP]:
    signal.signal(sig, finish)

def run(command, check=True):
    result = subprocess.run(command, capture_output=True, text=True)
    if check and result.returncode:
        # Do not let CalledProcessError print an argv containing a test secret.
        raise RuntimeError('iSCSI fixture command failed: ' + result.stderr.replace(password, '[redacted]')[-1000:])
    return result

def admin(*arguments, check=True):
    return run(['podman', 'exec', container, 'tgtadm', '--lld', 'iscsi', *arguments], check=check)

with tempfile.TemporaryDirectory(prefix='qrab-iscsi-test-') as directory:
    root = Path(directory)
    root.chmod(0o700)
    disk = root / 'lun.raw'
    with disk.open('wb') as file:
        file.truncate(128 * 1024 * 1024)
    password_file = root / 'chap-password'
    password_file.write_text(password)
    with socket.socket() as listener:
        listener.bind(('127.0.0.1', 0))
        port = listener.getsockname()[1]
    created = False
    published = False
    try:
        run(['podman', 'run', '-d', '--name', container, '--read-only', '--cap-drop', 'ALL', '--security-opt', 'no-new-privileges', '--tmpfs', '/run:rw,mode=1777', '--tmpfs', '/tmp:rw,mode=1777', '-p', f'127.0.0.1:{port}:3260', '-v', str(disk) + ':/lun.raw:Z', args.image])
        created = True
        for _ in range(100):
            if admin('--mode', 'target', '--op', 'show', check=False).returncode == 0:
                break
            time.sleep(.1)
        else:
            raise RuntimeError('iSCSI fixture control socket did not become ready')
        admin('--mode', 'target', '--op', 'new', '--tid', '1', '--targetname', target)
        admin('--mode', 'logicalunit', '--op', 'new', '--tid', '1', '--lun', '1', '--bstype', 'rdwr', '--backing-store', '/lun.raw')
        admin('--mode', 'account', '--op', 'new', '--user', username, '--password', password)
        admin('--mode', 'account', '--op', 'bind', '--tid', '1', '--user', username)
        admin('--mode', 'target', '--op', 'bind', '--tid', '1', '--initiator-name', initiator)
        metadata = {'pid': os.getpid(), 'container': container, 'portal': f'127.0.0.1:{port}', 'target': target, 'lun': 1, 'username': username, 'password_file': str(password_file), 'initiator_name': initiator, 'size_bytes': 128 * 1024 * 1024}
        with args.output.open('x') as file:
            json.dump(metadata, file)
            file.write('\n')
        published = True
        print('Fixture ready; private connection metadata: ' + str(args.output), flush=True)
        while not stop:
            time.sleep(.5)
    finally:
        if created:
            run(['podman', 'rm', '-f', container], check=False)
        if published:
            args.output.unlink(missing_ok=True)
