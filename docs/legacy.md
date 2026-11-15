# Retired confidential-computing and SVSM projects

These projects reached end of life on 2026-11-15 and were removed from the
active tree. The annotated Git tag `eol/confidential-computing-svsm-2026-11-15`
preserves their final source, build scripts, lockfiles and vendor gitlinks.

The archive includes the host namespace/SEV/SVSM manager API (`legacy-host`),
`qrab-init`, `qrab-guest-init`, `qrab-boot`, `qrab-boot-proto`,
`qrab-boot-verify`, `qrab-boot-tests`, `qrab-snp-proto`, `qrab-attest-verifier`,
`attest-server`, `qrab-real-uki-demo` and `qrab-real-uki-demo-tests`.
Their firmware/UKI builders, attestation keys and certificate tooling, SNP/SVSM
smokes, initrd module, edk2 patches and dedicated vendor pins were removed too.

Inspect the archive without restoring it into the active workspace:

```sh
git show eol/confidential-computing-svsm-2026-11-15:docs/legacy.md
git ls-tree eol/confidential-computing-svsm-2026-11-15 projects/
```

The tag preserves submodule revisions, not copies of external repositories.
The historical `qemu-svsm` pin was unavailable in the local mirror at retirement.
Fetch QEMU only from the local Gitea mirror; do not substitute upstream sources.

The pod-native manager, UEFI/Secure Boot and TPM identity support, guest agent,
operator, BMC and QAPI/QMP/QGA libraries remain active. Generated protocol types
still describe current QEMU confidential-guest commands as part of the schema.
See [manager.md](manager.md) and [testing.md](testing.md) for current development.
