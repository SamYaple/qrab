use heck::{ToPascalCase, ToSnakeCase};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::cmp::Ordering;

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

impl Struct {
    pub fn generate(&self, qtype: QapiType) -> TokenStream {
        let struct_name = format_ident!("{}", rustify_type(&self.name));

        let mut struct_attrs = vec![match_type(qtype)];
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

impl Enum {
    pub fn generate(&self, qtype: QapiType) -> TokenStream {
        let enum_name = format_ident!("{}", rustify_type(&self.name));

        let mut enum_attrs = vec![match_type(qtype)];
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
                EnumVariantKind::QapiEnum => {
                    quote! {
                        #variant_doc
                        #(#variant_attrs)*
                        #variant_name,
                    }
                }
                EnumVariantKind::QapiAlternate(type_name) => {
                    let type_ident = format_ident!("{}", rustify_type(type_name));
                    quote! {
                        #variant_doc
                        #(#variant_attrs)*
                        #variant_name(#type_ident),
                    }
                }
                EnumVariantKind::QapiUnion(fields) => {
                    let fields_tokens = fields.iter().map(|field| {
                        let field_name = format_ident!("{}", field.name);
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
                            #(#fields_tokens),*
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

#[derive(Debug, Eq, PartialEq)]
pub enum EnumVariantKind {
    QapiEnum,
    QapiAlternate(String),
    QapiUnion(Vec<StructField>),
}

#[derive(Debug, Eq, PartialEq)]
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

#[derive(Debug, Eq, PartialEq)]
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

#[derive(Debug, Default)]
pub struct Schema {
    pub alternates: Vec<Enum>,
    pub enums: Vec<Enum>,
    pub unions: Vec<Enum>,
    pub structs: Vec<Struct>,
    pub events: Vec<Struct>,
    pub commands: Vec<Struct>,
}
