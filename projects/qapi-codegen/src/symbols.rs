//! Plan generated symbols against declarations from every selected release.
use crate::{
    QapiSchema, QapiSchemaToken,
    qapi_ir::{Attribute, Enum, EnumVariantKind, Metadata, Struct, StructField, rustify_name},
};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Enum,
    Object,
    Alternate,
}

fn declaration<'a>(token: &QapiSchemaToken<'a>) -> Option<(&'a str, Kind)> {
    Some(match token {
        QapiSchemaToken::Enum(value) => (value.name, Kind::Enum),
        QapiSchemaToken::Struct(value) => (value.name, Kind::Object),
        QapiSchemaToken::Alternate(value) => (value.name, Kind::Alternate),
        QapiSchemaToken::Union(value) => (
            value.name,
            if value.legacy_alternate {
                Kind::Alternate
            } else {
                Kind::Object
            },
        ),
        _ => return None,
    })
}

#[derive(Default)]
pub(crate) struct SchemaNames {
    pub names: HashSet<String>,
    roles: BTreeMap<String, String>,
    declarations: BTreeMap<String, BTreeMap<(u32, u32, u32), Kind>>,
    type_runs: BTreeMap<String, BTreeMap<(u32, u32, u32), String>>,
}

impl SchemaNames {
    pub fn merge(&mut self, other: Self) {
        self.names.extend(other.names);
        self.roles.extend(other.roles);
        for (name, versions) in other.declarations {
            self.declarations.entry(name).or_default().extend(versions);
        }
    }

    pub fn collect(&mut self, schema: &QapiSchema<'_>) {
        for token in &schema.0 {
            if let Some((name, kind)) = declaration(token) {
                self.declarations
                    .entry(rustify_name(name))
                    .or_default()
                    .insert(crate::parser_version(), kind);
            }
            let name = match token {
                QapiSchemaToken::Enum(value) => value.name,
                QapiSchemaToken::Struct(value) => value.name,
                QapiSchemaToken::Alternate(value) => value.name,
                QapiSchemaToken::Union(value) => value.name,
                QapiSchemaToken::Command(value) => value.name,
                QapiSchemaToken::Event(value) => {
                    self.roles
                        .entry(rustify_name(&format!("{}Event", value.name)))
                        .or_default();
                    continue;
                }
                _ => continue,
            };
            let root = rustify_name(name);
            self.names.insert(root.clone());
            match token {
                QapiSchemaToken::Union(_) => {
                    self.roles.entry(format!("{root}Branch")).or_default();
                    self.roles.entry(format!("{root}Type")).or_default();
                }
                QapiSchemaToken::Command(_) => {
                    self.roles.entry(format!("{root}Command")).or_default();
                }
                _ => {}
            }
        }
    }

    pub fn finish(&mut self) {
        // A return to an earlier kind starts a distinct lifetime as well.
        for (root, versions) in &self.declarations {
            let mut runs = Vec::new();
            for (&version, &kind) in versions {
                if runs.last().is_none_or(|&(_, previous)| previous != kind) {
                    runs.push((version, kind));
                }
            }
            if runs.len() < 2 {
                continue;
            }
            let mut assigned = BTreeMap::new();
            for (index, &(since, _)) in runs.iter().rev().enumerate() {
                let name = if index == 0 {
                    root.clone()
                } else {
                    let base = format!("{root}Legacy");
                    let mut name = base.clone();
                    let mut suffix = 2;
                    while !self.names.insert(name.clone()) {
                        name = format!("{base}{suffix}");
                        suffix += 1;
                    }
                    name
                };
                assigned.insert(since, name);
            }
            self.type_runs.insert(root.clone(), assigned);
        }
        for (preferred, allocated) in &mut self.roles {
            let mut name = preferred.clone();
            let mut suffix = 0;
            while !self.names.insert(name.clone()) {
                suffix += 1;
                name = if suffix == 1 {
                    format!("{preferred}Generated")
                } else {
                    format!("{preferred}Generated{suffix}")
                };
            }
            *allocated = name;
        }
    }

    pub fn role(&self, preferred: &str) -> String {
        self.roles[preferred].clone()
    }

    fn type_name(&self, name: &str) -> String {
        let root = rustify_name(name);
        self.type_runs
            .get(&root)
            .and_then(|runs| runs.range(..=crate::parser_version()).next_back())
            .map_or(root.clone(), |(_, name)| name.clone())
    }

    pub fn rewrite_types(
        &self,
        declared: &HashSet<&str>,
        enums: &mut HashMap<String, Enum>,
        structs: &mut HashMap<String, Struct>,
    ) {
        if self.type_runs.is_empty() {
            return;
        }
        let raw: HashMap<_, _> = declared
            .iter()
            .filter_map(|&name| {
                let assigned = self.type_name(name);
                (assigned != rustify_name(name)).then_some((name, assigned))
            })
            .collect();
        if raw.is_empty() {
            return;
        }
        // Fields retain QAPI names; return attributes already contain Rust names.
        let rust: HashMap<_, _> = raw
            .iter()
            .map(|(name, assigned)| (rustify_name(name), assigned))
            .collect();
        let rewrite_fields = |fields: &mut Vec<StructField>| {
            for field in fields {
                if let Some(name) = raw.get(field.r#type.as_str()) {
                    field.r#type.clone_from(name);
                }
            }
        };
        let rewrite_name = |name: &mut String, meta: &mut Metadata| {
            if let Some(assigned) = raw.get(name.as_str()) {
                if !meta
                    .attributes
                    .iter()
                    .any(|attr| matches!(attr, Attribute::Item { name, .. } if name == "name"))
                {
                    meta.attributes.push(Attribute::with_value("name", &*name));
                }
                name.clone_from(assigned);
            }
        };
        let mut renamed_enums = HashMap::new();
        for (_, mut value) in enums.drain() {
            rewrite_name(&mut value.name, &mut value.meta);
            for variant in &mut value.variants {
                match &mut variant.kind {
                    EnumVariantKind::Tuple(ty) => {
                        if let Some(name) = raw.get(ty.as_str()) {
                            ty.clone_from(name);
                        }
                    }
                    EnumVariantKind::Struct(fields) => rewrite_fields(fields),
                    EnumVariantKind::Unit => {}
                }
            }
            renamed_enums.insert(rustify_name(&value.name), value);
        }
        *enums = renamed_enums;
        let mut renamed_structs = HashMap::new();
        for (_, mut value) in structs.drain() {
            rewrite_name(&mut value.name, &mut value.meta);
            rewrite_fields(&mut value.fields);
            for attr in &mut value.meta.attributes {
                if let Attribute::Item { name, value } = attr {
                    if name == "returns" {
                        if let Some(inner) =
                            value.strip_prefix("Vec<").and_then(|s| s.strip_suffix('>'))
                        {
                            if let Some(name) = rust.get(inner) {
                                *value = format!("Vec<{name}>");
                            }
                        } else if let Some(name) = rust.get(value) {
                            value.clone_from(name);
                        }
                    }
                }
            }
            renamed_structs.insert(rustify_name(&value.name), value);
        }
        *structs = renamed_structs;
    }

    pub fn declared_names<'a>(schemas: &[(&std::path::Path, QapiSchema<'a>)]) -> HashSet<&'a str> {
        schemas
            .iter()
            .flat_map(|(_, schema)| &schema.0)
            .filter_map(declaration)
            .map(|(name, _)| name)
            .collect()
    }
}
