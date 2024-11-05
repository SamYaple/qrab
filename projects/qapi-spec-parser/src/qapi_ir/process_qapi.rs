///
/// Here lies hot-garbage. Youve been warned
/// This code is a bandaid to go from the raw qapi parsed schema to the structs we
/// setup for rendering enum and struct variants as rust code.
///
/// We have to extract all documentation, if conditions, features, and the original
/// names of the various objects and fields. Those get set to attributes in the
/// generated code.
///
use super::{Attribute, Enum, EnumVariant, EnumVariantKind, Metadata, Struct, StructField};
use crate::qapi_ir::{rustify_field_name, rustify_name};
use crate::{
    MembersOrRef, QapiAlternate, QapiAlternative, QapiCommand, QapiEnum, QapiEnumValue, QapiEvent,
    QapiMember, QapiStruct, QapiTypeRef, QapiUnion,
};
use std::collections::HashMap;

macro_rules! add_feat {
    ($meta:expr, $feat:expr) => {
        if let Some(features) = $feat {
            for feature in features {
                $meta
                    .attributes
                    .push(Attribute::with_value("feature", feature.name));
                if let Some(condition) = &feature.r#if {
                    // TODO: I still dont like this feature conditional. Exposing it, we just
                    // ignore it in the macro currently.
                    $meta.attributes.push(Attribute::with_value(
                        format!("feature_{}_if", rustify_field_name(feature.name)),
                        condition,
                    ));
                }
            }
        }
    };
}

macro_rules! add_docs {
    ($meta:expr, $docs:expr, $name:expr, $variants:expr) => {
        if let Some(doc) = $docs {
            if doc.name != $name {
                todo! { "This is a validation error; Doc section name does not match" }
            }
            if doc.description.len() > 0 {
                $meta.doc = Some(doc.description.join("\n"));
            }
            if let Some(since) = doc.since {
                $meta.attributes.push(Attribute::with_value("since", since));
            }
            'outer_loop: for (name, desc) in &doc.fields {
                for value in $variants {
                    if &value.name == name {
                        value.meta.doc = Some(desc.join("\n"));
                        continue 'outer_loop;
                    }
                }
                todo! { "This is a validation error; Documented field does not exist" }
            }
        }
    };
}

fn field_name_attr(name: &str) -> Option<Attribute> {
    let rust_name = rustify_field_name(name);
    if name != &rust_name {
        Some(Attribute::with_value("name", name))
    } else {
        None
    }
}

fn name_attr(name: &str) -> Option<Attribute> {
    let rust_name = rustify_name(name);
    if name != &rust_name {
        Some(Attribute::with_value("name", name))
    } else {
        None
    }
}

fn process_type_ref(q: QapiTypeRef) -> (&str, bool) {
    match q {
        QapiTypeRef::Unset => unreachable! {"this should have failed the parser"},
        QapiTypeRef::Ref(v) => (v, false),
        QapiTypeRef::ArrayRef(v) => (v, true),
    }
}

fn process_alternative(q: QapiAlternative) -> EnumVariant {
    let (r#type, array) = process_type_ref(q.r#type);

    let mut meta = Metadata::default();
    if let Some(attr) = name_attr(q.name) {
        meta.attributes.push(attr);
    }
    if let Some(condition) = q.r#if {
        meta.attributes
            .push(Attribute::with_value("condition", condition));
    }

    EnumVariant {
        name: q.name.into(),
        kind: EnumVariantKind::Tuple(r#type.into()),
        meta,
        array,
    }
}

pub fn process_alternate(q: QapiAlternate) -> Enum {
    let mut variants = Vec::new();
    for v in q.data {
        let variant = process_alternative(v);
        variants.push(variant);
    }

    let mut meta = Metadata::default();
    //meta.attributes.push(Attribute::new("Alternate"));
    if let Some(attr) = name_attr(q.name) {
        meta.attributes.push(attr);
    }
    if let Some(condition) = q.r#if {
        meta.attributes
            .push(Attribute::with_value("condition", condition));
    }
    add_docs! {meta, q.doc, q.name, &mut variants};

    Enum {
        name: q.name.into(),
        variants,
        meta,
    }
}

fn process_enum_value(q: QapiEnumValue) -> EnumVariant {
    let mut meta = Metadata::default();
    if let Some(attr) = name_attr(q.name) {
        meta.attributes.push(attr);
    }
    if let Some(condition) = q.r#if {
        meta.attributes
            .push(Attribute::with_value("condition", condition));
    }
    add_feat! {meta, q.r#features};

    EnumVariant {
        name: q.name.into(),
        kind: EnumVariantKind::Unit,
        meta,
        array: false,
    }
}

