//! Resolve QEMU's implicit `ElementList` names to ordinary array references.
use crate::{MembersOrRef, QapiMembers, QapiSchema, QapiSchemaToken, QapiString, QapiTypeRef};
use anyhow::{Context, Result, ensure};
use std::{collections::HashMap, path::Path};

fn visit_type<'a>(
    value: &mut QapiTypeRef<'a>,
    visitor: &mut impl FnMut(&mut QapiTypeRef<'a>) -> Result<()>,
) -> Result<()> {
    visitor(value)?;
    if let QapiTypeRef::InlineStruct(fields) = value {
        visit_members(fields, visitor)?;
    }
    Ok(())
}

fn visit_members<'a>(
    fields: &mut QapiMembers<'a>,
    visitor: &mut impl FnMut(&mut QapiTypeRef<'a>) -> Result<()>,
) -> Result<()> {
    for field in &mut fields.0 {
        visit_type(&mut field.r#type, visitor)
            .with_context(|| format!("Invalid member '{}'", field.name))?;
    }
    Ok(())
}

fn visit_object<'a>(
    value: &mut MembersOrRef<'a>,
    visitor: &mut impl FnMut(&mut QapiTypeRef<'a>) -> Result<()>,
) -> Result<()> {
    if let MembersOrRef::Members(fields) = value {
        visit_members(fields, visitor)?;
    }
    Ok(())
}

fn visit<'a>(
    schema: &mut QapiSchema<'a>,
    mut visitor: impl FnMut(&mut QapiTypeRef<'a>) -> Result<()>,
) -> Result<()> {
    for token in &mut schema.0 {
        match token {
            QapiSchemaToken::Struct(value) => visit_members(&mut value.data, &mut visitor)?,
            QapiSchemaToken::Union(value) => {
                visit_object(&mut value.base, &mut visitor)?;
                for branch in &mut value.data.0 {
                    visit_type(&mut branch.r#type, &mut visitor)?;
                }
            }
            QapiSchemaToken::Alternate(value) => {
                for branch in &mut value.data.0 {
                    visit_type(&mut branch.r#type, &mut visitor)?;
                }
            }
            QapiSchemaToken::Command(value) => {
                if let Some(data) = &mut value.data {
                    visit_object(data, &mut visitor)?;
                }
                if let Some(returns) = &mut value.returns {
                    visit_type(returns, &mut visitor).context("Invalid return type")?;
                }
            }
            QapiSchemaToken::Event(value) => {
                if let Some(data) = &mut value.data {
                    visit_object(data, &mut visitor)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

pub(crate) fn normalize<'a>(schemas: &mut [(&Path, QapiSchema<'a>)], model: u8) -> Result<()> {
    if model == 0 {
        return Ok(());
    }
    let mut aliases = HashMap::new();
    for element in [
        "str", "number", "int", "int8", "int16", "int32", "int64", "uint8", "uint16", "uint32",
        "uint64", "size", "bool", "any",
    ] {
        aliases.insert(format!("{element}List"), QapiString::Borrowed(element));
    }
    if model >= 2 {
        aliases.insert("nullList".into(), QapiString::Borrowed("null"));
    }
    // Named arrays exist only after an explicit array use creates them.
    for (path, schema) in schemas.iter_mut() {
        for token in &schema.0 {
            let name = match token {
                QapiSchemaToken::Enum(value) => Some(value.name.as_ref()),
                QapiSchemaToken::Struct(value) => Some(value.name.as_ref()),
                QapiSchemaToken::Union(value) => Some(value.name.as_ref()),
                QapiSchemaToken::Alternate(value) => Some(value.name.as_ref()),
                _ => None,
            };
            if let Some(name) = name {
                ensure!(
                    !name.ends_with("List"),
                    "QAPI type '{name}' uses reserved List suffix in {}",
                    path.display()
                );
            }
        }
        visit(schema, |value| {
            if let QapiTypeRef::ArrayRef(element) = value {
                aliases
                    .entry(format!("{element}List"))
                    .or_insert_with(|| element.clone());
            }
            Ok(())
        })?;
    }
    for (path, schema) in schemas {
        visit(schema, |value| {
            let (name, array) = match &*value {
                QapiTypeRef::Ref(name) => (name.as_ref(), false),
                QapiTypeRef::ArrayRef(name) => (name.as_ref(), true),
                _ => return Ok(()),
            };
            if let Some(element) = aliases.get(name) {
                ensure!(
                    !array && !aliases.contains_key(element.as_ref()),
                    "Nested QAPI arrays are not permitted: '{name}'"
                );
                *value = QapiTypeRef::ArrayRef(element.clone());
            }
            Ok(())
        })
        .with_context(|| format!("Invalid QAPI array reference in {}", path.display()))?;
    }
    Ok(())
}
