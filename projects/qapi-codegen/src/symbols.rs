//! Plan generated symbols against declarations from every selected release.
use crate::{QapiSchema, QapiSchemaToken, qapi_ir::rustify_name};
use std::collections::{BTreeMap, HashSet};

#[derive(Default)]
pub(crate) struct SchemaNames {
    pub names: HashSet<String>,
    roles: BTreeMap<String, String>,
}

impl SchemaNames {
    pub fn merge(&mut self, other: Self) {
        self.names.extend(other.names);
        self.roles.extend(other.roles);
    }

    pub fn collect(&mut self, schema: &QapiSchema<'_>) {
        for token in &schema.0 {
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
}
