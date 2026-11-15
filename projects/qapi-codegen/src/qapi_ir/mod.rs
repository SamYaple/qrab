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

// Legacy QNames can contain Unicode outside Rust's identifier grammar.
// ASCII escapes also keep Rust normalization from merging distinct wire names.
fn ascii_identifier(input: &str) -> std::borrow::Cow<'_, str> {
    if input.is_ascii() {
        return input.into();
    }
    let mut name = String::with_capacity(input.len());
    for c in input.chars() {
        if c.is_ascii() {
            name.push(c);
        } else {
            use std::fmt::Write;
            write!(name, "_u{:x}_", c as u32).unwrap();
        }
    }
    name.into()
}

fn pascal_identifier(input: &str) -> String {
    let name = ascii_identifier(input).to_pascal_case();
    // Escapes can introduce an acronym boundary (E + U301); settle its case
    // before the resulting symbol is used again as a generated type reference.
    let name = if input.is_ascii() {
        name
    } else {
        name.to_pascal_case()
    };
    fix_leading_digit(&name)
}

/// Map a type name without shadowing Rust types, constructors or runtime traits.
pub fn rustify_name(input: &str) -> String {
    let name = pascal_identifier(input);
    match name.as_str() {
        "" => "QapiUnnamed".into(),
        "Self" | "String" | "Option" | "Vec" | "Box" | "Result" | "HashMap" | "Send" | "Sync"
        | "Sized" | "Drop" | "Fn" | "FnMut" | "FnOnce" | "Some" | "None" | "Ok" | "Err" => {
            format!("Qapi{name}")
        }
        _ => name,
    }
}

/// Enum variants have their own namespace; String and None remain ordinary variants.
pub(crate) fn rustify_variant(input: &str) -> String {
    let name = pascal_identifier(input);
    if name.is_empty() {
        "QapiUnnamed".into()
    } else if name == "Self" {
        "QapiSelf".into()
    } else {
        name
    }
}

