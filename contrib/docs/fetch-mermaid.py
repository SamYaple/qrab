#!/usr/bin/env python3
"""Fetch pinned rendering-only Mermaid assets, verifying the archive first."""
import argparse
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import tarfile
import urllib.request

VERSION = "12.3.0"
URL = f"https://registry.npmjs.org/mermaid/-/mermaid-{VERSION}.tgz"
SHA256 = "be0684e8d20bfc874df12c0f04927a5c482f57f2200eec54cca52a4583fd4040"

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("destination", type=Path)
args = parser.parse_args()
destination = args.destination.resolve()
destination.mkdir(parents=True, exist_ok=True)
with urllib.request.urlopen(URL, timeout=60) as response:
    archive = response.read(32 * 1024 * 1024 + 1)
if len(archive) > 32 * 1024 * 1024 or hashlib.sha256(archive).hexdigest() != SHA256:
    raise SystemExit("pinned Mermaid archive checksum/size mismatch")
with tarfile.open(fileobj=io.BytesIO(archive), mode="r:gz") as package:
    for entry in package.getmembers():
        name = PurePosixPath(entry.name)
        if not entry.isfile() or name.is_absolute() or ".." in name.parts:
            continue
        if not (entry.name.startswith("package/dist/") or entry.name in ["package/LICENSE", "package/package.json"]):
            continue
        output = (destination / entry.name).resolve()
        if not output.is_relative_to(destination):
            raise SystemExit("unsafe extraction path")
        output.parent.mkdir(parents=True, exist_ok=True)
        content = package.extractfile(entry)
        if content is None:
            raise SystemExit("missing archive content")
        output.write_bytes(content.read())
(destination / "provenance.json").write_text(json.dumps({"package": "mermaid", "version": VERSION, "tarball": URL, "sha256": SHA256}, indent=2) + "\n")
print(destination / "package/dist")
