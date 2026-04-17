# SEV-SNP Status Display: SevInfoBranch Deserialization Bug

## Summary

`query-sev` QMP command fails deserialization on QEMU 10.2 with error:
```
query-sev failed: no matching variant for SevInfoBranch
```

This causes the BMC security panel to show "Not available" for SEV even when SEV is active and the VM is running encrypted.

## Root Cause

`SevInfo` (generated in `qapi/src/generated/mod.rs:15756`) contains a **flattened discriminated union** added in QAPI 9.1.0:

```rust
pub struct SevInfo {
    pub api_major: u8,
    pub api_minor: u8,
    pub build_id: u8,
    pub enabled: bool,
    pub handle: Option<u32>,        // removed 9.1.0
    pub policy: Option<u32>,        // removed 9.1.0
    #[qapi(name = "sev-type")]
    #[qapi(discriminator)]
    #[qapi(added = "9.1.0")]
    pub sev_type: Option<SevGuestType>,
    pub state: SevState,
    #[qapi(union)]
    #[qapi(flatten)]
    #[qapi(added = "9.1.0")]
    pub u: Option<SevInfoBranch>,
}

pub enum SevInfoBranch {
    #[qapi(name = "sev")]
    Sev(SevGuestInfo),
    #[qapi(name = "sev-snp")]
    SevSnp(SevSnpGuestInfo),
}
```

QEMU 10.2 returns JSON like:
```json
{
  "api-major": 1,
  "api-minor": 55,
  "build-id": 49,
  "enabled": true,
  "sev-type": "sev",
  "state": "running",
  "handle": 1,
  "policy": 1
}
```

The `#[qapi]` proc macro's serde implementation for flattened discriminated unions fails to match the `"sev"` variant in `SevInfoBranch`. The discriminator (`sev_type`) parses correctly as `SevGuestType::Sev`, but the flattened union branch deserialization fails.

## Where to Fix

**`projects/qapi-macros/src/lib.rs`** — the proc macro that generates serde impls for `#[qapi]`-annotated structs with `#[qapi(union)]` + `#[qapi(flatten)]` fields.

The issue is specifically with **flattened discriminated unions where the discriminator is `Option<T>`** (version-gated). The macro needs to:

1. Parse the discriminator value from the already-deserialized `sev_type` field
2. Use it to select the correct `SevInfoBranch` variant
3. Deserialize the branch-specific fields from the flattened JSON

### Possible failure modes

- The macro may be trying to deserialize `SevInfoBranch` as a tagged enum from the JSON, but the JSON is flat (no nested object for the branch data)
- The discriminator field `sev-type` maps to variant names `"sev"` and `"sev-snp"`, but the macro may not be using the `#[qapi(name = "...")]` attributes on the enum variants for matching
- The `Option<SevInfoBranch>` wrapper (because `added = "9.1.0"` makes it optional) adds another layer the deserializer must handle
- When `sev_type` is `Some(SevGuestType::Sev)`, the union should produce `Some(SevInfoBranch::Sev(SevGuestInfo { ... }))`, but the branch data (`handle`, `policy`) overlaps with removed fields on `SevInfo` itself

### Related types to check

- `SevGuestInfo` at `mod.rs:15809` — fields for the `sev` branch
- `SevSnpGuestInfo` at `mod.rs` — fields for the `sev-snp` branch
- `SevGuestType` enum at `mod.rs:15744` — discriminator values

## Workaround (BMC)

The BMC already handles this gracefully — `.ok()` converts the error to `None`, and the security panel falls back to host capability display. However, SEV status from a running VM is never shown.

A possible BMC-level workaround without fixing the macro: use a raw `serde_json::Value` query to extract just the fields we need (`enabled`, `sev-type`, `state`, `api-major`, `api-minor`) without going through the full `SevInfo` type.

## Other Flattened Union Types to Audit

This bug likely affects all QAPI types with `#[qapi(union)] #[qapi(flatten)]`. Search for:
```
grep -n "qapi(union)" projects/qapi/src/generated/mod.rs | head -20
```

Any struct with both `#[qapi(discriminator)]` and `#[qapi(union)] #[qapi(flatten)]` fields may have the same deserialization issue.

## Session Work Log

This session accomplished:
1. Security panel wired to QMP `query-sev` / `query-tpm` (cached per VM session)
2. Host capability fallback when VM queries fail
3. ConfigPanel with KVM/SEV-SNP/TPM toggles + CPU/memory inputs
4. SEV cbitpos auto-detection from CPUID 0x8000001F
5. AmdSev OVMF firmware discovery (`find_amdsev_ovmf()`)
6. EPYC-v4 CPU model for SEV (instead of `-cpu host`)
7. Switched from `sev-snp-guest` to `sev-guest` (SNP lacks KVM_CAP_READONLY_MEM on kernel 6.18)
8. Pflash with readonly AmdSev OVMF for SEV firmware loading
9. Gentoo `edk2-amdsev` ebuild for building AmdSev OVMF variant
10. UKI/dracut cmdline debugging (`/etc/kernel/cmdline` was missing)
