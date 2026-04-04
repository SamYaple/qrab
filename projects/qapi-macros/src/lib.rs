use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    ext::IdentExt,
    parse_macro_input, AttributeArgs, DeriveInput, Fields, Lit, Meta, MetaList, NestedMeta, Type,
};

// ─── Attribute structs ───

#[derive(Default)]
struct TypeAttrs {
    name: Option<String>,
    since: Option<String>,
    until: Option<String>,
    condition: Option<String>,
    returns: Option<String>,
    allow_oob: bool,
    allow_preconfig: bool,
    no_success_response: bool,
    is_command: bool,
    is_event: bool,
}

#[derive(Default, Clone)]
struct ItemAttrs {
    name: Option<String>,
    since: Option<String>,
    until: Option<String>,
    condition: Option<String>,
    feature: Option<String>,
    flatten: bool,
    discriminator: bool,
    union_field: bool,
}

// ─── Parsing ───

fn parse_type_attrs(args: &[NestedMeta]) -> TypeAttrs {
    let mut attrs = TypeAttrs::default();
    for arg in args {
        match arg {
            NestedMeta::Meta(Meta::NameValue(nv)) => {
                if let Lit::Str(lit_str) = &nv.lit {
                    let val = lit_str.value();
                    if nv.path.is_ident("name") {
                        attrs.name = Some(val);
                    } else if nv.path.is_ident("since") {
                        attrs.since = Some(val);
                    } else if nv.path.is_ident("until") {
                        attrs.until = Some(val);
                    } else if nv.path.is_ident("condition") {
                        attrs.condition = Some(val);
                    } else if nv.path.is_ident("returns") {
                        attrs.returns = Some(val);
                        attrs.is_command = true;
                    }
                }
            }
            NestedMeta::Meta(Meta::Path(path)) => {
                if path.is_ident("allow_oob") {
                    attrs.allow_oob = true;
                    attrs.is_command = true;
                } else if path.is_ident("allow_preconfig") {
                    attrs.allow_preconfig = true;
                    attrs.is_command = true;
                } else if path.is_ident("no_success_response") {
                    attrs.no_success_response = true;
                    attrs.is_command = true;
                } else if path.is_ident("command") {
                    attrs.is_command = true;
                } else if path.is_ident("event") {
                    attrs.is_event = true;
                }
            }
            _ => {}
        }
    }
    attrs
}

fn parse_item_attrs(meta_list: &MetaList) -> ItemAttrs {
    let mut attrs = ItemAttrs::default();
    for nested_meta in &meta_list.nested {
        match nested_meta {
            NestedMeta::Meta(Meta::NameValue(nv)) => {
                if let Lit::Str(lit_str) = &nv.lit {
                    let val = lit_str.value();
                    if nv.path.is_ident("name") {
                        attrs.name = Some(val);
                    } else if nv.path.is_ident("since") {
                        attrs.since = Some(val);
                    } else if nv.path.is_ident("until") {
                        attrs.until = Some(val);
                    } else if nv.path.is_ident("condition") {
                        attrs.condition = Some(val);
                    } else if nv.path.is_ident("feature") {
                        attrs.feature = Some(val);
                    }
                }
            }
            NestedMeta::Meta(Meta::Path(path)) => {
                if path.is_ident("flatten") {
                    attrs.flatten = true;
                } else if path.is_ident("discriminator") {
                    attrs.discriminator = true;
                } else if path.is_ident("union") {
                    attrs.union_field = true;
                }
            }
            _ => {}
        }
    }
    attrs
}

fn extract_all_item_attrs(syn_attrs: &[syn::Attribute]) -> ItemAttrs {
    let mut result = ItemAttrs::default();
    for attr in syn_attrs {
        if attr.path.is_ident("qapi") {
            if let Ok(Meta::List(meta_list)) = attr.parse_meta() {
                let parsed = parse_item_attrs(&meta_list);
                result.name = result.name.or(parsed.name);
                result.since = result.since.or(parsed.since);
                result.until = result.until.or(parsed.until);
                result.condition = result.condition.or(parsed.condition);
                result.feature = result.feature.or(parsed.feature);
                result.flatten = result.flatten || parsed.flatten;
                result.discriminator = result.discriminator || parsed.discriminator;
                result.union_field = result.union_field || parsed.union_field;
            }
        }
    }
    result
}

