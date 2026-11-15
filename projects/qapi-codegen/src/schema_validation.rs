//! Resolve source-level references before Rust name mapping or IR synthesis.

use crate::{MembersOrRef, QapiMembers, QapiSchema, QapiSchemaToken, QapiTypeRef};
use anyhow::{Context, Result, ensure};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

fn member_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Result<()> {
    let mut seen = HashMap::new();
    for name in names {
        let key: String = name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        if let Some(previous) = seen.insert(key, name) {
            anyhow::bail!("Member '{name}' collides with '{previous}'");
        }
    }
    Ok(())
}

pub(crate) fn ir_members<'a>(
    fields: impl IntoIterator<Item = &'a crate::qapi_ir::StructField>,
) -> Result<()> {
    use crate::qapi_ir::Attribute;
    member_names(
        fields
            .into_iter()
            .filter(|field| !field.meta.attributes.contains(&Attribute::new("flatten")))
            .map(|field| {
                field
                    .meta
                    .attributes
                    .iter()
                    .find_map(|attr| match attr {
                        Attribute::Item { name, value } if name == "name" => Some(value.as_str()),
                        _ => None,
                    })
                    .unwrap_or(field.name.as_str())
            }),
    )
}

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

fn type_ref(value: &QapiTypeRef<'_>, types: &HashSet<&str>, inline: bool) -> Result<()> {
    match value {
        QapiTypeRef::Ref(name) | QapiTypeRef::ArrayRef(name) => known_type(name, types),
        QapiTypeRef::InlineStruct(fields) => {
            ensure!(
                inline,
                "Anonymous struct is only valid inside a struct definition"
            );
            members(fields, types, true)
        }
        QapiTypeRef::Unset => anyhow::bail!("Missing QAPI type reference"),
    }
}

