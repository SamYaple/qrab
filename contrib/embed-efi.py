#!/usr/bin/env python3
"""Embed qrab-boot.efi into a copy of the edk2 AmdSev OVMF tree.

The AmdSev OVMF firmware volume already contains a `UEFI_APPLICATION`
module that carries the platform-specific OS loader. Upstream ships it
as `OvmfPkg/AmdSev/Grub/Grub.inf`, which picks up a `grub.efi` binary
produced at build time by the `PREBUILD = sh OvmfPkg/AmdSev/Grub/grub.sh`
hook in `AmdSevX64.dsc`. `OvmfPkg/Library/PlatformBootManagerLib`
launches that application by its `FILE_GUID` (`gGrubFileGuid`).

We reuse that launch path without patching BDS: we substitute our own
PE32+ EFI binary for `grub.efi` and disable the prebuild hook so the
upstream script does not clobber or delete it.

Inputs  :
    --edk2        path to the *build copy* of the edk2 tree (must NOT be
                  the vendored submodule — we never mutate `vendor/edk2`).
    --efi         path to the built qrab-boot.efi.

Outputs :
    <edk2>/OvmfPkg/AmdSev/Grub/grub.efi          (overwritten)
    <edk2>/OvmfPkg/AmdSev/AmdSevX64.dsc          (PREBUILD neutralised)

Exits non-zero on any anomaly. This script is deliberately strict — a
silent fallback to upstream GRUB would defeat the entire point of
qrab-boot.
"""

from __future__ import annotations

import argparse
import pathlib
import shutil
import sys
import textwrap

PREBUILD_NEEDLE = "PREBUILD                       = sh OvmfPkg/AmdSev/Grub/grub.sh"
PREBUILD_REPLACEMENT = (
    "# PREBUILD neutralised by contrib/embed-efi.py — qrab-boot.efi has\n"
    "  # already been staged as OvmfPkg/AmdSev/Grub/grub.efi.\n"
    "  PREBUILD                       = true"
)


def die(msg: str) -> None:
    sys.stderr.write(f"embed-efi.py: error: {msg}\n")
    sys.exit(1)


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    ap.add_argument("--edk2", required=True, type=pathlib.Path, help="build copy of edk2 tree")
    ap.add_argument("--efi", required=True, type=pathlib.Path, help="path to qrab-boot.efi")
    args = ap.parse_args()

    if not args.edk2.is_dir():
        die(f"--edk2 {args.edk2} is not a directory")
    if not args.efi.is_file():
        die(f"--efi {args.efi} does not exist")

    guard = args.edk2.resolve()
    for part in guard.parts:
        if part == "vendor":
            die(
                textwrap.dedent(
                    f"""refusing to mutate {guard}: path contains `vendor/`.
                    This script must run against a build-time copy of edk2,
                    never the vendored submodule. See contrib/build-ovmf.sh."""
                )
            )

    grub_dir = args.edk2 / "OvmfPkg" / "AmdSev" / "Grub"
    if not grub_dir.is_dir():
        die(f"{grub_dir} missing — is --edk2 really an edk2 tree?")

    dst = grub_dir / "grub.efi"
    shutil.copyfile(args.efi, dst)
    print(f"embed-efi.py: {args.efi} -> {dst}")

    dsc = args.edk2 / "OvmfPkg" / "AmdSev" / "AmdSevX64.dsc"
    text = dsc.read_text()
    if PREBUILD_NEEDLE not in text:
        # Either already patched (idempotent re-run) or upstream shape
        # changed. Bail loudly rather than mis-embed.
        if "PREBUILD neutralised by contrib/embed-efi.py" in text:
            print("embed-efi.py: AmdSevX64.dsc already neutralised, nothing to do")
        else:
            die(
                "AmdSevX64.dsc: expected PREBUILD line not found; "
                "upstream edk2 layout may have changed"
            )
    else:
        dsc.write_text(text.replace(PREBUILD_NEEDLE, PREBUILD_REPLACEMENT, 1))
        print(f"embed-efi.py: neutralised PREBUILD in {dsc}")


if __name__ == "__main__":
    main()
