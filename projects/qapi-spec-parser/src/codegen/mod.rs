use heck::{ToPascalCase, ToSnakeCase};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::cmp::Ordering;

mod rust_enum;
mod rust_struct;
pub use rust_enum::*;
pub use rust_struct::*;

pub enum QapiType {
    Enum,
    Struct,
    Union,
    Event,
    Alternate,
    Command,
}

fn qapi_to_rust_type(qapi_type: &str) -> String {
    match qapi_type {
        "str" => "String".into(),
        "number" => "f64".into(),
        "int" => "i64".into(),
        "int8" => "i8".into(),
        "int16" => "i16".into(),
        "int32" => "i32".into(),
        "int64" => "i64".into(),
        "uint8" => "u8".into(),
        "uint16" => "u16".into(),
        "uint32" => "u32".into(),
        "uint64" => "u64".into(),
        "size" => "u64".into(),
        "bool" => "bool".into(),
        "null" => "FIXME_null_type".into(),
        "any" => "FIXME_any_type".into(),
        _ => qapi_type.to_pascal_case(),
    }
}

fn fix_leading_digit(input: &str) -> String {
    if input
        .chars()
        .next()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(false)
    {
        format!("_{}", input)
    } else {
        input.to_string()
    }
}

fn rustify_type(input: &str) -> String {
    let input = qapi_to_rust_type(input);
    fix_leading_digit(&input)
}

fn rustify_field(input: &str) -> String {
    let input = match input {
        "type" | "abstract" | "in" | "static" | "if" | "match" => format! {"r#{}", input},
        _ => input.to_string(),
    };
    input.to_snake_case()
}

fn match_type(qtype: QapiType) -> TokenStream {
    match qtype {
        QapiType::Enum => quote! {#[derive(QapiEnum)]},
        QapiType::Alternate => quote! {#[derive(QapiAlternate)]},
        QapiType::Union => quote! {#[derive(QapiUnion)]},
        QapiType::Struct => quote! {#[derive(QapiStruct)]},
        QapiType::Event => quote! {#[derive(QapiEvent)]},
        QapiType::Command => quote! {#[derive(QapiCommand)]},
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Attribute {
    pub name: String,
    pub value: Option<String>,
}

impl Attribute {
    pub fn new<T>(name: T, value: Option<T>) -> Self
    where
        String: From<T>,
    {
        let value = if let Some(v) = value {
            Some(v.into())
        } else {
            None
        };
        Self {
            name: name.into(),
            value,
        }
    }
}

impl Ord for Attribute {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(&other.name)
    }
}

impl PartialOrd for Attribute {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Default, Eq, PartialEq, Clone)]
pub struct Metadata {
    pub doc: Option<String>,
    pub attributes: Vec<Attribute>,
}

#[derive(Debug, Default)]
pub struct Schema {
    pub alternates: Vec<Enum>,
    pub enums: Vec<Enum>,
    pub unions: Vec<Enum>,
    pub structs: Vec<Struct>,
    pub events: Vec<Struct>,
    pub commands: Vec<Struct>,
}
