mod codegen;
use codegen::*;

mod helpers;
mod parsers;
pub use parsers::{
    qapi_alternate::{take_alternate, QapiAlternate},
    qapi_alternative::{take_alternative, QapiAlternative},
    qapi_alternatives::{take_alternatives, QapiAlternatives},
    qapi_branch::{take_branch, QapiBranch},
    qapi_branches::{take_branches, QapiBranches},
    qapi_command::{take_command, QapiCommand},
    qapi_cond::{take_cond, QapiCond},
    qapi_documentation::{QapiDocumentation, QapiSectionDocumentation},
    qapi_enum::{take_enum, QapiEnum},
    qapi_enum_value::{take_enum_value, QapiEnumValue},
    qapi_event::{take_event, QapiEvent},
    qapi_feature::{take_feature, QapiFeature},
    qapi_features::{take_features, QapiFeatures},
    qapi_include::{take_include, QapiInclude},
    qapi_member::{take_member, QapiMember},
    qapi_members::{take_members, take_members_or_ref, MembersOrRef, QapiMembers},
    qapi_pragma::{take_pragma, QapiPragma},
    qapi_schema::{take_schema, QapiSchema, QapiSchemaToken},
    qapi_struct::{take_struct, QapiStruct},
    qapi_type_ref::{take_type_ref, QapiTypeRef},
    qapi_union::{take_union, QapiUnion},
};

use crate::parsers::qapi_documentation::docstr_to_string;
use crate::parsers::qapi_documentation::docstr_to_string_keep_structure;
use anyhow::Result;
use heck::{ToPascalCase, ToSnakeCase};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

fn fix_leading_digit(input: &str) -> String {
    if input
        .chars()
        .next()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(false)
    {
        format!("_{}", input)
    } else {
        input.to_string()
    }
}

pub(crate) fn rustify_type(input: &str) -> String {
    match input {
        "str" => "String".into(),
        "number" => "f64".into(),
        "int" => "i64".into(),
        "int8" => "i8".into(),
        "int16" => "i16".into(),
        "int32" => "i32".into(),
        "int64" => "i64".into(),
        "uint8" => "u8".into(),
        "uint16" => "u16".into(),
        "uint32" => "u32".into(),
        "uint64" => "u64".into(),
        "size" => "u64".into(),
        "bool" => "bool".into(),
        "null" => "FIXME_null_type".into(),
        "any" => "FIXME_any_type".into(),
        _ => fix_leading_digit(&input.to_pascal_case()),
    }
}

pub(crate) fn rustify_field(input: &str) -> String {
    let input = match input {
        "type" | "abstract" | "in" | "static" | "if" | "match" => format! {"r#{}", input},
        _ => input.to_string(),
    };
    input.to_snake_case()
}

macro_rules! add_name {
    ($meta:expr, $name:expr) => {
        $meta.attributes.push(Attribute::new("name", Some($name)));
    };
}

macro_rules! add_cond {
    ($meta:expr, $cond:expr) => {
        if let Some(condition) = $cond {
            $meta.attributes.push(Attribute::new(
                "condition",
                Some(&condition.recursive_print()),
            ));
        }
    };
}

macro_rules! add_feat {
    ($meta:expr, $feat:expr) => {
        if let Some(features) = $feat {
            for feature in features {
                let name = feature.name;
                if let Some(condition) = &feature.r#if {
                    $meta.attributes.push(Attribute::new(
                        format!("feature_{}_if", name),
                        Some(condition.recursive_print()),
                    ));
                }
                $meta.attributes.push(Attribute::new("feature", Some(name)));
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
            if let Some(desc) = doc.description {
                $meta.doc = Some(docstr_to_string_keep_structure(desc));
            }
            if let Some(since) = doc.since {
                $meta.attributes.push(Attribute::new("since", Some(since)));
            }
            'outer_loop: for (name, desc) in &doc.fields {
                for mut value in $variants {
                    if &value.name == name {
                        value.meta.doc = Some(docstr_to_string_keep_structure(*desc));
                        continue 'outer_loop;
                    }
                }
                todo! { "This is a validation error; Documented field does not exist" }
            }
        }
    };
}

