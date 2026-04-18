# qrab-boot-tests fixtures

Generated test artifacts. Do not commit binary contents — the dir-level
`.gitignore` keeps only this README and the `.gitignore` itself tracked.

Populated by `contrib/build-test-ukis.sh`:

| File | Contents |
|---|---|
| `uki-unsigned.efi` | Unsigned copy of current `qrab-boot.efi`. |
| `uki-valid.efi`    | `uki-unsigned.efi` signed with `contrib/sb-keys/db.{key,crt}`. `sbverify --cert db.crt` succeeds. |
| `uki-tampered.efi` | `uki-valid.efi` with one byte at PE offset `0x1000` flipped. `sbverify` fails with hash mismatch. |

Phase 4+ will expand to per-scenario UKIs (real Linux kernel + initrd
bundled), one per entry in the scenario matrix. For Phase 2a these three
stand-ins are enough to prove the signing tools work end-to-end.
