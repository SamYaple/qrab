#!/usr/bin/env python3
"""Experiment B4: replay the link sequence that first crashed QEMU (exit 139).

main; playback with a wrong connection_id (closed); then record, smartcard,
usbredir, port, webdav each linked and closed at once; then playback again.
Usage: spice_crash_repro.py <spice.sock> [middle channel types, default 6,8,9,10,11]
"""
import pathlib
import struct
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from spice_link_probe import CHANNELS, link, read_message, answer_common  # noqa: E402

path = sys.argv[1]
main_sock, _ = link(path, 0, 1)
session, state = None, {}
while session is None:
    kind, payload = read_message(main_sock)
    if not answer_common(main_sock, kind, payload, state) and kind == 103:
        session = struct.unpack_from('<I', payload)[0]
steps = []
wrong, info = link(path, session ^ 0x5A5A5A5A, 5)
steps.append(('playback/wrong-id', info.get('auth_result', info['link_error'])))
if wrong:
    wrong.close()
middle = [int(t) for t in (sys.argv[2] if len(sys.argv) > 2 else '6,8,9,10,11').split(',')]
for channel_type in middle:
    try:
        sock, info = link(path, session, channel_type)
        steps.append((CHANNELS[channel_type], info.get('auth_result', info['link_error'])))
        if sock:
            sock.close()
    except OSError as error:
        steps.append((CHANNELS[channel_type], f'{type(error).__name__}: {error}'))
try:
    sock, info = link(path, session, 5)
    steps.append(('playback', info.get('auth_result', info['link_error'])))
except OSError as error:
    steps.append(('playback', f'{type(error).__name__}: {error}'))
print(' '.join(f'{name}={result}' for name, result in steps))
