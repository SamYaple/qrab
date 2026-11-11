#!/usr/bin/env python3
"""Explicit disposable-LUN integration test; never discovers or formats real disks.

The fixture metadata must reference an isolated CHAP target and a password file.
Only --initialize-disposable permits the guest's blank, bounded qrab-probe LUN
helper to initialize a filesystem. All connection metadata stays private.
"""
import argparse
import json
import os
from pathlib import Path
import queue
import signal
import socket
import struct
import subprocess
import tempfile
import threading
import time

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--fixture', type=Path, required=True)
p.add_argument('--manager', type=Path, required=True)
p.add_argument('--artifacts', type=Path, required=True)
p.add_argument('--guest', type=Path)
p.add_argument('--initialize-disposable', action='store_true', required=True)
p.add_argument('--verify-existing', action='store_true', help='reuse this isolated fixture without formatting it again')
a = p.parse_args()
fixture = json.loads(a.fixture.read_text())
assert 64 * 1024**2 <= fixture['size_bytes'] <= 256 * 1024**2
secret = Path(fixture['password_file']).read_bytes()
artifacts = a.artifacts.resolve()
guest = a.guest.resolve() if a.guest else artifacts/'demo'
env = dict(os.environ, LD_LIBRARY_PATH=str(artifacts / 'libs') + (':' + os.environ['LD_LIBRARY_PATH'] if os.environ.get('LD_LIBRARY_PATH') else ''))


def exact(stream, length):
    data = b''
    while len(data) < length:
        part = stream.recv(length - len(data))
        if not part:
            raise RuntimeError('VNC closed before initialization')
        data += part
    return data


def console(path):
    stream = socket.socket(socket.AF_UNIX)
    stream.settimeout(10)
    stream.connect(str(path))
    assert exact(stream, 12) == b'RFB 003.008\n'
    stream.sendall(b'RFB 003.008\n')
    count = exact(stream, 1)[0]
    assert 1 in exact(stream, count)
    stream.sendall(b'\x01')
    assert exact(stream, 4) == b'\0\0\0\0'
    stream.sendall(b'\x01')
    initialization = exact(stream, 24)
    exact(stream, int.from_bytes(initialization[20:24], 'big'))
    return stream


def key(stream, value, down):
    stream.sendall(struct.pack('>BBHI', 4, int(down), 0, value))
    time.sleep(.01)


def command(stream, text):
    for value in text:
        shifted = value.isupper() or value in '>'
        if shifted:
            key(stream, 0xffe1, True)
        key(stream, ord(value), True)
        key(stream, ord(value), False)
        if shifted:
            key(stream, 0xffe1, False)
    key(stream, 0xff0d, True)
    key(stream, 0xff0d, False)


def read_until(lines, marker, seconds=90):
    deadline = time.monotonic() + seconds
    observed = []
    while time.monotonic() < deadline:
        try:
            line = lines.get(timeout=min(1, max(.001, deadline-time.monotonic())))
        except queue.Empty:
            continue
        if line is None:
            raise RuntimeError('VM exited before expected guest evidence')
        observed.append(line)
        if marker in line:
            return observed
        if 'qrab-disk-probe:' in line:
            raise RuntimeError(line.strip())
    raise RuntimeError('Timed out waiting for guest evidence: ' + marker)


def evidence(lines):
    result = {}
    for line in lines:
        for name in ['UUID', 'TOKEN', 'SHA256']:
            prefix = 'QRAB_DISK_PROBE_' + name + '='
            if prefix in line:
                result[name] = line.split(prefix, 1)[1].strip()
    assert len(result) == 3
    return result


