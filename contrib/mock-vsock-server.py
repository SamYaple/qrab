#!/usr/bin/env python3
"""Minimal AF_VSOCK echo / attestation stub used by the qrab-boot
phase-3b smoke harness.

Listens on the given port (default 1337) via `AF_VSOCK`,
`SOCK_STREAM`, `VMADDR_CID_ANY`. On every accepted connection it:

  1. Drains up to 4 KiB of request bytes (non-blocking, best-effort).
  2. Writes one line back — either `QRAB_MOCK_PONG v0\n` if no
     `QRAB_MOCK_REPLY` env var is set, or the env var's contents
     verbatim.
  3. Closes the connection.

It loops until SIGINT / SIGTERM. Every interesting transition is
logged to stderr with a `mock-vsock:` prefix so the smoke script can
grep without colliding with qemu/guest log lines.

Prerequisites:
  - Kernel module `vhost_vsock` (host side) + `vsock` (same host).
  - The qemu VM launched with `-device vhost-vsock-pci,guest-cid=N`
    for some N >= 3. The host's CID is always 2.
  - Python >= 3.7 (AF_VSOCK was added in 3.7).

Exit codes:
   0 on clean shutdown, 1 on early setup failure. The smoke script
   treats the process as a side-channel; it reads the guest serial
   log for pass/fail, not this process's exit status.
"""

from __future__ import annotations

import argparse
import os
import signal
import socket
import sys
import threading

DEFAULT_PORT = 1337
RECV_LIMIT = 4096


def log(msg: str) -> None:
    print(f"mock-vsock: {msg}", file=sys.stderr, flush=True)


def handle(conn: socket.socket, peer: tuple[int, int], reply: bytes) -> None:
    try:
        conn.settimeout(0.2)
        try:
            data = conn.recv(RECV_LIMIT)
            if data:
                log(f"peer cid={peer[0]} port={peer[1]} sent {len(data)}B")
        except socket.timeout:
            pass
        conn.sendall(reply)
        log(f"peer cid={peer[0]} port={peer[1]} replied {len(reply)}B")
    finally:
        conn.close()


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawTextHelpFormatter)
    ap.add_argument("--port", type=int, default=DEFAULT_PORT,
                    help=f"vsock port to listen on (default {DEFAULT_PORT})")
    args = ap.parse_args()

    reply = os.environb.get(b"QRAB_MOCK_REPLY", b"QRAB_MOCK_PONG v0\n")

    if not hasattr(socket, "AF_VSOCK"):
        log("socket.AF_VSOCK unavailable — Python >= 3.7 and host vsock support required")
        return 1

    try:
        srv = socket.socket(socket.AF_VSOCK, socket.SOCK_STREAM)
    except OSError as e:
        log(f"cannot create AF_VSOCK socket: {e}")
        return 1

    try:
        srv.bind((socket.VMADDR_CID_ANY, args.port))
    except OSError as e:
        log(f"bind(port={args.port}) failed: {e} — is vhost_vsock loaded?")
        return 1
    srv.listen(8)
    log(f"listening on vsock cid=ANY port={args.port}; reply={reply!r}")

    stop = threading.Event()

    def shutdown(signum: int, _frame: object) -> None:
        log(f"caught signal {signum}, shutting down")
        stop.set()
        try:
            # Kick accept() loose by closing the listener.
            srv.close()
        except OSError:
            pass

    signal.signal(signal.SIGINT, shutdown)
    signal.signal(signal.SIGTERM, shutdown)

    while not stop.is_set():
        try:
            conn, peer = srv.accept()
        except OSError:
            break
        # Handle inline — we do not expect concurrent connections from a
        # single qrab-boot instance and keeping it single-threaded makes
        # the smoke log deterministic.
        handle(conn, peer, reply)

    log("shutdown complete")
    return 0


if __name__ == "__main__":
    sys.exit(main())
