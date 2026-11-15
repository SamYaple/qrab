//! Resolve legacy comments written before an intervening helper definition.
use crate::QapiSchema;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

pub(crate) fn resolve_legacy(schemas: &mut [(&Path, QapiSchema<'_>)]) {
    if crate::parser_version() >= (2, 8, 0) {
        return;
    }
    for (_, schema) in schemas {
        let names: HashSet<_> = schema
            .0
            .iter()
            .filter_map(|token| token.definition_name())
            .collect();
        let mut pending = HashMap::new();
        for token in &mut schema.0 {
            let Some(name) = token.definition_name() else {
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