fn parse_schema(input: &str) -> Result<QapiSchema<'_>> {
    let (_, schema) = take_schema(input).unwrap();
    Ok(schema)
}

fn read_schema(schema_file: &Path) -> Result<Vec<(PathBuf, String)>> {
    let mut sources = Vec::new();
    read_schema_file(&schema_file, &mut sources)?;
    sources.rotate_left(1); // move the initial file to the end of the stack
    Ok(sources)
}

fn read_schema_file(schema_file: &Path, sources: &mut Vec<(PathBuf, String)>) -> Result<()> {
    if sources.iter().any(|(path, _)| path == schema_file) {
        // we have already seen this file
        return Ok(());
    }
    let content = std::fs::read_to_string(&schema_file)?;
    sources.push((schema_file.to_path_buf(), content.clone()));
    let schema_tokens = parse_schema(&content)?;
    let includes: Vec<&str> = schema_tokens
        .into_iter()
        .filter_map(|token| match token {
            QapiSchemaToken::Include(filename) => Some(filename.0),
            _ => None,
        })
        .collect();
    for include in includes {
        let include_path = schema_file.parent().unwrap().join(include);
        read_schema_file(&include_path, sources)?;
    }
    Ok(())
}

fn main() -> Result<()> {
    let sort = true;
    let qemu_src_root = Path::new("/home/sam/repos/qemu");
    let schema_type = "qga";
    let schema_type = "storage-daemon/qapi";
    let schema_type = "qapi";
    let schema_file = qemu_src_root.join(schema_type).join("qapi-schema.json");
    let schema_file = qemu_src_root.join(schema_type).join("common.json");

    let b = std::fs::read_to_string(&schema_file)?;
    let tokens = parse_schema(&b)?;

    //  // We start with the provided root schema file then recursively parse the
    //  // includes and load the strings in the `sources` Vec.
    //  let sources = read_schema(&schema_file)?;

    //  // Here the we _reparse_ the String elements in the vec. This is the only
    //  // way I could figure out lifetimes while still keeping &str references to
    //  // the original String.
    //  let mut tokens: Vec<QapiSchemaToken> = Vec::new();
    //  for (path, source) in &sources {
    //      tokens.extend(parse_schema(source)?.0);
    //  }

    let mut alternates: HashMap<&str, QapiAlternate> = HashMap::new();
    let mut commands: HashMap<&str, QapiCommand> = HashMap::new();
    let mut enums: HashMap<&str, QapiEnum> = HashMap::new();
    let mut events: HashMap<&str, QapiEvent> = HashMap::new();
    let mut structs: HashMap<&str, QapiStruct> = HashMap::new();
    let mut unions: HashMap<&str, QapiUnion> = HashMap::new();
    dbg![&tokens];
    for token in tokens {
        match token {
            QapiSchemaToken::Alternate(v) => {
                alternates.insert(v.name, v);
            }
            QapiSchemaToken::Command(v) => {
                commands.insert(v.name, v);
            }
            QapiSchemaToken::Enum(v) => {
                enums.insert(v.name, v);
            }
            QapiSchemaToken::Event(v) => {
                events.insert(v.name, v);
            }
            QapiSchemaToken::Struct(v) => {
                structs.insert(v.name, v);
            }
            QapiSchemaToken::Union(v) => {
                unions.insert(v.name, v);
            }
            _ => continue,
        }
    }

    let mut schema = Schema::default();
    for (name, v) in enums.into_iter() {
        let mut meta = Metadata::default();
        meta.attributes.push(Attribute::new("name", Some(name)));
        add_cond! {meta, v.r#if};
        add_feat! {meta, v.r#features};

        // Convert variants
        let mut variants = Vec::new();
        for qenumvalue in v.data {
            let name = qenumvalue.name;
            let mut meta = Metadata::default();
            meta.attributes.push(Attribute::new("name", Some(name)));
            add_cond! {meta, qenumvalue.r#if};
            add_feat! {meta, qenumvalue.r#features};
            let variant = EnumVariant {
                name: name.into(),
                kind: EnumVariantKind::Unit,
                meta,
                array: false,
            };
            variants.push(variant);
        }

        add_docs! {meta, v.doc, name, &mut variants};

        // ASSEMBLE!
        let qenum = Enum {
            name: name.into(),
            variants,
            meta,
        };
        schema.enums.push(qenum);
    }

    for (name, v) in alternates.into_iter() {
        let mut meta = Metadata::default();
        meta.attributes.push(Attribute::new("name", Some(name)));
        add_cond! {meta, v.r#if};
        add_feat! {meta, v.r#features};

        // Convert variants
        let mut variants = Vec::new();
        for qaltvalue in v.data {
            let name = qaltvalue.name;
            let mut array = false;
            let r#type = match qaltvalue.r#type {
                QapiTypeRef::Unset => unreachable! {"this should have failed the parser"},
                QapiTypeRef::Ref(v) => v.into(),
                QapiTypeRef::ArrayRef(v) => {
                    array = true;
                    v.into()
                }
            };
            let mut meta = Metadata::default();
            meta.attributes.push(Attribute::new("name", Some(name)));
            add_cond! {meta, qaltvalue.r#if};
            let variant = EnumVariant {
                name: name.into(),
                kind: EnumVariantKind::Tuple(r#type),
                meta,
                array,
            };
            variants.push(variant);
        }

        add_docs! {meta, v.doc, name, &mut variants};

        // ASSEMBLE!
        let qalternate = Enum {
            name: name.into(),
            variants,
            meta,
        };
        schema.alternates.push(qalternate);
    }
    for (name, v) in structs.into_iter() {
        let name = v.name;
        let mut meta = Metadata::default();
        meta.attributes.push(Attribute::new("name", Some(name)));
        add_cond! {meta, v.r#if};
        add_feat! {meta, v.r#features};

        // Convert variants
        let mut fields = Vec::new();
        for field in v.data {
            let name = field.name;
            let optional = field.optional;
            let mut array = false;
            let r#type = match field.r#type {
                QapiTypeRef::Unset => unreachable! {"this should have failed the parser"},
                QapiTypeRef::Ref(v) => v.into(),
                QapiTypeRef::ArrayRef(v) => {
                    array = true;
                    v.into()
                }
            };
            let mut meta = Metadata::default();
            meta.attributes.push(Attribute::new("name", Some(name)));
            add_cond! {meta, field.r#if};
            add_feat! {meta, field.features};
            let field = StructField {
                name: name.into(),
                meta,
                r#type,
                optional,
                array,
            };
            fields.push(field);
        }

        add_docs! {meta, v.doc, name, &mut fields};

        // ASSEMBLE!
        let qstruct = Struct {
            name: name.into(),
            fields,
            meta,
        };
        schema.structs.push(qstruct);
    }
    //for (name, v) in unions.into_iter() {
    //    let name = v.name.expect("name is missing");
    //    let mut meta = Metadata::default();
    //    add_name! {meta, name};
    //    add_cond! {meta, &v.r#if};
    //    add_feat! {meta, &v.r#features};

    //    // Convert variants
    //    let mut fields = Vec::new();
    //    if let Some(base) = v.base.clone() {
    //        let base = match base {
    //            MembersOrRef::Members(v) => v,
    //            MembersOrRef::Ref(v) => {
    //                let struct_ref = structs.get(v).unwrap();
    //                if struct_ref.base.is_some() {
    //                    continue;
    //                };
    //                struct_ref.data.clone().unwrap()
    //            }
    //        };
    //        for field in base.0 {
    //            let name = field.name.expect("name is missing");
    //            let optional = field.optional;
    //            let mut array = false;
    //            let r#type = match field.r#type.expect("type is missing") {
    //                QapiTypeRef::Ref(v) => rustify_type(v),
    //                QapiTypeRef::ArrayRef(v) => {
    //                    array = true;
    //                    rustify_type(v)
    //                }
    //            };
    //            let mut meta = Metadata::default();
    //            add_name! {meta, name};
    //            add_cond! {meta, &field.r#if};
    //            add_feat! {meta, &field.features};
    //            let field = StructField {
    //                name: name.into(),
    //                r#type,
    //                meta,
    //                optional,
    //                array,
    //            };
    //            fields.push(field);
    //        }
    //    }

    //    add_docs! {meta, &v.doc, name, &mut fields};
    //    let fields = fields;

    //    let mut variants = Vec::new();
    //    for branch in v.data.clone().unwrap().0 {
    //        let name = branch.name.expect("name is missing");
    //        let mut array = false;
    //        let r#type = match branch.r#type.expect("type is missing") {
    //            QapiTypeRef::Ref(v) => rustify_type(v),
    //            QapiTypeRef::ArrayRef(v) => {
    //                array = true;
    //                rustify_type(v)
    //            }
    //        };
    //        let mut fields = fields.clone();
    //        // merge `type` into fields....
    //        let fields = fields;

    //        let mut meta = Metadata::default();
    //        add_name! {meta, name};
    //        add_cond! {meta, &branch.r#if};
    //        let variant = EnumVariant {
    //            name: name.into(),
    //            kind: EnumVariantKind::QapiUnion(fields),
    //            meta,
    //            array,
    //        };
    //        variants.push(variant);
    //    }

    //    // ASSEMBLE!
    //    let qunion = Enum {
    //        name: name.into(),
    //        variants,
    //        meta,
    //    };
    //    schema.unions.push(qunion);
    //}
    //for token in &tokens {
    //    match token {
    //        QapiSchemaToken::Struct(v) => {
    //            if let Some(name) = &v.base {
    //                let new_data = {
    //                    let s2 = structs.get(name).unwrap();
    //                    if s2.base.is_some() {
    //                            panic!{"asda"};
    //                    }
    //                    s2.data.clone().unwrap().0
    //                };
    //                let s1 = structs.get_mut(v.name.unwrap()).unwrap();
    //                let mut data = s1.data.clone().unwrap();
    //                data.0.extend(new_data);
    //                s1.data = Some(data);
    //                s1.base = None;
    //            };
    //        },
    //        _ => continue,
    //    };
    //}
    dbg![schema];
    //for (name, v) in commands.into_iter() {
    //    let name = v.name.expect("name is missing");
    //    let mut meta = Metadata::default();
    //    add_name! {meta, name};
    //    add_cond! {meta, &v.r#if};
    //    add_feat! {meta, &v.r#features};

    //    // Convert variants
    //    let mut fields = Vec::new();
    //    if let Some(data) = v.data.clone() {
    //        let data = match data {
    //            MembersOrRef::Members(v) => v,
    //            MembersOrRef::Ref(v) => {
    //                let struct_ref = structs.get(v).unwrap_or(continue);
    //                if struct_ref.base.is_some() {
    //                    continue;
    //                };
    //                struct_ref.data.clone().unwrap()
    //            }
    //        };
    //        for field in data.0 {
    //            let name = field.name.expect("name is missing");
    //            let optional = field.optional;
    //            let mut array = false;
    //            let r#type = match field.r#type.expect("type is missing") {
    //                QapiTypeRef::Ref(v) => rustify_type(v),
    //                QapiTypeRef::ArrayRef(v) => {
    //                    array = true;
    //                    rustify_type(v)
    //                }
    //            };
    //            let mut meta = Metadata::default();
    //            add_name! {meta, name};
    //            add_cond! {meta, &field.r#if};
    //            add_feat! {meta, &field.features};
    //            let field = StructField {
    //                name: name.into(),
    //                r#type,
    //                meta,
    //                optional,
    //                array,
    //            };
    //            fields.push(field);
    //        }
    //    } else {
    //        if let Some(boxed) = v.boxed.clone() {
    //            if boxed.parse().unwrap() {
    //                unimplemented! {"This is a validation issue in the qapi spec file. The data field is empty, but `boxed` is set to true"};
    //            }
    //        }
    //    }

    //    add_docs! {meta, &v.doc, name, &mut fields};

    //    // ASSEMBLE!
    //    let qcommand = Struct {
    //        name: name.into(),
    //        fields,
    //        meta,
    //    };
    //    schema.commands.push(qcommand);
    //}
    //for (name, v) in events.into_iter() {
    //    let name = v.name.expect("name is missing");
    //    let mut meta = Metadata::default();
    //    add_name! {meta, name};
    //    add_cond! {meta, &v.r#if};
    //    add_feat! {meta, &v.r#features};

    //    // Convert variants
    //    let mut fields = Vec::new();
    //    if let Some(data) = v.data.clone() {
    //        let data = match data {
    //            MembersOrRef::Members(v) => v,
    //            MembersOrRef::Ref(v) => {
    //                let struct_ref = structs.get(v).unwrap();
    //                if struct_ref.base.is_some() {
    //                    continue;
    //                };
    //                struct_ref.data.clone().unwrap()
    //            }
    //        };
    //        for field in data.0 {
    //            let name = field.name.expect("name is missing");
    //            let optional = field.optional;
    //            let mut array = false;
    //            let r#type = match field.r#type.expect("type is missing") {
    //                QapiTypeRef::Ref(v) => rustify_type(v),
    //                QapiTypeRef::ArrayRef(v) => {
    //                    array = true;
    //                    rustify_type(v)
    //                }
    //            };
    //            let mut meta = Metadata::default();
    //            add_name! {meta, name};
    //            add_cond! {meta, &field.r#if};
    //            add_feat! {meta, &field.features};
    //            let field = StructField {
    //                name: name.into(),
    //                r#type,
    //                meta,
    //                optional,
    //                array,
    //            };
    //            fields.push(field);
    //        }
    //    } else {
    //        if let Some(boxed) = v.boxed.clone() {
    //            if boxed.parse().unwrap() {
    //                unimplemented! {"This is a validation issue in the qapi spec file. The data field is empty, but `boxed` is set to true"};
    //            }
    //        }
    //    }

    //    add_docs! {meta, &v.doc, name, &mut fields};

    //    // ASSEMBLE!
    //    let qevent = Struct {
    //        name: name.into(),
    //        fields,
    //        meta,
    //    };
    //    schema.events.push(qevent);
    //}
    //for e in schema.alternates {
    //    dbg![e];
    //}
    //for (_, v) in &mut lookup {
    //    match v {
    //        QapiSchemaToken::Struct(ref mut v) => {
    //            if let Some(base) = v.base {
    //                if let Some(n) = lookup.get(base) {
    //                    match n {
    //                        QapiSchemaToken::Struct(nv) => {
    //                            if nv.base.is_some() {
    //                                panic!{"asda"};
    //                            }
    //                        },
    //                        _ => panic!{"panic debugging"},
    //                    }
    //                } else {
    //                    panic!("base not found");
    //                }
    //            }
    //        },
    //        _ => continue,
    //    }
    //}
    return Ok(());

    //   // read schema file start loop
    //   let mut sources = Vec::new();
    //   sources.push(std::fs::read_to_string(schema_file)?);
    //   let string = sources.last().unwrap();
    //   let schema_tokens = parse_schema(&string)?;
    //   let includes: Vec<String> = schema_tokens.0
    //       .into_iter()
    //       .filter_map(|token| {
    //           match token {
    //               QapiSchemaToken::Include(filename) => Some(filename.0.to_string()),
    //               _ => None,
    //           }
    //       })
    //       .collect();
    //   // [projects/qapi-spec-parser/src/main.rs:125:5] &includes = [
    //   //     "common.json",
    //   //     "crypto.json",
    //   //     "job.json",
    //   //     "sockets.json",
    //   // ]

    //   // loop over `includes` and GOTO `read schema file start loop`

    return Ok(());

    //let mut members = Vec::new();
    //for token in &mut schema_tokens {
    //    match token {
    //        QapiSchema::Struct(ref mut v) => {
    //            members.push(data)
    //            if let Some(b) = v.base {
    //                if let Some(ref mut data) = v.data {
    //                for t in &schema_tokens {
    //                    match t {
    //                        QapiSchema::Struct(new_v) => {
    //                            if new_v.name == v.name {
    //                                    data.0.extend(new_v.data.clone().expect("no data").0);
    //                                }
    //                        },
    //                        _ => {},
    //                    }
    //                }
    //                            }
    //                dbg![b];
    //            }
    //        },
    //        _ => {},
    //    }
    //}

    //for token in &schema_tokens {
    //    match token {
    //        QapiSchema::Struct(v) => {
    //            let name = v.name.expect("name is missing");
    //            let mut meta = Metadata::default();
    //            add_name! {meta, name};
    //            add_cond! {meta, &v.r#if};
    //            add_feat! {meta, &v.r#features};

    //            // Convert variants
    //            let mut fields = Vec::new();
    //            for field in v.data.clone().unwrap().0 {
    //                let name = field.name.expect("name is missing");
    //                let optional = field.optional;
    //                let mut array = false;
    //                let r#type = match field.r#type.expect("type is missing") {
    //                    QapiTypeRef::Ref(v) => v.into(),
    //                    QapiTypeRef::ArrayRef(v) => {
    //                        array = true;
    //                        v.into()
    //                    }
    //                };
    //                let mut meta = Metadata::default();
    //                add_name! {meta, name};
    //                add_cond! {meta, &field.r#if};
    //                add_feat! {meta, &field.features};
    //                let field = StructField {
    //                    name: name.into(),
    //                    r#type,
    //                    meta,
    //                    optional,
    //                    array,
    //                };
    //                fields.push(field);
    //            }

    //            add_docs! {meta, &v.doc, name, &mut fields};

    //            // ASSEMBLE!
    //            let qstruct = Struct {
    //                name: name.into(),
    //                fields,
    //                meta,
    //            };
    //            schema.structs.push(qstruct);
    //        }
    //        QapiSchema::Alternate(v) => {
    //            let name = v.name.expect("name is missing");
    //            let mut meta = Metadata::default();
    //            add_name! {meta, name};
    //            add_cond! {meta, &v.r#if};
    //            add_feat! {meta, &v.r#features};

    //            // Convert variants
    //            let mut variants = Vec::new();
    //            for qaltvalue in v.data.clone().unwrap().0 {
    //                let name = qaltvalue.name.expect("name is missing");
    //                let r#type = match qaltvalue.r#type.expect("type is missing") {
    //                    QapiTypeRef::Ref(v) => v.into(),
    //                    QapiTypeRef::ArrayRef(v) => format! {"Vec<{v}>"},
    //                };
    //                let mut meta = Metadata::default();
    //                add_name! {meta, name};
    //                add_cond! {meta, &qaltvalue.r#if};
    //                let variant = EnumVariant {
    //                    name: name.into(),
    //                    kind: EnumVariantKind::QapiAlternate(r#type),
    //                    meta,
    //                };
    //                variants.push(variant);
    //            }

    //            add_docs! {meta, &v.doc, name, &mut variants};

    //            // ASSEMBLE!
    //            let qalt = Enum {
    //                name: name.into(),
    //                variants,
    //                meta,
    //            };
    //            schema.alternates.push(qalt);
    //        }
    //        QapiSchema::Enum(v) => {
    //            let name = v.name.expect("name is missing");
    //            let mut meta = Metadata::default();
    //            add_name! {meta, name};
    //            add_cond! {meta, &v.r#if};
    //            add_feat! {meta, &v.r#features};

    //            // Convert variants
    //            let mut variants = Vec::new();
    //            for qenumvalue in &v.data {
    //                let name = qenumvalue.name.expect("name is missing");
    //                let mut meta = Metadata::default();
    //                add_name! {meta, name};
    //                add_cond! {meta, &qenumvalue.r#if};
    //                add_feat! {meta, &qenumvalue.r#features};
    //                let variant = EnumVariant {
    //                    name: name.into(),
    //                    kind: EnumVariantKind::QapiEnum,
    //                    meta,
    //                };
    //                variants.push(variant);
    //            }

    //            add_docs! {meta, &v.doc, name, &mut variants};

    //            // ASSEMBLE!
    //            let qenum = Enum {
    //                name: name.into(),
    //                variants,
    //                meta,
    //            };
    //            schema.enums.push(qenum);
    //        }
    //        _ => {}
    //    }
    //}
    //if sort {
    //    schema.enums.sort();
    //    for e in &mut schema.enums {
    //        e.variants.sort();
    //    }
    //}
    //for e in &schema.alternates {
    //    let code = e.generate(QapiType::Alternate);
    //    let syntree: syn::File = syn::parse2(code)?;
    //    let prettycode = prettyplease::unparse(&syntree);
    //    println!("{}", prettycode);
    //}
    //for e in &schema.enums {
    //    let code = e.generate(QapiType::Enum);
    //    let syntree: syn::File = syn::parse2(code)?;
    //    let prettycode = prettyplease::unparse(&syntree);
    //    println!("{}", prettycode);
    //}
    //for e in &schema.structs {
    //    let code = e.generate(QapiType::Struct);
    //    let syntree: syn::File = syn::parse2(code)?;
    //    let prettycode = prettyplease::unparse(&syntree);
    //    println!("{}", prettycode);
    //}

    //return Ok(());
    //let mut raw_schemas: HashMap<PathBuf, String> = HashMap::new();
    //let mut schemas: HashMap<PathBuf, Vec<QapiSchema>> = HashMap::new();

    //let mut loaded_files: HashSet<PathBuf> = HashSet::new();
    //let mut parsed_files: HashSet<PathBuf> = HashSet::new();
    //let mut ready_to_load: Vec<PathBuf> = vec![schema_file];
    //let mut ready_to_parse: Vec<PathBuf> = vec![];
    //let mut ready_to_include: Vec<PathBuf> = vec![];

    //while let Some(file) = ready_to_load.pop() {
    //    if !loaded_files.insert(file.clone()) {
    //        continue;
    //    }
    //    let string = std::fs::read_to_string(file.clone())?;
    //    raw_schemas.insert(file.clone(), string);
    //    ready_to_parse.push(file);
    //}
    //while let Some(file) = ready_to_parse.pop() {
    //    if !parsed_files.insert(file.clone()) {
    //        continue;
    //    }
    //    if let Some(ref raw) = raw_schemas.get(&file) {
    //        let schema = parse_schema(raw)?;
    //        schemas.insert(file.clone(), schema);
    //        ready_to_include.push(file);
    //    }
    //}
    //while let Some(file) = ready_to_include.pop() {
    //    if let Some(schema) = schemas.get(&file) {
    //        for token in schema {
    //            match token {
    //                QapiSchema::Include(include) => {
    //                    let relative_path = include.0;
    //                    let parent_dir = file.parent().unwrap();
    //                    let included_path = parent_dir.join(relative_path);
    //                    ready_to_load.push(included_path);
    //                }
    //                _ => {}
    //            }
    //        }
    //    }
    //}
    Ok(())
}
