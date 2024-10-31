use super::{rustify_field, rustify_type, Metadata, StructField};
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

        let mut enum_attrs = Vec::new(); // vec![match_type(qtype)];
        for attr in &self.meta.attributes {
            let attr_name = format_ident!("{}", attr.name);
            let attr_value = &attr.value;
            enum_attrs.push(if let Some(attr_value) = &attr.value {
                quote! {
                    #[qapi(#attr_name = #attr_value)]
                }
            } else {
                quote! {
                    #[qapi(#attr_name)]
                }
            });
        }

        let enum_doc = self.meta.doc.as_ref().map(|doc| {
            quote! {
                #[doc = #doc]
            }
        });

        let variants = self.variants.iter().map(|variant| {
            let variant_name = format_ident!("{}", rustify_type(&variant.name));

            let variant_attrs = variant.meta.attributes.iter().map(|attr| {
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

            let variant_doc = variant.meta.doc.as_ref().map(|doc| {
                quote! {
                    #[doc = #doc]
                }
            });

            match &variant.kind {
                EnumVariantKind::Unit => {
                    quote! {
                        #variant_doc
                        #(#variant_attrs)*
                        #variant_name,
                    }
                }
                EnumVariantKind::Tuple(type_name) => {
                    let type_ident = format_ident!("{}", rustify_type(type_name));
                    quote! {
                        #variant_doc
                        #(#variant_attrs)*
                        #variant_name(#type_ident),
                    }
                }
                EnumVariantKind::Struct(fields) => {
                    let fields_tokens = fields.iter().map(|field| {
                        let field_name = format_ident!("{}", rustify_field(&field.name));
                        let field_type = format_ident!("{}", field.r#type);

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
