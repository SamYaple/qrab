pub mod parsers;
pub mod qapi_ir;
pub mod unified;
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
use std::path::{Path, PathBuf};

/// Parse a single schema file's content into tokens.
pub fn parse_schema(input: &str) -> Result<QapiSchema<'_>> {
    let (i, schema) = take_schema(input).map_err(|e| anyhow::anyhow!("Parse error: {}", e))?;
    anyhow::ensure!(
        i.is_empty(),
        "Unparsed input remaining ({} bytes): {:?}",
        i.len(),
        &i[..i.len().min(200)]
    );
    Ok(schema)
}

/// Read schema files following includes, with version-aware parser dispatch.
/// No preprocessing is applied — the parser handles all format variations directly.
pub fn read_schema_with_version(
    schema_file: &Path,
    version: Option<(u32, u32, u32)>,
) -> Result<Vec<(PathBuf, String)>> {
    let mut sources = Vec::new();
    read_schema_file(schema_file, &mut sources, version)?;
    if sources.len() > 1 {
        sources.rotate_left(1);
    }
    Ok(sources)
}

fn read_schema_file(
    schema_file: &Path,
    sources: &mut Vec<(PathBuf, String)>,
    version: Option<(u32, u32, u32)>,
) -> Result<()> {
    if sources.iter().any(|(path, _)| path == schema_file) {
        return Ok(());
    }

    // Set parser version for version-aware parsing
    if let Some(v) = version {
        set_parser_version(v);
    } else {
        set_parser_version((99, 0, 0));
    }

    let content = std::fs::read_to_string(schema_file)
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
        let include_path = schema_file.parent().unwrap().join(include);
        read_schema_file(&include_path, sources, version)?;
    }
    Ok(())
}