fn rustify_field_name(input: &str) -> String {
    let name = fix_leading_digit(&ascii_identifier(input).to_snake_case());
    match name.as_str() {
        "" => "qapi_unnamed".into(),
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

/// Render QAPI reStructuredText prose as literal-safe Rustdoc.
fn rustdoc_prose(line: &str) -> String {
    let trimmed = line.trim();
    if (trimmed.starts_with("https://") || trimmed.starts_with("http://"))
        && !trimmed.chars().any(char::is_whitespace)
    {
        return line.replacen(trimmed, &format!("<{trimmed}>"), 1);
    }
    let mut output = String::with_capacity(line.len());
    let mut rest = line;
    let mut code_ticks = 0;
    while !rest.is_empty() {
        if code_ticks == 0
            && rest.starts_with('\\')
            && let Some(c) = rest[1..].chars().next()
            && matches!(c, '[' | ']' | '<' | '>' | '`' | '\\')
        {
            let end = 1 + c.len_utf8();
            output.push_str(&rest[..end]);
            rest = &rest[end..];
            continue;
        }
        if rest.starts_with('`') {
            let count = rest.bytes().take_while(|byte| *byte == b'`').count();
            if code_ticks == 0 {
                code_ticks = count;
            } else if code_ticks == count {
                code_ticks = 0;
            }
            output.push_str(&rest[..count]);
            rest = &rest[count..];
            continue;
        }
        if code_ticks == 0 {
            // Preserve existing Markdown links and autolinks.
            let link_end = if rest.starts_with('[') {
                rest.find("](")
                    .and_then(|label| rest[label + 2..].find(')').map(|end| label + 3 + end))
            } else if rest.starts_with("<https://") || rest.starts_with("<http://") {
                rest.find('>').map(|end| end + 1)
            } else {
                None
            };
            if let Some(end) = link_end {
                output.push_str(&rest[..end]);
                rest = &rest[end..];
                continue;
            }
        }
        let c = rest.chars().next().unwrap();
        if code_ticks == 0 {
            match c {
                '[' => output.push_str("\\["),
                ']' => output.push_str("\\]"),
                '<' => output.push_str("&lt;"),
                '>' => output.push_str("&gt;"),
                _ => output.push(c),
            }
        } else {
            output.push(c);
        }
        rest = &rest[c.len_utf8()..];
    }
    output
}

/// Convert a doc string into `#[doc = "..."]` token stream.
/// Each line gets its own `#[doc]` attribute so `prettyplease` renders them as `///`.
/// Fence indented schema prose as text, accounting for the doc-comment space.
pub fn doc_to_tokens(doc: &str) -> proc_macro2::TokenStream {
    let mut result_lines: Vec<String> = Vec::new();
    let mut in_code_block = false;
    let mut explicit_fence = None;

    for line in doc.lines() {
        let is_indented =
            (line.starts_with("   ") || line.starts_with('\t')) && !line.trim().is_empty();

        let marker = line.trim_start();
        let fence = marker
            .chars()
            .next()
            .filter(|c| matches!(c, '`' | '~'))
            .and_then(|c| {
                let count = marker.chars().take_while(|next| *next == c).count();
                (count >= 3).then_some((c, count))
            });
        if let Some((character, count)) = fence.filter(|_| explicit_fence.is_some() || !is_indented)
        {
            if let Some((open, length)) = explicit_fence {
                if character == open && count >= length && marker[count..].trim().is_empty() {
                    explicit_fence = None;
                }
            } else if !is_indented {
                if in_code_block {
                    result_lines.push(" ```".into());
                    in_code_block = false;
                }
                explicit_fence = Some((character, count));
            }
            result_lines.push(format!(" {line}"));
            continue;
        }
        if explicit_fence.is_some() {
            result_lines.push(format!(" {line}"));
            continue;
        }

        if is_indented && !in_code_block {
            result_lines.push(" ```text".to_string());
            in_code_block = true;
        } else if !is_indented && in_code_block {
            result_lines.push(" ```".to_string());
            in_code_block = false;
        }

        let line = if in_code_block {
            line.to_string()
        } else {
            rustdoc_prose(line)
        };
        result_lines.push(format!(" {line}"));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_prose_preserves_literals_and_links_in_rustdoc() {
        assert_eq!(
            rustdoc_prose(r"Escaped \[4\] \<id\> \`text"),
            r"Escaped \[4\] \<id\> \`text"
        );
        assert_eq!(
            rustdoc_prose("Bit[3:0] and child<subtype>"),
            "Bit\\[3:0\\] and child&lt;subtype&gt;"
        );
        assert_eq!(
            rustdoc_prose("Use `Bit[4] <id>` or ``[literal]<id>``."),
            "Use `Bit[4] <id>` or ``[literal]<id>``."
        );
        assert_eq!(
            rustdoc_prose("https://example.test/path#anchor"),
            "<https://example.test/path#anchor>"
        );
        assert_eq!(
            rustdoc_prose("[link](https://example.test/) and <https://example.test/>"),
            "[link](https://example.test/) and <https://example.test/>"
        );
        let tokens = doc_to_tokens("Text [4]\n```text\n    <id> [4]\n```\nAfter <id>");
        let item: syn::ItemStruct = syn::parse2(quote! { #tokens struct Example; }).unwrap();
        let lines: Vec<_> = item
            .attrs
            .iter()
            .map(|attr| {
                let syn::Meta::NameValue(value) = &attr.meta else {
                    panic!("doc attribute")
                };
                let syn::Expr::Lit(value) = &value.value else {
                    panic!("doc literal")
                };
                let syn::Lit::Str(value) = &value.lit else {
                    panic!("doc string")
                };
                value.value()
            })
            .collect();
        assert_eq!(
            lines,
            [
                " Text \\[4\\]",
                " ```text",
                "     <id> [4]",
                " ```",
                " After &lt;id&gt;"
            ]
        );
    }

    #[test]
    fn legacy_unicode_names_have_stable_ascii_identifiers() {
        for raw in ["١Thing", "²", "É", "E\u{301}", "𐀀", "Ⅸ", "汉字", "Καλημέρα"]
        {
            let name = rustify_name(raw);
            assert!(name.is_ascii(), "{raw}: {name}");
            syn::parse_str::<syn::Ident>(&name).unwrap();
            assert_eq!(rustify_name(&name), name);
            let field = rustify_field(raw);
            assert!(field.is_ascii(), "{raw}: {field}");
            syn::parse_str::<syn::Ident>(&field).unwrap();
            assert_eq!(rustify_field(&field), field);
            let variant = rustify_variant(raw);
            syn::parse_str::<syn::Ident>(&variant).unwrap();
            assert_eq!(rustify_variant(&variant), variant);
        }
    }
}
