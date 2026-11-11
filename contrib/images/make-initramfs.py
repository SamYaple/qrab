#!/usr/bin/env python3
"""Write a reproducible newc initramfs, including virtual console device nodes.

Device metadata is encoded directly; building the guest needs no host mknod or
privileges. The guest kernel creates these nodes when it unpacks the archive.
"""
import gzip
import os
from pathlib import Path
import stat
import sys

root, output = map(Path, sys.argv[1:])
archive = bytearray()
entries = [('.', root.lstat().st_mode, b'', 0, 0)]
for path in sorted(root.rglob('*')):
    mode = path.lstat().st_mode
    data = path.read_bytes() if stat.S_ISREG(mode) else os.readlink(path).encode() if stat.S_ISLNK(mode) else b''
    entries.append((path.relative_to(root).as_posix(), mode, data, 0, 0))
entries += [('dev/console', stat.S_IFCHR | 0o600, b'', 5, 1), ('dev/null', stat.S_IFCHR | 0o666, b'', 1, 3), ('TRAILER!!!', 0, b'', 0, 0)]
for inode, (name, mode, data, major, minor) in enumerate(entries, 1):
    name = name.encode() + b'\0'
    fields = [inode, mode, 0, 0, 1, 0, len(data), 0, 0, major, minor, len(name), 0]
    archive.extend(('070701' + ''.join(f'{value:08x}' for value in fields)).encode())
    archive.extend(name)
    archive.extend(b'\0' * (-len(archive) % 4))
    archive.extend(data)
    archive.extend(b'\0' * (-len(archive) % 4))
output.write_bytes(gzip.compress(archive, mtime=0))
