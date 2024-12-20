/// qir -- QAPI Intermediate Representation
///
/// The qapi dsl changes from time to time, but the representation in rust
/// remains the same in rust. This IR serves to seperate the parser structs
/// from the codegen structs. All documentation is also properly merged into
/// these structs using the doc attr: `#[doc("Some flavor text")]`
use heck::{ToPascalCase, ToSnakeCase};

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

mod process_qapi;
pub use process_qapi::*;

mod rust_enum;
mod rust_struct;
pub use rust_enum::*;
pub use rust_struct::*;

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
        "any" => "serde_json::Value".into(),
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

fn rustify_name(input: &str) -> String {
    let input = input.to_pascal_case();
    fix_leading_digit(&input)
}

fn rustify_field_name(input: &str) -> String {
    let input = input.to_snake_case();
    fix_leading_digit(&input)
}

fn rustify_type(input: &str) -> String {
    let input = qapi_to_rust_type(input);
    fix_leading_digit(&input)
}

fn rustify_field(input: &str) -> String {
    match input {
        "type" | "abstract" | "in" | "static" | "if" | "match" | "use" => format! {"r#{}", input},
        _ => rustify_field_name(input),
    }
}

pub fn generate_attribute(attribute: &Attribute) -> TokenStream {
    let options = match attribute {
        Attribute::List(attributes) => attributes
            .iter()
            .flat_map(generate_options)
            .collect::<Vec<_>>(),
        _ => generate_options(attribute),
    };
    quote! {
        #[qapi( #(#options),* )]
    }
}

pub fn generate_options(attribute: &Attribute) -> Vec<TokenStream> {
    match attribute {
        Attribute::Item { name, value } => {
            let name = format_ident!("{}", name);
            vec![quote! { #name = #value }]
        }
        Attribute::Unit(name) => {
            let name = format_ident!("{}", name);
            vec![quote! { #name }]
        }
        Attribute::List(attributes) => attributes.iter().flat_map(generate_options).collect(),
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum Attribute {
    Item { name: String, value: String },
    List(Vec<Attribute>),
    Unit(String),
}

impl Attribute {
    pub fn new<N: ToString>(name: N) -> Self {
        Self::Unit(name.to_string())
    }

    pub fn with_value<N: ToString, V: ToString>(name: N, value: V) -> Self {
        Self::Item {
            name: name.to_string(),
            value: value.to_string(),
        }
    }

    pub fn with_values<N: ToString, V: ToString>(values: Vec<(N, Option<V>)>) -> Self {
        let attrs = values
            .iter()
            .map(|i| {
                let name = i.0.to_string();
                if let Some(value) = &i.1 {
                    Self::Item {
                        name: name,
                        value: value.to_string(),
                    }
                } else {
                    Self::Unit(name)
                }
            })
            .collect();
        Self::List(attrs)
    }
}

#[derive(Debug, Default, Eq, PartialEq, Clone)]
pub struct Metadata {
    pub doc: Option<String>,
    pub attributes: Vec<Attribute>,
}
