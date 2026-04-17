#!/bin/bash
# Install edk2-amdsev ebuild into local portage repo and emerge it.
# Must be run as root (or with sudo).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
LOCAL_REPO="/var/db/repos/local"
PKG_DIR="${LOCAL_REPO}/sys-firmware/edk2-amdsev"
EDK2_PATCHES_DIR="/var/db/repos/gentoo/sys-firmware/edk2/files"

# Ensure local repo exists with proper structure
#if [[ ! -d "${LOCAL_REPO}" ]]; then
#    echo "Creating local portage repo at ${LOCAL_REPO}..."
#    mkdir -p "${LOCAL_REPO}"/{metadata,profiles}
#    echo "local" > "${LOCAL_REPO}/profiles/repo_name"
#    cat > "${LOCAL_REPO}/metadata/layout.conf" <<'EOF'
#masters = gentoo
#auto-sync = false
#thin-manifests = true
#EOF
#    echo "Local repo created."
#fi

# Create package directory
echo "Installing ebuild to ${PKG_DIR}..."
mkdir -p "${PKG_DIR}"

# Copy ebuild
cp "${SCRIPT_DIR}"/edk2-amdsev-*.ebuild "${PKG_DIR}/"

# Symlink patches from the main edk2 package (same source, same patches)
if [[ -d "${EDK2_PATCHES_DIR}" ]]; then
    ln -sfn "${EDK2_PATCHES_DIR}" "${PKG_DIR}/files"
    echo "Linked patches from ${EDK2_PATCHES_DIR}"
else
    echo "WARNING: edk2 patches dir not found at ${EDK2_PATCHES_DIR}"
    echo "You may need to create ${PKG_DIR}/files/ with the required patches."
    mkdir -p "${PKG_DIR}/files"
fi

# Generate manifest
echo "Generating Manifest..."
pushd "${PKG_DIR}" > /dev/null
ebuild edk2-amdsev-202511.ebuild manifest
popd > /dev/null

echo ""
echo "Ebuild installed. Now run:"
echo ""
echo "  sudo emerge -av sys-firmware/edk2-amdsev"
echo ""
echo "After install, firmware will be at:"
echo "  /usr/share/edk2/AmdSev/OVMF.fd"
