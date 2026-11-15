//! Rust IR for QAPI definitions, wire metadata and generated documentation.

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
        "null" => "()".into(),
        // Historical opaque references are lowercase; Visitor and Dict may be real types.
        "visitor" | "dict" => "serde_json::Value".into(),
        // Wildcard type used in older versions (e.g., netdev_add '*props': '**')
        "**" => "serde_json::Value".into(),
        // Catch-all map for commands with 'gen': false (e.g., device_add)
        "HashMap<String, serde_json::Value>" => "HashMap<String, serde_json::Value>".into(),
        _ => rustify_name(qapi_type),
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

/// Map a type name without shadowing Rust types, constructors or runtime traits.
pub fn rustify_name(input: &str) -> String {
    let name = fix_leading_digit(&input.to_pascal_case());
    match name.as_str() {
        "Self" | "String" | "Option" | "Vec" | "Box" | "Result" | "HashMap" | "Send" | "Sync"
        | "Sized" | "Drop" | "Fn" | "FnMut" | "FnOnce" | "Some" | "None" | "Ok" | "Err" => {
            format!("Qapi{name}")
        }
        _ => name,
    }
}

/// Enum variants have their own namespace; String and None remain ordinary variants.
pub(crate) fn rustify_variant(input: &str) -> String {
    let name = fix_leading_digit(&input.to_pascal_case());
    if name == "Self" {
        "QapiSelf".into()
    } else {
        name
    }
}

fn rustify_field_name(input: &str) -> String {
    let name = fix_leading_digit(&input.to_snake_case());
    match name.as_str() {
        "self" | "super" | "crate" => format!("{name}_"),
        _ => name,
    }
}

pub(crate) fn rustify_type(input: &str) -> String {
    let input = qapi_to_rust_type(input);
    fix_leading_digit(&input)
}

pub(crate) fn rustify_field(input: &str) -> String {
    // Check if the snake_case version of the input is a Rust keyword
    let snake = rustify_field_name(input);
    if syn::parse_str::<syn::Ident>(&snake).is_err() {
        format!("r#{}", snake)
    } else {
        snake
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
        Attribute::Nested { name, inner } => {
            let name = format_ident!("{}", name);
            let inner_opts: Vec<TokenStream> = inner.iter().flat_map(generate_options).collect();
            vec![quote! { #name(#(#inner_opts),*) }]
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum Attribute {
    Item {
        name: String,
        value: String,
    },
    List(Vec<Attribute>),
    Unit(String),
    /// Nested attribute: `name(key = "value", ...)` e.g. `allow_preconfig(added = "7.1.0")`
    Nested {
        name: String,
        inner: Vec<Attribute>,
    },
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
                        name,
                        value: value.to_string(),
                    }
                } else {
                    Self::Unit(name)
                }
            })
            .collect();
        Self::List(attrs)
    }

    /// Create a nested attribute like `name(added = "1.0.0", removed = "2.0.0")`.
    pub fn nested<N: ToString>(name: N, inner: Vec<Attribute>) -> Self {
        Self::Nested {
            name: name.to_string(),
            inner,
        }
    }
}

#[derive(Debug, Default, Eq, PartialEq, Clone)]
pub struct Metadata {
    pub doc: Option<String>,
    pub attributes: Vec<Attribute>,
}

/// Convert a doc string into `#[doc = "..."]` token stream.
/// Each line gets its own `#[doc]` attribute so `prettyplease` renders them as `///`.
/// Fence indented schema prose as text, accounting for the doc-comment space.
pub fn doc_to_tokens(doc: &str) -> proc_macro2::TokenStream {
    let mut result_lines: Vec<String> = Vec::new();
    let mut in_code_block = false;

    for line in doc.lines() {
        let is_indented =
            (line.starts_with("   ") || line.starts_with('\t')) && !line.trim().is_empty();

        if is_indented && !in_code_block {
            result_lines.push(" ```text".to_string());
            in_code_block = true;
        } else if !is_indented && in_code_block {
            result_lines.push(" ```".to_string());
            in_code_block = false;
        }

        result_lines.push(format!(" {}", line));
    }

    if in_code_block {
        result_lines.push(" ```".to_string());
    }

    let tokens: Vec<proc_macro2::TokenStream> = result_lines
        .iter()
        .map(|line| {
            quote! { #[doc = #line] }
        })
        .collect();
    quote! { #(#tokens)* }
}
