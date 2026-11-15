#![deny(unsafe_code)]

mod arrays;
mod builtins;
pub mod metadata_emit;
pub mod parsers;
pub mod qapi_ir;
mod schema_docs;
mod schema_validation;
mod symbols;
pub mod unified;
mod union_layout;
pub mod versions;

pub use parsers::{
    parser_version,
    qapi_alternate::{QapiAlternate, take_alternate},
    qapi_alternative::{QapiAlternative, take_alternative},
    qapi_alternatives::{QapiAlternatives, take_alternatives},
    qapi_branch::{QapiBranch, take_branch},
    qapi_branches::{QapiBranches, take_branches},
    qapi_command::{QapiCommand, take_command},
    qapi_cond::{QapiCond, take_cond},
    qapi_documentation::{
        QapiDocumentation, QapiFreeformDoc, QapiSectionDocumentation, extract_since_from_comment,
    },
    qapi_enum::{QapiEnum, take_enum},
    qapi_enum_value::{QapiEnumValue, take_enum_value},
    qapi_event::{QapiEvent, take_event},
    qapi_feature::{QapiFeature, take_feature},
    qapi_features::{QapiFeatures, take_features},
    qapi_include::{QapiInclude, take_include},
    qapi_member::{QapiMember, take_member},
    qapi_members::{MembersOrRef, QapiMembers, take_members, take_members_or_ref},
    qapi_pragma::{QapiPragma, take_pragma},
    qapi_schema::{QapiSchema, QapiSchemaToken, take_schema},
    qapi_struct::{QapiStruct, take_struct},
    qapi_type_ref::{QapiTypeRef, take_type_ref},
    qapi_union::{QapiUnion, take_union},
    qapi_variant::{QapiVariant, take_variant},
    set_parser_version,
};

pub use qapi_ir::*;

use anyhow::{Context, Result};
use nom::{Parser, combinator::all_consuming};
use std::path::{Path, PathBuf};

/// Parse a single schema file's content into tokens.
pub fn parse_schema(input: &str) -> Result<QapiSchema<'_>> {
    let (_, schema) = all_consuming(take_schema)
        .parse(input)
        .map_err(|error| match error {
            nom::Err::Error(error) | nom::Err::Failure(error) => {
                let remainder = error
                    .input
                    .trim_start_matches(parsers::common::is_whitespace);
                let offset = input.len().saturating_sub(remainder.len());
                let consumed = &input[..offset];
                let (mut line, mut column) = (1, 1);
                let mut chars = consumed.chars().peekable();
                while let Some(c) = chars.next() {
                    if c == '\r' || c == '\n' {
                        if c == '\r' && chars.peek() == Some(&'\n') {
                            chars.next();
                        }
                        line += 1;
                        column = 1;
                    } else {
                        column += 1;
                    }
                }
                anyhow::anyhow!(
                    "QAPI parse error at line {line}, column {column} ({:?}): {:?}",
                    error.code,
                    remainder.chars().take(160).collect::<String>()
                )
            }
            nom::Err::Incomplete(needed) => anyhow::anyhow!("Incomplete QAPI schema: {needed:?}"),
        })?;
    Ok(schema)
}

/// Read schema files following includes, with version-aware parser dispatch.
/// No preprocessing is applied — the parser handles all format variations directly.
pub fn read_schema_with_version(
    schema_file: &Path,
    version: Option<(u32, u32, u32)>,
) -> Result<Vec<(PathBuf, String)>> {
    let mut sources = Vec::new();
    read_schema_file(schema_file, &mut sources, &mut Vec::new(), version)?;
    if sources.len() > 1 {
        sources.rotate_left(1);
    }
    Ok(sources)
}

fn read_schema_file(
    schema_file: &Path,
    sources: &mut Vec<(PathBuf, String)>,
    active: &mut Vec<PathBuf>,
    version: Option<(u32, u32, u32)>,
) -> Result<()> {
    let schema_file = schema_file
        .canonicalize()
        .with_context(|| format!("Failed to resolve schema file: {}", schema_file.display()))?;
    anyhow::ensure!(
        !active.contains(&schema_file),
        "QAPI inclusion loop: {}",
        schema_file.display()
    );
    if sources.iter().any(|(path, _)| path == &schema_file) {
        return Ok(());
    }
    active.push(schema_file.clone());

    // Set parser version for version-aware parsing
    if let Some(v) = version {
        set_parser_version(v);
    } else {
        set_parser_version((99, 0, 0));
    }

    let content = std::fs::read_to_string(&schema_file)
        .with_context(|| format!("Failed to read schema file: {}", schema_file.display()))?;
    sources.push((schema_file.to_path_buf(), content.clone()));
    let schema_tokens = parse_schema(&content).with_context(|| {
        format!(
            "Failed to parse schema file: {} (version: {:?})",
            schema_file.display(),
            version
        )
    })?;
    let includes: Vec<&str> = schema_tokens
        .into_iter()
        .filter_map(|token| match token {
            QapiSchemaToken::Include(filename) => Some(filename.0),
            _ => None,
        })
        .collect();
    for include in includes {
        let include_path = schema_file
            .parent()
            .unwrap()
            .join(include.replace("\\\\", "\\"));
        read_schema_file(&include_path, sources, active, version)?;
    }
    active.pop();
    Ok(())
}
