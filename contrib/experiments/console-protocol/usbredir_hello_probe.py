#!/usr/bin/env python3
"""Experiment A-USB: read QEMU usb-redir's usbredir hello on a plain chardev socket.

Usage: usbredir_hello_probe.py <usbredir.sock>

usbredirproto.h: struct usb_redir_header {u32 type; u32 length; u32 id;} (the
first packet always has a 32-bit id), type 0 = usb_redir_hello, payload =
char version[64] + u32 caps[]. This is the stream a browser-side usbredir
"host" (WebUSB) would speak; QEMU is the usbredir guest side.
"""
import json
import socket
import struct
import sys

CAPS = ['bulk_streams', 'connect_device_version', 'filter', 'device_disconnect_ack',
        'ep_info_max_packet_size', '64bits_ids', '32bits_bulk_length', 'bulk_receiving']


def recv_exact(sock, n):
    buf = b''
    while len(buf) < n:
        chunk = sock.recv(n - len(buf))
        if not chunk:
            raise ConnectionError(f'closed after {len(buf)}/{n} bytes')
        buf += chunk
    return buf


sock = socket.socket(socket.AF_UNIX)
sock.connect(sys.argv[1])
sock.settimeout(3)
kind, length, packet_id = struct.unpack('<III', recv_exact(sock, 12))
payload = recv_exact(sock, length)
version = payload[:64].split(b'\0')[0].decode()
caps = struct.unpack_from(f'<{(length - 64) // 4}I', payload, 64)
names = [name for bit, name in enumerate(CAPS) if caps and caps[0] >> bit & 1]
# Answer with our own hello (no caps) and check QEMU keeps the link open,
# waiting for usb_redir_device_connect from the (absent) host side.
ours = b'qrab-experiment'.ljust(64, b'\0') + struct.pack('<I', 0)
sock.sendall(struct.pack('<III', 0, len(ours), 0) + ours)
try:
    extra = sock.recv(1)
    still_open = 'closed' if extra == b'' else 'sent more data'
except socket.timeout:
    still_open = 'open, waiting for device_connect'
print(json.dumps({'first_packet_type': kind, 'is_hello': kind == 0, 'id': packet_id,
                  'qemu_usbredir_version': version, 'qemu_caps': names,
                  'after_our_hello': still_open}, indent=2))
