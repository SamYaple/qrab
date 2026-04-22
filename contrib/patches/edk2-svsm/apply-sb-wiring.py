#!/usr/bin/env python3
"""Apply UEFI Secure Boot wiring patches to a staged coconut-svsm/edk2 tree.

The AmdSev OVMF variant ships with Secure Boot stubbed out (AuthVariableLibNull,
no SecurityPkg components). This script adds the SB wiring upstream already
provides for OvmfPkgX64.dsc so builds invoked with `-D SECURE_BOOT_ENABLE=TRUE`
produce an SB-enforcing firmware.

Idempotent: running twice is a no-op. All edits verified post-write by grep.

Usage:
    apply-sb-wiring.py <path-to-staged-edk2>

Exits 0 on success, 1 on anchor mismatch or verification failure.
"""
from __future__ import annotations
import argparse
import subprocess
import sys
from pathlib import Path

MARKER = "# QRAB_SVSM_SB_WIRING"
HERE = Path(__file__).resolve().parent


def patch_file(path: Path, edits: list[tuple[str, str, str]]) -> bool:
    """Apply `edits` to `path`. Each edit is (marker_key, anchor, block).
    If `marker_key` already present in file, skip. Otherwise split on first
    occurrence of `anchor` and insert `block` after it.

    Returns True on any change, False if no-op (already patched).
    """
    text = path.read_text()
    changed = False
    for marker_key, anchor, block in edits:
        tag = f"{MARKER} {marker_key}"
        if tag in text:
            continue
        if anchor not in text:
            raise SystemExit(f"{path}: anchor not found: {anchor!r}")
        # Append tag inside block so re-runs detect it.
        block_with_tag = block.rstrip() + f"  {tag}\n"
        # Insert after the *first* occurrence of the anchor LINE (anchor
        # must match a whole line; we operate line-wise).
        lines = text.splitlines(keepends=True)
        for i, line in enumerate(lines):
            if line.rstrip("\n") == anchor.rstrip("\n"):
                lines.insert(i + 1, block_with_tag)
                break
        else:
            raise SystemExit(f"{path}: anchor line not matched: {anchor!r}")
        text = "".join(lines)
        changed = True
    if changed:
        path.write_text(text)
    return changed


def replace_line(path: Path, marker_key: str, old_line: str, new_block: str) -> bool:
    """Replace `old_line` (exact match) with `new_block` in `path`. Idempotent
    via `marker_key`. Returns True on change.
    """
    text = path.read_text()
    tag = f"{MARKER} {marker_key}"
    if tag in text:
        return False
    if old_line not in text:
        raise SystemExit(f"{path}: line not found for replacement: {old_line!r}")
    replacement = new_block.rstrip() + f"  {tag}\n"
    text = text.replace(old_line, replacement, 1)
    path.write_text(text)
    return True


