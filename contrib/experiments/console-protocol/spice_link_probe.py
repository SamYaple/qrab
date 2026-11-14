#!/usr/bin/env python3
"""Experiment B: link SPICE channels on a QEMU unix SPICE socket and record what
the server says, the way a BMC-side gate would see each connection.

Usage: spice_link_probe.py <spice.sock> [--seconds N] [--second-main]

Wire format (spice-protocol 0.14.3 spice/protocol.h, little-endian, packed):
  client -> SpiceLinkHeader{magic "REDQ", major 2, minor 2, size}
            SpiceLinkMess{connection_id u32, channel_type u8, channel_id u8,
                          num_common_caps u32, num_channel_caps u32, caps_offset u32}
            caps words
  server -> SpiceLinkHeader + SpiceLinkReply{error u32, pub_key[162],
                          num_common_caps, num_channel_caps, caps_offset} + caps
  client -> u32 auth mechanism (1 = SPICE, when AUTH_SELECTION negotiated)
            + 128-byte RSA-OAEP(SHA-1) ticket (empty password; ticketing disabled)
  server -> u32 link result
  then SpiceMiniDataHeader{type u16, size u32} messages (MINI_HEADER negotiated).
The channel a connection serves is fixed by its first 22 bytes: a gate can read
SpiceLinkMess.channel_type at offset 16+4 = byte 20 and refuse before forwarding.
"""
import argparse
import json
import socket
import struct
import time

from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import padding

CHANNELS = {1: 'main', 2: 'display', 3: 'inputs', 4: 'cursor', 5: 'playback', 6: 'record',
            7: 'tunnel', 8: 'smartcard', 9: 'usbredir', 10: 'port', 11: 'webdav'}
LINK_ERRORS = ['OK', 'ERROR', 'INVALID_MAGIC', 'INVALID_DATA', 'VERSION_MISMATCH', 'NEED_SECURED',
               'NEED_UNSECURED', 'PERMISSION_DENIED', 'BAD_CONNECTION_ID', 'CHANNEL_NOT_AVAILABLE']
COMMON_CAPS = (1 << 0) | (1 << 1) | (1 << 3)  # AUTH_SELECTION, AUTH_SPICE, MINI_HEADER
CHANNEL_CAPS = {5: (1 << 1) | (1 << 2) | (1 << 3),  # playback: VOLUME, LATENCY, OPUS
                6: (1 << 1) | (1 << 2)}             # record: VOLUME, OPUS


def recv_exact(sock, n):
    buf = b''
    while len(buf) < n:
        chunk = sock.recv(n - len(buf))
        if not chunk:
            raise ConnectionError(f'closed after {len(buf)}/{n} bytes')
        buf += chunk
    return buf


def link_message(connection_id, channel_type, channel_id=0):
    caps = [COMMON_CAPS] + ([CHANNEL_CAPS[channel_type]] if channel_type in CHANNEL_CAPS else [])
    body = struct.pack('<IBBIII', connection_id, channel_type, channel_id, 1, len(caps) - 1, 18)
    body += b''.join(struct.pack('<I', c) for c in caps)
    return struct.pack('<4sIII', b'REDQ', 2, 2, len(body)) + body


def gate_view(first_bytes):
    """What a BMC gate decides from the first bytes of a browser connection."""
    magic, major, _minor, size = struct.unpack_from('<4sIII', first_bytes)
    _conn, channel_type, channel_id = struct.unpack_from('<IBB', first_bytes, 16)
    return {'magic': magic.decode(), 'major': major, 'size': size,
            'channel': CHANNELS.get(channel_type, channel_type), 'channel_id': channel_id}


def link(path, connection_id, channel_type, channel_id=0):
    sock = socket.socket(socket.AF_UNIX)
    sock.connect(path)
    sock.settimeout(2.0)
    hello = link_message(connection_id, channel_type, channel_id)
    sock.sendall(hello)
    try:
        magic, major, minor, size = struct.unpack('<4sIII', recv_exact(sock, 16))
    except ConnectionError:
        # spice-server closes an unknown non-main channel without a SpiceLinkReply.
        sock.close()
        return None, {'gate_view': gate_view(hello), 'link_error': 'closed without reply'}
    reply = recv_exact(sock, size)
    error = struct.unpack_from('<I', reply)[0]
    out = {'gate_view': gate_view(hello), 'reply_magic': magic.decode(), 'reply_version': [major, minor],
           'link_error': LINK_ERRORS[error] if error < len(LINK_ERRORS) else error}
    if error:
        sock.close()
        return None, out
    pub_key = reply[4:166]
    ncommon, nchannel, offset = struct.unpack_from('<III', reply, 166)
    words = struct.unpack_from(f'<{ncommon + nchannel}I', reply, offset)
    out['server_common_caps'] = hex(words[0]) if ncommon else None
    out['server_channel_caps'] = hex(words[ncommon]) if nchannel else None
    key = serialization.load_der_public_key(pub_key)
    ticket = key.encrypt(b'\0', padding.OAEP(mgf=padding.MGF1(hashes.SHA1()), algorithm=hashes.SHA1(), label=None))
    sock.sendall(struct.pack('<I', 1) + ticket)
    result = struct.unpack('<I', recv_exact(sock, 4))[0]
    out['auth_result'] = LINK_ERRORS[result] if result < len(LINK_ERRORS) else result
    return sock, out


