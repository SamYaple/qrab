pub mod preprocess;
pub mod qapi_ir;
pub mod parsers;
pub mod unified;
pub mod versions;

pub use parsers::{
    ParserConfig, set_parser_config, with_parser_config,
    qapi_alternate::{take_alternate, QapiAlternate},
    qapi_alternative::{take_alternative, QapiAlternative},
    qapi_alternatives::{take_alternatives, QapiAlternatives},
    qapi_branch::{take_branch, QapiBranch},
    qapi_branches::{take_branches, QapiBranches},
    qapi_variant::{take_variant, QapiVariant},
    qapi_command::{take_command, QapiCommand},
    qapi_cond::{take_cond, QapiCond},
    qapi_documentation::{extract_since_from_comment, QapiDocumentation, QapiSectionDocumentation, QapiFreeformDoc},
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

pub use qapi_ir::*;

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub fn parse_schema(input: &str) -> Result<QapiSchema<'_>> {
    let (i, schema) = take_schema(input)
        .map_err(|e| anyhow::anyhow!("Parse error: {}", e))?;
    anyhow::ensure!(i.is_empty(), "Unparsed input remaining ({} bytes): {:?}", i.len(), &i[..i.len().min(200)]);
    Ok(schema)
}

pub fn read_schema(schema_file: &Path) -> Result<Vec<(PathBuf, String)>> {
    read_schema_with_version(schema_file, None)
}

/// Read schema files with version-specific preprocessing.
pub fn read_schema_with_version(schema_file: &Path, version: Option<preprocess::Version>) -> Result<Vec<(PathBuf, String)>> {
    let mut sources = Vec::new();
    read_schema_file(schema_file, &mut sources, version)?;
    if sources.len() > 1 {
        sources.rotate_left(1);
    }
    Ok(sources)
}

fn read_schema_file(schema_file: &Path, sources: &mut Vec<(PathBuf, String)>, version: Option<preprocess::Version>) -> Result<()> {
    if sources.iter().any(|(path, _)| path == schema_file) {
        return Ok(());
    }

    // Set parser config for version-aware parsing
    if let Some(v) = version {
        set_parser_config(ParserConfig::for_version(v));
    } else {
        set_parser_config(ParserConfig::modern());
    }

    let raw_content = std::fs::read_to_string(schema_file)
        .with_context(|| format!("Failed to read schema file: {}", schema_file.display()))?;
    // Use minimal preprocessing only — parser handles format differences via ParserConfig
    let content = preprocess::minimal_preprocess(&raw_content);
    sources.push((schema_file.to_path_buf(), content.clone()));
    let schema_tokens = parse_schema(&content)
        .with_context(|| format!("Failed to parse schema file: {} (version: {:?})", schema_file.display(), version))?;
    let includes: Vec<&str> = schema_tokens
        .into_iter()
        .filter_map(|token| match token {
            QapiSchemaToken::Include(filename) => Some(filename.0),
            _ => None,
        })
        .collect();
    for include in includes {
        let include_path = schema_file.parent().unwrap().join(include);
        read_schema_file(&include_path, sources, version)?;
    }
    Ok(())
}

/// Run the full codegen pipeline: parse all schema files and generate Rust source code.
/// Returns the generated Rust source as a String.
pub fn generate_qapi_code(qemu_src_root: &Path, schema_file: &Path) -> Result<String> {
    generate_qapi_code_versioned(qemu_src_root, schema_file, None)
}

