//! Per-field widget dispatch.
//!
//! Optional fields are wrapped in a presence toggle that adds/removes the
//! key from `form_state` on click. The inner widget is selected from the
//! field's `TypeDto`:
//!
//! * primitive → text / number / checkbox
//! * enum (unit) → `<select>` of variants
//! * enum (alternate) → `<select>` of variant "kinds" then a sub-widget for the payload
//! * struct → a collapsible `<details>` containing a nested `StructBody`
//! * list → a repeater with +/- buttons

use std::collections::HashMap;

use dioxus::prelude::*;
use serde_json::Value;

use bmc::server::qmp_exec::{get_enum, get_struct};
use bmc::types::{EnumDto, EnumKindDto, FieldDto, PrimDto, StructDto, TypeDto, TypeKindDto};

use super::assemble::{get, path_to_string, remove, set, FormPath, PathSeg};
use super::struct_body::StructBody;

const PATH_PREFIX: &str = "/arguments";

fn describe_field(f: &FieldDto) -> String {
    let mut s = String::new();
    s.push_str(&f.wire_name);
    if f.optional {
        s.push_str(" (optional)");
    }
    if f.deprecated {
        s.push_str(" · deprecated");
    }
    if f.unstable {
        s.push_str(" · unstable");
    }
    s
}

#[component]
pub fn FieldInput(
    meta: FieldDto,
    path: FormPath,
    form_state: Signal<Value>,
    errors: Signal<HashMap<String, String>>,
) -> Element {
    let present = get(&form_state.read(), &path).is_some();
    let wire_name = meta.wire_name.clone();
    let label = describe_field(&meta);

    if meta.optional && !present {
        let path_for_add = path.clone();
        let default_for_type = default_value_for(&meta.ty);
        return rsx! {
            div { class: "field field-optional-absent",
                label {
                    input {
                        r#type: "checkbox",
                        checked: false,
                        onchange: move |_| {
                            let mut s = form_state.write();
                            set(&mut *s, &path_for_add, default_for_type.clone());
                        },
                    }
                    span { class: "field-label absent", "{label}" }
                }
            }
        };
    }

    let path_for_remove = path.clone();
    let control = render_type_control(meta.ty.clone(), path.clone(), form_state, errors);

    let err_key = path_to_string(PATH_PREFIX, &path);
    let err_msg = errors.read().get(&err_key).cloned();

    rsx! {
        div { class: "field field-present",
            label {
                if meta.optional {
                    input {
                        r#type: "checkbox",
                        checked: true,
                        onchange: move |_| {
                            let mut s = form_state.write();
                            remove(&mut *s, &path_for_remove);
                        },
                    }
                }
                span { class: "field-label", "{label}" }
            }
            if !meta.doc.is_empty() {
                div { class: "field-doc", "{meta.doc}" }
            }
            div { class: "field-control", {control} }
            if let Some(err) = err_msg {
                div { class: "field-error", "{err}" }
            }
            // Swallow the unused wire_name so the compiler doesn't complain
            // about the clone above; used for stable keys inside repeaters.
            span { class: "field-wire", hidden: true, "{wire_name}" }
        }
    }
}

fn render_type_control(
    ty: TypeDto,
    path: FormPath,
    form_state: Signal<Value>,
    errors: Signal<HashMap<String, String>>,
) -> Element {
    match ty.kind {
        TypeKindDto::Primitive => rsx! {
            PrimitiveInput {
                prim: ty.prim.unwrap_or(PrimDto::Str),
                path: path,
                form_state: form_state,
            }
        },
        TypeKindDto::Enum => rsx! {
            EnumInputAsync {
                enum_name: ty.name,
                path: path,
                form_state: form_state,
                errors: errors,
            }
        },
        TypeKindDto::Struct => rsx! {
            StructInputAsync {
                struct_name: ty.name,
                path: path,
                form_state: form_state,
                errors: errors,
            }
        },
        TypeKindDto::List => rsx! {
            ListInput {
                inner: ty.inner.expect("list without inner"),
                path: path,
                form_state: form_state,
                errors: errors,
            }
        },
    }
}

fn default_value_for(ty: &TypeDto) -> Value {
    match ty.kind {
        TypeKindDto::Primitive => match ty.prim.clone().unwrap_or(PrimDto::Str) {
            PrimDto::Str => Value::String(String::new()),
            PrimDto::Bool => Value::Bool(false),
            PrimDto::I8 | PrimDto::I16 | PrimDto::I32 | PrimDto::I64 => {
                Value::Number(serde_json::Number::from(0))
            }
            PrimDto::U8 | PrimDto::U16 | PrimDto::U32 | PrimDto::U64 => {
                Value::Number(serde_json::Number::from(0u64))
            }
            PrimDto::F64 => Value::Number(
                serde_json::Number::from_f64(0.0).unwrap_or_else(|| serde_json::Number::from(0)),
            ),
            PrimDto::Null => Value::Null,
            PrimDto::Any => Value::Null,
        },
        TypeKindDto::Enum => Value::String(String::new()),
        TypeKindDto::Struct => Value::Object(serde_json::Map::new()),
        TypeKindDto::List => Value::Array(Vec::new()),
    }
}

