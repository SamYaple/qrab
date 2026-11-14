#!/usr/bin/env python3
"""Experiment A: negotiate QEMU's VNC audio extension on a unix VNC socket.

Usage: vnc_audio_probe.py <vnc.sock> [--seconds N] [--no-audio-encoding]

Speaks RFB 3.8 with security None, advertises Raw plus (unless
--no-audio-encoding) the QEMU audio pseudo-encoding -259, waits for QEMU's
-259 acknowledgement rectangle, sends set-format (S16LE, 2 ch, 44100 Hz) and
enable, then counts QEMU audio messages (255/1/{0 end,1 begin,2 data}).
With --no-audio-encoding it sends the audio enable anyway, to show QEMU drops
the connection when the extension was not negotiated.
"""
import argparse
import json
import socket
import struct
import time

AUDIO = -259


def recv_exact(sock, n):
    buf = b''
    while len(buf) < n:
        chunk = sock.recv(n - len(buf))
        if not chunk:
            raise ConnectionError(f'closed after {len(buf)}/{n} bytes')
        buf += chunk
    return buf


def handshake(sock):
    version = recv_exact(sock, 12)
    sock.sendall(b'RFB 003.008\n')
    count = recv_exact(sock, 1)[0]
    types = list(recv_exact(sock, count))
    if 1 not in types:
        raise RuntimeError(f'no None security type: {types}')
    sock.sendall(b'\x01')
    if struct.unpack('>I', recv_exact(sock, 4))[0] != 0:
        raise RuntimeError('security failed')
    sock.sendall(b'\x01')  # ClientInit shared=1
    width, height = struct.unpack('>HH', recv_exact(sock, 4))
    pixel_format = recv_exact(sock, 16)
    name = recv_exact(sock, struct.unpack('>I', recv_exact(sock, 4))[0])
    return version.decode().strip(), width, height, pixel_format, name.decode()


def set_encodings(sock, encodings):
    sock.sendall(struct.pack('>BxH', 2, len(encodings)) + b''.join(struct.pack('>i', e) for e in encodings))


def read_message(sock, bpp):
    kind = recv_exact(sock, 1)[0]
    if kind == 0:  # FramebufferUpdate
        _, count = struct.unpack('>BH', recv_exact(sock, 3))
        rects = []
        for _ in range(count):
            x, y, w, h, enc = struct.unpack('>HHHHi', recv_exact(sock, 12))
            if enc == 0:
                recv_exact(sock, w * h * bpp)
            elif enc < 0 and enc != -223:
                pass  # pseudo-encodings without payload (-259 ack, -258 ext key)
            elif enc == -223:  # DesktopSize
                pass
            else:
                raise RuntimeError(f'unexpected encoding {enc}')
            rects.append(enc)
        return ('fbu', rects)
    if kind == 255:
        sub, op = struct.unpack('>BH', recv_exact(sock, 3))
        if sub == 1 and op == 2:
            size = struct.unpack('>I', recv_exact(sock, 4))[0]
            return ('audio-data', recv_exact(sock, size))
        return ('audio-begin' if op == 1 else 'audio-end' if op == 0 else f'qemu-{sub}-{op}', None)
    if kind == 1:  # SetColourMapEntries
        _, _, n = struct.unpack('>BHH', recv_exact(sock, 5))
        recv_exact(sock, n * 6)
        return ('colourmap', None)
    if kind == 2:
        return ('bell', None)
    if kind == 3:
        recv_exact(sock, 3)
        return ('cut', recv_exact(sock, struct.unpack('>I', recv_exact(sock, 4))[0]))
    raise RuntimeError(f'unknown server message {kind}')


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('socket')
    ap.add_argument('--seconds', type=float, default=3.0)
    ap.add_argument('--no-audio-encoding', action='store_true')
    args = ap.parse_args()
    sock = socket.socket(socket.AF_UNIX)
    sock.connect(args.socket)
    sock.settimeout(2.0)
    version, w, h, pf, name = handshake(sock)
    bpp = pf[0] // 8
    result = {'version': version, 'size': [w, h], 'name': name}
    set_encodings(sock, [0] if args.no_audio_encoding else [0, AUDIO])
    acked = False
    if not args.no_audio_encoding:
        try:
            kind, payload = read_message(sock, bpp)
            acked = kind == 'fbu' and AUDIO in payload
        except socket.timeout:
            pass
    result['audio_ack_rect'] = acked
    # set-format S16 (3), 2 channels, 44100 Hz; then enable.
    sock.sendall(struct.pack('>BBHBBI', 255, 1, 2, 3, 2, 44100) + struct.pack('>BBH', 255, 1, 0))
    counts = {}
    data_bytes = 0
    first = None
    nonzero = 0
    start = time.monotonic()
    try:
        while time.monotonic() - start < args.seconds:
            try:
                kind, payload = read_message(sock, bpp)
            except socket.timeout:
                continue
            counts[kind] = counts.get(kind, 0) + 1
            if kind == 'audio-data':
                data_bytes += len(payload)
                first = first or time.monotonic()
                nonzero += sum(1 for i in range(0, len(payload) - 1, 2) if payload[i:i + 2] != b'\0\0')
    except ConnectionError as error:
        result['closed_by_server'] = str(error)
    elapsed = time.monotonic() - (first or start)
    result.update({'messages': counts, 'audio_bytes': data_bytes,
                   'audio_bytes_per_second': round(data_bytes / elapsed) if data_bytes else 0,
                   'expected_bytes_per_second_s16_stereo_44k1': 44100 * 2 * 2,
                   'nonzero_samples': nonzero})
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
