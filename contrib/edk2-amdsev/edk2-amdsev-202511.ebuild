# Copyright 1999-2026 Gentoo Authors
# Distributed under the terms of the GNU General Public License v2

EAPI=8

PYTHON_REQ_USE="sqlite"
PYTHON_COMPAT=( python3_{12..14} )

inherit edo python-any-r1 toolchain-funcs

MY_PN="edk2"

DESCRIPTION="TianoCore EDK II OVMF firmware for AMD SEV-SNP confidential VMs"
HOMEPAGE="https://github.com/tianocore/edk2"

BUNDLED_BROTLI_SUBMODULE_SHA="e230f474b87134e8c6c85b630084c612057f253e"
BUNDLED_LIBSPDM_SUBMODULE_SHA="98ef964e1e9a0c39c7efb67143d3a13a819432e0"
BUNDLED_MBEDTLS_SUBMODULE_SHA="8c89224991adff88d53cd380f42a2baa36f91454"
BUNDLED_MIPI_SYS_T_SUBMODULE_SHA="370b5944c046bab043dd8b133727b2135af7747a"
BUNDLED_OPENSSL_SUBMODULE_P="openssl-3.5.1"

SRC_URI="
	https://github.com/tianocore/${MY_PN}/archive/${MY_PN}-stable202511.tar.gz
		-> ${MY_PN}-202511.tar.gz
	https://github.com/google/brotli/archive/${BUNDLED_BROTLI_SUBMODULE_SHA}.tar.gz
		-> brotli-${BUNDLED_BROTLI_SUBMODULE_SHA}.tar.gz
	https://github.com/DMTF/libspdm/archive/${BUNDLED_LIBSPDM_SUBMODULE_SHA}.tar.gz
		-> libspdm-${BUNDLED_LIBSPDM_SUBMODULE_SHA}.tar.gz
	https://github.com/Mbed-TLS/mbedtls/archive/${BUNDLED_MBEDTLS_SUBMODULE_SHA}.tar.gz
		-> mbedtls-${BUNDLED_MBEDTLS_SUBMODULE_SHA}.tar.gz
	https://github.com/MIPI-Alliance/public-mipi-sys-t/archive/${BUNDLED_MIPI_SYS_T_SUBMODULE_SHA}.tar.gz
		-> mipi-sys-t-${BUNDLED_MIPI_SYS_T_SUBMODULE_SHA}.tar.gz
	https://github.com/openssl/openssl/releases/download/${BUNDLED_OPENSSL_SUBMODULE_P}/${BUNDLED_OPENSSL_SUBMODULE_P}.tar.gz
"

S="${WORKDIR}/${MY_PN}-${MY_PN}-stable202511"
LICENSE="BSD-2-with-patent MIT"
SLOT="0"
KEYWORDS="-* amd64"

BDEPEND="
	${PYTHON_DEPS}
	>=sys-power/iasl-20160729
	>=dev-lang/nasm-2.0.7
"

PATCHES=(
	"${FILESDIR}/${MY_PN}-202511-werror.patch"
	"${FILESDIR}/${MY_PN}-202502-nasm-3.patch"
	"${FILESDIR}/${MY_PN}-202505-UninstallMemAttrProtocol.patch"
)

DIR="/usr/share/${MY_PN}/AmdSev"

link_mod() {
	rmdir "$2" && ln -sfT "$1" "$2" || die "linking ${2##*/} failed"
}

src_prepare() {
	# The AmdSev DSC has a prebuild script (grub.sh) that builds an
	# embedded GRUB EFI for LUKS secret injection. If grub-mkimage is
	# not available, create a dummy grub.efi so the prebuild exits
	# cleanly. The dummy is harmless — GRUB is only needed for
	# encrypted-disk SEV guests.
	if ! type grub-mkimage &>/dev/null; then
		touch OvmfPkg/AmdSev/Grub/grub.efi || die
		einfo "Created dummy grub.efi (grub-mkimage not available)"
	fi

	link_mod "${WORKDIR}/brotli-${BUNDLED_BROTLI_SUBMODULE_SHA}" \
		BaseTools/Source/C/BrotliCompress/brotli
	link_mod "${WORKDIR}/brotli-${BUNDLED_BROTLI_SUBMODULE_SHA}" \
		MdeModulePkg/Library/BrotliCustomDecompressLib/brotli
	link_mod "${WORKDIR}/libspdm-${BUNDLED_LIBSPDM_SUBMODULE_SHA}" \
		SecurityPkg/DeviceSecurity/SpdmLib/libspdm
	link_mod "${WORKDIR}/mbedtls-${BUNDLED_MBEDTLS_SUBMODULE_SHA}" \
		CryptoPkg/Library/MbedTlsLib/mbedtls
	link_mod "${WORKDIR}/public-mipi-sys-t-${BUNDLED_MIPI_SYS_T_SUBMODULE_SHA}" \
		MdePkg/Library/MipiSysTLib/mipisyst
	link_mod "${WORKDIR}/${BUNDLED_OPENSSL_SUBMODULE_P}" \
		CryptoPkg/Library/OpensslLib/openssl

	default
}

src_compile() {
	local TOOLCHAIN="GCC5"
	local BUILD_TARGET="RELEASE"

	tc-export_build_env
	emake -C BaseTools \
		CC="$(tc-getBUILD_CC)" \
		CXX="$(tc-getBUILD_CXX)" \
		EXTRA_OPTFLAGS="${BUILD_CFLAGS}" \
		EXTRA_LDFLAGS="${BUILD_LDFLAGS}"

	export \
		"${TOOLCHAIN}_X64_PREFIX=${CHOST}-" \
		"${TOOLCHAIN}_BIN=${CHOST}-"

	. ./edksetup.sh

	# Build AmdSev variant — no SMM, no SecureBoot, SNP support built-in.
	# This produces a single OVMF.fd with the SEV metadata GUID table
	# that QEMU needs for SNP_LAUNCH_UPDATE.
	edo build \
		-t "${TOOLCHAIN}" \
		-b "${BUILD_TARGET}" \
		-a X64 \
		-p OvmfPkg/AmdSev/AmdSevX64.dsc \
		-D NETWORK_HTTP_BOOT_ENABLE \
		-D NETWORK_IP6_ENABLE \
		-D NETWORK_TLS_ENABLE \
		-D TPM2_ENABLE \
		-D TPM2_CONFIG_ENABLE
}

src_install() {
	insinto "${DIR}"
	newins "Build/AmdSev/RELEASE_GCC5/FV/OVMF.fd" OVMF.fd
}

pkg_postinst() {
	elog "AMD SEV-SNP OVMF firmware installed to ${DIR}/OVMF.fd"
	elog ""
	elog "Usage with QEMU:"
	elog "  qemu-system-x86_64 \\"
	elog "    -machine q35,confidential-guest-support=sev0,memory-backend=mem0 \\"
	elog "    -bios ${DIR}/OVMF.fd \\"
	elog "    -cpu EPYC-v4 \\"
	elog "    -object sev-snp-guest,id=sev0,cbitpos=51,reduced-phys-bits=1 \\"
	elog "    -object memory-backend-memfd,id=mem0,size=256M,share=on \\"
	elog "    ..."
}
