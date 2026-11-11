#!/usr/bin/env python3
"""Build a real GPT ESP with a fallback UEFI UKI, without mounts/loop devices."""
import hashlib
import os
from pathlib import Path
import shutil
import subprocess

out = Path('/out')
cmdline = out / 'cmdline'
cmdline.write_text('console=tty0 console=ttyS0 rdinit=/init panic=0 quiet\n')
os_release = out / 'os-release'
os_release.write_text('ID=qrab\nNAME="qrab UEFI console fixture"\nPRETTY_NAME="qrab UEFI console fixture"\n')
uki = out / 'BOOTX64.EFI'
ukify = '/usr/lib/systemd/ukify'
if not Path(ukify).is_file():
    ukify = 'ukify'
subprocess.run([ukify, 'build', '--linux=' + str(out / 'vmlinuz'), '--initrd=' + str(out / 'initramfs.gz'), '--cmdline=@' + str(cmdline), '--os-release=@' + str(os_release), '--output=' + str(uki)], check=True, env=dict(os.environ, SOURCE_DATE_EPOCH='0'))

image = out / 'boot.img'
with image.open('wb') as file:
    file.truncate(96 * 1024 * 1024)
# Keep the shared read-only fixture's layout and GUIDs fixed and inspectable.
subprocess.run(['sfdisk', str(image)], input='label: gpt\nlabel-id: ed8e496b-9cf9-4d89-a713-717261622d76\nunit: sectors\n\nstart=2048, size=192512, type=U, uuid=60d5fa42-619a-4519-970e-717261622d76\n', text=True, check=True)
esp = out / 'esp.fat'
with esp.open('wb') as file:
    file.truncate(94 * 1024 * 1024)
subprocess.run(['mkfs.fat', '-F', '32', '-i', '51524142', '-n', 'QRABEFI', str(esp)], check=True)
drive = str(esp)
subprocess.run(['mmd', '-i', drive, '::/EFI', '::/EFI/BOOT'], check=True)
subprocess.run(['mcopy', '-i', drive, str(uki), '::/EFI/BOOT/BOOTX64.EFI'], check=True)
with image.open('r+b') as disk, esp.open('rb') as partition:
    disk.seek(1024 * 1024)
    shutil.copyfileobj(partition, disk)
esp.unlink()
(out / 'boot.sha256').write_text(''.join(hashlib.sha256(path.read_bytes()).hexdigest() + '  ' + path.name + '\n' for path in [image, uki]))
