# qrab-boot-tests fixtures

Staging directory for generated test artefacts. The dir-level
`.gitignore` keeps only this README and the `.gitignore` itself tracked.

Populated by:

| File | Producer | Consumer |
|---|---|---|
| `uki-real.efi` | `contrib/build-test-uki.sh` | `contrib/matrix-smoke.sh` scenario 1r |

Historical note: Phase 2b staged signed/tampered/detached-sig UKI
triplets here (`uki-valid.efi`, `uki-valid.efi.sig`, `uki-tampered.efi`,
`db.pubkey.der`) produced by a `contrib/build-test-ukis.sh` helper, to
exercise the detached RSA-PKCS#1 v1.5 verifier in `qrab-boot-verify`.
That verifier stood in for UEFI Secure Boot while AmdSev OVMF had no
`SecurityPkg`. With Phase 1.5 firmware SB landed (`issues/svsm-vtpm-
integration.md`), the detached path was retired and those fixtures
removed.
