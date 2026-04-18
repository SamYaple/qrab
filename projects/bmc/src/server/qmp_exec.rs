//! Schema-driven QMP command console — server side.
//!
//! Server fns:
//! - `qmp_queue_status` — live queue depth + capacity + OOB support.
//! - `current_qemu_version` — the negotiated QEMU version, if any.
//! - `list_commands` — every `CommandMeta` converted to DTO.
//! - `get_struct` / `get_enum` — individual DTO lookups.
//! - `execute_qmp` — validated raw QMP dispatch (OOB optional).
//!
//! The validator rejects envelopes whose shape doesn't match the schema
//! before they ever reach the QEMU process. Validation errors carry a JSON
//! path so the UI can highlight the offending field.

use dioxus::prelude::*;

use crate::types::{
    CommandDto, EnumDto, QueueStatus, StructDto,
};

// Server-side imports are scoped inside `#[cfg(feature = "server")]` blocks
// because `dioxus::prelude::server` macros strip the function bodies out of
// the wasm bundle and the imports must compile in both contexts.

/// Re-export of the validator entry point so integration tests can call
/// it without reaching into `server_impl` directly.
#[cfg(feature = "server")]
pub use server_impl::{build_qmp_request, validate_envelope, ValidationError};

// `server_impl` is publicly exposed (behind `feature = "server"`) so the
// integration test suite in `projects/bmc/tests/qmp_console.rs` can drive
// the validator and request builder directly without spinning up a full
// Dioxus fullstack harness.
#[cfg(feature = "server")]
pub mod server_impl {
    use super::*;
    use crate::types::{
        EnumKindDto, EnumVariantDto, FieldDto, PrimDto, TypeDto, TypeKindDto,
    };
    use qapi::metadata::{
        self, CommandMeta, EnumKind, EnumMeta, EnumVariantMeta, FieldMeta, Prim, StructMeta,
        TypeKind, TypeRef,
    };

    pub fn convert_prim(p: Prim) -> PrimDto {
        match p {
            Prim::Str => PrimDto::Str,
            Prim::Bool => PrimDto::Bool,
            Prim::I8 => PrimDto::I8,
            Prim::I16 => PrimDto::I16,
            Prim::I32 => PrimDto::I32,
            Prim::I64 => PrimDto::I64,
            Prim::U8 => PrimDto::U8,
            Prim::U16 => PrimDto::U16,
            Prim::U32 => PrimDto::U32,
            Prim::U64 => PrimDto::U64,
            Prim::F64 => PrimDto::F64,
            Prim::Null => PrimDto::Null,
            Prim::Any => PrimDto::Any,
        }
    }

    pub fn convert_enum_kind(k: EnumKind) -> EnumKindDto {
        match k {
            EnumKind::Unit => EnumKindDto::Unit,
            EnumKind::Alternate => EnumKindDto::Alternate,
            EnumKind::FlatUnionBranch => EnumKindDto::FlatUnionBranch,
        }
    }

    /// Inline a `TypeRef` into a `TypeDto`. Lists recurse; other kinds leave
    /// `inner` empty and just carry the referenced type's name so the client
    /// can look it up later if the form needs to render its shape.
    pub fn convert_type_ref(idx: u32) -> TypeDto {
        let tr: &TypeRef = &metadata::TYPE_REFS[idx as usize];
        match tr.kind {
            TypeKind::Primitive => TypeDto {
                kind: TypeKindDto::Primitive,
                name: tr.name.to_string(),
                prim: Some(convert_prim(tr.prim)),
                inner: None,
            },
            TypeKind::Enum => TypeDto {
                kind: TypeKindDto::Enum,
                name: tr.name.to_string(),
                prim: None,
                inner: None,
            },
            TypeKind::Struct => TypeDto {
                kind: TypeKindDto::Struct,
                name: tr.name.to_string(),
                prim: None,
                inner: None,
            },
            TypeKind::List => TypeDto {
                kind: TypeKindDto::List,
                name: tr.name.to_string(),
                prim: None,
                inner: Some(Box::new(convert_type_ref(tr.inner))),
            },
        }
    }

    pub fn convert_field(f: &FieldMeta) -> FieldDto {
        FieldDto {
            name: f.name.to_string(),
            wire_name: f.wire_name.to_string(),
            ty: convert_type_ref(f.ty),
            optional: f.optional,
            is_discriminator: f.is_discriminator,
            recursive: f.recursive,
            since: f.since,
            until: f.until,
            deprecated: f.deprecated,
            unstable: f.unstable,
            doc: f.doc.to_string(),
        }
    }