with tempfile.TemporaryDirectory(prefix='qrab-iscsi-live-') as scratch:
    root = Path(scratch)
    disk = {name:fixture[name] for name in ['portal','target','lun','initiator_name','username','password_file']}
    disk.update(id='qrab-probe', read_only=False, bus='virtio')
    config = dict(name='iscsi-test', state_id='disposable-live-test', memory_mib=512,
                  qemu_binary=str(artifacts/'qemu/bin/qemu-system-x86_64'),
                  firmware_dir=str(artifacts/'qemu/share/qemu'),
                  uefi=dict(code=str(artifacts/'firmware/OVMF_CODE_4M.fd'), vars_template=str(artifacts/'firmware/OVMF_VARS_4M.fd'), boot_disk=str(guest/'boot.img')),
                  tpm=dict(swtpm_binary='/usr/bin/swtpm'), console=True,
                  state_dir=str(root/'state'), runtime_dir=str(root/'runtime'),
                  iscsi_disks=[disk], startup_timeout_secs=10, shutdown_grace_secs=0)
    for run in range(3):
        current = json.loads(json.dumps(config))
        current['runtime_dir'] = str(root/('runtime-'+str(run)))
        current['iscsi_disks'][0]['read_only'] = run == 1
        if run == 2:
            wrong = root/'wrong-password'
            wrong.write_text('incorrect-disposable-password')
            wrong.chmod(0o600)
            current['iscsi_disks'][0]['password_file'] = str(wrong)
        path = root/'vm.json'
        path.write_text(json.dumps(current))
        path.chmod(0o600)
        error_path = root/('stderr-'+str(run))
        with error_path.open('wb') as error:
            child = subprocess.Popen([str(a.manager.resolve()),'run','--config',str(path)], stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=error, env=env, start_new_session=True)
            lines = queue.Queue()
            serial_path = Path('/tmp/qrab-iscsi-private-serial.log')
            serial_fd = os.open(serial_path, os.O_WRONLY|os.O_CREAT|os.O_TRUNC,0o600)
            def reader():
                with os.fdopen(serial_fd,'wb',buffering=0) as serial:
                    for line in child.stdout:
                        serial.write(line)
                        lines.put(line.decode(errors='replace'))
                lines.put(None)
            thread = threading.Thread(target=reader, daemon=True)
            thread.start()
            try:
                if run == 2:
                    assert child.wait(timeout=45) != 0, 'wrong CHAP credential unexpectedly booted'
                    assert secret not in error_path.read_bytes(), 'secret appeared in error output'
                    print('PASS wrong CHAP credential fails startup without a local-disk fallback')
                    continue
                read_until(lines, 'QRAB_GUEST_NO_NIC_OK')
                time.sleep(.2)
                status = json.loads((Path(current['runtime_dir'])/'status.json').read_text())
                arguments = Path('/proc/'+str(status['qemu_pid'])+'/cmdline').read_bytes()
                assert secret not in arguments and secret not in json.dumps(current).encode()
                assert b'"driver":"iscsi"' in arguments and b'-netdev' not in arguments
                assert b'-nic\0none\0' in arguments
                session, = Path(current['runtime_dir']).glob('session-*')
                with console(session/'vnc.sock') as stream:
                    if run == 0:
                        command(stream, 'qrab-disk-probe '+('verify' if a.verify_existing else 'initialize')+' --serial')
                        first = evidence(read_until(lines, 'QRAB_DISK_PROBE_VERIFIED'))
                        command(stream, 'qrab-disk-probe initialize --serial')
                        read_until(lines, 'existing or unreadable signature')
                    command(stream, 'qrab-disk-probe verify --serial')
                    assert evidence(read_until(lines, 'QRAB_DISK_PROBE_VERIFIED')) == first
                child.send_signal(signal.SIGTERM)
                assert child.wait(timeout=20) == 0, 'manager did not prove a clean child exit'
                for field in ['qemu_pid','swtpm_pid']:
                    assert not Path('/proc/'+str(status[field])).exists()
                print('PASS '+(('native iSCSI RW '+('verify-existing' if a.verify_existing else 'initialize')+', second-init refusal and clean shutdown') if run == 0 else 'retained filesystem UUID/token/hash after replacement and readonly verify'))
            finally:
                if child.poll() is None:
                    os.killpg(child.pid, signal.SIGKILL)
                    child.wait(timeout=10)
                thread.join(timeout=2)