pub fn process_enum(q: QapiEnum) -> Enum {
    let mut variants = Vec::new();
    for v in q.data {
        let variant = process_enum_value(v);
        variants.push(variant);
    }

    let mut meta = Metadata::default();
    //meta.attributes.push(Attribute::new("Enum"));
    if let Some(attr) = name_attr(q.name) {
        meta.attributes.push(attr);
    }
    if let Some(condition) = q.r#if {
        meta.attributes
            .push(Attribute::with_value("condition", condition));
    }
    add_feat! {meta, q.r#features};
    add_docs! {meta, q.doc, q.name, &mut variants};

    Enum {
        name: q.name.into(),
        variants,
        meta,
    }
}

pub fn process_member(q: QapiMember) -> StructField {
    let (r#type, array) = process_type_ref(q.r#type);

    let mut meta = Metadata::default();
    if let Some(attr) = name_attr(q.name) {
        meta.attributes.push(attr);
    }
    if let Some(condition) = q.r#if {
        meta.attributes
            .push(Attribute::with_value("condition", condition));
    }
    add_feat! {meta, q.features};
    StructField {
        name: q.name.into(),
        meta,
        r#type: r#type.into(),
        optional: q.optional,
        array,
    }
}

pub fn process_struct(q: QapiStruct, structs_lookup: &HashMap<String, Struct>) -> Struct {
    let mut fields = Vec::new();
    if let Some(base) = q.base {
        let struct_ref = structs_lookup.get(base.into()).expect(&format!(
            "base struct not found for {} with base {}",
            q.name, base
        ));
        fields.extend(struct_ref.fields.clone());
    }
    for member in q.data {
        let field = process_member(member);
        fields.push(field);
    }
    let mut meta = Metadata::default();
    if let Some(attr) = name_attr(q.name) {
        meta.attributes.push(attr);
    }
    if let Some(condition) = q.r#if {
        meta.attributes
            .push(Attribute::with_value("condition", condition));
    }
    add_feat! {meta, q.r#features};
    add_docs! {meta, q.doc, q.name, &mut fields};

    Struct {
        name: q.name.into(),
        fields,
        meta,
    }
}