    pub fn convert_variant(v: &EnumVariantMeta) -> EnumVariantDto {
        EnumVariantDto {
            name: v.name.to_string(),
            wire_name: v.wire_name.to_string(),
            payload_struct: v.payload_struct.map(str::to_string),
            since: v.since,
            until: v.until,
            deprecated: v.deprecated,
            unstable: v.unstable,
            doc: v.doc.to_string(),
        }
    }

    pub fn convert_struct(s: &StructMeta) -> StructDto {
        StructDto {
            name: s.name.to_string(),
            doc: s.doc.to_string(),
            fields: s.fields.iter().map(convert_field).collect(),
            is_union: s.is_union,
            discriminator_field: s.discriminator_field.map(str::to_string),
            branch_enum: s.branch_enum.map(str::to_string),
            free_form: s.free_form,
            since: s.since,
            until: s.until,
        }
    }

    pub fn convert_enum(e: &EnumMeta) -> EnumDto {
        EnumDto {
            name: e.name.to_string(),
            doc: e.doc.to_string(),
            variants: e.variants.iter().map(convert_variant).collect(),
            kind: convert_enum_kind(e.kind),
            since: e.since,
            until: e.until,
        }
    }

    pub fn convert_command(c: &CommandMeta) -> CommandDto {
        CommandDto {
            wire_name: c.wire_name.to_string(),
            rust_name: c.rust_name.to_string(),
            args: convert_struct(c.args),
            allow_oob: c.allow_oob,
            allow_preconfig: c.allow_preconfig,
            success_response: c.success_response,
            since: c.since,
            until: c.until,
            deprecated: c.deprecated,
            unstable: c.unstable,
            doc: c.doc.to_string(),
            category: c.category.to_string(),
        }
    }

    // --- Validator ---

    #[derive(Debug)]
    pub struct ValidationError {
        pub path: String,
        pub reason: String,
    }

    impl ValidationError {
        pub fn new(path: impl Into<String>, reason: impl Into<String>) -> Self {
            Self {
                path: path.into(),
                reason: reason.into(),
            }
        }
        pub fn into_server(self) -> ServerFnError {
            ServerFnError::new(format!("{}: {}", self.path, self.reason))
        }
    }

    /// Return the QEMU version used for version-gated validation.
    ///
    /// If no version has been negotiated (`qapi::qemu_version()` returns
    /// `(0, 0, 0)`), we fall back to a very high version so historic
    /// fields that have been dropped (`until: Some(v)`) aren't treated
    /// as required.
    fn effective_version() -> (u32, u32, u32) {
        let v = qapi::qemu_version();
        if v == (0, 0, 0) {
            // Anything newer than every known `until` marker.
            (u32::MAX, 0, 0)
        } else {
            v
        }
    }

    fn is_alive(since: (u32, u32, u32), until: Option<(u32, u32, u32)>) -> bool {
        let v = effective_version();
        if v < since {
            return false;
        }
        if let Some(u) = until {
            if v >= u {
                return false;
            }
        }
        true
    }

