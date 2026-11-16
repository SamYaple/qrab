#!/usr/bin/python3
"""Assemble a pinned, offline root kit; never creates keys or changes networking."""
import argparse
import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def canonical_tools(root, target):
    root = root.resolve()
    with target.open("wb") as output, gzip.GzipFile(
            fileobj=output, mode="wb", filename="", mtime=0) as compressed:
        with tarfile.open(fileobj=compressed, mode="w", format=tarfile.USTAR_FORMAT) as archive:
            for path in sorted(root.rglob("*")):
                resolved = path.resolve()
                if not resolved.is_relative_to(root):
                    raise RuntimeError("tool image contains an external symlink")
                if not resolved.is_file():
                    continue
                data = resolved.read_bytes()
                info = tarfile.TarInfo(str(path.relative_to(root)))
                info.mode = 0o755 if path.stat().st_mode & 0o111 else 0o644
                info.size = len(data)
                archive.addfile(info, io.BytesIO(data))


def export_image(pin, root):
    with tempfile.TemporaryDirectory(prefix="qrab-root-kit-auth-",
                                     dir=os.environ["XDG_RUNTIME_DIR"]) as private:
        env = dict(os.environ, REGISTRY_AUTH_FILE=str(Path(private) / "auth.json"))
        token = subprocess.Popen(["towl", "tokens", "mint", "--audience", "zot", "--raw"],
                                 stdout=subprocess.PIPE)
        try:
            subprocess.run(["podman", "login", "--username", "oauth2", "--password-stdin",
                            "zot.yapplesauce.com"], stdin=token.stdout, env=env,
                           stdout=subprocess.DEVNULL, check=True)
        finally:
            token.stdout.close()
        if token.wait():
            raise RuntimeError("registry exchange failed")
        subprocess.run(["podman", "pull", pin], env=env, check=True, stdout=subprocess.DEVNULL)
        container = subprocess.check_output(["podman", "create", pin, "/unused"],
                                             env=env, text=True).strip()
        try:
            subprocess.run(["podman", "cp", container + ":/.", str(root)], env=env, check=True)
        finally:
            subprocess.run(["podman", "rm", container], env=env, check=True,
                           stdout=subprocess.DEVNULL)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--tools-tree", type=Path, help="already exported immutable tool image")
    args = parser.parse_args()
    source = Path(__file__).resolve().parent
    release = json.loads((source / "deployment-release.json").read_text())
    if args.output.exists():
        raise RuntimeError("output already exists; choose a new immutable kit directory")
    with tempfile.TemporaryDirectory(prefix="qrab-root-kit-") as work:
        root = args.tools_tree
        if root is None:
            root = Path(work) / "tools"
            root.mkdir(mode=0o700)
            export_image(release["tools_reference"], root)
        packed = Path(work) / "tools.tar.gz"
        canonical_tools(root, packed)
        if sha(packed) != release["tools_archive_sha256"]:
            raise RuntimeError("exported tool tree does not match the pinned archive")
        args.output.mkdir(mode=0o755, parents=True)
        for name in ("deploy-root.sh", "deploy_root.py", "deployment-release.json"):
            shutil.copyfile(source / name, args.output / name)
        shutil.copyfile(packed, args.output / "tools.tar.gz")
        sums = []
        for path in sorted(args.output.iterdir()):
            sums.append(sha(path) + "  " + path.name)
            path.chmod(0o555 if path.suffix in (".sh", ".py") else 0o444)
        checksum = args.output / "SHA256SUMS"
        checksum.write_text("\n".join(sums) + "\n")
        checksum.chmod(0o444)
    print(json.dumps({"kit": str(args.output.resolve()),
                      "checksums_sha256": sha(checksum),
                      "deploy": f"sudo {args.output.resolve()}/deploy-root.sh deploy"}))


if __name__ == "__main__":
    main()
