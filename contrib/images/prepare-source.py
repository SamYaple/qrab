#!/usr/bin/env python3
"""Export the locked local QEMU checkout plus audited build-dependency snapshots."""
import gzip
import hashlib
import io
import json
from pathlib import Path
import shutil
import subprocess
import tarfile

ROOT = Path(__file__).resolve().parents[2]
manifest = json.loads((ROOT / 'contrib/images/qemu-source.json').read_text())
repo = ROOT / 'vendor/qemu'
remote = subprocess.check_output(['git', '-C', str(repo), 'remote', 'get-url', 'origin'], text=True).strip()
if remote != manifest['qemu']['remote']:
    raise SystemExit('QEMU origin must be the locked local Gitea mirror')
revision = subprocess.check_output(['git', '-C', str(repo), 'rev-parse', 'HEAD'], text=True).strip()
if revision != manifest['qemu']['revision']:
    raise SystemExit('QEMU gitlink differs from image source lock')
context = ROOT / 'target/images/context'
source = context / 'qemu-source'
if source.exists():
    shutil.rmtree(source)
source.mkdir(parents=True)
archive = subprocess.check_output(['git', '-C', str(repo), 'archive', revision])
with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
    tar.extractall(source, filter='data')
for dependency in manifest['subprojects']:
    path = ROOT / dependency['archive']
    if hashlib.sha256(path.read_bytes()).hexdigest() != dependency['sha256']:
        raise SystemExit('Bad dependency archive: ' + str(path))
    wrap = (source / 'subprojects' / (dependency['name'] + '.wrap')).read_text()
    if 'revision = ' + dependency['revision'] not in wrap:
        raise SystemExit('Dependency revision differs from QEMU wrap: ' + dependency['name'])
    destination = source / 'subprojects' / dependency['name']
    destination.mkdir()
    with tarfile.open(fileobj=io.BytesIO(gzip.decompress(path.read_bytes()))) as tar:
        tar.extractall(destination, filter='data')
    patches = source / 'subprojects/packagefiles' / dependency['name']
    if patches.is_dir():
        shutil.copytree(patches, destination, dirs_exist_ok=True)
# qrab's own fixes on top of the locked revision, each pinned by digest.
for patch in manifest.get('patches', []):
    path = ROOT / patch['file']
    if hashlib.sha256(path.read_bytes()).hexdigest() != patch['sha256']:
        raise SystemExit('Bad QEMU patch digest: ' + str(path))
    # Not `git apply`: inside this repository it skips paths outside the cwd.
    subprocess.run(['patch', '-p1', '--forward', '--batch', '--silent', '-i', str(path)], cwd=source, check=True)
shutil.copyfile(ROOT / 'contrib/images/apt-https.sh', context / 'apt-https.sh')
(context / 'qemu-source.json').write_text(json.dumps(manifest, indent=2) + '\n')
print(context)