def bake_keys(edk2: Path, keys_dir: Path) -> None:
    """Regenerate AuthData.c from qrab PEM certs in `keys_dir`. The produced
    C file uses the same symbol names as the upstream (mMicrosoftKek /
    mMicrosoftPca / mMicrosoftUefiCa) so EnrollDefaultKeys.c does not need
    patching. Idempotent: subsequent runs with the same keys produce the
    same output (modulo a timestamp comment).
    """
    auth_c = edk2 / "OvmfPkg" / "EnrollDefaultKeys" / "AuthData.c"
    if not auth_c.is_file():
        raise SystemExit(f"missing: {auth_c}")
    converter = HERE / "certs-to-c.py"
    if not converter.is_file():
        raise SystemExit(f"missing converter: {converter}")

    result = subprocess.run(
        [sys.executable, str(converter), str(keys_dir), str(auth_c)],
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        sys.stderr.write(result.stderr.decode())
        raise SystemExit("certs-to-c.py failed")

    # Post-condition: the file must still define the three upstream symbols
    # (so EnrollDefaultKeys.c links) and now reference qrab (not Microsoft).
    text = auth_c.read_text()
    for sym in ("mMicrosoftKek", "mMicrosoftPca", "mMicrosoftUefiCa",
                "mSha256OfDevNull"):
        if f"CONST UINT8  {sym}[]" not in text:
            raise SystemExit(f"bake-keys: symbol {sym} missing in {auth_c}")
    if "qrab" not in text.lower():
        raise SystemExit(f"bake-keys: 'qrab' marker absent in {auth_c}")
    if "Microsoft Corporation KEK CA 2011" in text:
        raise SystemExit(f"bake-keys: MS KEK header still present in {auth_c}")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("edk2_root", help="path to staged edk2 tree")
    ap.add_argument(
        "--bake-keys",
        metavar="KEYS_DIR",
        type=Path,
        default=None,
        help="replace AuthData.c Microsoft certs with qrab certs from KEYS_DIR "
        "(must contain KEK.crt + db.crt in PEM format)",
    )
    args = ap.parse_args()

    edk2 = Path(args.edk2_root).resolve()
    if not (edk2 / "OvmfPkg").is_dir():
        print(f"not an edk2 tree: {edk2}", file=sys.stderr)
        return 1

    dsc = edk2 / "OvmfPkg" / "AmdSev" / "AmdSevX64.dsc"
    fdf = edk2 / "OvmfPkg" / "AmdSev" / "AmdSevX64.fdf"
    for p in (dsc, fdf):
        if not p.is_file():
            print(f"missing: {p}", file=sys.stderr)
            return 1

    # ---- AmdSevX64.dsc -----------------------------------------------------
    # Edit 1: replace AuthVariableLibNull line with SB-conditional block.
    replace_line(
        dsc,
        "lib-classes",
        "  AuthVariableLib|MdeModulePkg/Library/AuthVariableLibNull/AuthVariableLibNull.inf\n",
        """
!if $(SECURE_BOOT_ENABLE) == TRUE
  PlatformSecureLib|OvmfPkg/Library/PlatformSecureLib/PlatformSecureLib.inf
  AuthVariableLib|SecurityPkg/Library/AuthVariableLib/AuthVariableLib.inf
  SecureBootVariableLib|SecurityPkg/Library/SecureBootVariableLib/SecureBootVariableLib.inf
  PlatformPKProtectionLib|SecurityPkg/Library/PlatformPKProtectionLibVarPolicy/PlatformPKProtectionLibVarPolicy.inf
  SecureBootVariableProvisionLib|SecurityPkg/Library/SecureBootVariableProvisionLib/SecureBootVariableProvisionLib.inf
!else
  AuthVariableLib|MdeModulePkg/Library/AuthVariableLibNull/AuthVariableLibNull.inf
!endif
""",
    )

    # Edit 2: add PCDs after the existing OptionRom policy line.
    patch_file(
        dsc,
        [(
            "pcds",
            "  gEfiSecurityPkgTokenSpaceGuid.PcdOptionRomImageVerificationPolicy|0x00",
            """
!if $(SECURE_BOOT_ENABLE) == TRUE
  gUefiOvmfPkgTokenSpaceGuid.PcdSecureBootSupported|TRUE
  gEfiMdeModulePkgTokenSpaceGuid.PcdRequireSelfSignedPk|TRUE
!endif
""",
        )],
    )

    # Edit 3: inject DxeImageVerificationLib NULL class into SecurityStubDxe.
    # Anchor = the `<LibraryClasses>` line inside SecurityStubDxe component.
    # Identify uniquely by replacing the whole 3-line block.
    replace_line(
        dsc,
        "stub-lib",
        "  MdeModulePkg/Universal/SecurityStubDxe/SecurityStubDxe.inf {\n    <LibraryClasses>\n!include OvmfPkg/Include/Dsc/OvmfTpmSecurityStub.dsc.inc\n  }\n",
        """
  MdeModulePkg/Universal/SecurityStubDxe/SecurityStubDxe.inf {
    <LibraryClasses>
!if $(SECURE_BOOT_ENABLE) == TRUE
      NULL|SecurityPkg/Library/DxeImageVerificationLib/DxeImageVerificationLib.inf
!endif
!include OvmfPkg/Include/Dsc/OvmfTpmSecurityStub.dsc.inc
  }
""",
    )

    # Edit 4: append SB-gated components after IoMmuDxe [Components].
    patch_file(
        dsc,
        [(
            "components",
            "  OvmfPkg/IoMmuDxe/IoMmuDxe.inf",
            """
!if $(SECURE_BOOT_ENABLE) == TRUE
  SecurityPkg/VariableAuthenticated/SecureBootConfigDxe/SecureBootConfigDxe.inf
  OvmfPkg/EnrollDefaultKeys/EnrollDefaultKeys.inf
!endif
""",
        )],
    )

    # ---- AmdSevX64.fdf -----------------------------------------------------
    # Append SB-gated INF lines after SecurityStubDxe reference in FDF.
    patch_file(
        fdf,
        [(
            "fdf-sb",
            "INF  MdeModulePkg/Universal/SecurityStubDxe/SecurityStubDxe.inf",
            """
!if $(SECURE_BOOT_ENABLE) == TRUE
INF  SecurityPkg/VariableAuthenticated/SecureBootConfigDxe/SecureBootConfigDxe.inf
INF  OvmfPkg/EnrollDefaultKeys/EnrollDefaultKeys.inf
!endif
""",
        )],
    )

    # ---- post-conditions ---------------------------------------------------
    dsc_text = dsc.read_text()
    fdf_text = fdf.read_text()

    required = [
        (dsc, "SecurityPkg/Library/AuthVariableLib/AuthVariableLib.inf"),
        (dsc, "PcdSecureBootSupported|TRUE"),
        (dsc, "DxeImageVerificationLib/DxeImageVerificationLib.inf"),
        (dsc, "SecureBootConfigDxe/SecureBootConfigDxe.inf"),
        (dsc, "OvmfPkg/EnrollDefaultKeys/EnrollDefaultKeys.inf"),
        (fdf, "SecureBootConfigDxe/SecureBootConfigDxe.inf"),
        (fdf, "OvmfPkg/EnrollDefaultKeys/EnrollDefaultKeys.inf"),
    ]
    for path, needle in required:
        body = dsc_text if path == dsc else fdf_text
        if needle not in body:
            print(f"verification failed: {needle} not present in {path}",
                  file=sys.stderr)
            return 1

    # Ensure the null-stub fallback is preserved (idempotence guard).
    if "AuthVariableLibNull" not in dsc_text:
        print(f"verification failed: AuthVariableLibNull fallback missing",
              file=sys.stderr)
        return 1

    # Optional: bake qrab certs into AuthData.c.
    if args.bake_keys is not None:
        bake_keys(edk2, args.bake_keys.resolve())
        print(f"apply-sb-wiring: keys baked from {args.bake_keys}")

    print(f"apply-sb-wiring: OK on {edk2}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
