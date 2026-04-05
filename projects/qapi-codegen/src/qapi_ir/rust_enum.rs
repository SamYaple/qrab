use super::{doc_to_tokens, generate_attribute, rustify_field, rustify_name, rustify_type, Metadata, StructField};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::cmp::Ordering;

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum EnumVariantKind {
    Unit,
    Tuple(String),
    Struct(Vec<StructField>),
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct EnumVariant {
    pub name: String,
    pub meta: Metadata,
    pub kind: EnumVariantKind,
    pub array: bool,
}

impl Ord for EnumVariant {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(&other.name)
    }
}

impl PartialOrd for EnumVariant {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Enum {
    pub name: String,
    pub meta: Metadata,
    pub variants: Vec<EnumVariant>,
}

impl Ord for Enum {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(&other.name)
    }
}

impl PartialOrd for Enum {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Enum {
    pub fn generate(&self) -> TokenStream {
        let enum_name = format_ident!("{}", rustify_type(&self.name));

        // Always emit at least #[qapi] so inner variant attrs get processed
        let enum_attrs: Vec<TokenStream> = if self.meta.attributes.is_empty() {
            vec![quote! { #[qapi] }]
        } else {
            self.meta.attributes.iter().map(generate_attribute).collect()
        };
        let enum_doc = self.meta.doc.as_ref().map(|doc| {
            doc_to_tokens(doc)
        });

        let variants = self.variants.iter().map(|variant| {
            let variant_name = format_ident!("{}", rustify_name(&variant.name));

            let variant_attrs = variant.meta.attributes.iter().map(generate_attribute);
            let variant_doc = variant.meta.doc.as_ref().map(|doc| doc_to_tokens(doc));

            match &variant.kind {
                EnumVariantKind::Unit => {
                    quote! {
                        #variant_doc
                        #(#variant_attrs)*
                        #variant_name,
                    }
                }
                EnumVariantKind::Tuple(type_name) => {
                    let type_tokens: TokenStream = rustify_type(type_name).parse().unwrap();
                    quote! {
                        #variant_doc
                        #(#variant_attrs)*
                        #variant_name(#type_tokens),
                    }
                }
                EnumVariantKind::Struct(fields) => {
                    let fields_tokens = fields.iter().map(|field| {
                        let field_name: TokenStream = rustify_field(&field.name).parse().unwrap();
                        let field_type: TokenStream = rustify_type(&field.r#type).parse().unwrap();

                        let field_attrs = field.meta.attributes.iter().map(generate_attribute);
                        let field_doc = field.meta.doc.as_ref().map(|doc| doc_to_tokens(doc));

                        quote! {
                            #field_doc
                            #(#field_attrs)*
                            #field_name: #field_type,
                        }
                    });

                    quote! {
                        #variant_doc
                        #(#variant_attrs)*
                        #variant_name {
                            #(#fields_tokens)*
                        },
                    }
                }
            }
        });

        quote! {
            #enum_doc
            #(#enum_attrs)*
            pub enum #enum_name {

                #(#variants)*
            }
        }
    }
}
