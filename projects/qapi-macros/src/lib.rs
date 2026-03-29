use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    parse_macro_input, AttributeArgs, DeriveInput, Fields, Lit, Meta, MetaList, MetaNameValue,
    NestedMeta, Type,
};

#[proc_macro_attribute]
pub fn qapi(args: TokenStream, input: TokenStream) -> TokenStream {
    // Parse the attribute arguments
    let qapi_args = parse_macro_input!(args as AttributeArgs);

    // Parse the input tokens into a syntax tree
    let mut input_ast = parse_macro_input!(input as DeriveInput);

    // Process struct-level attributes
    let struct_since = extract_since_attribute(&qapi_args);

    // Collect field data
    let mut field_data_list = Vec::new();

    match &mut input_ast.data {
        syn::Data::Struct(data_struct) => {
            if let Fields::Named(fields_named) = &mut data_struct.fields {
                for field in &mut fields_named.named {
                    let mut rename: Option<String> = None;
                    let mut since: Option<String> = None;
                    let mut condition: Option<String> = None;

                    for attr in &field.attrs {
                        if attr.path.is_ident("qapi") {
                            let meta = attr.parse_meta().unwrap();
                            if let Meta::List(meta_list) = meta {
                                let attr_data = process_field_qapi_attributes(&meta_list);
                                if attr_data.rename.is_some() {
                                    rename = attr_data.rename;
                                }
                                if attr_data.since.is_some() {
                                    since = attr_data.since;
                                }
                                if attr_data.condition.is_some() {
                                    condition = attr_data.condition;
                                }
                            }
                        }
                    }

                    // Remove the original qapi attributes
                    field.attrs.retain(|attr| !attr.path.is_ident("qapi"));

                    // Add new attributes to the field
                    if let Some(ref rename_value) = rename {
                        let serde_attr: syn::Attribute = syn::parse_quote! {
                            #[serde(rename = #rename_value)]
                        };
                        field.attrs.push(serde_attr);
                    }

                    field_data_list.push(FieldData {
                        field: field.clone(),
                        rename,
                        since,
                        condition,
                    });
                }
            }
        }
        syn::Data::Enum(data_enum) => {
            let mut variant_data_list = Vec::new();

            for variant in &mut data_enum.variants {
                let mut rename: Option<String> = None;
                let mut since: Option<String> = None;
                let mut condition: Option<String> = None;

                for attr in &variant.attrs {
                    if attr.path.is_ident("qapi") {
                        let meta = attr.parse_meta().unwrap();
                        if let Meta::List(meta_list) = meta {
                            let attr_data = process_field_qapi_attributes(&meta_list);
                            if attr_data.rename.is_some() {
                                rename = attr_data.rename;
                            }
                            if attr_data.since.is_some() {
                                since = attr_data.since;
                            }
                            if attr_data.condition.is_some() {
                                condition = attr_data.condition;
                            }
                        }
                    }
                }

                // Remove the original qapi attributes from the variant
                variant.attrs.retain(|attr| !attr.path.is_ident("qapi"));

                // Add new attributes to the variant
                if let Some(ref rename_value) = rename {
                    let serde_attr: syn::Attribute = syn::parse_quote! {
                        #[serde(rename = #rename_value)]
                    };
                    variant.attrs.push(serde_attr);
                }

                // Collect variant data if needed
                variant_data_list.push(VariantData {
                    variant: variant.clone(),
                    rename,
                    since,
                    condition,
                });
            }
        }
        _ => {}
    }

    // Generate custom Serialize implementation
    let serialize_impl = generate_serialize_impl(&input_ast, struct_since, &field_data_list);

    // Generate the output tokens
    let expanded = quote! {
        #input_ast

        #serialize_impl
    };

    TokenStream::from(expanded)
}

fn extract_since_attribute(args: &AttributeArgs) -> Option<String> {
    for arg in args {
        if let NestedMeta::Meta(Meta::NameValue(MetaNameValue { path, lit, .. })) = arg {
            if path.is_ident("since") {
                if let Lit::Str(lit_str) = lit {
                    return Some(lit_str.value());
                }
            }
        }
    }
    None
}

