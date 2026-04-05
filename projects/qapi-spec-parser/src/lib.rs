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
use std::path::{Path, PathBuf};

/// Parse a single schema file's content into tokens.
pub fn parse_schema(input: &str) -> Result<QapiSchema<'_>> {
    let (i, schema) = take_schema(input)
        .map_err(|e| anyhow::anyhow!("Parse error: {}", e))?;
    anyhow::ensure!(i.is_empty(), "Unparsed input remaining ({} bytes): {:?}", i.len(), &i[..i.len().min(200)]);
    Ok(schema)
}

/// Read schema files with version-specific preprocessing, following includes.
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
