#!/usr/bin/env bash
# Create the qrab endorsement and DevID CAs for guest vTPM identity, once.
#
# Usage: contrib/qrab-tpm-ca-create.sh
#
# The private keys are generated in a private tmpfs directory, go straight
# into Secret qrab-tpm-issuer/qrab-tpm-issuer-ca (read only by the issuer's
# ServiceAccount), and are shredded. They are never written to disk or git.
# The public certificates also go into ConfigMap qrab-tpm-issuer-ca-certs,
# which contrib/spire-trust-qrab-cas.sh reads; the script prints their
# SHA-256 fingerprints, which that script requires as arguments.
#
# Refuses to run if the Secret already exists: rotating a CA invalidates
# every guest's DevID and is a separate, deliberate procedure.
set -euo pipefail

ns=qrab-tpm-issuer
secret=qrab-tpm-issuer-ca
certs=qrab-tpm-issuer-ca-certs
days=3650

if kubectl -n "$ns" get secret "$secret" >/dev/null 2>&1; then
  echo "secret $ns/$secret already exists; refusing to replace a CA" >&2
  exit 1
fi

work=$(mktemp -d /dev/shm/qrab-tpm-ca.XXXXXX)
chmod 0700 "$work"
trap 'find "$work" -type f -exec shred -u {} +; rm -rf "$work"' EXIT
umask 077

ca() { # name cn
  openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out "$work/$1.key"
  openssl req -new -x509 -key "$work/$1.key" -sha256 -days "$days" \
    -subj "/O=qrab/CN=$2" \
    -addext "basicConstraints=critical,CA:TRUE,pathlen:0" \
    -addext "keyUsage=critical,keyCertSign,cRLSign" \
    -addext "subjectKeyIdentifier=hash" \
    -out "$work/$1.pem"
}
ca ek-ca qrab-endorsement-ca-1
ca devid-ca qrab-devid-ca-1

kubectl get namespace "$ns" >/dev/null 2>&1 || kubectl apply -f - <<EOF
apiVersion: v1
kind: Namespace
metadata:
  name: $ns
  labels:
    pod-security.kubernetes.io/enforce: restricted
EOF

kubectl -n "$ns" create secret generic "$secret" \
  --from-file=ek-ca.pem="$work/ek-ca.pem" --from-file=ek-ca.key="$work/ek-ca.key" \
  --from-file=devid-ca.pem="$work/devid-ca.pem" --from-file=devid-ca.key="$work/devid-ca.key" \
  >/dev/null
kubectl -n "$ns" create configmap "$certs" \
  --from-file=ek-ca.pem="$work/ek-ca.pem" --from-file=devid-ca.pem="$work/devid-ca.pem" \
  >/dev/null

for f in ek-ca devid-ca; do
  printf '%s %s %s\n' "$f" \
    "$(openssl x509 -in "$work/$f.pem" -noout -subject -nameopt RFC2253 | sed 's/^subject=//')" \
    "$(openssl x509 -in "$work/$f.pem" -noout -fingerprint -sha256 | sed 's/.*=//; s/://g' | tr A-F a-f)"
done
