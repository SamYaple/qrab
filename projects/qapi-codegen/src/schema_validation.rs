//! Resolve source-level references before Rust name mapping or IR synthesis.

use crate::{MembersOrRef, QapiMembers, QapiSchema, QapiSchemaToken, QapiTypeRef};
use anyhow::{Context, Result, ensure};
use std::{collections::HashSet, path::Path};

fn known_type(name: &str, types: &HashSet<&str>) -> Result<()> {
    ensure!(
        matches!(
            name,
            "str"
                | "number"
                | "int"
                | "int8"
                | "int16"
                | "int32"
                | "int64"
                | "uint8"
                | "uint16"
                | "uint32"
                | "uint64"
                | "size"
                | "bool"
                | "any"
                | "null"
                | "visitor"
                | "dict"
                | "**"
        ) || types.contains(name),
        "Unknown QAPI type '{name}'"
    );
    Ok(())
}

fn type_ref(value: &QapiTypeRef<'_>, types: &HashSet<&str>) -> Result<()> {
    match value {
        QapiTypeRef::Ref(name) | QapiTypeRef::ArrayRef(name) => known_type(name, types),
        QapiTypeRef::InlineStruct(fields) => members(fields, types),
        QapiTypeRef::Unset => anyhow::bail!("Missing QAPI type reference"),
    }
}

fn members(fields: &QapiMembers<'_>, types: &HashSet<&str>) -> Result<()> {
    for field in &fields.0 {
        type_ref(&field.r#type, types)
            .with_context(|| format!("Invalid member '{}'", field.name))?;
    }
    Ok(())
}

fn members_or_ref(value: &MembersOrRef<'_>, types: &HashSet<&str>) -> Result<()> {
    match value {
        MembersOrRef::Ref(name) => known_type(name, types),
        MembersOrRef::Members(fields) => members(fields, types),
        MembersOrRef::Unset => Ok(()),
    }
}

pub(crate) fn validate(schemas: &[(&Path, QapiSchema<'_>)]) -> Result<()> {
    // Gather all files first: forward references are valid across includes.
    let types: HashSet<_> = schemas
        .iter()
        .flat_map(|(_, schema)| &schema.0)
        .filter_map(|token| match token {
            QapiSchemaToken::Enum(value) => Some(value.name),
            QapiSchemaToken::Struct(value) => Some(value.name),
            QapiSchemaToken::Union(value) => Some(value.name),
            QapiSchemaToken::Alternate(value) => Some(value.name),
            _ => None,
        })
        .collect();
    for (path, schema) in schemas {
        for token in &schema.0 {
            let (kind, name) = match token {
                QapiSchemaToken::Struct(value) => ("struct", value.name),
                QapiSchemaToken::Union(value) => ("union", value.name),
                QapiSchemaToken::Alternate(value) => ("alternate", value.name),
                QapiSchemaToken::Command(value) => ("command", value.name),
                QapiSchemaToken::Event(value) => ("event", value.name),
                _ => continue,
            };
            let check = || -> Result<()> {
                match token {
                    QapiSchemaToken::Struct(value) => {
                        if let Some(base) = value.base {
                            known_type(base, &types).context("Invalid base")?;
                        }
                        members(&value.data, &types)?;
                    }
                    QapiSchemaToken::Union(value) => {
                        members_or_ref(&value.base, &types).context("Invalid base")?;
                        for branch in &value.data.0 {
                            type_ref(&branch.r#type, &types)
                                .with_context(|| format!("Invalid branch '{}'", branch.name))?;
                        }
                    }
                    QapiSchemaToken::Alternate(value) => {
                        for branch in &value.data.0 {
                            type_ref(&branch.r#type, &types)
                                .with_context(|| format!("Invalid branch '{}'", branch.name))?;
                        }
                    }
                    QapiSchemaToken::Command(value) => {
                        if let Some(data) = &value.data {
                            members_or_ref(data, &types).context("Invalid arguments")?;
                        }
                        if let Some(returns) = &value.returns {
                            type_ref(returns, &types).context("Invalid return type")?;
                        }
                    }
                    QapiSchemaToken::Event(value) => {
                        if let Some(data) = &value.data {
                            members_or_ref(data, &types).context("Invalid event data")?;
                        }
                    }
                    _ => unreachable!(),
                }
                Ok(())
            };
            check()
                .with_context(|| format!("Invalid QAPI {kind} '{name}' in {}", path.display()))?;
        }
    }
    Ok(())
}
