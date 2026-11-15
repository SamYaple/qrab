//! Plan generated symbols against declarations from every selected release.
use crate::{
    QapiSchema, QapiSchemaToken,
    qapi_ir::{Attribute, Enum, EnumVariantKind, Metadata, Struct, StructField, rustify_name},
};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Enum,
    Object,
    Alternate,
}

fn declaration<'a>(token: &'a QapiSchemaToken<'_>) -> Option<(&'a str, Kind)> {
    Some(match token {
        QapiSchemaToken::Enum(value) => (value.name.as_ref(), Kind::Enum),
        QapiSchemaToken::Struct(value) => (value.name.as_ref(), Kind::Object),
        QapiSchemaToken::Alternate(value) => (value.name.as_ref(), Kind::Alternate),
        QapiSchemaToken::Union(value) => (
            value.name.as_ref(),
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
    union_owners: BTreeSet<String>,
    aliases: BTreeMap<String, String>,
    shared_requests: BTreeSet<(String, String, (u32, u32, u32))>,
    shared_roots: HashSet<String>,
    declarations: BTreeMap<String, BTreeMap<(u32, u32, u32), Kind>>,
    type_runs: BTreeMap<String, BTreeMap<(u32, u32, u32), String>>,
}

impl SchemaNames {
    pub fn merge(&mut self, other: Self) {
        self.names.extend(other.names);
        self.roles.extend(other.roles);
        self.union_owners.extend(other.union_owners);
        self.shared_requests.extend(other.shared_requests);
        for (name, versions) in other.declarations {
            self.declarations.entry(name).or_default().extend(versions);
        }
    }

    pub fn collect(&mut self, schema: &QapiSchema<'_>) {
        for token in &schema.0 {
            if let Some((name, kind)) = declaration(token) {
                self.declarations
                    .entry(name.to_owned())
                    .or_default()
                    .insert(crate::parser_version(), kind);
            }
            let name = match token {
                QapiSchemaToken::Enum(value) => value.name.as_ref(),
                QapiSchemaToken::Struct(value) => value.name.as_ref(),
                QapiSchemaToken::Alternate(value) => value.name.as_ref(),
                QapiSchemaToken::Union(value) => value.name.as_ref(),
                QapiSchemaToken::Command(value) => value.name.as_ref(),
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
                    self.union_owners.insert(name.to_owned());
                }
                QapiSchemaToken::Command(value) => {
                    self.roles.entry(format!("{root}Command")).or_default();
                    if let Some(crate::MembersOrRef::Ref(data)) = &value.data
                        && root == rustify_name(data)
                    {
                        self.shared_requests.insert((
                            root.clone(),
                            data.to_string(),
                            crate::parser_version(),
                        ));
                    }
                }
                _ => {}
            }
        }
    }

    pub fn finish(&mut self) {
        let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for raw in self.declarations.keys() {
            groups
                .entry(rustify_name(raw))
                .or_default()
                .push(raw.clone());
        }
        let mut histories: BTreeMap<String, BTreeMap<(u32, u32, u32), Kind>> = BTreeMap::new();
        for (root, mut names) in groups {
            names.sort_by_key(|name| (name != &root, name.clone()));
            // Exclusive historical spellings can share a slot; coexisting types cannot.
            let mut slots: Vec<(String, Vec<String>)> = Vec::new();
            for raw in names {
                let versions = &self.declarations[&raw];
                let slot = slots.iter().position(|(_, members)| {
                    members.iter().all(|member| {
                        !versions
                            .keys()
                            .any(|version| self.declarations[member].contains_key(version))
                    })
                });
                let slot = slot.unwrap_or_else(|| {
                    let assigned = if slots.is_empty() {
                        root.clone()
                    } else {
                        let mut name = format!("{root}Generated");
                        let mut suffix = 2;
                        while !self.names.insert(name.clone()) {
                            name = format!("{root}Generated{suffix}");
                            suffix += 1;
                        }
                        name
                    };
                    slots.push((assigned, Vec::new()));
                    slots.len() - 1
                });
                let (assigned, members) = &mut slots[slot];
                if assigned != &root {
                    self.aliases.insert(raw.clone(), assigned.clone());
                }
                histories
                    .entry(assigned.clone())
                    .or_default()
                    .extend(versions);
                members.push(raw);
            }
        }
        // A return to an earlier kind starts a distinct lifetime as well.
        for (root, versions) in &histories {
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
        // A later named DTO can continue an earlier anonymous request type.
        for (root, data, version) in &self.shared_requests {
            if &self.type_name_at(data, *version) == root {
                self.shared_roots.insert(root.clone());
            }
        }
        for owner in &self.union_owners {
            let root = self.alias_name(owner);
            self.roles.entry(format!("{root}Branch")).or_default();
            self.roles.entry(format!("{root}Type")).or_default();
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

    pub fn alias_name(&self, name: &str) -> String {
        self.aliases
            .get(name)
            .cloned()
            .unwrap_or_else(|| rustify_name(name))
    }

    pub fn data_symbols(&self) -> HashSet<String> {
        self.declarations
            .keys()
            .map(|name| self.alias_name(name))
            .chain(
                self.type_runs
                    .values()
                    .flat_map(|runs| runs.values().cloned()),
            )
            .filter(|name| !self.shared_roots.contains(name))
            .collect()
    }

    pub fn type_name(&self, name: &str) -> String {
        self.type_name_at(name, crate::parser_version())
    }

    fn type_name_at(&self, name: &str, version: (u32, u32, u32)) -> String {
        let root = self.alias_name(name);
        self.type_runs
            .get(&root)
            .and_then(|runs| runs.range(..=version).next_back())
            .map_or(root.clone(), |(_, name)| name.clone())
    }

    pub fn rewrite_types(
        &self,
        declared: &HashSet<String>,
        enums: &mut HashMap<String, Enum>,
        structs: &mut HashMap<String, Struct>,
    ) {
        if self.type_runs.is_empty() && self.aliases.is_empty() {
            return;
        }
        let raw: HashMap<_, _> = declared
            .iter()
            .filter_map(|name| {
                let name = name.as_str();
                let assigned = self.type_name(name);
                (assigned != rustify_name(name)).then_some((name, assigned))
            })
            .collect();
        if raw.is_empty() {
            return;
        }
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
        for value in enums.values_mut() {
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
        }
        for value in structs.values_mut() {
            rewrite_name(&mut value.name, &mut value.meta);
            rewrite_fields(&mut value.fields);
        }
    }

    pub fn declared_names<'a>(schemas: &[(&std::path::Path, QapiSchema<'a>)]) -> HashSet<String> {
        schemas
            .iter()
            .flat_map(|(_, schema)| &schema.0)
            .filter_map(declaration)
            .map(|(name, _)| name.to_owned())
            .collect()
    }
}
