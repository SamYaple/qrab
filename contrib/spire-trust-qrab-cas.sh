#!/usr/bin/env bash
# Make the native SPIRE servers on kube01-03 trust the qrab guest vTPM CAs.
#
# Usage: contrib/spire-trust-qrab-cas.sh <ek-ca sha256> <devid-ca sha256>
#   The fingerprints are the ones contrib/qrab-tpm-ca-create.sh printed. The
#   certificates come from ConfigMap qrab-tpm-issuer/qrab-tpm-issuer-ca-certs
#   and must match them, so whoever can write that ConfigMap still cannot
#   choose what SPIRE trusts.
#
# Per node, one at a time and only while the other servers are healthy, it:
#   - appends ek-ca.pem to /etc/spire/ca/endorsement-ca.pem and devid-ca.pem
#     to /etc/spire/ca/devid-ca.pem, keeping the node CAs. It backs up each
#     replaced file as *.before-qrab-<UTC>.
#   - validates server.conf, restarts spire-server and waits for its health.
# A node whose bundles already hold both certificates is left alone, so a
# re-run changes nothing. Afterwards it checks that the registration-entry
# and agent counts are what they were before.
set -euo pipefail

want_ek=${1:?ek-ca sha256}; want_devid=${2:?devid-ca sha256}
nodes=(kube01 kube02 kube03)
server=/opt/spire/bin/spire-server
sock=/run/spire-server/private/api.sock
stamp=$(date -u +%Y%m%dT%H%M%SZ)

local_dir=$(mktemp -d)
trap 'rm -rf "$local_dir"' EXIT
kubectl -n qrab-tpm-issuer get configmap qrab-tpm-issuer-ca-certs -o json | python3 -c '
import json,sys
data=json.load(sys.stdin)["data"]
for name in ("ek-ca.pem","devid-ca.pem"):
    open(sys.argv[1]+"/"+name,"w").write(data[name])
' "$local_dir"

check_cert() { # file expected-sha256
  ! grep -q "PRIVATE KEY" "$1" || { echo "$1 contains a private key" >&2; exit 1; }
  [ "$(grep -c 'BEGIN CERTIFICATE' "$1")" = 1 ] || { echo "$1 must hold exactly one certificate" >&2; exit 1; }
  openssl x509 -in "$1" -noout -ext basicConstraints | grep -q "CA:TRUE" || { echo "$1 is not a CA" >&2; exit 1; }
  have=$(openssl x509 -in "$1" -noout -fingerprint -sha256 | sed 's/.*=//; s/://g' | tr A-F a-f)
  [ "$have" = "$2" ] || { echo "$1 fingerprint $have != expected $2" >&2; exit 1; }
}
check_cert "$local_dir/ek-ca.pem" "$want_ek"
check_cert "$local_dir/devid-ca.pem" "$want_devid"

healthy() { ssh "$1" sudo -n "$server" healthcheck -socketPath "$sock" >/dev/null 2>&1; }
counts() {
  ssh kube01 sudo -n "$server" entry count -socketPath "$sock"
  ssh kube01 sudo -n "$server" agent count -socketPath "$sock"
}

for n in "${nodes[@]}"; do healthy "$n" || { echo "$n spire-server unhealthy before start" >&2; exit 1; }; done
before=$(counts)
echo "before: $(echo "$before" | tr '\n' ' ')"

for n in "${nodes[@]}"; do
  for o in "${nodes[@]}"; do
    [ "$o" = "$n" ] || healthy "$o" || { echo "$o unhealthy; not touching $n" >&2; exit 1; }
  done
  stage=$(ssh "$n" mktemp -d)
  scp -q "$local_dir/ek-ca.pem" "$local_dir/devid-ca.pem" "$n:$stage/"
  ssh "$n" sudo -n bash -s -- "$stage" "$stamp" "$server" "$sock" <<'REMOTE'
set -euo pipefail
stage=$1 stamp=$2 server=$3 sock=$4
fingerprints() { # bundle -> one sha256 per certificate
  python3 - "$1" <<'PY'
import subprocess,sys
pem=open(sys.argv[1]).read()
for block in pem.split('-----END CERTIFICATE-----')[:-1]:
    cert=block[block.index('-----BEGIN CERTIFICATE-----'):]+'-----END CERTIFICATE-----\n'
    out=subprocess.run(['openssl','x509','-noout','-fingerprint','-sha256'],input=cert.encode(),capture_output=True,check=True).stdout.decode()
    print(out.strip().split('=')[1].replace(':','').lower())
PY
}
changed=0
for pair in ek-ca.pem:endorsement-ca.pem devid-ca.pem:devid-ca.pem; do
  src=$stage/${pair%%:*} dst=/etc/spire/ca/${pair##*:}
  fp=$(fingerprints "$src")
  trusted=$(fingerprints "$dst")
  if grep -qx "$fp" <<<"$trusted"; then
    echo "$(hostname): ${dst##*/} already trusts $fp"
    continue
  fi
  cp -a "$dst" "$dst.before-qrab-$stamp"
  { cat "$dst.before-qrab-$stamp"; [ -z "$(tail -c1 "$dst.before-qrab-$stamp")" ] || echo; cat "$src"; } >"$dst.new"
  chmod --reference="$dst" "$dst.new"; chown --reference="$dst" "$dst.new"
  mv "$dst.new" "$dst"
  echo "$(hostname): ${dst##*/} now trusts $fp ($(grep -c 'BEGIN CERTIFICATE' "$dst") certificates)"
  changed=1
done
rm -rf "$stage"
"$server" validate -config /etc/spire/server.conf
if [ "$changed" = 1 ]; then
  systemctl restart spire-server
  for _ in $(seq 1 30); do "$server" healthcheck -socketPath "$sock" >/dev/null 2>&1 && break; sleep 2; done
  "$server" healthcheck -socketPath "$sock"
fi
systemctl is-active spire-server spire-agent
REMOTE
done

after=$(counts)
echo "after: $(echo "$after" | tr '\n' ' ')"
[ "$before" = "$after" ] || { echo "entry or agent count changed across the rollout" >&2; exit 1; }