// ─── Helpers ───

fn wire_name(attrs_name: &Option<String>, ident: &syn::Ident) -> String {
    attrs_name.clone().unwrap_or_else(|| ident.unraw().to_string())
}

fn parse_version(version: &str) -> (u32, u32, u32) {
    let parts: Vec<&str> = version.split('.').collect();
    let major = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
    let minor = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let patch = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    (major, minor, patch)
}

fn version_check(version: &str) -> TokenStream2 {
    let (major, minor, patch) = parse_version(version);
    quote! { qapi::qemu_version() >= (#major, #minor, #patch) }
}

fn version_range_check(since: Option<&str>, until: Option<&str>) -> Option<TokenStream2> {
    match (since, until) {
        (Some(s), Some(u)) => {
            let since_chk = version_check(s);
            let (umaj, umin, upatch) = parse_version(u);
            Some(quote! { (#since_chk) && qapi::qemu_version() < (#umaj, #umin, #upatch) })
        }
        (Some(s), None) => Some(version_check(s)),
        (None, Some(u)) => {
            let (umaj, umin, upatch) = parse_version(u);
            Some(quote! { qapi::qemu_version() < (#umaj, #umin, #upatch) })
        }
        (None, None) => None,
    }
}

fn condition_cfg(condition: &str) -> TokenStream2 {
    match condition {
        "CONFIG_LINUX" => quote! { #[cfg(target_os = "linux")] },
        "CONFIG_WIN32" => quote! { #[cfg(target_os = "windows")] },
        "CONFIG_POSIX" => quote! { #[cfg(unix)] },
        // All other conditions (CONFIG_*, TARGET_*, HAVE_*) are QEMU build-time
        // options that don't map to the Rust compilation target. We always compile
        // these types and handle availability at runtime.
        _ => quote! {},
    }
}

fn condition_cfg_attr(condition: &str) -> Option<syn::Attribute> {
    match condition {
        "CONFIG_LINUX" => Some(syn::parse_quote! { #[cfg(target_os = "linux")] }),
        "CONFIG_WIN32" => Some(syn::parse_quote! { #[cfg(target_os = "windows")] }),
        "CONFIG_POSIX" => Some(syn::parse_quote! { #[cfg(unix)] }),
        _ => None,
    }
}

fn is_option_type(ty: &Type) -> bool {
    matches!(ty, Type::Path(tp) if tp.path.segments.last().map(|s| s.ident == "Option").unwrap_or(false))
}

// ─── Variant / Field info ───

struct VariantInfo {
    ident: syn::Ident,
    attrs: ItemAttrs,
    kind: VariantKind,
}

enum VariantKind {
    Unit,
    Tuple(Box<Type>),
}

struct FieldInfo {
    ident: syn::Ident,
    ty: Type,
    attrs: ItemAttrs,
}

// ─── Entry point ───

#[proc_macro_attribute]
pub fn qapi(args: TokenStream, input: TokenStream) -> TokenStream {
    let qapi_args = parse_macro_input!(args as AttributeArgs);
    let mut input_ast = parse_macro_input!(input as DeriveInput);

    // Parse primary args + merge any additional #[qapi(...)] on the type
    let mut ta = parse_type_attrs(&qapi_args);
    for attr in &input_ast.attrs {
        if attr.path.is_ident("qapi") {
            if let Ok(Meta::List(ml)) = attr.parse_meta() {
                let nested: Vec<NestedMeta> = ml.nested.into_iter().collect();
                let extra = parse_type_attrs(&nested);
                ta.name = ta.name.or(extra.name);
                ta.since = ta.since.or(extra.since);
                ta.until = ta.until.or(extra.until);
                ta.condition = ta.condition.or(extra.condition);
                ta.returns = ta.returns.or(extra.returns);
                ta.allow_oob = ta.allow_oob || extra.allow_oob;
                ta.allow_preconfig = ta.allow_preconfig || extra.allow_preconfig;
                ta.no_success_response = ta.no_success_response || extra.no_success_response;
                ta.is_command = ta.is_command || extra.is_command;
                ta.is_event = ta.is_event || extra.is_event;
            }
        }
    }
    input_ast.attrs.retain(|a| !a.path.is_ident("qapi"));

    let output = match input_ast.data {
        syn::Data::Enum(_) => gen_enum(&mut input_ast, &ta),
        syn::Data::Struct(_) => gen_struct(&mut input_ast, &ta),
        _ => quote! { #input_ast },
    };

    TokenStream::from(output)
}

// ─── Enum codegen ───

fn gen_enum(input_ast: &mut DeriveInput, ta: &TypeAttrs) -> TokenStream2 {
    let enum_name = input_ast.ident.clone();

    let data_enum = match &mut input_ast.data {
        syn::Data::Enum(d) => d,
        _ => unreachable!(),
    };

    let mut variants: Vec<VariantInfo> = Vec::new();
    for v in &mut data_enum.variants {
        let attrs = extract_all_item_attrs(&v.attrs);
        v.attrs.retain(|a| !a.path.is_ident("qapi"));

        // Apply cfg attribute to the variant definition itself (only for OS-level conditions)
        if let Some(ref cond) = attrs.condition {
            if let Some(cfg_attr) = condition_cfg_attr(cond) {
                v.attrs.push(cfg_attr);
            }
        }

        let kind = match &v.fields {
            Fields::Unit => VariantKind::Unit,
            Fields::Unnamed(f) => VariantKind::Tuple(Box::new(f.unnamed.first().unwrap().ty.clone())),
            _ => VariantKind::Unit, // shouldn't happen for our codegen
        };

        variants.push(VariantInfo {
            ident: v.ident.clone(),
            attrs,
            kind,
        });
    }

    let all_unit = variants.iter().all(|v| matches!(v.kind, VariantKind::Unit));

    let cond = ta.condition.as_ref().map(|c| condition_cfg(c));

    let ser = if all_unit {
        gen_unit_enum_ser(&enum_name, &variants, ta)
    } else {
        gen_alt_ser(&enum_name, &variants)
    };

    let de = if all_unit {
        gen_unit_enum_de(&enum_name, &variants)
    } else {
        gen_alt_de(&enum_name, &variants)
    };

    quote! {
        #cond
        #input_ast
        #cond
        #ser
        #cond
        #de
    }
}

fn gen_unit_enum_ser(
    name: &syn::Ident,
    variants: &[VariantInfo],
    ta: &TypeAttrs,
) -> TokenStream2 {
    let arms: Vec<TokenStream2> = variants.iter().map(|v| {
        let ident = &v.ident;
        let wn = wire_name(&v.attrs.name, &v.ident);
        let cond = v.attrs.condition.as_ref().map(|c| condition_cfg(c));
        quote! {
            #cond
            #name::#ident => serializer.serialize_str(#wn),
        }
    }).collect();

    let body = if let Some(chk) = version_range_check(ta.since.as_deref(), ta.until.as_deref()) {
        quote! {
            if !(#chk) {
                return Err(serde::ser::Error::custom(concat!("Type ", stringify!(#name), " not available in this QEMU version")));
            }
            match self { #(#arms)* }
        }
    } else {
        quote! { match self { #(#arms)* } }
    };

    quote! {
        impl serde::Serialize for #name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where S: serde::Serializer {
                #body
            }
        }
    }
}

fn gen_unit_enum_de(name: &syn::Ident, variants: &[VariantInfo]) -> TokenStream2 {
    let arms: Vec<TokenStream2> = variants.iter().map(|v| {
        let ident = &v.ident;
        let wn = wire_name(&v.attrs.name, &v.ident);
        let cond = v.attrs.condition.as_ref().map(|c| condition_cfg(c));
        quote! {
            #cond
            #wn => Ok(#name::#ident),
        }
    }).collect();

    let wn_strs: Vec<String> = variants.iter().map(|v| wire_name(&v.attrs.name, &v.ident)).collect();

    quote! {
        impl<'de> serde::Deserialize<'de> for #name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where D: serde::Deserializer<'de> {
                let s = <String as serde::Deserialize>::deserialize(deserializer)?;
                match s.as_str() {
                    #(#arms)*
                    other => Err(serde::de::Error::unknown_variant(other, &[#(#wn_strs),*])),
                }
            }
        }
    }
}

fn gen_alt_ser(name: &syn::Ident, variants: &[VariantInfo]) -> TokenStream2 {
    let arms: Vec<TokenStream2> = variants.iter().map(|v| {
        let ident = &v.ident;
        let cond = v.attrs.condition.as_ref().map(|c| condition_cfg(c));
        match &v.kind {
            VariantKind::Tuple(_) => {
                quote! { #cond #name::#ident(ref inner) => serde::Serialize::serialize(inner, serializer), }
            }
            VariantKind::Unit => {
                quote! { #cond #name::#ident => serializer.serialize_unit(), }
            }
        }
    }).collect();

    quote! {
        impl serde::Serialize for #name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where S: serde::Serializer {
                match self { #(#arms)* }
            }
        }
    }
}

fn gen_alt_de(name: &syn::Ident, variants: &[VariantInfo]) -> TokenStream2 {
    let try_arms: Vec<TokenStream2> = variants.iter().map(|v| {
        let ident = &v.ident;
        let cond = v.attrs.condition.as_ref().map(|c| condition_cfg(c));
        match &v.kind {
            VariantKind::Tuple(ty) => {
                quote! {
                    #cond
                    if let Ok(v) = serde_json::from_value::<#ty>(value.clone()) {
                        return Ok(#name::#ident(v));
                    }
                }
            }
            VariantKind::Unit => {
                quote! {
                    #cond
                    if value.is_null() {
                        return Ok(#name::#ident);
                    }
                }
            }
        }
    }).collect();

    quote! {
        impl<'de> serde::Deserialize<'de> for #name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where D: serde::Deserializer<'de> {
                let value = <serde_json::Value as serde::Deserialize>::deserialize(deserializer)?;
                #(#try_arms)*
                Err(serde::de::Error::custom(concat!("no matching variant for ", stringify!(#name))))
            }
        }
    }
}

// ─── Struct codegen ───

fn gen_struct(input_ast: &mut DeriveInput, ta: &TypeAttrs) -> TokenStream2 {
    let struct_name = input_ast.ident.clone();

    let data_struct = match &mut input_ast.data {
        syn::Data::Struct(d) => d,
        _ => unreachable!(),
    };

    let mut fields: Vec<FieldInfo> = Vec::new();
    if let Fields::Named(named) = &mut data_struct.fields {
        for f in &mut named.named {
            let attrs = extract_all_item_attrs(&f.attrs);
            f.attrs.retain(|a| !a.path.is_ident("qapi"));

            // Apply cfg attribute to the field definition itself (only for OS-level conditions)
            if let Some(ref cond) = attrs.condition {
                if let Some(cfg_attr) = condition_cfg_attr(cond) {
                    f.attrs.push(cfg_attr);
                }
            }

            fields.push(FieldInfo {
                ident: f.ident.clone().unwrap(),
                ty: f.ty.clone(),
                attrs,
            });
        }
    }

    let wn = ta.name.clone().unwrap_or_else(|| struct_name.to_string());
    let has_flatten = fields.iter().any(|f| f.attrs.flatten);

    let cond = ta.condition.as_ref().map(|c| condition_cfg(c));

    let ser = if has_flatten {
        gen_struct_ser_flatten(&struct_name, &fields, ta)
    } else {
        gen_struct_ser(&struct_name, &wn, &fields, ta)
    };

    let de = if has_flatten {
        gen_struct_de_flatten(&struct_name, &fields)
    } else {
        gen_struct_de(&struct_name, &fields)
    };

    let cmd = if ta.is_command {
        let impl_tokens = gen_command_impl(&struct_name, &wn, ta);
        quote! { #cond #impl_tokens }
    } else {
        TokenStream2::new()
    };

    let evt = if ta.is_event {
        let impl_tokens = gen_event_impl(&struct_name, &wn);
        quote! { #cond #impl_tokens }
    } else {
        TokenStream2::new()
    };

    quote! {
        #cond
        #input_ast
        #cond
        #ser
        #cond
        #de
        #cmd
        #evt
    }
}

fn gen_struct_ser(
    name: &syn::Ident,
    wn: &str,
    fields: &[FieldInfo],
    ta: &TypeAttrs,
) -> TokenStream2 {
    let field_count = fields.len();
    let ser_fields: Vec<TokenStream2> = fields.iter().map(|f| {
        let ident = &f.ident;
        let fwn = wire_name(&f.attrs.name, &f.ident);
        let is_opt = is_option_type(&f.ty);
        let cond = f.attrs.condition.as_ref().map(|c| condition_cfg(c));

        let code = if is_opt {
            quote! { if let Some(ref value) = self.#ident { state.serialize_field(#fwn, value)?; } }
        } else {
            quote! { state.serialize_field(#fwn, &self.#ident)?; }
        };

        let code = if let Some(chk) = version_range_check(f.attrs.since.as_deref(), f.attrs.until.as_deref()) {
            quote! { if #chk { #code } }
        } else {
            code
        };

        quote! { #cond { #code } }
    }).collect();

    let body = quote! {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct(#wn, #field_count)?;
        #(#ser_fields)*
        state.end()
    };

    let body = if let Some(chk) = version_range_check(ta.since.as_deref(), ta.until.as_deref()) {
        quote! {
            if !(#chk) {
                return Err(serde::ser::Error::custom(concat!("Type ", stringify!(#name), " not available in this QEMU version")));
            }
            #body
        }
    } else {
        body
    };

    quote! {
        impl serde::Serialize for #name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where S: serde::Serializer { #body }
        }
    }
}

fn gen_struct_ser_flatten(
    name: &syn::Ident,
    fields: &[FieldInfo],
    ta: &TypeAttrs,
) -> TokenStream2 {
    let ser_fields: Vec<TokenStream2> = fields.iter().map(|f| {
        let ident = &f.ident;
        let fwn = wire_name(&f.attrs.name, &f.ident);
        let is_opt = is_option_type(&f.ty);
        let cond = f.attrs.condition.as_ref().map(|c| condition_cfg(c));

        if f.attrs.flatten {
            // Serialize flattened struct's fields inline into the map.
            // Convert to serde_json::Value and merge its fields.
            quote! {
                #cond
                {
                    let flat_value = serde_json::to_value(&self.#ident).map_err(serde::ser::Error::custom)?;
                    if let serde_json::Value::Object(flat_map) = flat_value {
                        for (k, v) in flat_map {
                            map.serialize_entry(&k, &v)?;
                        }
                    }
                }
            }
        } else if is_opt {
            quote! {
                #cond
                if let Some(ref value) = self.#ident { map.serialize_entry(#fwn, value)?; }
            }
        } else {
            quote! {
                #cond
                map.serialize_entry(#fwn, &self.#ident)?;
            }
        }
    }).collect();

    let body = quote! {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(None)?;
        #(#ser_fields)*
        map.end()
    };

    let body = if let Some(chk) = version_range_check(ta.since.as_deref(), ta.until.as_deref()) {
        quote! {
            if !(#chk) {
                return Err(serde::ser::Error::custom(concat!("Type ", stringify!(#name), " not available in this QEMU version")));
            }
            #body
        }
    } else {
        body
    };

    quote! {
        impl serde::Serialize for #name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where S: serde::Serializer { #body }
        }
    }
}

fn gen_struct_de(name: &syn::Ident, fields: &[FieldInfo]) -> TokenStream2 {
    let field_idents: Vec<&syn::Ident> = fields.iter().map(|f| &f.ident).collect();
    let field_wires: Vec<String> = fields.iter().map(|f| wire_name(&f.attrs.name, &f.ident)).collect();
    let field_vars: Vec<syn::Ident> = fields.iter().map(|f| format_ident!("__field_{}", f.ident)).collect();

    let none_inits: Vec<TokenStream2> = field_vars.iter().map(|v| quote! { let mut #v = None; }).collect();

    let match_arms: Vec<TokenStream2> = fields.iter().enumerate().map(|(i, f)| {
        let w = &field_wires[i];
        let v = &field_vars[i];
        let cond = f.attrs.condition.as_ref().map(|c| condition_cfg(c));
        quote! { #cond #w => { #v = Some(map.next_value()?); } }
    }).collect();

    let inits: Vec<TokenStream2> = fields.iter().enumerate().map(|(i, f)| {
        let v = &field_vars[i];
        let id = &f.ident;
        let w = &field_wires[i];
        if is_option_type(&f.ty) {
            quote! { let #id = #v.unwrap_or(None); }
        } else {
            quote! { let #id = #v.ok_or_else(|| serde::de::Error::missing_field(#w))?; }
        }
    }).collect();

    quote! {
        impl<'de> serde::Deserialize<'de> for #name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where D: serde::Deserializer<'de> {
                struct __Visitor;
                impl<'de> serde::de::Visitor<'de> for __Visitor {
                    type Value = #name;
                    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                        f.write_str(stringify!(#name))
                    }
                    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
                    where A: serde::de::MapAccess<'de> {
                        #(#none_inits)*
                        while let Some(key) = map.next_key::<String>()? {
                            match key.as_str() {
                                #(#match_arms)*
                                _ => { let _ = map.next_value::<serde::de::IgnoredAny>()?; }
                            }
                        }
                        #(#inits)*
                        Ok(#name { #(#field_idents),* })
                    }
                }
                deserializer.deserialize_map(__Visitor)
            }
        }
    }
}

fn gen_struct_de_flatten(name: &syn::Ident, fields: &[FieldInfo]) -> TokenStream2 {
    let non_flat: Vec<&FieldInfo> = fields.iter().filter(|f| !f.attrs.flatten).collect();
    let flat: Vec<&FieldInfo> = fields.iter().filter(|f| f.attrs.flatten).collect();

    let extractions: Vec<TokenStream2> = non_flat.iter().map(|f| {
        let id = &f.ident;
        let w = wire_name(&f.attrs.name, &f.ident);
        let ty = &f.ty;
        if is_option_type(ty) {
            quote! {
                let #id: #ty = match map.remove(#w) {
                    Some(v) => Some(serde_json::from_value(v).map_err(serde::de::Error::custom)?),
                    None => None,
                };
            }
        } else {
            quote! {
                let #id: #ty = match map.remove(#w) {
                    Some(v) => serde_json::from_value(v).map_err(serde::de::Error::custom)?,
                    None => return Err(serde::de::Error::missing_field(#w)),
                };
            }
        }
    }).collect();

    let flat_extractions: Vec<TokenStream2> = flat.iter().map(|f| {
        let id = &f.ident;
        let ty = &f.ty;
        quote! {
            let #id: #ty = serde_json::from_value(serde_json::Value::Object(map.clone())).map_err(serde::de::Error::custom)?;
        }
    }).collect();

    let all_ids: Vec<&syn::Ident> = fields.iter().map(|f| &f.ident).collect();

    quote! {
        impl<'de> serde::Deserialize<'de> for #name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where D: serde::Deserializer<'de> {
                let value = <serde_json::Value as serde::Deserialize>::deserialize(deserializer)?;
                let mut map = match value {
                    serde_json::Value::Object(m) => m,
                    _ => return Err(serde::de::Error::custom("expected object")),
                };
                #(#extractions)*
                #(#flat_extractions)*
                Ok(#name { #(#all_ids),* })
            }
        }
    }
}

fn gen_command_impl(name: &syn::Ident, wn: &str, ta: &TypeAttrs) -> TokenStream2 {
    let returns: TokenStream2 = ta.returns.as_deref().unwrap_or("()").parse().unwrap();
    let oob = ta.allow_oob;
    let preconfig = ta.allow_preconfig;
    let success = !ta.no_success_response;

    let since_const = if let Some(ref s) = ta.since {
        let (maj, min, pat) = parse_version(s);
        quote! { const AVAILABLE_SINCE: (u32, u32, u32) = (#maj, #min, #pat); }
    } else {
        quote! { const AVAILABLE_SINCE: (u32, u32, u32) = (0, 0, 0); }
    };

    let until_const = if let Some(ref u) = ta.until {
        let (maj, min, pat) = parse_version(u);
        quote! { const REMOVED_IN: Option<(u32, u32, u32)> = Some((#maj, #min, #pat)); }
    } else {
        quote! { const REMOVED_IN: Option<(u32, u32, u32)> = None; }
    };

    quote! {
        impl qapi::QapiCommand for #name {
            type Returns = #returns;
            const NAME: &'static str = #wn;
            const ALLOW_OOB: bool = #oob;
            const ALLOW_PRECONFIG: bool = #preconfig;
            const SUCCESS_RESPONSE: bool = #success;
            #since_const
            #until_const
        }
    }
}

fn gen_event_impl(name: &syn::Ident, wn: &str) -> TokenStream2 {
    quote! {
        impl qapi::QapiEvent for #name {
            const NAME: &'static str = #wn;
        }
    }
}
