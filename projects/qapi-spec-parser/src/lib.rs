pub mod qapi_ir;
pub mod parsers;
pub mod versions;

pub use parsers::{
    qapi_alternate::{take_alternate, QapiAlternate},
    qapi_alternative::{take_alternative, QapiAlternative},
    qapi_alternatives::{take_alternatives, QapiAlternatives},
    qapi_branch::{take_branch, QapiBranch},
    qapi_branches::{take_branches, QapiBranches},
    qapi_variant::{take_variant, QapiVariant},
    qapi_command::{take_command, QapiCommand},
    qapi_cond::{take_cond, QapiCond},
    qapi_documentation::{extract_since_from_comment, QapiDocumentation, QapiSectionDocumentation},
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
    anyhow::ensure!(i.is_empty(), "Unparsed input remaining: {:?}", &i[..i.len().min(100)]);
    Ok(schema)
}

pub fn read_schema(schema_file: &Path) -> Result<Vec<(PathBuf, String)>> {
    let mut sources = Vec::new();
    read_schema_file(schema_file, &mut sources)?;
    if sources.len() > 1 {
        sources.rotate_left(1);
    }
    Ok(sources)
}

fn read_schema_file(schema_file: &Path, sources: &mut Vec<(PathBuf, String)>) -> Result<()> {
    if sources.iter().any(|(path, _)| path == schema_file) {
        return Ok(());
    }

    let content = std::fs::read_to_string(schema_file)
        .with_context(|| format!("Failed to read schema file: {}", schema_file.display()))?;
    sources.push((schema_file.to_path_buf(), content.clone()));
    let schema_tokens = parse_schema(&content)
        .with_context(|| format!("Failed to parse schema file: {}", schema_file.display()))?;
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

/// Run the full codegen pipeline: parse all schema files and generate Rust source code.
/// Returns the generated Rust source as a String.
pub fn generate_qapi_code(qemu_src_root: &Path, schema_file: &Path) -> Result<String> {
    let sources = read_schema(schema_file)?;

    let mut unprocessed_structs = Vec::new();
    let mut unprocessed_unions = Vec::new();
    let mut unprocessed_commands = Vec::new();
    let mut unprocessed_events = Vec::new();
    let mut structs_lookup = HashMap::new();
    let mut enums_lookup = HashMap::new();

    for (_path, source) in &sources {
        for token in parse_schema(source)?.0 {
            match token {
                QapiSchemaToken::Enum(v) => {
                    let processed = process_enum(v);
                    enums_lookup.insert(processed.name.clone(), processed);
                }
                QapiSchemaToken::Alternate(v) => {
                    let processed = process_alternate(v);
                    enums_lookup.insert(processed.name.clone(), processed);
                }
                QapiSchemaToken::Command(v) => {
                    unprocessed_commands.push(v);
                }
                QapiSchemaToken::Event(v) => {
                    unprocessed_events.push(v);
                }
                QapiSchemaToken::Struct(v) => {
                    if v.base.is_some() {
                        unprocessed_structs.push(v);
                        continue;
                    }
                    let processed = process_struct(v, &structs_lookup);
                    structs_lookup.insert(processed.name.clone(), processed);
                }
                QapiSchemaToken::Union(v) => {
                    unprocessed_unions.push(v);
                }
                _ => continue,
            }
        }
    }

    for v in unprocessed_structs.drain(..) {
        let processed = process_struct(v, &structs_lookup);
        structs_lookup.insert(processed.name.clone(), processed);
    }
    for v in unprocessed_unions.drain(..) {
        let (processed_struct, processed_enum) =
            process_union(v, &structs_lookup, &enums_lookup);
        enums_lookup.insert(processed_enum.name.clone(), processed_enum);
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
    output.push_str("use qapi_macros::qapi;\n");
    output.push_str("use serde;\n");
    output.push_str("use serde_json;\n");
    output.push_str("use std::str::FromStr;\n");
    output.push_str("use std::collections::HashMap;\n");

    for (path, source) in &sources {
        output.push_str(&format!(
            "// path begin:\t{}\n",
            path.strip_prefix(qemu_src_root).unwrap_or(path).display()
        ));
        for token in parse_schema(source)?.0 {
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
                    structs_lookup.get(v.name).unwrap().generate()
                }
                QapiSchemaToken::Struct(v) => {
                    structs_lookup.get(v.name).unwrap().generate()
                }
                QapiSchemaToken::Union(v) => {
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
