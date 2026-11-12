#!/usr/bin/env bash
# Stage a known OS image as a qrab base image (docs/dashboard.md, "Base images").
#
#   stage-base-image.sh [--catalog-only] NAME VERSION GENERATION
#   stage-base-image.sh cirros 0.6.3 1
#
# Downloads the release, verifies its upstream SHA-256, converts it to raw and
# copies it to /var/tmp/qrab-image-<name>-g<gen>.raw. It then prints the root
# command that imports and seals it as tank/qrab-iscsi/image-<name>-g<gen>@sealed,
# waits for the operator to run it, and records the image in ConfigMap
# qrab/qrab-base-images (key images.json, server-side apply). --catalog-only
# skips the download and import and records the catalog entry from the staged
# raw file. Needs curl, qemu-img, sha256sum, python3 and kubectl.
set -euo pipefail

usage() {
  sed -n '4,5p' "$0" >&2
  echo 'Known images: cirros 0.6.3' >&2
  exit 2
}

catalog_only=false
if [[ ${1:-} == --catalog-only ]]; then
  catalog_only=true
  shift
fi
[[ $# -eq 3 ]] || usage
name=$1 version=$2 generation=$3
[[ $generation =~ ^[1-9][0-9]{0,5}$ ]] || { echo 'GENERATION must be 1-999999' >&2; exit 2; }

# Known images: download URL, upstream SHA-256 of that file, its format,
# catalog title and OS. Add a row to stage another release.
case "$name/$version" in
  cirros/0.6.3)
    url=https://download.cirros-cloud.net/0.6.3/cirros-0.6.3-x86_64-disk.img
    upstream_sha256=7d6355852aeb6dbcd191bcda7cd74f1536cfe5cbf8a10495a7283a8396e4b75b
    format=qcow2
    title='CirrOS 0.6.3'
    os=cirros
    ;;
  *) echo "unknown image $name $version" >&2; usage ;;
esac

namespace=${QRAB_NAMESPACE:-qrab}
staged=/var/tmp/qrab-image-$name-g$generation.raw

if ! $catalog_only; then
  work=$(mktemp -d)
  trap 'rm -rf "$work"' EXIT
  chmod 0700 "$work"
  # The upstream URL redirects to GitHub; stay on HTTPS throughout.
  curl -fL --proto '=https' --proto-redir '=https' --retry 3 -o "$work/source.img" "$url"
  echo "$upstream_sha256  $work/source.img" | sha256sum --check --quiet - ||
    { echo "upstream SHA-256 mismatch for $url" >&2; exit 1; }
  qemu-img convert -f "$format" -O raw "$work/source.img" "$work/image.raw"
  virtual=$(qemu-img info -f "$format" --output=json "$work/source.img" |
    python3 -I -c 'import json,sys; print(json.load(sys.stdin)["virtual-size"])')
  [[ $(stat -c %s "$work/image.raw") == "$virtual" ]] ||
    { echo "raw size differs from the image's virtual size $virtual" >&2; exit 1; }
  # The root import re-checks this digest, so a file replaced in /var/tmp is refused.
  install -m 0644 "$work/image.raw" "$staged"
fi

[[ -f $staged ]] || { echo "$staged is missing; run without --catalog-only first" >&2; exit 1; }
size_bytes=$(stat -c %s "$staged")
raw_sha256=$(sha256sum "$staged" | cut -d' ' -f1)

if ! $catalog_only; then
  cat <<EOF

Staged $staged ($size_bytes bytes, raw SHA-256 $raw_sha256).
As root on the storage node, import and seal it:

  sudo /usr/local/sbin/qrab-iscsi-target vm-image-import --image $name --image-generation $generation --source $staged --sha256 $raw_sha256

EOF
  read -r -p 'Press Enter after the import succeeded: ' _
fi

# Merge by (name, generation) into the existing catalog, keep it sorted, and
# apply only this ConfigMap server-side.
existing=$(kubectl -n "$namespace" get configmap qrab-base-images --ignore-not-found -o json)
manifest=$(
  EXISTING=$existing NAMESPACE=$namespace NAME=$name GENERATION=$generation TITLE=$title \
  OS=$os VERSION=$version SIZE=$size_bytes SHA256=$raw_sha256 UPSTREAM=$upstream_sha256 URL=$url \
  python3 -I - <<'PY'
import json, os
e = os.environ
images = []
if e['EXISTING'].strip():
    raw = json.loads(e['EXISTING']).get('data', {}).get('images.json', '[]')
    images = json.loads(raw)
    if not isinstance(images, list):
        raise SystemExit('qrab-base-images/images.json is not a JSON array; fix it by hand first')
entry = {
    'name': e['NAME'], 'generation': int(e['GENERATION']), 'title': e['TITLE'], 'os': e['OS'],
    'version': e['VERSION'], 'size_bytes': int(e['SIZE']), 'sha256': e['SHA256'],
    'upstream_sha256': e['UPSTREAM'], 'source_url': e['URL'], 'boot': 'uefi',
}
images = [i for i in images
          if not (isinstance(i, dict) and (i.get('name'), i.get('generation')) == (entry['name'], entry['generation']))]
images.append(entry)
images.sort(key=lambda i: (str(i.get('name', '')), i.get('generation', 0)) if isinstance(i, dict) else ('', 0))
print(json.dumps({
    'apiVersion': 'v1', 'kind': 'ConfigMap',
    'metadata': {'name': 'qrab-base-images', 'namespace': e['NAMESPACE'],
                 'labels': {'app.kubernetes.io/part-of': 'qrab'}},
    'data': {'images.json': json.dumps(images, indent=1, sort_keys=True) + '\n'},
}))
PY
)
kubectl apply --server-side --field-manager=qrab-stage-base-image -f - <<<"$manifest"
echo "Recorded $title ($name g$generation) in $namespace/qrab-base-images."
