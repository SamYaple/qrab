#!/usr/bin/env python3
"""Loopback-only TLS termination for the disposable Redfish router fixture.

This transports HTTP unchanged; it does not inject, translate or bypass auth.
The throwaway CA private key stays in memory. The leaf key is written 0600
under the ignored output directory so Python's SSLContext can load it.
"""

import argparse
from datetime import datetime, timedelta, timezone
from http.client import HTTPConnection
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import ipaddress
import os
from pathlib import Path
import ssl

from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import ec
from cryptography.x509.oid import NameOID


HOP_HEADERS = {"connection", "keep-alive", "proxy-authenticate", "proxy-authorization",
               "te", "trailer", "transfer-encoding", "upgrade"}


def certificate_files(directory):
    directory.mkdir(mode=0o700, parents=True, exist_ok=False)
    current = datetime.now(timezone.utc)
    ca_key = ec.generate_private_key(ec.SECP256R1())
    ca_name = x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, "qrab disposable interop CA")])
    ca = (x509.CertificateBuilder().subject_name(ca_name).issuer_name(ca_name)
          .public_key(ca_key.public_key()).serial_number(x509.random_serial_number())
          .not_valid_before(current - timedelta(minutes=1)).not_valid_after(current + timedelta(hours=2))
          .add_extension(x509.BasicConstraints(ca=True, path_length=0), critical=True)
          .add_extension(x509.SubjectKeyIdentifier.from_public_key(ca_key.public_key()), critical=False)
          .add_extension(x509.AuthorityKeyIdentifier.from_issuer_public_key(ca_key.public_key()), critical=False)
          .add_extension(x509.KeyUsage(digital_signature=False, content_commitment=False,
                         key_encipherment=False, data_encipherment=False, key_agreement=False,
                         key_cert_sign=True, crl_sign=True, encipher_only=None, decipher_only=None), critical=True)
          .sign(ca_key, hashes.SHA256()))
    leaf_key = ec.generate_private_key(ec.SECP256R1())
    leaf_name = x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, "localhost")])
    leaf = (x509.CertificateBuilder().subject_name(leaf_name).issuer_name(ca_name)
            .public_key(leaf_key.public_key()).serial_number(x509.random_serial_number())
            .not_valid_before(current - timedelta(minutes=1)).not_valid_after(current + timedelta(hours=2))
            .add_extension(x509.BasicConstraints(ca=False, path_length=None), critical=True)
            .add_extension(x509.SubjectKeyIdentifier.from_public_key(leaf_key.public_key()), critical=False)
            .add_extension(x509.AuthorityKeyIdentifier.from_issuer_public_key(ca_key.public_key()), critical=False)
            .add_extension(x509.KeyUsage(digital_signature=True, content_commitment=False,
                           key_encipherment=False, data_encipherment=False, key_agreement=False,
                           key_cert_sign=False, crl_sign=False, encipher_only=None, decipher_only=None), critical=True)
            .add_extension(x509.SubjectAlternativeName([
                x509.DNSName("localhost"), x509.IPAddress(ipaddress.ip_address("127.0.0.1"))]), critical=False)
            .add_extension(x509.ExtendedKeyUsage([x509.oid.ExtendedKeyUsageOID.SERVER_AUTH]), critical=False)
            .sign(ca_key, hashes.SHA256()))
    for name, contents in (
        ("ca.pem", ca.public_bytes(serialization.Encoding.PEM)),
        ("server.pem", leaf.public_bytes(serialization.Encoding.PEM)),
        ("server.key", leaf_key.private_bytes(serialization.Encoding.PEM,
         serialization.PrivateFormat.PKCS8, serialization.NoEncryption())),
    ):
        descriptor = os.open(directory / name, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
        with os.fdopen(descriptor, "wb") as output:
            output.write(contents)


class Relay(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def __getattr__(self, name):
        if name.startswith("do_"):
            return self.relay  # Let Rust handle even deliberately unknown verbs.
        raise AttributeError(name)

    def log_message(self, *_):
        pass  # Credentials and response bodies never enter logs.

    def relay(self):
        upstream = HTTPConnection("127.0.0.1", self.server.upstream_port, timeout=10)
        try:
            size = int(self.headers.get("Content-Length", "0"))
            if size < 0 or size > 1048576 or "Transfer-Encoding" in self.headers:
                self.send_error(413)
                return
            body = self.rfile.read(size) if size else None
            upstream.putrequest(self.command, self.path, skip_host=True, skip_accept_encoding=True)
            for name, value in self.headers.items():
                if name.lower() not in HOP_HEADERS:
                    upstream.putheader(name, value)
            upstream.putheader("Connection", "close")
            upstream.endheaders(body)
            response = upstream.getresponse()
            # Fixture resources are finite; never exercise SSE/downloads here.
            payload = response.read(1048577)
            if len(payload) > 1048576:
                self.send_error(502)
                return
            self.send_response_only(response.status)
            for name, value in response.getheaders():
                if name.lower() not in HOP_HEADERS | {"content-length"}:
                    self.send_header(name, value)
            length = response.getheader("Content-Length") if self.command == "HEAD" else str(len(payload))
            self.send_header("Content-Length", length or "0")
            self.send_header("Connection", "close")
            self.end_headers()
            if self.command != "HEAD":
                self.wfile.write(payload)
        except (OSError, ValueError):
            self.send_error(502)
        finally:
            upstream.close()
            self.close_connection = True

    do_GET = do_HEAD = do_POST = do_PATCH = do_DELETE = do_PUT = do_OPTIONS = do_TRACE = relay


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tls-directory", type=Path, required=True)
    parser.add_argument("--port", type=int, default=18882)
    parser.add_argument("--upstream-port", type=int, default=18881)
    args = parser.parse_args()
    certificate_files(args.tls_directory)
    server = ThreadingHTTPServer(("127.0.0.1", args.port), Relay)
    server.daemon_threads = True
    server.upstream_port = args.upstream_port
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.minimum_version = ssl.TLSVersion.TLSv1_2
    context.load_cert_chain(args.tls_directory / "server.pem", args.tls_directory / "server.key")
    server.socket = context.wrap_socket(server.socket, server_side=True)
    print(f"Disposable TLS fixture: https://127.0.0.1:{args.port}; trust {args.tls_directory / 'ca.pem'}", flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
