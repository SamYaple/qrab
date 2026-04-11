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
        "null" => "()".into(),
        // Special opaque types used by QOM commands (qom-get, qom-set, object-add)
        "Visitor" | "visitor" => "serde_json::Value".into(),
        "Dict" | "dict" => "serde_json::Value".into(),
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

pub fn rustify_name(input: &str) -> String {
    // Avoid collision with Rust built-in types
    if input == "String" {
        return "QapiString".to_string();
    }
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
/// Indented blocks (4+ spaces) are fenced with ` ```text` to prevent Rust doctests.
pub fn doc_to_tokens(doc: &str) -> proc_macro2::TokenStream {
    let mut result_lines: Vec<String> = Vec::new();
    let mut in_code_block = false;

    for line in doc.lines() {
        // Strip leading `# ` (QAPI comment prefix) if present
        let line = line
            .strip_prefix("# ")
            .or_else(|| line.strip_prefix("#"))
            .unwrap_or(line);

        let is_indented = line.starts_with("    ") && !line.trim().is_empty();

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
