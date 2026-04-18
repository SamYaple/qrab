//! Struct-shaped form body with flat-union splicing.
//!
//! `free_form` structs render a single JSON textarea at `path`. For
//! unions, the discriminator field is rendered first and the selected
//! branch's payload fields are spliced in at the same level.

use std::collections::HashMap;

use dioxus::prelude::*;
use serde_json::Value;

use bmc::server::qmp_exec::get_struct;
use bmc::types::{FieldDto, StructDto};

use super::assemble::{get, set, FormPath, PathSeg};
use super::field_input::FieldInput;

#[component]
pub fn StructBody(
    meta: StructDto,
    path: FormPath,
    form_state: Signal<Value>,
    errors: Signal<HashMap<String, String>>,
) -> Element {
    if meta.free_form {
        let current = get(&form_state.read(), &path).cloned().unwrap_or(Value::Null);
        let display = serde_json::to_string_pretty(&current).unwrap_or_default();
        let path_for_input = path.clone();
        return rsx! {
            div { class: "struct-body free-form",
                div { class: "struct-hint", "Free-form JSON" }
                textarea {
                    rows: "6",
                    value: "{display}",
                    oninput: move |e| {
                        let v: Value =
                            serde_json::from_str(&e.value()).unwrap_or(Value::Null);
                        let mut s = form_state.write();
                        set(&mut *s, &path_for_input, v);
                    },
                }
            }
        };
    }

    // Splice union payload fields in at the same level once the user has
    // picked a discriminator value.
    let discriminator_wire = meta.discriminator_field.as_deref().and_then(|rust_name| {
        meta.fields
            .iter()
            .find(|f| f.name == rust_name)
            .map(|f| f.wire_name.clone())
    });
    let disc_value = discriminator_wire.as_ref().and_then(|w| {
        let mut p = path.clone();
        p.push(PathSeg::Field(w.clone()));
        get(&form_state.read(), &p)
            .and_then(|v| v.as_str())
            .map(str::to_string)
    });

    let base_fields: Vec<FieldDto> = meta
        .fields
        .iter()
        .filter(|f| f.name != "u" && f.wire_name != "u")
        .cloned()
        .collect();

    rsx! {
        div { class: "struct-body",
            if !meta.doc.is_empty() {
                div { class: "struct-doc", "{meta.doc}" }
            }
            for field in base_fields.iter().cloned() {
                {
                    let mut child = path.clone();
                    child.push(PathSeg::Field(field.wire_name.clone()));
                    rsx! {
                        FieldInput {
                            key: "{field.wire_name}",
                            meta: field,
                            path: child,
                            form_state: form_state,
                            errors: errors,
                        }
                    }
                }
            }
            if meta.is_union {
                if let Some(disc) = disc_value.clone() {
                    if !disc.is_empty() {
                        UnionPayload {
                            branch_enum: meta.branch_enum.clone().unwrap_or_default(),
                            variant_wire: disc,
                            path: path.clone(),
                            form_state: form_state,
                            errors: errors,
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn UnionPayload(
    branch_enum: String,
    variant_wire: String,
    path: FormPath,
    form_state: Signal<Value>,
    errors: Signal<HashMap<String, String>>,
) -> Element {
    // Resolve branch enum → variant → payload_struct → StructDto.
    // The branch enum DTO is fetched lazily; the payload struct is
    // fetched once the variant is known.
    let mut payload: Signal<Option<StructDto>> = use_signal(|| None);
    let mut active_variant = use_signal(String::new);

    {
        let branch_enum = branch_enum.clone();
        let variant_wire = variant_wire.clone();
        use_future(move || {
            let branch_enum = branch_enum.clone();
            let variant_wire = variant_wire.clone();
            async move {
                let enum_dto = match bmc::server::qmp_exec::get_enum(branch_enum).await {
                    Ok(Some(e)) => e,
                    _ => return,
                };
                let variant = enum_dto
                    .variants
                    .into_iter()
                    .find(|v| v.wire_name == variant_wire);
                if let Some(v) = variant {
                    if active_variant.read().as_str() != v.wire_name {
                        active_variant.set(v.wire_name.clone());
                        payload.set(None);
                    }
                    if let Some(payload_name) = v.payload_struct {
                        if let Ok(Some(s)) = get_struct(payload_name).await {
                            payload.set(Some(s));
                        }
                    }
                }
            }
        });
    }

    let payload_value = payload.read().clone();
    match payload_value {
        None => rsx! { span { class: "union-payload-loading", "…" } },
        Some(p) => rsx! {
            div { class: "union-payload",
                for field in p.fields.iter().cloned() {
                    {
                        let mut child = path.clone();
                        child.push(PathSeg::Field(field.wire_name.clone()));
                        rsx! {
                            FieldInput {
                                key: "u-{field.wire_name}",
                                meta: field,
                                path: child,
                                form_state: form_state,
                                errors: errors,
                            }
                        }
                    }
                }
            }
        },
    }
}