fn members(fields: &QapiMembers<'_>, types: &HashSet<&str>, inline: bool) -> Result<()> {
    member_names(fields.0.iter().map(|field| field.name))?;
    for field in &fields.0 {
        type_ref(&field.r#type, types, inline)
            .with_context(|| format!("Invalid member '{}'", field.name))?;
    }
    Ok(())
}

fn members_or_ref(value: &MembersOrRef<'_>, types: &HashSet<&str>) -> Result<()> {
    match value {
        MembersOrRef::Ref(name) => known_type(name, types),
        MembersOrRef::Members(fields) => members(fields, types, false),
        MembersOrRef::Unset => Ok(()),
    }
}

fn boxed_arguments(
    data: &Option<MembersOrRef<'_>>,
    boxed: Option<&str>,
    declarations: &HashMap<&str, &QapiSchemaToken<'_>>,
) -> Result<()> {
    let Some(boxed) = boxed else {
        return Ok(());
    };
    ensure!(boxed == "true", "Flag 'boxed' may only use true value");
    let Some(MembersOrRef::Ref(name)) = data else {
        anyhow::bail!("Boxed arguments require a named 'data' type");
    };
    let object = match declarations.get(name) {
        Some(QapiSchemaToken::Struct(_)) => true,
        Some(QapiSchemaToken::Union(value)) => !value.legacy_alternate,
        _ => false,
    };
    ensure!(
        object,
        "Boxed arguments require an object type, not '{name}'"
    );
    Ok(())
}

fn alternate_branches(
    branches: &crate::QapiAlternatives<'_>,
    declarations: &HashMap<&str, &QapiSchemaToken<'_>>,
) -> Result<()> {
    #[derive(Clone, Copy, PartialEq, Eq, Hash)]
    enum Kind {
        Null,
        Bool,
        Integer,
        Float,
        Number,
        String,
        Object,
        Array,
    }
    let unified_numbers = crate::parser_version() >= (2, 10, 0);
    let mut seen = HashMap::new();
    for branch in &branches.0 {
        let (kind, enumeration) = match &branch.r#type {
            QapiTypeRef::ArrayRef(_) => (Some(Kind::Array), None),
            QapiTypeRef::Ref(name) => {
                let enumeration = match declarations.get(name) {
                    Some(QapiSchemaToken::Enum(value)) => Some(value),
                    _ => None,
                };
                let kind = match *name {
                    "str" => Some(Kind::String),
                    "bool" => Some(Kind::Bool),
                    "null" => Some(Kind::Null),
                    "number" => Some(if unified_numbers {
                        Kind::Number
                    } else {
                        Kind::Float
                    }),
                    "int" | "int8" | "int16" | "int32" | "int64" | "uint8" | "uint16"
                    | "uint32" | "uint64" | "size" => Some(if unified_numbers {
                        Kind::Number
                    } else {
                        Kind::Integer
                    }),
                    _ => match declarations.get(name) {
                        Some(QapiSchemaToken::Enum(_)) => Some(Kind::String),
                        Some(QapiSchemaToken::Struct(_)) => Some(Kind::Object),
                        Some(QapiSchemaToken::Union(value)) if !value.legacy_alternate => {
                            Some(Kind::Object)
                        }
                        _ => None,
                    },
                };
                (kind, enumeration)
            }
            _ => (None, None),
        };
        let kind = kind.with_context(|| {
            format!(
                "Alternate branch '{}' cannot use type {:?}",
                branch.name, branch.r#type
            )
        })?;
        let mut conflicting = vec![kind];
        if unified_numbers && kind == Kind::String {
            if let Some(enumeration) = enumeration {
                for member in &enumeration.data {
                    if matches!(member.name, "on" | "off") {
                        conflicting.push(Kind::Bool);
                    }
                    if member
                        .name
                        .starts_with(|c: char| c.is_ascii_digit() || matches!(c, '-' | '+' | '.'))
                    {
                        conflicting.push(Kind::Number);
                    }
                }
            } else {
                conflicting.extend([Kind::Bool, Kind::Number]);
            }
        }
        for kind in conflicting.iter().copied() {
            if let Some(previous) = seen.get(&kind) {
                ensure!(
                    *previous == branch.name,
                    "Alternate branch '{}' cannot be distinguished from '{}'",
                    branch.name,
                    previous
                );
            }
        }
        for kind in conflicting {
            seen.insert(kind, branch.name);
        }
    }
    Ok(())
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
    let declarations: HashMap<_, _> = schemas
        .iter()
        .flat_map(|(_, schema)| &schema.0)
        .filter_map(|token| match token {
            QapiSchemaToken::Enum(value) => Some((value.name, token)),
            QapiSchemaToken::Struct(value) => Some((value.name, token)),
            QapiSchemaToken::Union(value) => Some((value.name, token)),
            QapiSchemaToken::Alternate(value) => Some((value.name, token)),
            _ => None,
        })
        .collect();
    for (path, schema) in schemas {
        for token in &schema.0 {
            let (kind, name) = match token {
                QapiSchemaToken::Enum(value) => ("enum", value.name),
                QapiSchemaToken::Struct(value) => ("struct", value.name),
                QapiSchemaToken::Union(value) => ("union", value.name),
                QapiSchemaToken::Alternate(value) => ("alternate", value.name),
                QapiSchemaToken::Command(value) => ("command", value.name),
                QapiSchemaToken::Event(value) => ("event", value.name),
                _ => continue,
            };
            let check = || -> Result<()> {
                match token {
                    QapiSchemaToken::Enum(value) => {
                        member_names(value.data.iter().map(|value| value.name))?;
                    }
                    QapiSchemaToken::Struct(value) => {
                        if let Some(base) = value.base {
                            known_type(base, &types).context("Invalid base")?;
                        }
                        members(&value.data, &types, true)?;
                    }
                    QapiSchemaToken::Union(value) => {
                        member_names(value.data.0.iter().map(|branch| branch.name))?;
                        members_or_ref(&value.base, &types).context("Invalid base")?;
                        for branch in &value.data.0 {
                            type_ref(&branch.r#type, &types, false)
                                .with_context(|| format!("Invalid branch '{}'", branch.name))?;
                        }
                    }
                    QapiSchemaToken::Alternate(value) => {
                        member_names(value.data.0.iter().map(|branch| branch.name))?;
                        for branch in &value.data.0 {
                            type_ref(&branch.r#type, &types, false)
                                .with_context(|| format!("Invalid branch '{}'", branch.name))?;
                        }
                        alternate_branches(&value.data, &declarations)?;
                    }
                    QapiSchemaToken::Command(value) => {
                        boxed_arguments(&value.data, value.boxed, &declarations)?;
                        if let Some(data) = &value.data {
                            members_or_ref(data, &types).context("Invalid arguments")?;
                        }
                        if let Some(returns) = &value.returns {
                            type_ref(returns, &types, false).context("Invalid return type")?;
                        }
                    }
                    QapiSchemaToken::Event(value) => {
                        boxed_arguments(&value.data, value.boxed, &declarations)?;
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
