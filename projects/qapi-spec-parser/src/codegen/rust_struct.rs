use super::{rustify_field, rustify_type, Metadata};
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

#[derive(Debug, Eq, PartialEq)]
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

        let mut struct_attrs = Vec::new(); //vec![match_type(qtype)];
        for attr in &self.meta.attributes {
            let attr_name = format_ident!("{}", attr.name);
            struct_attrs.push(if let Some(attr_value) = &attr.value {
                quote! {
                    #[qapi(#attr_name = #attr_value)]
                }
            } else {
                quote! {
                    #[qapi(#attr_name)]
                }
            });
        }

        let struct_doc = self.meta.doc.as_ref().map(|doc| {
            quote! {
                #[doc = #doc]
            }
        });

        let fields = self.fields.iter().map(|field| {
            let field_name = format_ident!("{}", rustify_field(&field.name));
            let mut field_type: TokenStream = rustify_type(&field.r#type).parse().unwrap();
            if field.array {
                field_type = quote!( Vec<#field_type> );
            }
            if field.optional {
                field_type = quote!( Option<#field_type> );
            }

            let field_attrs = field.meta.attributes.iter().map(|attr| {
                let attr_name = format_ident!("{}", attr.name);
                if let Some(attr_value) = &attr.value {
                    quote! {
                        #[qapi(#attr_name = #attr_value)]
                    }
                } else {
                    quote! {
                        #[qapi(#attr_name)]
                    }
                }
            });

            let field_doc = field.meta.doc.as_ref().map(|doc| {
                quote! {
                    #[doc = #doc]
                }
            });

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