struct FieldAttributeData {
    rename: Option<String>,
    since: Option<String>,
    condition: Option<String>,
}

fn process_field_qapi_attributes(meta_list: &MetaList) -> FieldAttributeData {
    let mut rename: Option<String> = None;
    let mut since: Option<String> = None;
    let mut condition: Option<String> = None;

    for nested_meta in &meta_list.nested {
        if let NestedMeta::Meta(Meta::NameValue(MetaNameValue { path, lit, .. })) = nested_meta {
            if path.is_ident("name") {
                if let Lit::Str(lit_str) = lit {
                    rename = Some(lit_str.value());
                }
            } else if path.is_ident("since") {
                if let Lit::Str(lit_str) = lit {
                    since = Some(lit_str.value());
                }
            } else if path.is_ident("condition") {
                if let Lit::Str(lit_str) = lit {
                    condition = Some(lit_str.value());
                }
            }
        }
    }

    FieldAttributeData {
        rename,
        since,
        condition,
    }
}

struct VariantData {
    variant: syn::Variant,
    rename: Option<String>,
    since: Option<String>,
    condition: Option<String>,
}

struct FieldData {
    field: syn::Field,
    rename: Option<String>,
    since: Option<String>,
    condition: Option<String>,
}

fn generate_serialize_impl(
    input_ast: &DeriveInput,
    struct_since: Option<String>,
    field_data_list: &[FieldData],
) -> TokenStream2 {
    let struct_name = &input_ast.ident;

    // Collect field serialization code
    let mut serialize_fields = Vec::new();

    for field_data in field_data_list {
        let field = &field_data.field;
        let field_name = &field.ident;
        let field_name_str = field_name.as_ref().unwrap().to_string();
        let field_ty = &field.ty;

        let field_since_version = field_data.since.clone();
        let condition = field_data.condition.clone();

        // Determine the effective since version
        let effective_since = field_since_version.or_else(|| struct_since.clone());

        let is_option = matches!(field_ty, Type::Path(type_path) if type_path.path.segments.last().unwrap().ident == "Option");

        // Generate code considering condition and since
        let mut serialize_code = TokenStream2::new();

        if let Some(cond) = condition {
            if let Some(cfg_meta) = map_condition_to_cfg(&cond) {
                serialize_code.extend(quote! {
                    #[#cfg_meta]
                });
            }
        }

        // Generate serialization code with runtime version check
        let field_serialize_code = if is_option {
            if let Some(version) = effective_since {
                quote! {
                    if runtime_version() >= #version {
                        if let Some(ref value) = self.#field_name {
                            state.serialize_field(#field_name_str, value)?;
                        }
                    }
                }
            } else {
                quote! {
                    if let Some(ref value) = self.#field_name {
                        state.serialize_field(#field_name_str, value)?;
                    }
                }
            }
        } else {
            if let Some(version) = effective_since {
                quote! {
                    if runtime_version() >= #version {
                        state.serialize_field(#field_name_str, &self.#field_name)?;
                    }
                }
            } else {
                quote! {
                    state.serialize_field(#field_name_str, &self.#field_name)?;
                }
            }
        };

        serialize_code.extend(field_serialize_code);

        serialize_fields.push(serialize_code);
    }

    let serializer_core = quote! {
        let mut state = serializer.serialize_struct(stringify!(#struct_name), 0)?;
        #(#serialize_fields)*
        state.end()
    };
    let runtime_version_check = if let Some(version) = struct_since {
        quote! {
            if runtime_version() >= #version {
                #serializer_core
            } else {
                Err(serde::ser::Error::custom(format!("Struct {} not available in runtime version", stringify!(#struct_name))))
            }
        }
    } else {
        serializer_core
    };

    quote! {
        impl serde::Serialize for #struct_name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                #runtime_version_check
                //#serializer_core
            }
        }
    }
}

fn map_condition_to_cfg(condition: &str) -> Option<syn::Meta> {
    //match condition {
    //    "CONFIG_LINUX" => {
    //        // Map CONFIG_LINUX to cfg(target_os = "linux")
    //        Some(syn::parse_quote!(cfg(target_os = "linux")))
    //    },
    //    // Map other conditions as needed
    //    _ => None,
    //}
    None
}