pub fn process_union(
    q: QapiUnion,
    structs_lookup: &HashMap<String, Struct>,
    enums_lookup: &HashMap<String, Enum>,
) -> (Struct, Enum) {
    let mut discriminator_opt = None;
    let mut fields = process_members_or_ref(q.base, structs_lookup);
    for field in &mut fields {
        if field.name == q.discriminator {
            field.meta.attributes.push(Attribute::new("discriminator"));
            discriminator_opt = Some(field.r#type.clone());
        }
    }
    let discriminator = discriminator_opt.expect("discriminator field not found");
    let _base_enum = enums_lookup.get(&discriminator).expect(&format!(
        "{} could not find a base enum named {}",
        q.name, discriminator
    ));

    let mut variants = Vec::new();
    for branch in q.data {
        let (r#type, array) = process_type_ref(branch.r#type);
        assert!(array == false);
        let mut meta = Metadata::default();
        if let Some(attr) = name_attr(branch.name) {
            meta.attributes.push(attr);
        }
        if let Some(condition) = branch.r#if {
            meta.attributes
                .push(Attribute::with_value("condition", condition));
        }
        variants.push(EnumVariant {
            name: branch.name.into(),
            kind: EnumVariantKind::Tuple(r#type.into()),
            meta,
            array: false,
        });
    }
    let e = Enum {
        name: q.name.to_owned() + "Branch",
        variants,
        meta: Metadata::default(),
    };

    let mut meta = Metadata::default();
    meta.attributes.push(Attribute::new("union"));
    fields.push(StructField {
        name: "u".into(),
        meta,
        r#type: q.name.to_owned() + "Branch",
        optional: true,
        array: false,
    });
    let mut meta = Metadata::default();
    if let Some(attr) = name_attr(q.name) {
        meta.attributes.push(attr);
    }
    if let Some(condition) = q.r#if {
        meta.attributes
            .push(Attribute::with_value("condition", condition));
    }
    add_feat! {meta, q.r#features};
    add_docs! {meta, q.doc, q.name, &mut fields};
    let s = Struct {
        name: q.name.into(),
        fields,
        meta,
    };
    (s, e)
}

fn process_members_or_ref(
    q: MembersOrRef,
    structs_lookup: &HashMap<String, Struct>,
) -> Vec<StructField> {
    let mut fields = Vec::new();
    match q {
        MembersOrRef::Unset => unreachable! {"this should have failed the parser"},
        MembersOrRef::Members(v) => {
            for field in v {
                let (r#type, array) = process_type_ref(field.r#type);
                let mut meta = Metadata::default();
                if let Some(attr) = field_name_attr(field.name) {
                    meta.attributes.push(attr);
                }
                if let Some(condition) = field.r#if {
                    meta.attributes
                        .push(Attribute::with_value("condition", condition));
                }
                add_feat! {meta, field.features};
                let field = StructField {
                    name: field.name.into(),
                    r#type: r#type.into(),
                    meta,
                    optional: field.optional,
                    array,
                };
                fields.push(field);
            }
        }
        MembersOrRef::Ref(v) => {
            let struct_ref = structs_lookup
                .get(v)
                .expect(&format!("failed to lookup base ref for {}", v));
            fields.extend(struct_ref.fields.clone());
        }
    }
    fields
}

pub fn process_event(q: QapiEvent, structs_lookup: &HashMap<String, Struct>) -> Struct {
    let mut fields = Vec::new();
    if let Some(data) = q.data {
        fields.extend(process_members_or_ref(data, structs_lookup));
    }
    let mut meta = Metadata::default();
    //meta.attributes.push(Attribute::new("Event"));
    if let Some(attr) = name_attr(q.name) {
        meta.attributes.push(attr);
    }
    if let Some(condition) = q.r#if {
        meta.attributes
            .push(Attribute::with_value("condition", condition));
    }
    add_feat! {meta, q.r#features};
    add_docs! {meta, q.doc, q.name, &mut fields};

    Struct {
        name: q.name.into(),
        fields,
        meta,
    }
}

fn command_process_members_or_ref(q: MembersOrRef) -> Vec<StructField> {
    let mut fields = Vec::new();
    match q {
        MembersOrRef::Unset => unreachable! {"this should have failed the parser"},
        MembersOrRef::Members(v) => {
            for field in v {
                let (r#type, array) = process_type_ref(field.r#type);
                let mut meta = Metadata::default();
                if let Some(attr) = field_name_attr(field.name) {
                    meta.attributes.push(attr);
                }
                if let Some(condition) = field.r#if {
                    meta.attributes
                        .push(Attribute::with_value("condition", condition));
                }
                add_feat! {meta, field.features};
                let field = StructField {
                    name: field.name.into(),
                    r#type: r#type.into(),
                    meta,
                    optional: field.optional,
                    array,
                };
                fields.push(field);
            }
        }
        MembersOrRef::Ref(v) => {
            let mut meta = Metadata::default();
            meta.attributes.push(Attribute::new("flatten"));
            let field = StructField {
                name: "data".into(),
                r#type: v.into(),
                meta,
                optional: false,
                array: false,
            };
            fields.push(field);
        }
    }
    fields
}

pub fn process_command(q: QapiCommand) -> Struct {
    let mut fields = Vec::new();
    if let Some(data) = q.data {
        fields.extend(command_process_members_or_ref(data));
    }
    let mut meta = Metadata::default();
    //meta.attributes.push(Attribute::new("Command"));
    if let Some(attr) = name_attr(q.name) {
        meta.attributes.push(attr);
    }
    if let Some(condition) = q.r#if {
        meta.attributes
            .push(Attribute::with_value("condition", condition));
    }
    add_feat! {meta, q.r#features};
    add_docs! {meta, q.doc, q.name, &mut fields};
    if let Some(returns) = q.returns {
        let (r#type, array) = process_type_ref(returns);
        let r#type = if array {
            format! {"Vec<{}>", r#type}
        } else {
            r#type.into()
        };
        meta.attributes
            .push(Attribute::with_value("returns", r#type));
    } else {
        meta.attributes.push(Attribute::with_value("returns", "()"));
    }
    if let Some(b) = q.allow_oob {
        if b.parse().unwrap() {
            meta.attributes.push(Attribute::new("allow_oob"));
        }
    }
    if let Some(b) = q.allow_preconfig {
        if b.parse().unwrap() {
            meta.attributes.push(Attribute::new("allow_preconfig"));
        }
    }
    if let Some(b) = q.success_response {
        if !b.parse::<bool>().unwrap() {
            meta.attributes.push(Attribute::new("no_success_response"));
        }
    }

    Struct {
        name: q.name.into(),
        fields,
        meta,
    }
}
