#!/usr/bin/env python3
"""Assemble the RAM-only graphical console fixture; no guest network tools."""
import pathlib
import shutil
import subprocess

root = pathlib.Path('/guest')
out = pathlib.Path('/out')
for directory in ['bin', 'dev', 'proc', 'sys', 'tmp', 'run', 'var/run', 'var/log', 'etc', 'usr/sbin']:
    (root / directory).mkdir(parents=True, exist_ok=True)
out.mkdir(exist_ok=True)
kernel, = pathlib.Path('/boot').glob('vmlinuz-*')
version = kernel.name.removeprefix('vmlinuz-')
shutil.copyfile(kernel, out / 'vmlinuz')
(out / 'vmlinuz').chmod(0o644)
shutil.copy('/bin/busybox', root / 'bin/busybox')
(root / 'bin/sh').symlink_to('busybox')

modules = []
for name in ['xhci_pci', 'usbhid', 'hid_generic', 'mousedev', 'evdev']:
    dependencies = subprocess.check_output(['modprobe', '--set-version', version, '--show-depends', name], text=True)
    for line in dependencies.splitlines():
        if not line.startswith('insmod '):
            continue
        source = pathlib.Path(line.split()[1])
        # Insmod uncompressed modules in dependency order, without relying on
        # BusyBox supporting the host distribution's compression/depmod format.
        destination = pathlib.Path('/lib/qrab-modules') / (source.name.split('.ko')[0] + '.ko')
        if str(destination) in modules:
            continue
        target = root / destination.relative_to('/')
        target.parent.mkdir(parents=True, exist_ok=True)
        if source.suffix == '.xz':
            import lzma
            target.write_bytes(lzma.decompress(source.read_bytes()))
        elif source.suffix == '.zst':
            target.write_bytes(subprocess.check_output(['zstd', '-dc', str(source)]))
        else:
            shutil.copyfile(source, target)
        modules.append(str(destination))
(root / 'etc/qrab-modules').write_text('\n'.join(modules) + '\n')

(out / 'guest-packages.txt').write_bytes(subprocess.check_output(['dpkg-query', '-W', 'linux-image*', 'busybox-static']))
