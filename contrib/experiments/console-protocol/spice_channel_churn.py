#!/usr/bin/env python3
"""Experiment B2: link the main channel, then link and immediately close one other
channel type, then check that the SPICE server still answers a fresh link.

Usage: spice_channel_churn.py <spice.sock> <channel_type>[,<channel_type>...] [--hold SECONDS]
Several types are linked in order, then closed in order, before the liveness check.
Prints one JSON line.
"""
import argparse
import json
import pathlib
import struct
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from spice_link_probe import CHANNELS, link, read_message, answer_common  # noqa: E402


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('socket')
    ap.add_argument('channel_types')
    ap.add_argument('--hold', type=float, default=0.0)
    args = ap.parse_args()
    types = [int(t) for t in args.channel_types.split(',')]
    out = {'channels': [CHANNELS.get(t, t) for t in types], 'hold_seconds': args.hold}
    main_sock, info = link(args.socket, 0, 1)
    session, state = None, {}
    while session is None:
        kind, payload = read_message(main_sock)
        if not answer_common(main_sock, kind, payload, state) and kind == 103:
            session = struct.unpack_from('<I', payload)[0]
    socks, out['link'] = [], []
    for channel_type in types:
        sock, info = link(args.socket, session, channel_type)
        out['link'].append(info.get('auth_result', info['link_error']))
        if sock:
            socks.append(sock)
    time.sleep(args.hold)
    for sock in socks:
        sock.close()
    time.sleep(1.0)
    try:
        probe, info = link(args.socket, session, 5)
        out['server_alive_after_close'] = info.get('auth_result', info['link_error'])
    except OSError as error:
        out['server_alive_after_close'] = f'no: {type(error).__name__}: {error}'
    print(json.dumps(out))


if __name__ == '__main__':
    main()
