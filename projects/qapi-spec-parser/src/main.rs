mod qapi_ir;
use qapi_ir::*;

mod parsers;
pub use parsers::{
    qapi_alternate::{take_alternate, QapiAlternate},
    qapi_alternative::{take_alternative, QapiAlternative},
    qapi_alternatives::{take_alternatives, QapiAlternatives},
    qapi_branch::{take_branch, QapiBranch},
    qapi_branches::{take_branches, QapiBranches},
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

use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
fn parse_schema(input: &str) -> Result<QapiSchema<'_>> {
    let (i, schema) = take_schema(input).unwrap();
    assert![i == ""];
    Ok(schema)
}

fn read_schema(schema_file: &Path) -> Result<Vec<(PathBuf, String)>> {
    let mut sources = Vec::new();
    read_schema_file(&schema_file, &mut sources)?;
    sources.rotate_left(1); // move the initial file to the end of the stack; this is wrong. TODO FIXME
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
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Please provide qemu source path as argument");
        std::process::exit(1);
    }
    let qemu_src_root = Path::new(&args[1]);
    //let schema_type = "qga";
    //let schema_type = "storage-daemon/qapi";
    let schema_type = "qapi";
    let schema_file = qemu_src_root.join(schema_type).join("qapi-schema.json");

    // We start with the provided root schema file then recursively parse the
    // includes and load the strings in the `sources` Vec.
    let sources = read_schema(&schema_file)?;

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
                    // Some structs reference other structs but the ordering
                    // within a single qapi spec file means we may not have
                    // processed the referenced struct yet. We skip for now
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
    // Process remaining structs
    for v in unprocessed_structs.drain(..) {
        let processed = process_struct(v, &structs_lookup);
        structs_lookup.insert(processed.name.clone(), processed);
    }
    // With all structs and enums processed, we can assemble the unions
    for v in unprocessed_unions.drain(..) {
        let (processed_struct, processed_enum) = process_union(v, &structs_lookup, &enums_lookup);
        enums_lookup.insert(processed_enum.name.clone(), processed_enum);
        structs_lookup.insert(processed_struct.name.clone(), processed_struct);
    }
    // Events can reference structs which we expand out (TODO: Is this correct behaviour?)
    for v in unprocessed_events.drain(..) {
        let processed = process_event(v, &structs_lookup);
        structs_lookup.insert(processed.name.clone(), processed);
    }
    // Commands can reference structs and unions
    for v in unprocessed_commands.drain(..) {
        let processed = process_command(v);
        structs_lookup.insert(processed.name.clone(), processed);
    }

    // With `enums_lookup` and `structs_lookup` in hand, we loop over all the
    // paths and tokens once more and render everything in the same order as the
    // QAPI spec expects. This might be helpful to anyone reading the generated
    // code, but it doesn't matter at all during compliation.
    println!("use qapi_macros::qapi;");
    println!("use serde_json;");
    for (path, source) in &sources {
        println!(
            "// path begin:\t{}",
            path.strip_prefix(qemu_src_root).unwrap().display()
        );
        for token in parse_schema(source)?.0 {
            match token {
                QapiSchemaToken::Enum(v) => {
                    let qir = enums_lookup.get(v.name).unwrap();
                    let code = qir.generate();
                    let syntree: syn::File = syn::parse2(code)?;
                    let prettycode = prettyplease::unparse(&syntree);
                    print!("{}", prettycode);
                }
                QapiSchemaToken::Alternate(v) => {
                    let qir = enums_lookup.get(v.name).unwrap();
                    let code = qir.generate();
                    let syntree: syn::File = syn::parse2(code)?;
                    let prettycode = prettyplease::unparse(&syntree);
                    print!("{}", prettycode);
                }
                QapiSchemaToken::Command(v) => {
                    let qir = structs_lookup.get(v.name).unwrap();
                    let code = qir.generate();
                    let syntree: syn::File = syn::parse2(code)?;
                    let prettycode = prettyplease::unparse(&syntree);
                    print!("{}", prettycode);
                }
                QapiSchemaToken::Event(v) => {
                    let qir = structs_lookup.get(v.name).unwrap();
                    let code = qir.generate();
                    let syntree: syn::File = syn::parse2(code)?;
                    let prettycode = prettyplease::unparse(&syntree);
                    print!("{}", prettycode);
                }
                QapiSchemaToken::Struct(v) => {
                    let qir = structs_lookup.get(v.name).unwrap();
                    let code = qir.generate();
                    let syntree: syn::File = syn::parse2(code)?;
                    let prettycode = prettyplease::unparse(&syntree);
                    print!("{}", prettycode);
                }
                QapiSchemaToken::Union(v) => {
                    let qir = enums_lookup.get(&(v.name.to_owned() + "Branch")).unwrap();
                    let code = qir.generate();
                    let syntree: syn::File = syn::parse2(code)?;
                    let prettycode = prettyplease::unparse(&syntree);
                    print!("{}", prettycode);

                    let qir = structs_lookup.get(v.name).unwrap();
                    let code = qir.generate();
                    let syntree: syn::File = syn::parse2(code)?;
                    let prettycode = prettyplease::unparse(&syntree);
                    print!("{}", prettycode);
                }
                _ => continue,
            }
        }
        println!(
            "// path end:\t{}",
            path.strip_prefix(qemu_src_root).unwrap().display()
        );
    }
    Ok(())
}
