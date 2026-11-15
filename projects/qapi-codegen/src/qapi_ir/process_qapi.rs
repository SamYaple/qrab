//! Lower parsed QAPI definitions into Rust IR with wire names and schema metadata.
//! Dependency tables retain exact QAPI names until resolution is complete.

use super::{Attribute, Enum, EnumVariant, EnumVariantKind, Metadata, Struct, StructField};
use crate::qapi_ir::{rustify_field_name, rustify_name, rustify_variant};
use crate::{
    MembersOrRef, QapiAlternate, QapiAlternative, QapiCommand, QapiEnum, QapiEnumValue, QapiEvent,
    QapiMember, QapiStruct, QapiTypeRef, QapiUnion, extract_since_from_comment,
};
use std::collections::{HashMap, HashSet};

/// Normalize a since version string to always include patch: "1.2" → "1.2.0"
/// Also handles: trailing dots ("2.9." → "2.9.0"), extra text after version.
fn normalize_since_version(s: &str) -> String {
    let s = s.trim();
    // Extract just the version number part (digits and dots)
    let version_part: String = s
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    // Strip trailing dots
    let version_part = version_part.trim_end_matches('.');
    let parts: Vec<&str> = version_part.split('.').collect();
    match parts.len() {
        2 => format!("{}.{}.0", parts[0], parts[1]),
        3 => format!("{}.{}.{}", parts[0], parts[1], parts[2]),
        _ => format!("{}.0.0", version_part), // best effort
    }
}

macro_rules! add_feat {
    ($meta:expr, $feat:expr) => {
        if let Some(features) = $feat {
            for feature in features {
                let mut attrs = Vec::new();
                attrs.push(("feature", Some(feature.name.to_string())));
                if let Some(condition) = &feature.r#if {
                    attrs.push(("if", Some(condition.to_string())));
                }
                $meta.attributes.push(Attribute::with_values(attrs));
            }
        }
    };
}

/// Remove the schema indentation used for member-description continuations.
fn member_documentation(lines: &[&str]) -> String {
    let start = usize::from(
        lines
            .first()
            .is_some_and(|line| !line.starts_with([' ', '\t'])),
    );
    let indent = lines
        .iter()
        .skip(start)
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start_matches([' ', '\t']).len())
        .min()
        .unwrap_or(0);
    lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            if index >= start && !line.trim().is_empty() {
                &line[indent..]
            } else {
                *line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

macro_rules! add_docs {
    ($meta:expr, $docs:expr, $name:expr, $variants:expr) => {
        if let Some(doc) = $docs {
            if doc.name != $name {
                eprintln!(
                    "WARNING: Doc section name '{}' does not match '{}'",
                    doc.name, $name
                );
            }
            let body = if doc.body.is_empty() {
                &doc.description
            } else {
                &doc.body
            };
            if !body.is_empty() {
                $meta.doc = Some(body.join("\n"));
            }
            if let Some(since) = doc.since {
                $meta.attributes.push(Attribute::with_value(
                    "added",
                    normalize_since_version(since),
                ));
            }
            'outer_loop: for (name, desc) in &doc.fields {
                // Try exact match first
                for value in &mut *$variants {
                    if &value.name == name {
                        let since = extract_since_from_comment(desc);
                        value.meta.doc = Some(member_documentation(desc));
                        if let Some(since) = since {
                            value.meta.attributes.push(Attribute::with_value(
                                "added",
                                normalize_since_version(&since),
                            ));
                        }
                        continue 'outer_loop;
                    }
                }
                // Try dot-notation match: "bus.number" → match field "bus"
                if let Some(prefix) = name.split('.').next() {
                    for value in &mut *$variants {
                        if value.name == prefix {
                            // Append sub-field doc to the parent field
                            if let Some(ref mut doc) = value.meta.doc {
                                doc.push('\n');
                                doc.push_str(&format!(
                                    "  .{}: {}",
                                    &name[prefix.len() + 1..],
                                    desc.join(" ")
                                ));
                            }
                            continue 'outer_loop;
                        }
                    }
                }
                // Silently ignore doc fields that reference cross-type fields,
                // discriminated union branches, or are clearly doc-text references
                // (e.g., "blockdev-change-medium" referencing a command name)
            }
        }
    };
}