def read_message(sock):
    kind, size = struct.unpack('<HI', recv_exact(sock, 6))
    return kind, recv_exact(sock, size)


def answer_common(sock, kind, payload, state):
    if kind == 3:  # SET_ACK {generation u32, window u32}
        generation, window = struct.unpack_from('<II', payload)
        state['window'] = window
        sock.sendall(struct.pack('<HI', 1, 4) + struct.pack('<I', generation))  # ACK_SYNC
        return True
    if kind == 4:  # PING {id u32, timestamp u64, data}
        sock.sendall(struct.pack('<HI', 3, 12) + payload[:12])  # PONG
        return True
    return False


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('socket')
    ap.add_argument('--seconds', type=float, default=3.0)
    ap.add_argument('--second-main', action='store_true')
    args = ap.parse_args()
    report = {}

    main_sock, report['main'] = link(args.socket, 0, 1)
    state = {}
    session = None
    channels = None
    deadline = time.monotonic() + 3
    while time.monotonic() < deadline and channels is None:
        kind, payload = read_message(main_sock)
        if answer_common(main_sock, kind, payload, state):
            continue
        if kind == 103:  # MAIN_INIT
            session = struct.unpack_from('<I', payload)[0]
            main_sock.sendall(struct.pack('<HI', 104, 0))  # ATTACH_CHANNELS
        elif kind == 104:  # CHANNELS_LIST {num u32, (type u8, id u8)*}
            count = struct.unpack_from('<I', payload)[0]
            channels = [f'{CHANNELS.get(payload[4 + 2 * i], payload[4 + 2 * i])}:{payload[5 + 2 * i]}'
                        for i in range(count)]
    report['session_id_present'] = session is not None
    report['channels_list'] = channels

    playback, report['playback'] = link(args.socket, session, 5)
    counts, data_bytes, mode, start = {}, 0, None, None
    began = time.monotonic()
    while playback and time.monotonic() - began < args.seconds:
        try:
            kind, payload = read_message(playback)
        except socket.timeout:
            continue
        counts[kind] = counts.get(kind, 0) + 1
        if answer_common(playback, kind, payload, state):
            continue
        if kind == 102:  # PLAYBACK_MODE {time u32, mode u16, data}
            mode = {1: 'raw', 2: 'celt', 3: 'opus'}.get(struct.unpack_from('<H', payload, 4)[0])
        elif kind == 103:  # PLAYBACK_START {channels u32, format u16, frequency u32, time u32}
            ch, fmt, freq, _ = struct.unpack_from('<IHII', payload)
            start = {'channels': ch, 'format': 'S16' if fmt == 1 else fmt, 'frequency': freq}
        elif kind == 101:  # PLAYBACK_DATA {time u32, data}
            data_bytes += len(payload) - 4
    seconds = time.monotonic() - began
    report['playback_stream'] = {'message_counts': {str(k): v for k, v in sorted(counts.items())},
                                 'mode': mode, 'start': start, 'payload_bytes': data_bytes,
                                 'payload_bytes_per_second': round(data_bytes / seconds),
                                 'data_messages_per_second': round(counts.get(101, 0) / seconds, 1)}

    wrong, info = link(args.socket, (session or 0) ^ 0x5A5A5A5A, 5)
    report['bad_connection_id'] = info.get('auth_result', info['link_error'])
    if wrong:
        wrong.close()
    held = []  # kept open: closing a linked port channel crashed QEMU (see spice_channel_churn.py)
    for channel_type in (6, 8, 9, 10, 11):
        try:
            sock, info = link(args.socket, session, channel_type)
        except OSError as error:
            report[f'link_{CHANNELS[channel_type]}'] = f'{type(error).__name__}: {error}'
            continue
        report[f'link_{CHANNELS[channel_type]}'] = info.get('auth_result', info['link_error'])
        if sock:
            held.append(sock)

    if args.second_main:
        second, info = link(args.socket, 0, 1)
        report['second_main'] = info.get('auth_result', info['link_error'])
        time.sleep(1)
        try:
            main_sock.settimeout(1)
            while True:
                read_message(main_sock)
        except TimeoutError:
            report['first_main_after_second'] = 'still connected'
        except (ConnectionError, OSError) as error:
            report['first_main_after_second'] = f'disconnected: {type(error).__name__}: {error}'
        if second:
            second.close()
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