#[component]
fn PrimitiveInput(prim: PrimDto, path: FormPath, form_state: Signal<Value>) -> Element {
    let current = get(&form_state.read(), &path).cloned().unwrap_or(Value::Null);
    match prim {
        PrimDto::Bool => {
            let checked = current.as_bool().unwrap_or(false);
            let path_for_change = path.clone();
            rsx! {
                input {
                    r#type: "checkbox",
                    checked: checked,
                    onchange: move |e| {
                        let v = e.value() == "true";
                        let mut s = form_state.write();
                        set(&mut *s, &path_for_change, Value::Bool(v));
                    },
                }
            }
        }
        PrimDto::Str => {
            let val = current.as_str().map(str::to_string).unwrap_or_default();
            let path_for_input = path.clone();
            rsx! {
                input {
                    r#type: "text",
                    value: "{val}",
                    oninput: move |e| {
                        let mut s = form_state.write();
                        set(&mut *s, &path_for_input, Value::String(e.value()));
                    },
                }
            }
        }
        PrimDto::I8 | PrimDto::I16 | PrimDto::I32 | PrimDto::I64
        | PrimDto::U8 | PrimDto::U16 | PrimDto::U32 | PrimDto::U64
        | PrimDto::F64 => {
            let display = match &current {
                Value::Number(n) => n.to_string(),
                _ => String::new(),
            };
            let path_for_input = path.clone();
            let signed = matches!(
                prim,
                PrimDto::I8 | PrimDto::I16 | PrimDto::I32 | PrimDto::I64
            );
            let is_float = matches!(prim, PrimDto::F64);
            rsx! {
                input {
                    r#type: "number",
                    value: "{display}",
                    oninput: move |e| {
                        let raw = e.value();
                        let v = if is_float {
                            raw.parse::<f64>()
                                .ok()
                                .and_then(serde_json::Number::from_f64)
                                .map(Value::Number)
                                .unwrap_or(Value::Null)
                        } else if signed {
                            raw.parse::<i64>()
                                .map(|n| Value::Number(serde_json::Number::from(n)))
                                .unwrap_or(Value::Null)
                        } else {
                            raw.parse::<u64>()
                                .map(|n| Value::Number(serde_json::Number::from(n)))
                                .unwrap_or(Value::Null)
                        };
                        let mut s = form_state.write();
                        set(&mut *s, &path_for_input, v);
                    },
                }
            }
        }
        PrimDto::Null => rsx! { span { class: "prim-null", "null" } },
        PrimDto::Any => {
            let display = serde_json::to_string_pretty(&current).unwrap_or_default();
            let path_for_input = path.clone();
            rsx! {
                textarea {
                    rows: "3",
                    value: "{display}",
                    oninput: move |e| {
                        let v: Value = serde_json::from_str(&e.value())
                            .unwrap_or(Value::String(e.value()));
                        let mut s = form_state.write();
                        set(&mut *s, &path_for_input, v);
                    },
                }
            }
        }
    }
}

/// Enum selector — the DTO isn't carried; we fetch it lazily when the
/// widget first renders. Cached in a signal so clicks don't refetch.
#[component]
fn EnumInputAsync(
    enum_name: String,
    path: FormPath,
    form_state: Signal<Value>,
    errors: Signal<HashMap<String, String>>,
) -> Element {
    let mut dto = use_signal(|| None::<EnumDto>);
    {
        let name_for_fetch = enum_name.clone();
        use_future(move || {
            let name = name_for_fetch.clone();
            async move {
                if dto.read().is_some() {
                    return;
                }
                if let Ok(Some(e)) = get_enum(name).await {
                    dto.set(Some(e));
                }
            }
        });
    }

    let current = dto.read().clone();
    match current {
        None => rsx! { span { class: "enum-loading", "…" } },
        Some(e) => rsx! {
            EnumInput {
                dto: e,
                path: path,
                form_state: form_state,
                errors: errors,
            }
        },
    }
}