fn field_name_attr(name: &str) -> Option<Attribute> {
    let rust_name = rustify_field_name(name);
    if name != rust_name {
        Some(Attribute::with_value("name", name))
    } else {
        None
    }
}

fn name_attr(name: &str) -> Option<Attribute> {
    let rust_name = rustify_name(name);
    if name != rust_name {
        Some(Attribute::with_value("name", name))
    } else {
        None
    }
}

fn variant_name_attr(name: &str) -> Option<Attribute> {
    (name != rustify_variant(name)).then(|| Attribute::with_value("name", name))
}

fn process_type_ref<'a>(q: &'a QapiTypeRef<'a>) -> (&'a str, bool) {
    match q {
        QapiTypeRef::Unset => unreachable! {"this should have failed the parser"},
        QapiTypeRef::Ref(v) => (v, false),
        QapiTypeRef::ArrayRef(v) => (v, true),
        QapiTypeRef::InlineStruct(_) => {
            panic!(
                "InlineStruct should be handled by process_member_with_inline, not process_type_ref"
            )
        }
    }
}

fn process_alternative(q: QapiAlternative) -> EnumVariant {
    let (r#type, array) = process_type_ref(&q.r#type);

    let mut meta = Metadata::default();
    if let Some(attr) = variant_name_attr(q.name) {
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
    if let Some(attr) = name_attr(q.name) {
        meta.attributes.push(attr);
    }
    if let Some(condition) = q.r#if {
        meta.attributes
            .push(Attribute::with_value("condition", condition));
    }
    add_feat! {meta, q.features};
    add_docs! {meta, q.doc, q.name, &mut variants};

    Enum {
        name: q.name.into(),
        variants,
        meta,
    }
}

fn process_enum_value(q: QapiEnumValue) -> EnumVariant {
    let mut meta = Metadata::default();
    if let Some(attr) = variant_name_attr(q.name) {
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
    let (r#type, array) = process_type_ref(&q.r#type);

    let mut meta = Metadata::default();
    if let Some(attr) = field_name_attr(q.name) {
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

/// Process a member that may contain an inline struct definition.
/// Return the field and all recursively extracted anonymous structs.
fn process_member_with_inline(
    q: QapiMember,
    parent_name: &str,
    names: &mut HashSet<String>,
) -> (StructField, Vec<Struct>) {
    if let QapiTypeRef::InlineStruct(members) = q.r#type {
        // Generate a name: ParentNameFieldName (PascalCase)
        let preferred = format!("{}{}", rustify_name(parent_name), rustify_name(q.name));
        let mut inline_name = preferred.clone();
        let mut suffix = 0;
        while !names.insert(inline_name.clone()) {
            suffix += 1;
            inline_name = if suffix == 1 {
                format!("{preferred}Inline")
            } else {
                format!("{preferred}Inline{suffix}")
            };
        }
        let mut fields = Vec::new();
        let mut extracted = Vec::new();
        for member in members {
            let (field, nested) = process_member_with_inline(member, &inline_name, names);
            fields.push(field);
            extracted.extend(nested);
        }
        extracted.push(Struct {
            name: inline_name.clone(),
            fields,
            meta: Metadata::default(),
        });

        let mut meta = Metadata::default();
        if let Some(attr) = field_name_attr(q.name) {
            meta.attributes.push(attr);
        }
        if let Some(condition) = q.r#if {
            meta.attributes
                .push(Attribute::with_value("condition", condition));
        }
        add_feat! {meta, q.features};
        let field = StructField {
            name: q.name.into(),
            meta,
            r#type: inline_name,
            optional: q.optional,
            array: false,
        };
        (field, extracted)
    } else {
        (process_member(q), Vec::new())
    }
}

/// Process a struct, returning the struct and any inline structs extracted from its members.
pub fn process_struct(
    q: QapiStruct,
    structs_lookup: &HashMap<String, Struct>,
) -> (Struct, Vec<Struct>) {
    let mut names = structs_lookup
        .keys()
        .map(|name| rustify_name(name))
        .collect();
    process_struct_with_names(q, structs_lookup, &mut names)
}

pub(crate) fn process_struct_with_names(
    q: QapiStruct,
    structs_lookup: &HashMap<String, Struct>,
    names: &mut HashSet<String>,
) -> (Struct, Vec<Struct>) {
    names.insert(rustify_name(q.name));
    let mut fields = Vec::new();
    let mut extracted = Vec::new();
    if let Some(base) = q.base {
        let struct_ref = structs_lookup
            .get(base)
            .unwrap_or_else(|| panic!("base struct not found for {} with base {}", q.name, base));
        fields.extend(struct_ref.fields.clone());
    }
    for member in q.data {
        let (field, inline) = process_member_with_inline(member, q.name, names);
        fields.push(field);
        extracted.extend(inline);
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

    (
        Struct {
            name: q.name.into(),
            fields,
            meta,
        },
        extracted,
    )
}

/// Lower a tagged union; legacy untagged unions are dispatched as alternates.
pub fn process_union(
    q: QapiUnion,
    structs_lookup: &HashMap<String, Struct>,
    enums_lookup: &HashMap<String, Enum>,
) -> (Struct, Enum, Option<Enum>) {
    let is_simple = q.discriminator.is_none();
    let q_name = q.name.to_string();

    let mut fields = if q.base == MembersOrRef::Unset {
        Vec::new()
    } else {
        process_members_or_ref(q.base, structs_lookup)
    };

    if !is_simple {
        let disc_name = q.discriminator.as_ref().unwrap();
        let mut discriminator_opt = None;
        for field in &mut fields {
            if field.name == *disc_name {
                field.meta.attributes.push(Attribute::new("discriminator"));
                discriminator_opt = Some(field.r#type.clone());
            }
        }
        let discriminator = discriminator_opt.expect("discriminator field not found");
        if enums_lookup.get(&discriminator).is_none() {
            eprintln!(
                "WARNING: {} discriminator type '{}' is not an enum",
                q.name, discriminator
            );
        }
    } else {
        // Simple union: synthesize a 'type' discriminator field
        let type_name = format!("{}Type", rustify_name(q.name));
        let mut meta = Metadata::default();
        meta.attributes.push(Attribute::new("discriminator"));
        meta.attributes.push(Attribute::with_value("name", "type"));
        fields.push(StructField {
            name: "r#type".into(),
            meta,
            r#type: type_name,
            optional: false,
            array: false,
        });
    }

    let mut variants = Vec::new();
    for branch in q.data {
        let (r#type, array) = process_type_ref(&branch.r#type);
        let mut meta = Metadata::default();
        if let Some(attr) = variant_name_attr(branch.name) {
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
            array,
        });
    }
    if !is_simple {
        // QAPI flat unions implicitly select an empty branch for valid
        // discriminator enum values omitted from `data` (QEMU's
        // docs/devel/qapi-code-gen.rst, "Union types"). Model these explicitly
        // rather than treating valid tags such as CpuInfoFast.x86_64 as unknown.
        // The typed discriminator still refuses values outside its own enum.
        let discriminator = fields
            .iter()
            .find(|field| field.name == q.discriminator.unwrap())
            .expect("discriminator field not found");
        if let Some(tags) = enums_lookup.get(&discriminator.r#type) {
            for tag in &tags.variants {
                if !variants.iter().any(|branch| branch.name == tag.name) {
                    variants.push(EnumVariant {
                        name: tag.name.clone(),
                        kind: EnumVariantKind::Unit,
                        meta: tag.meta.clone(),
                        array: false,
                    });
                }
            }
        }
    }
    // Keep the synthesized branch's peer-build condition with its union.
    let mut branch_meta = Metadata::default();
    branch_meta.attributes.push(Attribute::new("union_branch"));
    if let Some(condition) = &q.r#if {
        branch_meta
            .attributes
            .push(Attribute::with_value("condition", condition));
    }
    let e = Enum {
        name: q.name.to_owned() + "Branch",
        variants,
        meta: branch_meta,
    };

    let mut meta = Metadata::default();
    meta.attributes.push(Attribute::new("union"));
    // Discriminated union variant payloads must be serialized inline at the
    // same JSON level as the discriminator. Without `flatten`, the variant
    // would nest under "u" and QEMU rejects with "Parameter X is missing".
    meta.attributes.push(Attribute::new("flatten"));
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
    if let Some(condition) = &q.r#if {
        meta.attributes
            .push(Attribute::with_value("condition", condition));
    }
    add_feat! {meta, q.r#features};
    add_docs! {meta, q.doc, q.name, &mut fields};
    if is_simple {
        meta.attributes.push(Attribute::new("simple_union"));
    }
    let s = Struct {
        name: q.name.into(),
        fields,
        meta,
    };
    // For simple unions, generate a discriminator type enum with unit variants
    let disc_enum = if is_simple {
        let disc_variants = e
            .variants
            .iter()
            .map(|v| {
                let mut meta = Metadata::default();
                if let Some(attr) = variant_name_attr(&v.name) {
                    meta.attributes.push(attr);
                }
                EnumVariant {
                    name: v.name.clone(),
                    kind: EnumVariantKind::Unit,
                    meta,
                    array: false,
                }
            })
            .collect();
        let mut disc_meta = Metadata::default();
        if let Some(condition) = &q.r#if {
            disc_meta
                .attributes
                .push(Attribute::with_value("condition", condition));
        }
        Some(Enum {
            name: format!("{}Type", rustify_name(&q_name)),
            variants: disc_variants,
            meta: disc_meta,
        })
    } else {
        None
    };

    (s, e, disc_enum)
}

/// Resolve a `MembersOrRef` to a flat list of struct fields. When the source
/// is a `Ref(name)`, the referenced struct's fields are inlined (duplicated)
/// directly into the parent so that QMP wire serialization produces a flat
/// object without needing serde-flatten indirection.
fn process_members_or_ref(
    q: MembersOrRef,
    structs_lookup: &HashMap<String, Struct>,
) -> Vec<StructField> {
    let mut fields = Vec::new();
    match q {
        MembersOrRef::Unset => unreachable!("this should have failed the parser"),
        MembersOrRef::Members(v) => {
            for field in v {
                fields.push(process_member(field));
            }
        }
        MembersOrRef::Ref(v) => {
            let struct_ref = structs_lookup
                .get(v)
                .unwrap_or_else(|| panic!("failed to lookup base ref for {}", v));
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
    meta.attributes.push(Attribute::new("event"));
    // Always store the original wire name for events
    meta.attributes.push(Attribute::with_value("name", q.name));
    if let Some(condition) = q.r#if {
        meta.attributes
            .push(Attribute::with_value("condition", condition));
    }
    add_feat! {meta, q.r#features};
    add_docs! {meta, q.doc, q.name, &mut fields};

    // Suffix event struct names with "Event" to avoid name collisions with commands
    let event_name = format!("{}Event", q.name);
    Struct {
        name: event_name,
        fields,
        meta,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{QapiEnumValue, QapiFeature, QapiFeatures, QapiMembers, QapiTypeRef};

    fn make_enum_value(name: &str) -> QapiEnumValue<'_> {
        QapiEnumValue {
            name,
            r#if: None,
            features: None,
        }
    }

    #[test]
    fn historical_simple_union_arrays_lower_without_panicking() {
        crate::set_parser_version((2, 0, 0));
        let (_, input) =
            QapiUnion::parse("{ 'union': 'Example', 'data': {'text': 'str', 'values': ['int']} }")
                .unwrap();
        let (value, branches, _) = process_union(input, &HashMap::new(), &HashMap::new());
        assert!(
            value
                .meta
                .attributes
                .contains(&Attribute::new("simple_union"))
        );
        let array = branches
            .variants
            .iter()
            .find(|variant| variant.name == "values")
            .unwrap();
        assert!(array.array);
        assert!(branches.generate().to_string().contains("Vec < i64 >"));
    }

    #[test]
    fn member_comments_remove_syntax_indent_but_keep_nested_literal_indent() {
        assert_eq!(
            member_documentation(&["intro", "    continuation", "", "        code", "    after"]),
            "intro\ncontinuation\n\n    code\nafter"
        );
        assert_eq!(
            member_documentation(&["    new-line description", "    continuation"]),
            "new-line description\ncontinuation"
        );
    }

    #[test]
    fn test_process_enum_simple() {
        let q = QapiEnum {
            name: "StatusCode",
            data: vec![make_enum_value("ok"), make_enum_value("error")].into(),
            prefix: None,
            r#if: None,
            features: None,
            doc: None,
        };
        let e = process_enum(q);
        assert_eq!(e.name, "StatusCode");
        assert_eq!(e.variants.len(), 2);
        assert_eq!(e.variants[0].name, "ok");
        assert_eq!(e.variants[0].kind, EnumVariantKind::Unit);
        assert_eq!(e.variants[1].name, "error");
    }

    #[test]
    fn test_process_enum_with_features() {
        let q = QapiEnum {
            name: "MyEnum",
            data: vec![QapiEnumValue {
                name: "deprecated-val",
                r#if: None,
                features: Some(QapiFeatures(vec![QapiFeature {
                    name: "deprecated",
                    r#if: None,
                }])),
            }]
            .into(),
            prefix: None,
            r#if: None,
            features: None,
            doc: None,
        };
        let e = process_enum(q);
        assert_eq!(e.variants.len(), 1);
        let feat_attrs: Vec<_> = e.variants[0]
            .meta
            .attributes
            .iter()
            .filter(|a| matches!(a, Attribute::List(_)))
            .collect();
        assert!(!feat_attrs.is_empty());
    }

    #[test]
    fn test_process_struct_simple() {
        let q = QapiStruct {
            name: "MyStruct",
            data: QapiMembers(vec![QapiMember {
                name: "field-name",
                optional: false,
                r#type: QapiTypeRef::Ref("str"),
                r#if: None,
                features: None,
            }]),
            base: None,
            r#if: None,
            features: None,
            doc: None,
        };
        let (s, _) = process_struct(q, &HashMap::new());
        assert_eq!(s.name, "MyStruct");
        assert_eq!(s.fields.len(), 1);
        assert_eq!(s.fields[0].name, "field-name");
        assert_eq!(s.fields[0].r#type, "str");
        assert!(!s.fields[0].optional);
    }

    #[test]
    fn test_process_struct_with_base() {
        let mut lookup = HashMap::new();
        lookup.insert(
            "BaseStruct".to_string(),
            Struct {
                name: "BaseStruct".into(),
                fields: vec![StructField {
                    name: "base-field".into(),
                    r#type: "int".into(),
                    optional: false,
                    array: false,
                    meta: Metadata::default(),
                }],
                meta: Metadata::default(),
            },
        );

        let q = QapiStruct {
            name: "Child",
            data: QapiMembers(vec![QapiMember {
                name: "extra",
                optional: true,
                r#type: QapiTypeRef::Ref("bool"),
                r#if: None,
                features: None,
            }]),
            base: Some("BaseStruct"),
            r#if: None,
            features: None,
            doc: None,
        };
        let (s, _) = process_struct(q, &lookup);
        assert_eq!(s.name, "Child");
        assert_eq!(s.fields.len(), 2);
        assert_eq!(s.fields[0].name, "base-field");
        assert_eq!(s.fields[1].name, "extra");
        assert!(s.fields[1].optional);
    }

    #[test]
    fn test_process_member_optional_and_array() {
        let m = QapiMember {
            name: "items",
            optional: true,
            r#type: QapiTypeRef::ArrayRef("SomeType"),
            r#if: None,
            features: None,
        };
        let f = process_member(m);
        assert_eq!(f.name, "items");
        assert!(f.optional);
        assert!(f.array);
        assert_eq!(f.r#type, "SomeType");
    }

    #[test]
    fn test_process_struct_with_inline_struct() {
        // Simulates VersionInfo.qemu from v1.1.0:
        // { 'type': 'VersionInfo', 'data': { 'qemu': {'major': 'int', 'minor': 'int', 'micro': 'int'}, ... } }
        let q = QapiStruct {
            name: "VersionInfo",
            data: QapiMembers(vec![
                QapiMember {
                    name: "qemu",
                    optional: false,
                    r#type: QapiTypeRef::InlineStruct(QapiMembers(vec![
                        QapiMember {
                            name: "major",
                            optional: false,
                            r#type: QapiTypeRef::Ref("int"),
                            r#if: None,
                            features: None,
                        },
                        QapiMember {
                            name: "minor",
                            optional: false,
                            r#type: QapiTypeRef::Ref("int"),
                            r#if: None,
                            features: None,
                        },
                        QapiMember {
                            name: "micro",
                            optional: false,
                            r#type: QapiTypeRef::Ref("int"),
                            r#if: None,
                            features: None,
                        },
                    ])),
                    r#if: None,
                    features: None,
                },
                QapiMember {
                    name: "package",
                    optional: true,
                    r#type: QapiTypeRef::Ref("str"),
                    r#if: None,
                    features: None,
                },
            ]),
            base: None,
            r#if: None,
            features: None,
            doc: None,
        };
        let (s, extracted) = process_struct(q, &HashMap::new());
        assert_eq!(s.name, "VersionInfo");
        assert_eq!(s.fields.len(), 2);
        // The inline struct field should now reference the extracted type
        assert_eq!(s.fields[0].name, "qemu");
        assert_eq!(s.fields[0].r#type, "VersionInfoQemu");
        assert!(!s.fields[0].optional);
        assert_eq!(s.fields[1].name, "package");

        // One inline struct should have been extracted
        assert_eq!(extracted.len(), 1);
        assert_eq!(extracted[0].name, "VersionInfoQemu");
        assert_eq!(extracted[0].fields.len(), 3);
        assert_eq!(extracted[0].fields[0].name, "major");
        assert_eq!(extracted[0].fields[0].r#type, "int");
    }

    #[test]
    fn test_process_command_simple() {
        let q = QapiCommand {
            name: "query-status",
            data: Some(MembersOrRef::Members(QapiMembers(vec![]))),
            returns: Some(QapiTypeRef::Ref("StatusInfo")),
            r#if: None,
            features: None,
            doc: None,
            success_response: None,
            r#gen: None,
            boxed: None,
            allow_oob: None,
            allow_preconfig: None,
            coroutine: None,
        };
        let s = process_command(q, &HashMap::new());
        assert_eq!(s.name, "query-status");
        let returns_attr = s
            .meta
            .attributes
            .iter()
            .find(|a| matches!(a, Attribute::Item { name, .. } if name == "returns"));
        assert!(returns_attr.is_some());
    }

    #[test]
    fn test_process_command_with_flags() {
        let q = QapiCommand {
            name: "test-cmd",
            data: None,
            returns: None,
            r#if: None,
            features: None,
            doc: None,
            success_response: Some("false"),
            r#gen: None,
            boxed: None,
            allow_oob: Some("true"),
            allow_preconfig: Some("true"),
            coroutine: None,
        };
        let s = process_command(q, &HashMap::new());
        let attr_names: Vec<_> = s
            .meta
            .attributes
            .iter()
            .filter_map(|a| match a {
                Attribute::Unit(n) => Some(n.as_str()),
                _ => None,
            })
            .collect();
        assert!(attr_names.contains(&"allow_oob"));
        assert!(attr_names.contains(&"allow_preconfig"));
        assert!(attr_names.contains(&"no_success_response"));
    }

    #[test]
    fn test_process_alternate() {
        let q = QapiAlternate {
            name: "StrOrInt",
            data: crate::QapiAlternatives(vec![
                crate::QapiAlternative {
                    name: "s",
                    r#type: QapiTypeRef::Ref("str"),
                    r#if: None,
                },
                crate::QapiAlternative {
                    name: "i",
                    r#type: QapiTypeRef::Ref("int"),
                    r#if: None,
                },
            ]),
            r#if: None,
            features: None,
            doc: None,
        };
        let e = process_alternate(q);
        assert_eq!(e.name, "StrOrInt");
        assert_eq!(e.variants.len(), 2);
        assert_eq!(e.variants[0].kind, EnumVariantKind::Tuple("str".into()));
        assert_eq!(e.variants[1].kind, EnumVariantKind::Tuple("int".into()));
    }

    #[test]
    fn test_enum_codegen_roundtrip() {
        let q = QapiEnum {
            name: "TestEnum",
            data: vec![make_enum_value("value-one"), make_enum_value("value-two")].into(),
            prefix: None,
            r#if: None,
            features: None,
            doc: None,
        };
        let e = process_enum(q);
        let tokens = e.generate();
        let code = prettyplease::unparse(
            &syn::parse2(tokens).expect("generated tokens should be valid Rust"),
        );
        assert!(code.contains("pub enum TestEnum"));
        assert!(code.contains("ValueOne"));
        assert!(code.contains("ValueTwo"));
    }

    #[test]
    fn test_struct_codegen_roundtrip() {
        let q = QapiStruct {
            name: "MyData",
            data: QapiMembers(vec![
                QapiMember {
                    name: "name",
                    optional: false,
                    r#type: QapiTypeRef::Ref("str"),
                    r#if: None,
                    features: None,
                },
                QapiMember {
                    name: "count",
                    optional: true,
                    r#type: QapiTypeRef::Ref("int"),
                    r#if: None,
                    features: None,
                },
            ]),
            base: None,
            r#if: None,
            features: None,
            doc: None,
        };
        let (s, _) = process_struct(q, &HashMap::new());
        let tokens = s.generate();
        let code = prettyplease::unparse(
            &syn::parse2(tokens).expect("generated tokens should be valid Rust"),
        );
        assert!(code.contains("pub struct MyData"));
        assert!(code.contains("pub name: String"));
        assert!(code.contains("pub count: Option<i64>"));
    }
    #[test]
    fn flat_union_synthesizes_only_known_implicit_empty_discriminator_branches() {
        let (_,q)=QapiUnion::parse_v3("{'union':'Cpu','base':{'target':'Target'},'discriminator':'target','data':{'s390x':'S390Details'}}").unwrap();
        let mut tags = process_enum(QapiEnum {
            name: "Target",
            data: vec![
                make_enum_value("x86_64"),
                make_enum_value("aarch64"),
                make_enum_value("s390x"),
            ],
            ..Default::default()
        });
        tags.variants[1]
            .meta
            .attributes
            .push(Attribute::with_value("condition", "TARGET_AARCH64"));
        let (_, branches, _) = process_union(
            q,
            &HashMap::new(),
            &HashMap::from([("Target".into(), tags)]),
        );
        assert_eq!(branches.variants.len(), 3);
        assert_eq!(
            branches
                .variants
                .iter()
                .find(|v| v.name == "x86_64")
                .unwrap()
                .kind,
            EnumVariantKind::Unit
        );
        assert_eq!(
            branches
                .variants
                .iter()
                .find(|v| v.name == "s390x")
                .unwrap()
                .kind,
            EnumVariantKind::Tuple("S390Details".into())
        );
        assert!(
            branches
                .variants
                .iter()
                .find(|v| v.name == "aarch64")
                .unwrap()
                .meta
                .attributes
                .contains(&Attribute::with_value("condition", "TARGET_AARCH64"))
        );
        assert!(branches.variants.iter().all(|v| v.name != "unknown"));
    }
}

pub fn process_command(q: QapiCommand, structs_lookup: &HashMap<String, Struct>) -> Struct {
    let mut fields = Vec::new();
    if let Some(data) = q.data {
        fields.extend(process_members_or_ref(data, structs_lookup));
    }
    let mut meta = Metadata::default();
    meta.attributes.push(Attribute::new("command"));
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
        let (r#type, array) = process_type_ref(&returns);
        let rust_type = super::rustify_type(r#type);
        let r#type = if array {
            format!("Vec<{}>", rust_type)
        } else {
            rust_type
        };
        meta.attributes
            .push(Attribute::with_value("returns", r#type));
    } else {
        meta.attributes.push(Attribute::with_value("returns", "()"));
    }
    if q.allow_oob == Some("true") {
        meta.attributes.push(Attribute::new("allow_oob"));
    }
    if q.allow_preconfig == Some("true") {
        meta.attributes.push(Attribute::new("allow_preconfig"));
    }
    if q.success_response == Some("false") {
        meta.attributes.push(Attribute::new("no_success_response"));
    }

    // Commands with 'gen': false accept arbitrary additional properties
    // beyond the declared fields (e.g., device_add takes device-specific props).
    // Add a flattened HashMap catch-all field to support this.
    // The field defaults to an empty map so callers don't need to specify it.
    if q.r#gen == Some("false") || q.r#gen == Some("no") {
        fields.push(StructField {
            name: "arguments".into(),
            r#type: "HashMap<String, serde_json::Value>".into(),
            optional: true,
            array: false,
            meta: {
                let mut m = Metadata::default();
                m.doc = Some("Additional command arguments not modeled by this schema.".into());
                m.attributes.push(Attribute::new("flatten"));
                m
            },
        });
    }

    Struct {
        name: q.name.into(),
        fields,
        meta,
    }
}
