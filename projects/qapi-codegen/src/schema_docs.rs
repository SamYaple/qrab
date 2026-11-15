//! Resolve legacy comments written before an intervening helper definition.
use crate::{MembersOrRef, QapiFeatures, QapiMembers, QapiSchema, QapiSchemaToken};
use anyhow::{Result, ensure};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

pub(crate) fn resolve_legacy(schemas: &mut [(&Path, QapiSchema<'_>)]) {
    if crate::parser_version() >= (2, 9, 0) {
        return;
    }
    for (_, schema) in schemas {
        let names: HashSet<_> = schema
            .0
            .iter()
            .filter_map(|token| token.definition_name().map(str::to_owned))
            .collect();
        let mut pending = HashMap::new();
        for token in &mut schema.0 {
            let Some(name) = token.definition_name().map(str::to_owned) else {
                continue;
            };
            let Some(slot) = token.documentation_mut() else {
                continue;
            };
            if slot
                .as_ref()
                .is_some_and(|doc| doc.name != name && names.contains(doc.name))
            {
                let doc = slot.take().unwrap();
                pending.entry(doc.name).or_insert(doc);
            }
        }
        for token in &mut schema.0 {
            if let Some(doc) = token
                .definition_name()
                .and_then(|name| pending.remove(name))
            {
                token.documentation_mut().unwrap().get_or_insert(doc);
            }
        }
    }
}

fn add_features(features: Option<&QapiFeatures<'_>>, names: &mut HashSet<String>) {
    if let Some(features) = features {
        names.extend(features.iter().map(|feature| feature.name.to_string()));
    }
}

fn add_members(
    fields: &QapiMembers<'_>,
    members: &mut HashSet<String>,
    features: &mut HashSet<String>,
) {
    for field in fields.iter() {
        members.insert(field.name.to_string());
        add_features(field.features.as_ref(), features);
    }
}

pub(crate) fn validate(token: &QapiSchemaToken<'_>) -> Result<()> {
    let version = crate::parser_version();
    if version < (2, 9, 0) {
        return Ok(());
    }
    let Some(doc) = token.documentation() else {
        return Ok(());
    };
    let name = token.definition_name().unwrap();
    ensure!(
        doc.name == name,
        "Documentation symbol '{}' does not match '{name}'",
        doc.name
    );
    let mut members = HashSet::new();
    let mut features = HashSet::new();
    let own_features = match token {
        QapiSchemaToken::Struct(value) => {
            add_members(&value.data, &mut members, &mut features);
            &value.features
        }
        QapiSchemaToken::Enum(value) => {
            for member in &value.data {
                members.insert(member.name.to_string());
                add_features(member.features.as_ref(), &mut features);
            }
            &value.features
        }
        QapiSchemaToken::Alternate(value) => {
            members.extend(value.data.iter().map(|branch| branch.name.to_string()));
            &value.features
        }
        QapiSchemaToken::Union(value) => {
            if value.legacy_alternate {
                members.extend(value.data.iter().map(|branch| branch.name.to_string()));
            } else if let MembersOrRef::Members(fields) = &value.base {
                add_members(fields, &mut members, &mut features);
            } else if value.discriminator.is_none() {
                members.insert("type".into());
            }
            &value.features
        }
        QapiSchemaToken::Command(value) => {
            if let Some(MembersOrRef::Members(fields)) = &value.data {
                add_members(fields, &mut members, &mut features);
            }
            ensure!(
                version < (9, 0, 0) || value.returns.is_some() || doc.returns.is_empty(),
                "Returns section, but command does not return anything"
            );
            &value.features
        }
        QapiSchemaToken::Event(value) => {
            if let Some(MembersOrRef::Members(fields)) = &value.data {
                add_members(fields, &mut members, &mut features);
            }
            &value.features
        }
        _ => return Ok(()),
    };
    add_features(own_features.as_ref(), &mut features);
    if !matches!(token, QapiSchemaToken::Command(_)) {
        ensure!(
            doc.returns.is_empty(),
            "Returns section is only valid for commands"
        );
        ensure!(
            version < (9, 0, 0) || doc.errors.is_empty(),
            "Errors section is only valid for commands"
        );
    }
    // Global member/feature link checks arrived in 4.2; older callable docs
    // can describe a named boxed payload with virtual parameter names.
    if version < (4, 2, 0) {
        return Ok(());
    }
    for (name, _) in &doc.fields {
        ensure!(
            members.contains(*name),
            "Documented member '{name}' does not exist"
        );
    }
    for (name, _) in &doc.features {
        ensure!(
            features.contains(*name),
            "Documented feature '{name}' does not exist"
        );
    }
    for name in features {
        ensure!(
            doc.features
                .iter()
                .any(|(documented, _)| *documented == name),
            "Feature '{name}' lacks documentation"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::QapiSchemaToken;

    #[test]
    fn legacy_forward_docs_follow_the_named_definition_without_overwriting_its_docs() {
        for (target_doc, expected) in [
            ("", "Forward description."),
            (
                "##\n# @Backend:\n# Own description.\n##\n",
                "Own description.",
            ),
        ] {
            crate::set_parser_version((2, 3, 0));
            let input = format!(
                "##\n# @Backend:\n# Forward description.\n##\n{{'type':'Helper','data':{{}}}}\n{target_doc}{{'union':'Backend','data':{{'empty':'Helper'}}}}"
            );
            let schema = crate::parse_schema(&input).unwrap();
            let mut schemas = [(Path::new("legacy.json"), schema)];
            resolve_legacy(&mut schemas);
            for token in &schemas[0].1.0 {
                match token {
                    QapiSchemaToken::Struct(value) => assert!(value.doc.is_none()),
                    QapiSchemaToken::Union(value) => {
                        let doc = value.doc.as_ref().unwrap();
                        assert_eq!(doc.name, "Backend");
                        assert!(doc.body.contains(&expected));
                    }
                    _ => {}
                }
            }
        }
    }

    #[test]
    fn an_exact_declared_target_wins_over_a_case_only_match() {
        crate::set_parser_version((2, 3, 0));
        let input = "##\n# @Example:\n# Exact target.\n##\n{'type':'example','data':{}}\n{'type':'Example','data':{}}";
        let mut schemas = [(
            Path::new("legacy.json"),
            crate::parse_schema(input).unwrap(),
        )];
        resolve_legacy(&mut schemas);
        for token in &schemas[0].1.0 {
            if let QapiSchemaToken::Struct(value) = token {
                assert_eq!(value.doc.is_some(), value.name == "Example");
            }
        }
    }

    #[test]
    fn harmless_case_typos_and_unknown_legacy_doc_targets_stay_attached() {
        crate::set_parser_version((2, 3, 0));
        for name in ["EXAMPLE", "Unknown"] {
            let input =
                format!("##\n# @{name}:\n# Description.\n##\n{{'type':'Example','data':{{}}}}");
            let mut schemas = [(
                Path::new("legacy.json"),
                crate::parse_schema(&input).unwrap(),
            )];
            resolve_legacy(&mut schemas);
            let token = schemas[0]
                .1
                .0
                .iter_mut()
                .find(|token| token.definition_name().is_some())
                .unwrap();
            assert_eq!(
                token.documentation_mut().unwrap().as_ref().unwrap().name,
                name
            );
        }
    }
}