    pub fn validate_envelope(envelope: &serde_json::Value) -> Result<&'static CommandMeta, ValidationError> {
        let execute = envelope
            .get("execute")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ValidationError::new("/execute", "missing or not a string"))?;

        let cmd = metadata::COMMANDS
            .iter()
            .find(|c| c.wire_name == execute)
            .ok_or_else(|| {
                ValidationError::new("/execute", format!("unknown command '{execute}'"))
            })?;

        let empty = serde_json::Value::Object(serde_json::Map::new());
        let args = envelope.get("arguments").unwrap_or(&empty);
        validate_struct(args, cmd.args, "/arguments")?;
        Ok(cmd)
    }

    fn validate_struct(
        value: &serde_json::Value,
        meta: &'static StructMeta,
        path: &str,
    ) -> Result<(), ValidationError> {
        if meta.free_form {
            if !value.is_object() {
                return Err(ValidationError::new(path, "expected object"));
            }
            return Ok(());
        }

        let obj = value.as_object().ok_or_else(|| {
            ValidationError::new(path, format!("expected object for struct {}", meta.name))
        })?;

        // Collect expected fields; for unions, splice in the selected
        // branch's payload fields at the same level. Drop fields that
        // the active QEMU version has outgrown — e.g. `blockdev-add`
        // carries a historical `options` field that was dropped in 2.8,
        // so modern guests must not see it as required.
        let mut expected_fields: Vec<&FieldMeta> = meta
            .fields
            .iter()
            .filter(|f| is_alive(f.since, f.until))
            .collect();
        if meta.is_union {
            let disc_rust = meta.discriminator_field.ok_or_else(|| {
                ValidationError::new(
                    path,
                    format!("union {} missing discriminator_field", meta.name),
                )
            })?;
            let disc_meta = meta
                .fields
                .iter()
                .find(|f| f.name == disc_rust)
                .ok_or_else(|| {
                    ValidationError::new(
                        path,
                        format!(
                            "union {} discriminator '{}' not in fields",
                            meta.name, disc_rust
                        ),
                    )
                })?;
            let disc_wire = disc_meta.wire_name;

            let disc_value = obj.get(disc_wire).and_then(|v| v.as_str()).ok_or_else(|| {
                ValidationError::new(
                    format!("{path}/{disc_wire}"),
                    "missing discriminator",
                )
            })?;

            let branch_enum_name = meta.branch_enum.ok_or_else(|| {
                ValidationError::new(path, format!("union {} missing branch_enum", meta.name))
            })?;
            let branch_enum: &EnumMeta =
                metadata::ENUMS.get(branch_enum_name).ok_or_else(|| {
                    ValidationError::new(
                        path,
                        format!("branch enum {branch_enum_name} missing from metadata"),
                    )
                })?;

            let variant = branch_enum
                .variants
                .iter()
                .find(|v| v.wire_name == disc_value && is_alive(v.since, v.until))
                .ok_or_else(|| {
                    ValidationError::new(
                        format!("{path}/{disc_wire}"),
                        format!("'{disc_value}' is not a valid {branch_enum_name} variant"),
                    )
                })?;

            if let Some(payload_name) = variant.payload_struct {
                let payload: &StructMeta =
                    metadata::STRUCTS.get(payload_name).ok_or_else(|| {
                        ValidationError::new(
                            path,
                            format!("payload struct {payload_name} missing from metadata"),
                        )
                    })?;
                // The generated `u` flatten field shouldn't leak into
                // user input; drop any `u` field from the base list
                // defensively even though current metadata doesn't emit
                // one.
                expected_fields.retain(|f| f.name != "u" && f.wire_name != "u");
                expected_fields.extend(payload.fields.iter().filter(|f| is_alive(f.since, f.until)));
            }
        }

        for field in &expected_fields {
            match obj.get(field.wire_name) {
                Some(v) => validate_type(v, field.ty, &format!("{path}/{}", field.wire_name))?,
                None if !field.optional => {
                    return Err(ValidationError::new(
                        format!("{path}/{}", field.wire_name),
                        "required field missing",
                    ));
                }
                None => {}
            }
        }

        Ok(())
    }

    fn validate_type(
        value: &serde_json::Value,
        ty_idx: u32,
        path: &str,
    ) -> Result<(), ValidationError> {
        let ty: &TypeRef = &metadata::TYPE_REFS[ty_idx as usize];
        match ty.kind {
            TypeKind::Primitive => validate_prim(value, ty.prim, path),
            TypeKind::Enum => {
                let em: &EnumMeta = metadata::ENUMS.get(ty.name).ok_or_else(|| {
                    ValidationError::new(path, format!("enum {} missing from metadata", ty.name))
                })?;
                match em.kind {
                    EnumKind::Unit | EnumKind::FlatUnionBranch => {
                        let s = value
                            .as_str()
                            .ok_or_else(|| ValidationError::new(path, "expected string variant"))?;
                        if !em.variants.iter().any(|v| v.wire_name == s) {
                            return Err(ValidationError::new(
                                path,
                                format!("'{s}' is not a valid {} variant", ty.name),
                            ));
                        }
                        Ok(())
                    }
                    EnumKind::Alternate => {
                        // Try each alternate variant's payload type; accept
                        // the value if any matches. Empty variant lists are
                        // treated as a shape check only.
                        if em.variants.is_empty() {
                            return Ok(());
                        }
                        let mut last_err: Option<ValidationError> = None;
                        for v in em.variants {
                            if let Some(payload_name) = v.payload_struct {
                                // Payload is a struct
                                if let Some(sm) = metadata::STRUCTS.get(payload_name) {
                                    match validate_struct(value, sm, path) {
                                        Ok(()) => return Ok(()),
                                        Err(e) => last_err = Some(e),
                                    }
                                } else if let Some(inner_em) = metadata::ENUMS.get(payload_name) {
                                    // Enum payload: try as enum string
                                    if value.is_string() {
                                        let s = value.as_str().unwrap();
                                        if inner_em.variants.iter().any(|x| x.wire_name == s) {
                                            return Ok(());
                                        }
                                        last_err = Some(ValidationError::new(
                                            path,
                                            format!(
                                                "'{s}' not valid in alternate variant {payload_name}"
                                            ),
                                        ));
                                    }
                                }
                            } else {
                                // No payload: match on the variant shape. For
                                // `StrOrNull`-like alternates the wire_name is
                                // the JSON type (`str`, `null`, ...) — accept
                                // structural match against Prim names.
                                match v.wire_name {
                                    "str" if value.is_string() => return Ok(()),
                                    "null" if value.is_null() => return Ok(()),
                                    "bool" if value.is_boolean() => return Ok(()),
                                    "int" | "int64" if value.is_i64() => return Ok(()),
                                    "number" if value.is_number() => return Ok(()),
                                    _ => {}
                                }
                            }
                        }
                        Err(last_err.unwrap_or_else(|| {
                            ValidationError::new(
                                path,
                                format!("no alternate variant matched for {}", ty.name),
                            )
                        }))
                    }
                }
            }
            TypeKind::Struct => {
                let sm: &StructMeta = metadata::STRUCTS.get(ty.name).ok_or_else(|| {
                    ValidationError::new(path, format!("struct {} missing from metadata", ty.name))
                })?;
                validate_struct(value, sm, path)
            }
            TypeKind::List => {
                let arr = value.as_array().ok_or_else(|| {
                    ValidationError::new(path, format!("expected array for {}", ty.name))
                })?;
                for (i, elem) in arr.iter().enumerate() {
                    validate_type(elem, ty.inner, &format!("{path}/{i}"))?;
                }
                Ok(())
            }
        }
    }

    /// Transform a client-supplied `{execute, arguments}` envelope into
    /// the raw QMP request dispatched onto the socket. In OOB mode the
    /// verb is renamed to `exec-oob`; any other keys the caller smuggled
    /// (for example a client-side `id`) are stripped so they can't
    /// clobber the `Qmp` client's id counter.
    pub fn build_qmp_request(envelope: &serde_json::Value, oob: bool) -> serde_json::Value {
        let execute = envelope
            .get("execute")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let mut req = serde_json::Map::new();
        if oob {
            req.insert("exec-oob".into(), execute);
        } else {
            req.insert("execute".into(), execute);
        }
        if let Some(args) = envelope.get("arguments") {
            req.insert("arguments".into(), args.clone());
        }
        serde_json::Value::Object(req)
    }

    fn validate_prim(
        value: &serde_json::Value,
        prim: Prim,
        path: &str,
    ) -> Result<(), ValidationError> {
        let ok = match prim {
            Prim::Str => value.is_string(),
            Prim::Bool => value.is_boolean(),
            Prim::I8 | Prim::I16 | Prim::I32 | Prim::I64 => value.is_i64(),
            Prim::U8 | Prim::U16 | Prim::U32 | Prim::U64 => {
                value.is_u64() || (value.is_i64() && value.as_i64().unwrap() >= 0)
            }
            Prim::F64 => value.is_number(),
            Prim::Null => value.is_null(),
            Prim::Any => true,
        };
        if ok {
            Ok(())
        } else {
            Err(ValidationError::new(
                path,
                format!("expected {:?} primitive", prim),
            ))
        }
    }
}

