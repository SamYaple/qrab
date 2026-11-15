//! QAPI definitions supplied implicitly by QEMU rather than schema files.
use crate::{
    MembersOrRef, QapiEnum, QapiEnumValue, QapiMembers, QapiSchema, QapiSchemaToken, QapiStruct,
    QapiTypeRef,
};
use anyhow::{Context, Result, ensure};
use std::path::{Path, PathBuf};

#[derive(Default)]
struct Usage {
    qtype: bool,
    empty: bool,
}

impl Usage {
    fn reference(&mut self, name: &str) {
        match name {
            "QType" => self.qtype = true,
            "q_empty" => self.empty = true,
            _ => {}
        }
    }
}

/// Different implicit definitions must not share a schema-cache hash.
pub(crate) fn model(version: (u32, u32, u32)) -> u8 {
    if version < (2, 6, 0) {
        0
    } else if version < (2, 10, 0) {
        1
    } else {
        2
    }
}

fn reserved(name: &str, model: u8) -> bool {
    matches!(name, "str" | "number" | "int" | "bool")
        || model >= 1
            && matches!(
                name,
                "int8"
                    | "int16"
                    | "int32"
                    | "int64"
                    | "uint8"
                    | "uint16"
                    | "uint32"
                    | "uint64"
                    | "size"
                    | "any"
                    | "QType"
                    | "q_empty"
            )
        || model >= 2 && name == "null"
}

fn type_ref(value: &QapiTypeRef<'_>, used: &mut Usage) {
    match value {
        QapiTypeRef::Ref(name) | QapiTypeRef::ArrayRef(name) => {
            used.reference(name);
        }
        QapiTypeRef::InlineStruct(fields) => members(fields, used),
        QapiTypeRef::Unset => {}
    }
}

fn members(fields: &QapiMembers<'_>, used: &mut Usage) {
    for field in &fields.0 {
        type_ref(&field.r#type, used);
    }
}

fn members_or_ref(value: &MembersOrRef<'_>, used: &mut Usage) {
    match value {
        MembersOrRef::Ref(name) => {
            used.reference(name);
        }
        MembersOrRef::Members(fields) => members(fields, used),
        MembersOrRef::Unset => {}
    }
}

pub(crate) fn parse_sources(sources: &[(PathBuf, String)]) -> Result<Vec<(&Path, QapiSchema<'_>)>> {
    let mut schemas = sources
        .iter()
        .map(|(path, content)| {
            let schema = crate::parse_schema(content)
                .with_context(|| format!("Failed to parse QAPI source {}", path.display()))?;
            Ok((path.as_path(), schema))
        })
        .collect::<Result<Vec<_>>>()?;
    let model = model(crate::parser_version());
    let mut used = Usage::default();
    for (path, schema) in &schemas {
        for token in &schema.0 {
            let name = match token {
                QapiSchemaToken::Enum(value) => Some(value.name),
                QapiSchemaToken::Struct(value) => Some(value.name),
                QapiSchemaToken::Union(value) => Some(value.name),
                QapiSchemaToken::Alternate(value) => Some(value.name),
                QapiSchemaToken::Command(value) => Some(value.name),
                QapiSchemaToken::Event(value) => Some(value.name),
                _ => None,
            };
            if let Some(name) = name {
                ensure!(
                    !reserved(name, model),
                    "Predefined QAPI type '{name}' cannot be redefined in {}",
                    path.display()
                );
            }
            match token {
                QapiSchemaToken::Struct(value) => {
                    if let Some(base) = value.base {
                        used.reference(base);
                    }
                    members(&value.data, &mut used);
                }
                QapiSchemaToken::Union(value) => {
                    members_or_ref(&value.base, &mut used);
                    for branch in &value.data.0 {
                        type_ref(&branch.r#type, &mut used);
                    }
                }
                QapiSchemaToken::Alternate(value) => {
                    for branch in &value.data.0 {
                        type_ref(&branch.r#type, &mut used);
                    }
                }
                QapiSchemaToken::Command(value) => {
                    if let Some(data) = &value.data {
                        members_or_ref(data, &mut used);
                    }
                    if let Some(returns) = &value.returns {
                        type_ref(returns, &mut used);
                    }
                }
                QapiSchemaToken::Event(value) => {
                    if let Some(data) = &value.data {
                        members_or_ref(data, &mut used);
                    }
                }
                _ => {}
            }
        }
    }
    if model == 0 {
        return Ok(schemas);
    }
    let mut predefined = Vec::new();
    if used.qtype {
        // QEMU 2.10 unified integer and floating-point QObject numbers.
        let values = if model == 1 {
            &[
                "none", "qnull", "qint", "qstring", "qdict", "qlist", "qfloat", "qbool",
            ][..]
        } else {
            &[
                "none", "qnull", "qnum", "qstring", "qdict", "qlist", "qbool",
            ][..]
        };
        predefined.push(QapiSchemaToken::Enum(QapiEnum {
            name: "QType",
            prefix: Some("QTYPE"),
            data: values
                .iter()
                .map(|&name| QapiEnumValue {
                    name,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }));
    }
    if used.empty {
        predefined.push(QapiSchemaToken::Struct(QapiStruct {
            name: "q_empty",
            ..Default::default()
        }));
    }
    if !predefined.is_empty() {
        schemas.push((Path::new("<predefined QAPI types>"), QapiSchema(predefined)));
    }
    Ok(schemas)
}
