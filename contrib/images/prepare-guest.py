#!/usr/bin/env python3
"""Assemble the RAM-only UEFI/TPM2 console fixture without guest networking."""
import pathlib
import re
import shutil
import subprocess

root = pathlib.Path('/guest')
out = pathlib.Path('/out')
for directory in ['bin', 'dev', 'proc', 'sys', 'tmp', 'run', 'var/run', 'var/log', 'etc', 'etc/qrab', 'usr/bin', 'usr/sbin']:
    (root / directory).mkdir(parents=True, exist_ok=True)
out.mkdir(exist_ok=True)
kernel, = pathlib.Path('/boot').glob('vmlinuz-*')
version = kernel.name.removeprefix('vmlinuz-')
shutil.copyfile(kernel, out / 'vmlinuz')
(out / 'vmlinuz').chmod(0o644)
shutil.copy('/bin/busybox', root / 'bin/busybox')
(root / 'bin/sh').symlink_to('busybox')

modules = []
for name in ['xhci_pci', 'usbhid', 'hid_generic', 'mousedev', 'evdev', 'tpm_tis', 'virtio_pci', 'virtio_console', 'virtio_blk', 'virtio_scsi', 'sd_mod', 'crc32c_generic', 'ext4']:
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

# These tools exercise the real guest TPM rather than asserting that QEMU
# merely received -tpmdev. Include dlopen's device TCTI as well as ELF deps.
def copy_binary_with_libraries(source):
    source = pathlib.Path(source)
    paths = [source, *map(pathlib.Path, re.findall(r'/[^\s()]+', subprocess.check_output(['ldd', str(source)], text=True)))]
    for path in paths:
        target = root / path.relative_to('/')
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, target)
        target.chmod(path.stat().st_mode)

for name in ['tpm2_getcap', 'tpm2_pcrread', 'tpm2_createek', 'tpm2_flushcontext', 'tpm2_eventlog']:
    copy_binary_with_libraries('/usr/bin/' + name)
copy_binary_with_libraries('/usr/lib/x86_64-linux-gnu/libtss2-tcti-device.so.0')
for binary in ['/usr/sbin/mke2fs', '/usr/sbin/blkid', '/usr/sbin/blockdev', '/usr/bin/cmp']:
    copy_binary_with_libraries(binary)
shutil.copyfile('/etc/mke2fs.conf', root / 'etc/mke2fs.conf')
(out / 'guest-packages.txt').write_bytes(subprocess.check_output(['dpkg-query', '-W', 'linux-image*', 'busybox-static', 'tpm2-tools', 'systemd-boot-efi', 'systemd-ukify', 'e2fsprogs', 'diffutils']))