pub fn generate_qapi_code_versioned(qemu_src_root: &Path, schema_file: &Path, version: Option<preprocess::Version>) -> Result<String> {
    let sources = read_schema_with_version(schema_file, version)?;

    // Parse each schema file once and reuse for both processing and codegen.
    let parsed: Vec<_> = sources
        .iter()
        .map(|(path, source)| {
            let schema = parse_schema(source)
                .with_context(|| format!("Failed to parse: {}", path.display()))?;
            Ok((path, schema))
        })
        .collect::<Result<Vec<_>>>()?;

    let mut unprocessed_structs = Vec::new();
    let mut unprocessed_unions = Vec::new();
    let mut unprocessed_commands = Vec::new();
    let mut unprocessed_events = Vec::new();
    let mut structs_lookup = HashMap::new();
    let mut enums_lookup = HashMap::new();

    for (_path, schema) in &parsed {
        for token in &schema.0 {
            match token {
                QapiSchemaToken::Enum(v) => {
                    let processed = process_enum(v.clone());
                    enums_lookup.insert(processed.name.clone(), processed);
                }
                QapiSchemaToken::Alternate(v) => {
                    let processed = process_alternate(v.clone());
                    enums_lookup.insert(processed.name.clone(), processed);
                }
                QapiSchemaToken::Command(v) => {
                    unprocessed_commands.push(v.clone());
                }
                QapiSchemaToken::Event(v) => {
                    unprocessed_events.push(v.clone());
                }
                QapiSchemaToken::Struct(v) => {
                    if v.base.is_some() {
                        unprocessed_structs.push(v.clone());
                        continue;
                    }
                    let (processed, extracted) = process_struct(v.clone(), &structs_lookup);
                    structs_lookup.insert(processed.name.clone(), processed);
                    for e in extracted {
                        structs_lookup.insert(e.name.clone(), e);
                    }
                }
                QapiSchemaToken::Union(v) => {
                    unprocessed_unions.push(v.clone());
                }
                _ => continue,
            }
        }
    }

    for v in unprocessed_structs.drain(..) {
        let (processed, extracted) = process_struct(v, &structs_lookup);
        structs_lookup.insert(processed.name.clone(), processed);
        for e in extracted {
            structs_lookup.insert(e.name.clone(), e);
        }
    }
    for v in unprocessed_unions.drain(..) {
        let (processed_struct, processed_enum, disc_enum) =
            process_union(v, &structs_lookup, &enums_lookup);
        enums_lookup.insert(processed_enum.name.clone(), processed_enum);
        if let Some(de) = disc_enum {
            enums_lookup.insert(de.name.clone(), de);
        }
        structs_lookup.insert(processed_struct.name.clone(), processed_struct);
    }
    for v in unprocessed_events.drain(..) {
        let processed = process_event(v, &structs_lookup);
        structs_lookup.insert(processed.name.clone(), processed);
    }
    for v in unprocessed_commands.drain(..) {
        let processed = process_command(v);
        structs_lookup.insert(processed.name.clone(), processed);
    }

    let mut output = String::new();
    output.push_str("#![allow(unused_imports, non_camel_case_types, dead_code)]\n");
    output.push_str("use crate as qapi;\n");
    output.push_str("use qapi_macros::qapi;\n");
    output.push_str("use std::collections::HashMap;\n");

    for (path, schema) in &parsed {
        output.push_str(&format!(
            "// path begin:\t{}\n",
            path.strip_prefix(qemu_src_root).unwrap_or(path).display()
        ));
        for token in &schema.0 {
            let code = match token {
                QapiSchemaToken::Enum(v) => {
                    enums_lookup.get(v.name).unwrap().generate()
                }
                QapiSchemaToken::Alternate(v) => {
                    enums_lookup.get(v.name).unwrap().generate()
                }
                QapiSchemaToken::Command(v) => {
                    structs_lookup.get(v.name).unwrap().generate()
                }
                QapiSchemaToken::Event(v) => {
                    let event_key = format!("{}Event", v.name);
                    structs_lookup.get(&event_key).unwrap().generate()
                }
                QapiSchemaToken::Struct(v) => {
                    structs_lookup.get(v.name).unwrap().generate()
                }
                QapiSchemaToken::Union(v) => {
                    // Emit discriminator enum for simple unions
                    let disc_key = format!("{}Type", rustify_name(v.name));
                    if let Some(disc_enum) = enums_lookup.get(&disc_key) {
                        let disc_code = disc_enum.generate();
                        let syntree: syn::File = syn::parse2(disc_code)?;
                        output.push_str(&prettyplease::unparse(&syntree));
                    }
                    let enum_code = enums_lookup
                        .get(&(v.name.to_owned() + "Branch"))
                        .unwrap()
                        .generate();
                    let struct_code = structs_lookup.get(v.name).unwrap().generate();
                    let syntree: syn::File = syn::parse2(enum_code)?;
                    output.push_str(&prettyplease::unparse(&syntree));
                    struct_code
                }
                _ => continue,
            };
            let syntree: syn::File = syn::parse2(code)?;
            output.push_str(&prettyplease::unparse(&syntree));
        }
        output.push_str(&format!(
            "// path end:\t{}\n",
            path.strip_prefix(qemu_src_root).unwrap_or(path).display()
        ));
    }

    Ok(output)
}