#[component]
fn EnumInput(
    dto: EnumDto,
    path: FormPath,
    form_state: Signal<Value>,
    errors: Signal<HashMap<String, String>>,
) -> Element {
    let current = get(&form_state.read(), &path).cloned().unwrap_or(Value::Null);
    match dto.kind {
        EnumKindDto::Unit | EnumKindDto::FlatUnionBranch => {
            let val = current.as_str().map(str::to_string).unwrap_or_default();
            let path_for_change = path.clone();
            let variants = dto.variants.clone();
            rsx! {
                select {
                    value: "{val}",
                    onchange: move |e| {
                        let mut s = form_state.write();
                        set(&mut *s, &path_for_change, Value::String(e.value()));
                    },
                    option { value: "", disabled: true, selected: val.is_empty(), "— choose —" }
                    for v in variants.iter() {
                        option {
                            value: "{v.wire_name}",
                            selected: v.wire_name == val,
                            "{v.wire_name}"
                            if v.deprecated { " (deprecated)" }
                            if v.unstable { " (unstable)" }
                        }
                    }
                }
            }
        }
        EnumKindDto::Alternate => {
            // Pick-a-kind + recursive widget. The "kind" isn't stored on
            // the wire — it's local UI state only.
            let kind_key = "__alt_kind".to_string();
            let kind_path: FormPath = {
                let mut p = path.clone();
                p.push(PathSeg::Field(kind_key.clone()));
                p
            };
            let kind_val = get(&form_state.read(), &kind_path)
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .unwrap_or_default();

            let variants = dto.variants.clone();
            let path_for_kind = kind_path.clone();

            let active_variant = variants
                .iter()
                .find(|v| v.wire_name == kind_val)
                .cloned();

            let payload_widget = match active_variant {
                Some(v) => {
                    if let Some(payload_name) = v.payload_struct.clone() {
                        // Struct payload — write into path directly; this
                        // replaces any extra fields from previous kinds.
                        rsx! {
                            StructInputAsync {
                                struct_name: payload_name,
                                path: path.clone(),
                                form_state: form_state,
                                errors: errors,
                            }
                        }
                    } else {
                        // Primitive / plain payload — use a best-effort
                        // primitive widget.
                        let prim = match v.wire_name.as_str() {
                            "str" => PrimDto::Str,
                            "bool" => PrimDto::Bool,
                            "null" => PrimDto::Null,
                            "number" => PrimDto::F64,
                            _ => PrimDto::Str,
                        };
                        rsx! {
                            PrimitiveInput {
                                prim: prim,
                                path: path.clone(),
                                form_state: form_state,
                            }
                        }
                    }
                }
                None => rsx! { span { class: "alt-empty", "Pick a kind…" } },
            };

            rsx! {
                div { class: "alt-input",
                    select {
                        value: "{kind_val}",
                        onchange: move |e| {
                            let mut s = form_state.write();
                            set(&mut *s, &path_for_kind, Value::String(e.value()));
                        },
                        option { value: "", disabled: true, selected: kind_val.is_empty(), "— kind —" }
                        for v in variants.iter() {
                            option {
                                value: "{v.wire_name}",
                                selected: v.wire_name == kind_val,
                                "{v.wire_name}"
                            }
                        }
                    }
                    {payload_widget}
                }
            }
        }
    }
}

#[component]
fn StructInputAsync(
    struct_name: String,
    path: FormPath,
    form_state: Signal<Value>,
    errors: Signal<HashMap<String, String>>,
) -> Element {
    let mut dto = use_signal(|| None::<StructDto>);
    {
        let name_for_fetch = struct_name.clone();
        use_future(move || {
            let name = name_for_fetch.clone();
            async move {
                if dto.read().is_some() {
                    return;
                }
                if let Ok(Some(s)) = get_struct(name).await {
                    dto.set(Some(s));
                }
            }
        });
    }

    let current = dto.read().clone();
    match current {
        None => rsx! { span { class: "struct-loading", "…" } },
        Some(s) => rsx! {
            details { class: "struct-details",
                summary { "{s.name}" }
                StructBody {
                    meta: s,
                    path: path,
                    form_state: form_state,
                    errors: errors,
                }
            }
        },
    }
}

#[component]
fn ListInput(
    inner: Box<TypeDto>,
    path: FormPath,
    form_state: Signal<Value>,
    errors: Signal<HashMap<String, String>>,
) -> Element {
    let arr_len = get(&form_state.read(), &path)
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);

    let inner_for_add = (*inner).clone();
    let path_for_add = path.clone();
    let path_for_remove = path.clone();
    let inner_clone = (*inner).clone();

    rsx! {
        div { class: "list-input",
            for i in 0..arr_len {
                div { class: "list-row",
                    span { class: "list-index", "[{i}]" }
                    {render_type_control(
                        inner_clone.clone(),
                        {
                            let mut p = path.clone();
                            p.push(PathSeg::Index(i));
                            p
                        },
                        form_state,
                        errors,
                    )}
                    button {
                        r#type: "button",
                        class: "list-rm",
                        onclick: {
                            let path_for_this = path_for_remove.clone();
                            move |_| {
                                let mut s = form_state.write();
                                let mut p = path_for_this.clone();
                                p.push(PathSeg::Index(i));
                                remove(&mut *s, &p);
                            }
                        },
                        "–"
                    }
                }
            }
            button {
                r#type: "button",
                class: "list-add",
                onclick: move |_| {
                    let mut s = form_state.write();
                    let cur_len = get(&*s, &path_for_add)
                        .and_then(|v| v.as_array())
                        .map(|a| a.len())
                        .unwrap_or(0);
                    let mut p = path_for_add.clone();
                    p.push(PathSeg::Index(cur_len));
                    set(&mut *s, &p, default_value_for(&inner_for_add));
                },
                "+ add item"
            }
        }
    }
}