/// Live QMP queue status. Polled by the QMP console panel to display
/// a queue-depth badge and the OOB availability toggle.
#[server]
pub async fn qmp_queue_status() -> Result<QueueStatus, ServerFnError> {
    use crate::state::{bmc, VmState};

    let state = bmc();
    let vm_guard = state.vm.lock().await;
    match &*vm_guard {
        VmState::Running(vm) => Ok(QueueStatus {
            depth: vm.qmp.queue_depth(),
            capacity: vm.qmp.queue_capacity(),
            oob_supported: vm.qmp.oob_supported(),
        }),
        _ => Ok(QueueStatus::default()),
    }
}

/// Negotiated QEMU version, if a VM has ever run in this process.
#[server]
pub async fn current_qemu_version() -> Result<Option<(u32, u32, u32)>, ServerFnError> {
    let v = qapi::qemu_version();
    if v == (0, 0, 0) {
        Ok(None)
    } else {
        Ok(Some(v))
    }
}

/// Every known command as a DTO. Called once on panel mount to seed the
/// command picker; cached in the client thereafter.
#[server]
pub async fn list_commands() -> Result<Vec<CommandDto>, ServerFnError> {
    use server_impl::convert_command;
    Ok(qapi::metadata::COMMANDS.iter().map(convert_command).collect())
}

