mod codegen;
use codegen::*;

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
fn parse_schema(input: &str) -> Result<QapiSchema<'_>> {
    let (i, schema) = take_schema(input).unwrap();
    assert![i == ""];
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
    let qemu_src_root = Path::new("/home/sam/repos/qemu");
    //let schema_type = "qga";
    //let schema_type = "storage-daemon/qapi";
    let schema_type = "qapi";
    let schema_file = qemu_src_root.join(schema_type).join("qapi-schema.json");

    // We start with the provided root schema file then recursively parse the
    // includes and load the strings in the `sources` Vec.
    let sources = read_schema(&schema_file)?;

    // Here we _reparse_ the String elements in the vec. This is the only way
    // I could figure out lifetimes while still keeping &str references to the
    // original String across many files.
    let mut tokens: Vec<QapiSchemaToken> = Vec::new();
    for (path, source) in &sources {
        tokens.extend(parse_schema(source)?.0);
    }

    let mut schema = Schema::default();
    for token in tokens {
        match token {
            QapiSchemaToken::Alternate(v) => {
                schema.alternates.push(process_alternate(v));
            }
            QapiSchemaToken::Command(v) => {
                schema.commands.push(process_command(v));
            }
            QapiSchemaToken::Enum(v) => {
                schema.enums.push(process_enum(v));
            }
            QapiSchemaToken::Event(v) => {
                schema.events.push(process_event(v));
            }
            QapiSchemaToken::Struct(v) => {
                schema.structs.push(process_struct(v));
            }
            QapiSchemaToken::Union(v) => {
                schema.unions.push(process_union(v));
            }
            _ => continue,
        }
    }

    for e in &schema.alternates {
        let code = e.generate();
        let syntree: syn::File = syn::parse2(code)?;
        let prettycode = prettyplease::unparse(&syntree);
        println!("{}", prettycode);
    }
    for e in &schema.enums {
        let code = e.generate();
        let syntree: syn::File = syn::parse2(code)?;
        let prettycode = prettyplease::unparse(&syntree);
        println!("{}", prettycode);
    }
    for e in &schema.structs {
        let code = e.generate();
        let syntree: syn::File = syn::parse2(code)?;
        let prettycode = prettyplease::unparse(&syntree);
        println!("{}", prettycode);
    }
    for (e, s) in &schema.unions {
        let code = e.generate();
        let syntree: syn::File = syn::parse2(code)?;
        let prettycode = prettyplease::unparse(&syntree);
        println!("{}", prettycode);

        let code = s.generate();
        let syntree: syn::File = syn::parse2(code)?;
        let prettycode = prettyplease::unparse(&syntree);
        println!("{}", prettycode);
    }
    for e in &schema.events {
        let code = e.generate();
        let syntree: syn::File = syn::parse2(code)?;
        let prettycode = prettyplease::unparse(&syntree);
        println!("{}", prettycode);
    }
    for e in &schema.commands {
        let code = e.generate();
        let syntree: syn::File = syn::parse2(code)?;
        let prettycode = prettyplease::unparse(&syntree);
        println!("{}", prettycode);
    }
    Ok(())
}
