//! Validate members sharing a JSON object across nested union branches.
use crate::{MembersOrRef, QapiMembers, QapiSchemaToken, QapiTypeRef, QapiUnion};
use anyhow::{Context, Result, ensure};
use std::collections::{HashMap, HashSet};

#[derive(Clone)]
struct Member {
    name: String,
    conditional: bool,
}

fn members<'i>(fields: &QapiMembers<'i>) -> Vec<Member> {
    fields
        .iter()
        .map(|field| Member {
            name: field.name.to_string(),
            conditional: field.r#if.is_some(),
        })
        .collect()
}

fn inherited<'i>(
    name: &str,
    declarations: &HashMap<&str, &QapiSchemaToken<'i>>,
    parents: &mut HashSet<String>,
) -> Result<Vec<Member>> {
    ensure!(
        parents.insert(name.to_owned()),
        "Inheritance cycle involving '{name}'"
    );
    let Some(QapiSchemaToken::Struct(value)) = declarations.get::<str>(name.as_ref()) else {
        anyhow::bail!("Object base '{name}' requires a struct");
    };
    let mut fields = match value.base.as_deref() {
        Some(base) => inherited(base, declarations, parents)?,
        None => Vec::new(),
    };
    fields.extend(members(&value.data));
    parents.remove(name);
    Ok(fields)
}

fn base_members<'i>(
    base: &MembersOrRef<'i>,
    declarations: &HashMap<&str, &QapiSchemaToken<'i>>,
) -> Result<Vec<Member>> {
    match base {
        MembersOrRef::Ref(name) => inherited(name, declarations, &mut HashSet::new()),
        MembersOrRef::Members(fields) => Ok(members(fields)),
        MembersOrRef::Unset => Ok(Vec::new()),
    }
}

fn layout<'i>(
    value: &QapiUnion<'i>,
    declarations: &HashMap<&str, &QapiSchemaToken<'i>>,
    outer: &[Member],
    parents: &mut HashSet<String>,
) -> Result<()> {
    ensure!(
        parents.insert(value.name.to_string()),
        "Union branch cycle involving '{}'",
        value.name
    );
    let mut fields = outer.to_vec();
    fields.extend(base_members(&value.base, declarations)?);
    if value.discriminator.is_none() {
        fields.extend([
            Member {
                name: "type".into(),
                conditional: false,
            },
            Member {
                name: "data".into(),
                conditional: false,
            },
        ]);
    }
    crate::schema_validation::member_names(fields.iter().map(|field| field.name.as_str()))?;
    // Simple-union payloads have their own namespace below the data key.
    if value.discriminator.is_some() {
        for branch in &value.data.0 {
            if let QapiTypeRef::Ref(name) = &branch.r#type {
                let mut check = || -> Result<()> {
                    match declarations.get::<str>(name.as_ref()) {
                        Some(QapiSchemaToken::Struct(_)) => {
                            let payload = inherited(name, declarations, &mut HashSet::new())?;
                            crate::schema_validation::member_names(
                                fields
                                    .iter()
                                    .chain(&payload)
                                    .map(|field| field.name.as_str()),
                            )
                            .with_context(|| format!("Invalid payload '{name}'"))?;
                        }
                        Some(QapiSchemaToken::Union(child)) if !child.legacy_alternate => {
                            layout(child, declarations, &fields, parents)?
                        }
                        _ => {}
                    }
                    Ok(())
                };
                check().with_context(|| {
                    format!("Invalid union '{}' branch '{}'", value.name, branch.name)
                })?;
            }
        }
    }
    parents.remove(value.name.as_ref());
    Ok(())
}

pub(crate) fn validate<'i>(
    value: &QapiUnion<'i>,
    declarations: &HashMap<&str, &QapiSchemaToken<'i>>,
) -> Result<()> {
    let Some(discriminator) = value.discriminator.as_deref() else {
        return Ok(());
    };
    let fields = base_members(&value.base, declarations)?;
    ensure!(
        !fields
            .iter()
            .any(|field| field.name == discriminator && field.conditional),
        "Discriminator '{discriminator}' must not be conditional"
    );
    for branch in &value.data.0 {
        ensure!(
            !matches!(branch.r#type, QapiTypeRef::ArrayRef(_)),
            "Flat union branch '{}' must reference a struct, not an array",
            branch.name
        );
        if crate::parser_version() >= (6, 2, 0) {
            ensure!(
                matches!(&branch.r#type, QapiTypeRef::Ref(name) if matches!(declarations.get::<str>(name.as_ref()), Some(QapiSchemaToken::Struct(_)))
                || matches!(declarations.get::<str>(name.as_ref()), Some(QapiSchemaToken::Union(child)) if !child.legacy_alternate)),
                "Flat union branch '{}' must reference a struct or union",
                branch.name
            );
        }
    }
    layout(value, declarations, &[], &mut HashSet::new())
}