/// Single struct lookup for union branch payloads and nested field shapes.
#[server]
pub async fn get_struct(name: String) -> Result<Option<StructDto>, ServerFnError> {
    use server_impl::convert_struct;
    Ok(qapi::metadata::STRUCTS.get(name.as_str()).map(|s| convert_struct(s)))
}

/// Single enum lookup for `<select>` option rendering.
#[server]
pub async fn get_enum(name: String) -> Result<Option<EnumDto>, ServerFnError> {
    use server_impl::convert_enum;
    Ok(qapi::metadata::ENUMS.get(name.as_str()).map(|e| convert_enum(e)))
}

/// Validate the supplied QMP envelope against the compiled-in schema, then
/// dispatch it to the running VM. `oob=true` uses the `exec-oob` envelope.
///
/// Envelope shape:
/// ```json
/// { "execute": "query-status", "arguments": { ... } }
/// ```
/// `arguments` is optional for commands with empty args.
#[server]
pub async fn execute_qmp(
    envelope: serde_json::Value,
    oob: bool,
) -> Result<serde_json::Value, ServerFnError> {
    use crate::state::{bmc, VmState};
    use server_impl::{validate_envelope, ValidationError};

    // Validate shape first so bogus envelopes never hit QMP.
    let cmd = validate_envelope(&envelope).map_err(ValidationError::into_server)?;
    if oob && !cmd.allow_oob {
        return Err(ServerFnError::new(format!(
            "command '{}' does not support OOB",
            cmd.wire_name
        )));
    }

    // Dispatch under the vm lock.
    let state = bmc();
    let vm_guard = state.vm.lock().await;
    match &*vm_guard {
        VmState::Running(vm) => {
            let req_value = server_impl::build_qmp_request(&envelope, oob);
            let fut = async {
                if oob {
                    vm.qmp.execute_raw_oob(req_value).await
                } else {
                    vm.qmp.execute_raw(req_value).await
                }
            };
            fut.await.map_err(|e| ServerFnError::new(e.to_string()))
        }
        _ => Err(ServerFnError::new("VM is not running")),
    }
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;
    use server_impl::{build_qmp_request, validate_envelope};

    #[test]
    fn request_builder_non_oob() {
        let envelope = serde_json::json!({
            "execute": "query-status",
            "arguments": {}
        });
        let r = build_qmp_request(&envelope, false);
        assert_eq!(r["execute"], "query-status");
        assert!(r.get("exec-oob").is_none());
        assert!(r.get("arguments").is_some());
    }

    #[test]
    fn request_builder_oob() {
        let envelope = serde_json::json!({"execute": "quit", "arguments": {}});
        let r = build_qmp_request(&envelope, true);
        assert_eq!(r["exec-oob"], "quit");
        assert!(r.get("execute").is_none());
    }

    #[test]
    fn request_builder_strips_foreign_keys() {
        // A smuggled `id` must not leak into the wire request — Qmp
        // manages its own id counter and a colliding id would break
        // response routing.
        let envelope = serde_json::json!({
            "execute": "query-status",
            "id": 999,
            "arguments": {}
        });
        let r = build_qmp_request(&envelope, false);
        assert!(r.get("id").is_none(), "id must not survive: {r}");
    }

    #[test]
    fn validator_rejects_unknown_command() {
        let env = serde_json::json!({"execute": "nonsense"});
        let err = validate_envelope(&env).err().expect("should reject");
        assert!(err.reason.contains("unknown command"), "got {err:?}");
    }

    #[test]
    fn validator_rejects_missing_discriminator() {
        let env = serde_json::json!({
            "execute": "blockdev-add",
            "arguments": {}
        });
        let err = validate_envelope(&env).err().expect("should reject");
        assert!(err.path.contains("/driver"), "got {err:?}");
    }

    #[test]
    fn validator_accepts_flat_union() {
        let env = serde_json::json!({
            "execute": "blockdev-add",
            "arguments": {
                "driver": "file",
                "node-name": "n",
                "filename": "/tmp/x"
            }
        });
        validate_envelope(&env).expect("should accept");
    }

    #[test]
    fn validator_accepts_query_status() {
        let env = serde_json::json!({"execute": "query-status"});
        validate_envelope(&env).expect("should accept");
    }

    #[test]
    fn validator_accepts_empty_args_object() {
        let env = serde_json::json!({"execute": "query-status", "arguments": {}});
        validate_envelope(&env).expect("should accept");
    }
}
