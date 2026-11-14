use super::{Metadata, doc_to_tokens, generate_attribute, rustify_field, rustify_type};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::cmp::Ordering;

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct StructField {
    pub name: String,
    pub meta: Metadata,
    pub r#type: String,
    pub optional: bool,
    pub array: bool,
}

impl Ord for StructField {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(&other.name)
    }
}

impl PartialOrd for StructField {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Struct {
    pub name: String,
    pub meta: Metadata,
    pub fields: Vec<StructField>,
}

impl Ord for Struct {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(&other.name)
    }
}

impl PartialOrd for Struct {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Struct {
    pub fn generate(&self) -> TokenStream {
        let struct_name = format_ident!("{}", rustify_type(&self.name));
        // Always emit at least #[qapi] so inner field attrs get processed
        let struct_attrs: Vec<TokenStream> = if self.meta.attributes.is_empty() {
            vec![quote! { #[qapi] }]
        } else {
            self.meta
                .attributes
                .iter()
                .map(generate_attribute)
                .collect()
        };
        let struct_doc = self.meta.doc.as_ref().map(|doc| doc_to_tokens(doc));

        let fields = self.fields.iter().map(|field| {
            let field_name: TokenStream = rustify_field(&field.name).parse().unwrap();
            let mut field_type: TokenStream = rustify_type(&field.r#type).parse().unwrap();
            if field.array {
                field_type = quote!( Vec<#field_type> );
            }
            // Check if field needs Box wrapping (recursive types)
            let is_boxed = field
                .meta
                .attributes
                .iter()
                .any(|a| matches!(a, super::Attribute::Unit(n) if n == "boxed"));
            if is_boxed {
                field_type = quote!( Box<#field_type> );
            }
            if field.optional {
                field_type = quote!( Option<#field_type> );
            }

            // Filter out the "boxed" attribute — it's internal, not a qapi attr
            let field_attrs = field
                .meta
                .attributes
                .iter()
                .filter(|a| !matches!(a, super::Attribute::Unit(n) if n == "boxed"))
                .map(generate_attribute);
            let field_doc = field.meta.doc.as_ref().map(|doc| doc_to_tokens(doc));

            quote! {
                 #field_doc
                 #(#field_attrs)*
                 pub #field_name: #field_type,

            }
        });

        quote! {
            #struct_doc
            #(#struct_attrs)*
            pub struct #struct_name {
                #(#fields)*

            }
        }
    }
}
