#!/bin/sh
# CA trust is copied from the immutable Rust builder before the first fetch.
# Keep APT signatures and certificate checks; refuse redirect downgrades.
set -eu
sources_found=0
for source_file in /etc/apt/sources.list /etc/apt/sources.list.d/*.list /etc/apt/sources.list.d/*.sources; do
    [ -f "$source_file" ] || continue
    sources_found=1
    sed -i 's|http://|https://|g' "$source_file"
    if grep -Eq '(http|ftp|mirror)://' "$source_file"; then
        echo 'Non-HTTPS package source remains' >&2
        exit 1
    fi
done
[ "$sources_found" = 1 ] || { echo 'No package sources found' >&2; exit 1; }
[ -s /etc/ssl/certs/ca-certificates.crt ] || { echo 'Pinned bootstrap CA store missing' >&2; exit 1; }
cat > /etc/apt/apt.conf.d/99qrab-https <<'EOF'
Acquire::https::CaInfo "/etc/ssl/certs/ca-certificates.crt";
Acquire::https::Verify-Peer "true";
Acquire::https::Verify-Host "true";
Acquire::https::AllowRedirect "false";
Acquire::http::AllowRedirect "false";
EOF
