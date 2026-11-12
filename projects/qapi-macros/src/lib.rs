#![deny(unsafe_code)]

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    ext::IdentExt, parse_macro_input, AttributeArgs, DeriveInput, Fields, Lit, Meta, MetaList,
    NestedMeta, Type,
};

// ─── Attribute structs ───

/// A version-ranged attribute: tracks whether it's present and optionally
/// when it was added/removed. Supports both bare `#[qapi(attr)]` and nested
/// `#[qapi(attr(added = "X", removed = "Y"))]` syntax.
#[derive(Default, Clone)]
struct AttrRange {
    present: bool,
    added: Option<String>,
    removed: Option<String>,
}

impl AttrRange {
    fn bare() -> Self {
        Self {
            present: true,
            added: None,
            removed: None,
        }
    }
}

#[derive(Default)]
struct TypeAttrs {
    name: Option<String>,
    since: Option<String>,
    until: Option<String>,
    deprecated: AttrRange,
    unstable: AttrRange,
    condition: Option<String>,
    returns: Option<String>,
    allow_oob: bool,
    allow_preconfig: AttrRange,
    no_success_response: AttrRange,
    is_command: bool,
    is_event: bool,
}

#[derive(Default, Clone)]
struct ItemAttrs {
    name: Option<String>,
    since: Option<String>,
    until: Option<String>,
    deprecated: Option<String>,
    condition: Option<String>,
    feature: Option<String>,
    flatten: bool,
    discriminator: bool,
    union_field: bool,
}

// ─── Compile-time version bounds ───

struct VersionBounds {
    min: Option<(u32, u32, u32)>,
    max: Option<(u32, u32, u32)>,
}

fn get_version_bounds() -> VersionBounds {
    let min = std::env::var("QEMU_MIN_VERSION").ok().and_then(|s| {
        let v = parse_version(&s);
        if v == (0, 0, 0) {
            None
        } else {
            Some(v)
        }
    });
    let max = std::env::var("QEMU_MAX_VERSION").ok().and_then(|s| {
        let v = parse_version(&s);
        if v == (0, 0, 0) {
            None
        } else {
            Some(v)
        }
    });
    VersionBounds { min, max }
}

/// Check if a type/field/variant should be excluded based on compile-time bounds.
/// Returns true if the item should be EXCLUDED (not emitted).
fn excluded_by_bounds(bounds: &VersionBounds, since: Option<&str>, until: Option<&str>) -> bool {
    // If added after the max version, exclude it
    if let (Some(max), Some(s)) = (bounds.max, since) {
        let since_ver = parse_version(s);
        if since_ver > max {
            return true;
        }
    }
    // If removed before the min version, exclude it
    if let (Some(min), Some(u)) = (bounds.min, until) {
        let until_ver = parse_version(u);
        if until_ver <= min {
            return true;
        }
    }
    false
}

/// Check if something should get Rust `#[deprecated]` based on bounds.
/// Returns true if the item was deprecated at or before the minimum
/// targeted QEMU version — i.e. it is deprecated across the entire
/// supported range, so the user should be warned at compile time.
fn rust_deprecated_by_bounds(bounds: &VersionBounds, deprecated: Option<&str>) -> bool {
    if let (Some(min), Some(dep)) = (bounds.min, deprecated) {
        let dep_ver = parse_version(dep);
        // Symmetric with `excluded_by_bounds`'s `until <= min` rule: an
        // item deprecated at or before our minimum target is deprecated
        // for every supported version, so flag it. Items deprecated only
        // in newer versions are still valid for older targets and must
        // not get a blanket `#[deprecated]` attribute.
        return dep_ver <= min;
    }
    false
}

// ─── Parsing ───

/// Parse a nested `Meta::List` into an `AttrRange`.
/// Handles: `attr(added = "X")`, `attr(added = "X", removed = "Y")`.
fn parse_attr_range(
    nested: &syn::punctuated::Punctuated<NestedMeta, syn::token::Comma>,
) -> AttrRange {
    let mut range = AttrRange {
        present: true,
        added: None,
        removed: None,
    };
    for item in nested {
        if let NestedMeta::Meta(Meta::NameValue(nv)) = item {
            if let Lit::Str(lit_str) = &nv.lit {
                let val = lit_str.value();
                if nv.path.is_ident("added") || nv.path.is_ident("since") {
                    range.added = Some(val);
                } else if nv.path.is_ident("removed") || nv.path.is_ident("until") {
                    range.removed = Some(val);
                }
            }
        }
    }
    range
}

fn parse_type_attrs(args: &[NestedMeta]) -> TypeAttrs {
    let mut attrs = TypeAttrs::default();
    for arg in args {
        match arg {
            NestedMeta::Meta(Meta::NameValue(nv)) => {
                if let Lit::Str(lit_str) = &nv.lit {
                    let val = lit_str.value();
                    if nv.path.is_ident("name") {
                        attrs.name = Some(val);
                    } else if nv.path.is_ident("since") || nv.path.is_ident("added") {
                        attrs.since = Some(val);
                    } else if nv.path.is_ident("until") || nv.path.is_ident("removed") {
                        attrs.until = Some(val);
                    } else if nv.path.is_ident("deprecated") {
                        // Backward compat: deprecated = "X" → AttrRange with added
                        attrs.deprecated = AttrRange {
                            present: true,
                            added: Some(val),
                            removed: None,
                        };
                    } else if nv.path.is_ident("condition") {
                        attrs.condition = Some(val);
                    } else if nv.path.is_ident("returns") {
                        attrs.returns = Some(val);
                        attrs.is_command = true;
                    } else if nv.path.is_ident("unstable") {
                        // Backward compat: unstable = "X" → AttrRange with added
                        attrs.unstable = AttrRange {
                            present: true,
                            added: Some(val),
                            removed: None,
                        };
                    }
                }
            }
            NestedMeta::Meta(Meta::List(list)) => {
                // Nested syntax: attr(added = "X", removed = "Y")
                if list.path.is_ident("allow_preconfig") {
                    attrs.allow_preconfig = parse_attr_range(&list.nested);
                    attrs.is_command = true;
                } else if list.path.is_ident("unstable") {
                    attrs.unstable = parse_attr_range(&list.nested);
                } else if list.path.is_ident("deprecated") {
                    attrs.deprecated = parse_attr_range(&list.nested);
                } else if list.path.is_ident("no_success_response") {
                    attrs.no_success_response = parse_attr_range(&list.nested);
                    attrs.is_command = true;
                }
            }
            NestedMeta::Meta(Meta::Path(path)) => {
                if path.is_ident("allow_oob") {
                    attrs.allow_oob = true;
                    attrs.is_command = true;
                } else if path.is_ident("allow_preconfig") {
                    attrs.allow_preconfig = AttrRange::bare();
                    attrs.is_command = true;
                } else if path.is_ident("no_success_response") {
                    attrs.no_success_response = AttrRange::bare();
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
                    } else if nv.path.is_ident("since") || nv.path.is_ident("added") {
                        attrs.since = Some(val);
                    } else if nv.path.is_ident("until") || nv.path.is_ident("removed") {
                        attrs.until = Some(val);
                    } else if nv.path.is_ident("deprecated") {
                        attrs.deprecated = Some(val);
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
                result.deprecated = result.deprecated.or(parsed.deprecated);
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
    attrs_name
        .clone()
        .unwrap_or_else(|| ident.unraw().to_string())
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

/// Generate a once-guarded deprecation warning for runtime.
fn deprecation_warning(
    type_name: &str,
    field_name: Option<&str>,
    deprecated_version: &str,
) -> TokenStream2 {
    let (dmaj, dmin, dpatch) = parse_version(deprecated_version);
    let msg = match field_name {
        Some(fname) => format!(
            "qapi warning: {}.{} is deprecated since QEMU {}",
            type_name, fname, deprecated_version
        ),
        None => format!(
            "qapi warning: {} is deprecated since QEMU {}",
            type_name, deprecated_version
        ),
    };
    quote! {
        {
            use std::sync::Once;
            static __QAPI_DEP_WARNED: Once = Once::new();
            if qapi::qemu_version() >= (#dmaj, #dmin, #dpatch) {
                __QAPI_DEP_WARNED.call_once(|| {
                    eprintln!(#msg);
                });
            }
        }
    }
}

/// Field-level deprecation warning generator used by both the plain and
/// flattened struct serializers. Returns `None` when the field is not
/// marked deprecated. For `Option<_>` fields the warning fires only when a
/// value is actually present, matching the behavior of the serializer.
fn field_deprecation_warn(type_name: &str, f: &FieldInfo) -> Option<TokenStream2> {
    let dep_ver = f.attrs.deprecated.as_deref()?;
    let field_name = wire_name(&f.attrs.name, &f.ident);
    let ident = &f.ident;
    let warn = deprecation_warning(type_name, Some(&field_name), dep_ver);
    Some(if is_option_type(&f.ty) {
        quote! { if self.#ident.is_some() { #warn } }
    } else {
        warn
    })
}

/// Generate a once-guarded warning for a removed field that has a value set.
fn removed_field_warning(type_name: &str, field_name: &str, removed_version: &str) -> TokenStream2 {
    let msg = format!(
        "qapi warning: {}.{} was removed in QEMU {}, value will be ignored",
        type_name, field_name, removed_version
    );
    quote! {
        {
            use std::sync::Once;
            static __QAPI_REM_WARNED: Once = Once::new();
            __QAPI_REM_WARNED.call_once(|| {
                eprintln!(#msg);
            });
        }
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

/// Build an `impl std::fmt::Display` block for `name`. The wrapping boilerplate
/// (fn signature, optional outer `#cond` cfg, optional `#[allow(...)]` on the
/// impl) is identical across all call sites; only the `body` (the contents of
/// `fn fmt`) varies per type shape.
fn gen_display_impl(
    name: &syn::Ident,
    cond: &Option<TokenStream2>,
    allows: TokenStream2,
    body: TokenStream2,
) -> TokenStream2 {
    quote! {
        #cond
        #allows
        impl std::fmt::Display for #name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                #body
            }
        }
    }
}

fn is_option_type(ty: &Type) -> bool {
    matches!(ty, Type::Path(tp) if tp.path.segments.last().map(|s| s.ident == "Option").unwrap_or(false))
}

/// Extract the inner type `T` from `Option<T>`. Returns `None` if the type
/// is not `Option`.
fn inner_option_type(ty: &Type) -> Option<&Type> {
    if let Type::Path(tp) = ty {
        if let Some(seg) = tp.path.segments.last() {
            if seg.ident == "Option" {
                if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                    if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
                        return Some(inner);
                    }
                }
            }
        }
    }
    None
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
                if extra.deprecated.present {
                    ta.deprecated = extra.deprecated;
                }
                ta.condition = ta.condition.or(extra.condition);
                ta.returns = ta.returns.or(extra.returns);
                if extra.unstable.present {
                    ta.unstable = extra.unstable;
                }
                ta.allow_oob = ta.allow_oob || extra.allow_oob;
                if extra.allow_preconfig.present {
                    ta.allow_preconfig = extra.allow_preconfig;
                }
                if extra.no_success_response.present {
                    ta.no_success_response = extra.no_success_response;
                }
                ta.is_command = ta.is_command || extra.is_command;
                ta.is_event = ta.is_event || extra.is_event;
            }
        }
    }
    input_ast.attrs.retain(|a| !a.path.is_ident("qapi"));

    // ── Compile-time version bounds: exclude entire type if out of range ──
    let bounds = get_version_bounds();
    if excluded_by_bounds(&bounds, ta.since.as_deref(), ta.until.as_deref()) {
        // Type is outside the declared version range — emit nothing
        return TokenStream::new();
    }

    // Add Rust #[deprecated] attribute if the type is deprecated and bounds are set
    if rust_deprecated_by_bounds(&bounds, ta.deprecated.added.as_deref()) {
        let dep_ver = ta.deprecated.added.as_deref().unwrap();
        let msg = format!("deprecated since QEMU {}", dep_ver);
        input_ast
            .attrs
            .push(syn::parse_quote! { #[deprecated(note = #msg)] });
    }

    // Mark unstable types with a doc comment (not #[deprecated] — wrong semantics)
    if ta.unstable.present && ta.unstable.removed.is_none() {
        let msg = if let Some(ref ver) = ta.unstable.added {
            format!(
                " **Unstable** since QEMU {} — this interface may change without notice.",
                ver
            )
        } else {
            " **Unstable** — this interface may change without notice.".to_string()
        };
        input_ast.attrs.push(syn::parse_quote! { #[doc = #msg] });
    }

    // Inject derive(Debug, Clone) on all QAPI types
    input_ast
        .attrs
        .push(syn::parse_quote! { #[derive(Debug, Clone)] });

    let output = match input_ast.data {
        syn::Data::Enum(_) => gen_enum(&mut input_ast, &ta, &bounds),
        syn::Data::Struct(_) => gen_struct(&mut input_ast, &ta, &bounds),
        _ => Ok(quote! { #input_ast }),
    };

    match output {
        Ok(ts) => TokenStream::from(ts),
        Err(e) => TokenStream::from(e.to_compile_error()),
    }
}

// ─── Enum codegen ───

fn gen_enum(
    input_ast: &mut DeriveInput,
    ta: &TypeAttrs,
    bounds: &VersionBounds,
) -> syn::Result<TokenStream2> {
    let enum_name = input_ast.ident.clone();

    // Entry-point dispatches on the same `input_ast.data` variant, so this
    // branch is an internal invariant, not user-reachable.
    let data_enum = match &mut input_ast.data {
        syn::Data::Enum(d) => d,
        _ => unreachable!("gen_enum called with non-enum DeriveInput"),
    };

    // Filter variants by compile-time bounds
    data_enum.variants = data_enum
        .variants
        .iter()
        .filter(|v| {
            let attrs = extract_all_item_attrs(&v.attrs);
            !excluded_by_bounds(bounds, attrs.since.as_deref(), attrs.until.as_deref())
        })
        .cloned()
        .collect();

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

        // Add Rust #[deprecated] if variant is deprecated and bounds are set
        if rust_deprecated_by_bounds(bounds, attrs.deprecated.as_deref()) {
            let dep_ver = attrs.deprecated.as_deref().unwrap();
            let msg = format!("deprecated since QEMU {}", dep_ver);
            v.attrs
                .push(syn::parse_quote! { #[deprecated(note = #msg)] });
        } else if attrs.feature.as_deref() == Some("deprecated") {
            let msg = "deprecated QAPI variant";
            v.attrs
                .push(syn::parse_quote! { #[deprecated(note = #msg)] });
        }

        // Mark unstable variants with a doc comment (not #[deprecated])
        if attrs.feature.as_deref() == Some("unstable") {
            let msg = " **Unstable** — this variant may change without notice.";
            v.attrs.push(syn::parse_quote! { #[doc = #msg] });
        }

        let kind = match &v.fields {
            Fields::Unit => VariantKind::Unit,
            Fields::Unnamed(f) => {
                let first = f.unnamed.first().ok_or_else(|| {
                    syn::Error::new_spanned(
                        f,
                        "qapi: tuple enum variants must wrap exactly one type",
                    )
                })?;
                VariantKind::Tuple(Box::new(first.ty.clone()))
            }
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

    // Generate Display impl for unit enums (prints the QAPI wire name)
    let display = if all_unit {
        let display_arms: Vec<TokenStream2> = variants
            .iter()
            .map(|v| {
                let ident = &v.ident;
                let wn = wire_name(&v.attrs.name, &v.ident);
                let cond = v.attrs.condition.as_ref().map(|c| condition_cfg(c));
                quote! {
                    #cond
                    #enum_name::#ident => f.write_str(#wn),
                }
            })
            .collect();
        gen_display_impl(
            &enum_name,
            &cond,
            quote! { #[allow(deprecated)] },
            quote! { match self { #(#display_arms)* } },
        )
    } else {
        TokenStream2::new()
    };

    // Generate QapiFlatUnion impl for non-unit enums (union branch enums)
    let flat_union = if !all_unit {
        let fu = gen_flat_union_impl(&enum_name, &variants);
        quote! { #cond #fu }
    } else {
        TokenStream2::new()
    };

    Ok(quote! {
        #cond
        #input_ast
        #cond
        #ser
        #cond
        #de
        #display
        #flat_union
    })
}

fn gen_unit_enum_ser(name: &syn::Ident, variants: &[VariantInfo], ta: &TypeAttrs) -> TokenStream2 {
    let type_name_str = name.to_string();

    let arms: Vec<TokenStream2> = variants
        .iter()
        .map(|v| {
            let ident = &v.ident;
            let wn = wire_name(&v.attrs.name, &v.ident);
            let cond = v.attrs.condition.as_ref().map(|c| condition_cfg(c));
            quote! {
                #cond
                #name::#ident => serializer.serialize_str(#wn),
            }
        })
        .collect();

    // Type-level deprecation warning
    let dep_warn = ta
        .deprecated
        .added
        .as_deref()
        .map(|dep_ver| deprecation_warning(&type_name_str, None, dep_ver));

    let body = if let Some(chk) = version_range_check(ta.since.as_deref(), ta.until.as_deref()) {
        quote! {
            if !(#chk) {
                return Err(serde::ser::Error::custom(qapi::unavailable_type_message(stringify!(#name))));
            }
            #dep_warn
            match self { #(#arms)* }
        }
    } else {
        quote! {
            #dep_warn
            match self { #(#arms)* }
        }
    };

    quote! {
        #[allow(deprecated)]
        impl serde::Serialize for #name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where S: serde::Serializer {
                #body
            }
        }
    }
}

fn gen_unit_enum_de(name: &syn::Ident, variants: &[VariantInfo]) -> TokenStream2 {
    let arms: Vec<TokenStream2> = variants
        .iter()
        .map(|v| {
            let ident = &v.ident;
            let wn = wire_name(&v.attrs.name, &v.ident);
            let cond = v.attrs.condition.as_ref().map(|c| condition_cfg(c));
            quote! {
                #cond
                #wn => Ok(#name::#ident),
            }
        })
        .collect();

    let wn_strs: Vec<String> = variants
        .iter()
        .map(|v| wire_name(&v.attrs.name, &v.ident))
        .collect();

    quote! {
        #[allow(deprecated)]
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
        #[allow(deprecated)]
        impl serde::Serialize for #name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where S: serde::Serializer {
                match self { #(#arms)* }
            }
        }
    }
}

/// Classify a variant's inner type into a specificity bucket used to order
/// alternate deserialization attempts. Lower numbers are tried first.
///
/// QAPI alternate types accept one of several shapes. Trying variants in
/// declaration order is wrong when variants overlap (e.g. a struct with all
/// optional fields would happily accept any object, including one intended
/// for a later variant). We try the least ambiguous shapes first:
///
///   0: null-matching variants (unit enum variants, or tuples wrapping `()`)
///   1: primitive JSON scalars (bool / number / string)
///   2: JSON arrays (`Vec<_>`, `[_; N]`)
///   3: everything else (object-like / unknown — highest ambiguity risk)
fn alt_variant_priority(kind: &VariantKind) -> u8 {
    let ty = match kind {
        VariantKind::Unit => return 0,
        VariantKind::Tuple(ty) => ty.as_ref(),
    };
    match ty {
        // `Null(())` — empty tuple type, matches only JSON null
        Type::Tuple(t) if t.elems.is_empty() => 0,
        Type::Array(_) | Type::Slice(_) => 2,
        Type::Path(tp) => {
            let name = match tp.path.segments.last() {
                Some(seg) => seg.ident.to_string(),
                None => return 3,
            };
            match name.as_str() {
                // Rust primitives that map to JSON scalars
                "bool" | "char" | "String" | "str" | "i8" | "i16" | "i32" | "i64" | "i128"
                | "isize" | "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "f32" | "f64" => 1,
                // Array-like
                "Vec" => 2,
                // Everything else is object-like (structs, tagged unions, …)
                _ => 3,
            }
        }
        _ => 3,
    }
}

fn gen_alt_de(name: &syn::Ident, variants: &[VariantInfo]) -> TokenStream2 {
    // Stable-sort variants by specificity so overlapping shapes (e.g. a
    // primitive vs an all-optional-fields struct) resolve to the least
    // ambiguous match instead of silently picking whichever happens to
    // come first in declaration order.
    let mut ordered: Vec<&VariantInfo> = variants.iter().collect();
    ordered.sort_by_key(|v| alt_variant_priority(&v.kind));

    let try_arms: Vec<TokenStream2> = ordered
        .into_iter()
        .map(|v| {
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
        })
        .collect();

    quote! {
        #[allow(deprecated)]
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

/// Generate a `QapiFlatUnion` impl for non-unit enums (union branch enums).
///
/// Maps discriminator wire values to specific variant constructors, enabling
/// deterministic variant selection instead of brute-force trial deserialization.
fn gen_flat_union_impl(name: &syn::Ident, variants: &[VariantInfo]) -> TokenStream2 {
    let arms: Vec<TokenStream2> = variants
        .iter()
        .filter_map(|v| {
            let ident = &v.ident;
            let wn = wire_name(&v.attrs.name, &v.ident);
            let cond = v.attrs.condition.as_ref().map(|c| condition_cfg(c));
            match &v.kind {
                VariantKind::Tuple(_) => Some(quote! {
                    #cond
                    #wn => Ok(#name::#ident(
                        serde_json::from_value(serde_json::Value::Object(map))
                            .map_err(|e| serde_json::Error::custom(format!(
                                "deserializing {} variant '{}': {}",
                                stringify!(#name), discriminator, e
                            )))?
                    )),
                }),
                VariantKind::Unit => Some(quote! {
                    #cond
                    #wn => Ok(#name::#ident),
                }),
            }
        })
        .collect();

    if arms.is_empty() {
        return TokenStream2::new();
    }

    quote! {
        #[allow(deprecated)]
        impl qapi::QapiFlatUnion for #name {
            fn deserialize_variant(
                discriminator: &str,
                map: serde_json::Map<String, serde_json::Value>,
            ) -> Result<Self, serde_json::Error> {
                use serde::de::Error as _;
                match discriminator {
                    #(#arms)*
                    other => Err(serde_json::Error::custom(format!(
                        "unknown {} discriminator '{}'", stringify!(#name), other
                    ))),
                }
            }
        }
    }
}

// ─── Struct codegen ───

fn gen_struct(
    input_ast: &mut DeriveInput,
    ta: &TypeAttrs,
    bounds: &VersionBounds,
) -> syn::Result<TokenStream2> {
    let struct_name = input_ast.ident.clone();

    // Entry-point dispatches on the same `input_ast.data` variant, so this
    // branch is an internal invariant, not user-reachable.
    let data_struct = match &mut input_ast.data {
        syn::Data::Struct(d) => d,
        _ => unreachable!("gen_struct called with non-struct DeriveInput"),
    };

    // Filter fields by compile-time bounds and collect field info
    let mut fields: Vec<FieldInfo> = Vec::new();
    if let Fields::Named(named) = &mut data_struct.fields {
        // Remove fields that are outside the version bounds
        named.named = named
            .named
            .iter()
            .filter(|f| {
                let attrs = extract_all_item_attrs(&f.attrs);
                !excluded_by_bounds(bounds, attrs.since.as_deref(), attrs.until.as_deref())
            })
            .cloned()
            .collect();

        for f in &mut named.named {
            let attrs = extract_all_item_attrs(&f.attrs);
            f.attrs.retain(|a| !a.path.is_ident("qapi"));

            // Apply cfg attribute to the field definition itself (only for OS-level conditions)
            if let Some(ref cond) = attrs.condition {
                if let Some(cfg_attr) = condition_cfg_attr(cond) {
                    f.attrs.push(cfg_attr);
                }
            }

            // Add Rust #[deprecated] if field is deprecated and bounds are set
            if rust_deprecated_by_bounds(bounds, attrs.deprecated.as_deref()) {
                let dep_ver = attrs.deprecated.as_deref().unwrap();
                let msg = format!("deprecated since QEMU {}", dep_ver);
                f.attrs
                    .push(syn::parse_quote! { #[deprecated(note = #msg)] });
            } else if attrs.feature.as_deref() == Some("deprecated") && attrs.deprecated.is_none() {
                let msg = "deprecated QAPI field";
                f.attrs
                    .push(syn::parse_quote! { #[deprecated(note = #msg)] });
            }

            // Mark unstable fields with a doc comment (not #[deprecated])
            if attrs.feature.as_deref() == Some("unstable") {
                let msg = " **Unstable** — this field may change without notice.";
                f.attrs.push(syn::parse_quote! { #[doc = #msg] });
            }

            // Every field of a `Fields::Named` is required by syn to carry
            // an identifier — this unwrap is an internal invariant.
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
        let impl_tokens = gen_command_impl(&struct_name, &wn, ta)?;
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

    // Generate Display impl for all structs
    let display = {
        let display_fields: Vec<TokenStream2> = fields
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let ident = &f.ident;
                let fwn = wire_name(&f.attrs.name, &f.ident);
                let is_opt = is_option_type(&f.ty);
                let cond = f.attrs.condition.as_ref().map(|c| condition_cfg(c));
                let sep = if i == 0 {
                    quote! {}
                } else {
                    quote! { if !first { f.write_str(", ")?; } }
                };

                if is_opt {
                    quote! {
                        #cond
                        if let Some(ref value) = self.#ident {
                            #sep
                            write!(f, concat!(#fwn, "={:?}"), value)?;
                            first = false;
                        }
                    }
                } else {
                    quote! {
                        #cond
                        #sep
                        write!(f, concat!(#fwn, "={:?}"), self.#ident)?;
                        first = false;
                    }
                }
            })
            .collect();

        if ta.is_command {
            // Command structs: just print the wire name
            gen_display_impl(
                &struct_name,
                &cond,
                TokenStream2::new(),
                quote! { f.write_str(#wn) },
            )
        } else if fields.is_empty() {
            // Empty structs: just print the wire name
            gen_display_impl(
                &struct_name,
                &cond,
                quote! { #[allow(deprecated)] },
                quote! { f.write_str(#wn) },
            )
        } else {
            // Data structs: wire_name(field=value, field2=value2)
            gen_display_impl(
                &struct_name,
                &cond,
                quote! { #[allow(deprecated, unused_mut, unused_assignments)] },
                quote! {
                    f.write_str(#wn)?;
                    f.write_str("(")?;
                    let mut first = true;
                    #(#display_fields)*
                    f.write_str(")")
                },
            )
        }
    };

    Ok(quote! {
        #cond
        #input_ast
        #cond
        #ser
        #cond
        #de
        #cmd
        #evt
        #display
    })
}

fn gen_struct_ser(
    name: &syn::Ident,
    wn: &str,
    fields: &[FieldInfo],
    ta: &TypeAttrs,
) -> TokenStream2 {
    let type_name_str = name.to_string();
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

        // Field-level deprecation warning
        let dep_warn = field_deprecation_warn(&type_name_str, f);

        // Version range gating with removed-field warning
        let code = if let Some(chk) = version_range_check(f.attrs.since.as_deref(), f.attrs.until.as_deref()) {
            if is_opt {
                if let Some(ref until_ver) = f.attrs.until {
                    let rem_warn = removed_field_warning(&type_name_str, &fwn, until_ver);
                    quote! {
                        if #chk { #code }
                        else if self.#ident.is_some() { #rem_warn }
                    }
                } else {
                    quote! { if #chk { #code } }
                }
            } else {
                quote! { if #chk { #code } }
            }
        } else {
            code
        };

        quote! { #cond { #dep_warn #code } }
    }).collect();

    let body = quote! {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct(#wn, #field_count)?;
        #(#ser_fields)*
        state.end()
    };

    // Type-level deprecation warning
    let dep_warn = ta
        .deprecated
        .added
        .as_deref()
        .map(|dep_ver| deprecation_warning(&type_name_str, None, dep_ver));

    let body = if let Some(chk) = version_range_check(ta.since.as_deref(), ta.until.as_deref()) {
        quote! {
            if !(#chk) {
                return Err(serde::ser::Error::custom(qapi::unavailable_type_message(stringify!(#name))));
            }
            #dep_warn
            #body
        }
    } else {
        quote! {
            #dep_warn
            #body
        }
    };

    quote! {
        #[allow(deprecated)]
        impl serde::Serialize for #name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where S: serde::Serializer { #body }
        }
    }
}

fn gen_struct_ser_flatten(name: &syn::Ident, fields: &[FieldInfo], ta: &TypeAttrs) -> TokenStream2 {
    let type_name_str = name.to_string();
    let ser_fields: Vec<TokenStream2> = fields.iter().map(|f| {
        let ident = &f.ident;
        let fwn = wire_name(&f.attrs.name, &f.ident);
        let is_opt = is_option_type(&f.ty);
        let cond = f.attrs.condition.as_ref().map(|c| condition_cfg(c));

        // Field-level deprecation warning
        let dep_warn = field_deprecation_warn(&type_name_str, f);

        if f.attrs.flatten {
            // Serialize flattened struct's fields inline into the map.
            // Convert to serde_json::Value and merge its fields.
            quote! {
                #cond
                {
                    #dep_warn
                    let flat_value = serde_json::to_value(&self.#ident).map_err(serde::ser::Error::custom)?;
                    if let serde_json::Value::Object(flat_map) = flat_value {
                        for (k, v) in flat_map {
                            map.serialize_entry(&k, &v)?;
                        }
                    }
                }
            }
        } else if is_opt {
            if let Some(ref until_ver) = f.attrs.until {
                let chk = version_range_check(f.attrs.since.as_deref(), f.attrs.until.as_deref()).unwrap();
                let rem_warn = removed_field_warning(&type_name_str, &fwn, until_ver);
                quote! {
                    #cond
                    {
                        #dep_warn
                        if #chk {
                            if let Some(ref value) = self.#ident { map.serialize_entry(#fwn, value)?; }
                        } else if self.#ident.is_some() {
                            #rem_warn
                        }
                    }
                }
            } else if f.attrs.since.is_some() {
                let chk = version_range_check(f.attrs.since.as_deref(), None).unwrap();
                quote! {
                    #cond
                    {
                        #dep_warn
                        if #chk {
                            if let Some(ref value) = self.#ident { map.serialize_entry(#fwn, value)?; }
                        }
                    }
                }
            } else {
                quote! {
                    #cond
                    {
                        #dep_warn
                        if let Some(ref value) = self.#ident { map.serialize_entry(#fwn, value)?; }
                    }
                }
            }
        } else {
            quote! {
                #cond
                {
                    #dep_warn
                    map.serialize_entry(#fwn, &self.#ident)?;
                }
            }
        }
    }).collect();

    let body = quote! {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(None)?;
        #(#ser_fields)*
        map.end()
    };

    // Type-level deprecation warning
    let dep_warn = ta
        .deprecated
        .added
        .as_deref()
        .map(|dep_ver| deprecation_warning(&type_name_str, None, dep_ver));

    let body = if let Some(chk) = version_range_check(ta.since.as_deref(), ta.until.as_deref()) {
        quote! {
            if !(#chk) {
                return Err(serde::ser::Error::custom(qapi::unavailable_type_message(stringify!(#name))));
            }
            #dep_warn
            #body
        }
    } else {
        quote! {
            #dep_warn
            #body
        }
    };

    quote! {
        #[allow(deprecated)]
        impl serde::Serialize for #name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where S: serde::Serializer { #body }
        }
    }
}

/// Wire names carried by more than one field: version alternates such as a
/// discriminator whose enum type changed (`ImageInfoSpecific.type` is
/// `ImageInfoSpecificType` until 6.2 and `ImageInfoSpecificKind` after). Each
/// alternate is optional and takes the value only if its own type accepts it.
fn shared_wires(fields: &[FieldInfo]) -> std::collections::BTreeSet<String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut shared = std::collections::BTreeSet::new();
    for f in fields.iter().filter(|f| !f.attrs.flatten) {
        let w = wire_name(&f.attrs.name, &f.ident);
        if !seen.insert(w.clone()) {
            shared.insert(w);
        }
    }
    shared
}

/// Lenient extraction of one version alternate from a raw JSON value.
fn alternate_init(f: &FieldInfo, raw: &TokenStream2) -> TokenStream2 {
    let id = &f.ident;
    let ty = &f.ty;
    assert!(
        is_option_type(ty),
        "fields sharing a wire name must be optional version alternates"
    );
    quote! {
        let #id: #ty = #raw.as_ref().and_then(|v| serde_json::from_value::<#ty>(v.clone()).ok().flatten());
    }
}

/// Fails when a shared wire name was present but no alternate accepted it.
fn alternates_check(
    name: &syn::Ident,
    wire: &str,
    raw: &TokenStream2,
    ids: &[&syn::Ident],
) -> TokenStream2 {
    quote! {
        if let Some(v) = #raw.as_ref() {
            if #(#ids.is_none())&&* {
                return Err(serde::de::Error::custom(format!(
                    "{}: no version of field '{}' accepts {}", stringify!(#name), #wire, v
                )));
            }
        }
    }
}

fn raw_ident(wire: &str) -> syn::Ident {
    format_ident!("__raw_{}", wire.replace(['-', '.'], "_"))
}

fn gen_struct_de(name: &syn::Ident, fields: &[FieldInfo]) -> TokenStream2 {
    let field_idents: Vec<&syn::Ident> = fields.iter().map(|f| &f.ident).collect();
    let field_wires: Vec<String> = fields
        .iter()
        .map(|f| wire_name(&f.attrs.name, &f.ident))
        .collect();
    let field_vars: Vec<syn::Ident> = fields
        .iter()
        .map(|f| format_ident!("__field_{}", f.ident))
        .collect();

    let none_inits: Vec<TokenStream2> = field_vars
        .iter()
        .map(|v| quote! { let mut #v = None; })
        .collect();

    let shared = shared_wires(fields);
    let mut match_arms: Vec<TokenStream2> = fields
        .iter()
        .enumerate()
        .filter(|(i, _)| !shared.contains(&field_wires[*i]))
        .map(|(i, f)| {
            let w = &field_wires[i];
            let v = &field_vars[i];
            let cond = f.attrs.condition.as_ref().map(|c| condition_cfg(c));
            quote! { #cond #w => { #v = Some(map.next_value()?); } }
        })
        .collect();
    let mut none_inits: Vec<TokenStream2> = none_inits
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !shared.contains(&field_wires[*i]))
        .map(|(_, t)| t)
        .collect();
    for w in &shared {
        let raw = raw_ident(w);
        none_inits.push(quote! { let mut #raw: Option<serde_json::Value> = None; });
        match_arms.push(quote! { #w => { #raw = Some(map.next_value()?); } });
    }

    let mut inits: Vec<TokenStream2> = fields
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let v = &field_vars[i];
            let id = &f.ident;
            let w = &field_wires[i];
            if shared.contains(w) {
                let raw = raw_ident(w);
                alternate_init(f, &quote! { #raw })
            } else if is_option_type(&f.ty) {
                quote! { let #id = #v.unwrap_or(None); }
            } else {
                quote! { let #id = #v.ok_or_else(|| serde::de::Error::missing_field(#w))?; }
            }
        })
        .collect();
    for w in &shared {
        let raw = raw_ident(w);
        let ids: Vec<&syn::Ident> = fields
            .iter()
            .filter(|f| !f.attrs.flatten && &wire_name(&f.attrs.name, &f.ident) == w)
            .map(|f| &f.ident)
            .collect();
        inits.push(alternates_check(name, w, &quote! { #raw }, &ids));
    }

    quote! {
        #[allow(deprecated)]
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

    let shared = shared_wires(fields);
    let mut extractions: Vec<TokenStream2> = shared
        .iter()
        .map(|w| {
            let raw = raw_ident(w);
            quote! { let #raw: Option<serde_json::Value> = map.remove(#w); }
        })
        .collect();
    extractions.extend(non_flat.iter().map(|f| {
        let id = &f.ident;
        let w = wire_name(&f.attrs.name, &f.ident);
        let ty = &f.ty;
        if shared.contains(&w) {
            let raw = raw_ident(&w);
            alternate_init(f, &quote! { #raw })
        } else if is_option_type(ty) {
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
    }));
    for w in &shared {
        let raw = raw_ident(w);
        let ids: Vec<&syn::Ident> = non_flat
            .iter()
            .filter(|f| &wire_name(&f.attrs.name, &f.ident) == w)
            .map(|f| &f.ident)
            .collect();
        extractions.push(alternates_check(name, w, &quote! { #raw }, &ids));
    }

    // Find discriminator field (if any) for gating union deserialization
    let discriminators: Vec<&FieldInfo> = fields.iter().filter(|f| f.attrs.discriminator).collect();
    let discriminator_field: Option<&FieldInfo> = discriminators.first().copied();

    let flat_extractions: Vec<TokenStream2> = flat.iter().map(|f| {
        let id = &f.ident;
        let ty = &f.ty;
        if f.attrs.union_field {
            if let Some(disc_f) = discriminator_field {
                let disc_id = &disc_f.ident;
                if discriminators.len() > 1 {
                    // Version-alternate discriminators: use whichever decoded,
                    // preferring one that is not removed.
                    let inner_ty = inner_option_type(ty)
                        .expect("union field with alternate discriminators must be Option<T>");
                    let mut ordered = discriminators.clone();
                    ordered.sort_by_key(|f| f.attrs.until.is_some());
                    let ids: Vec<&syn::Ident> = ordered.iter().map(|f| &f.ident).collect();
                    quote! {
                        let __disc: Option<String> = None #(.or_else(|| #ids.as_ref().map(|d| d.to_string())))*;
                        let #id: #ty = match __disc {
                            Some(d) => Some(<#inner_ty as qapi::QapiFlatUnion>::deserialize_variant(
                                &d,
                                full_map.clone(),
                            ).map_err(serde::de::Error::custom)?),
                            None => None,
                        };
                    }
                } else if is_option_type(&disc_f.ty) {
                    // Optional discriminator (e.g. added in later version):
                    // when None, union is None; when Some, use discriminator to
                    // select the correct variant via QapiFlatUnion trait.
                    let inner_ty = inner_option_type(ty)
                        .expect("union field with optional discriminator must be Option<T>");
                    quote! {
                        let #id: #ty = if let Some(ref __disc_val) = #disc_id {
                            Some(<#inner_ty as qapi::QapiFlatUnion>::deserialize_variant(
                                &__disc_val.to_string(),
                                full_map.clone(),
                            ).map_err(serde::de::Error::custom)?)
                        } else {
                            None
                        };
                    }
                } else {
                    // Required discriminator: use discriminator value to select variant.
                    if is_option_type(ty) {
                        let inner_ty = inner_option_type(ty).unwrap();
                        quote! {
                            let #id: #ty = Some(<#inner_ty as qapi::QapiFlatUnion>::deserialize_variant(
                                &#disc_id.to_string(),
                                full_map.clone(),
                            ).map_err(serde::de::Error::custom)?);
                        }
                    } else {
                        quote! {
                            let #id: #ty = <#ty as qapi::QapiFlatUnion>::deserialize_variant(
                                &#disc_id.to_string(),
                                full_map.clone(),
                            ).map_err(serde::de::Error::custom)?;
                        }
                    }
                }
            } else {
                // Union field without discriminator — fall back to full map deser
                quote! {
                    let #id: #ty = serde_json::from_value(serde_json::Value::Object(full_map.clone())).map_err(serde::de::Error::custom)?;
                }
            }
        } else {
            quote! {
                let #id: #ty = serde_json::from_value(serde_json::Value::Object(full_map.clone())).map_err(serde::de::Error::custom)?;
            }
        }
    }).collect();

    let all_ids: Vec<&syn::Ident> = fields.iter().map(|f| &f.ident).collect();

    quote! {
        #[allow(deprecated)]
        impl<'de> serde::Deserialize<'de> for #name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where D: serde::Deserializer<'de> {
                let value = <serde_json::Value as serde::Deserialize>::deserialize(deserializer)?;
                let mut map = match value {
                    serde_json::Value::Object(m) => m,
                    _ => return Err(serde::de::Error::custom("expected object")),
                };
                let full_map = map.clone();
                #(#extractions)*
                #(#flat_extractions)*
                Ok(#name { #(#all_ids),* })
            }
        }
    }
}

fn gen_command_impl(name: &syn::Ident, wn: &str, ta: &TypeAttrs) -> syn::Result<TokenStream2> {
    // Validate the user-supplied `returns = "…"` value as a Rust type.
    // Raw `str::parse` panics on malformed input; parse through `syn` so a
    // bad attribute becomes a compile_error pointing at the struct ident
    // instead of an ICE.
    let returns_src = ta.returns.as_deref().unwrap_or("()");
    let returns: Type = syn::parse_str(returns_src).map_err(|e| {
        syn::Error::new(
            name.span(),
            format!(
                "qapi: `returns = \"{}\"` is not a valid Rust type: {}",
                returns_src, e
            ),
        )
    })?;
    let oob = ta.allow_oob;
    let preconfig = ta.allow_preconfig.present;
    let success = !ta.no_success_response.present;
    let unstable = ta.unstable.present && ta.unstable.removed.is_none();

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

    // PRECONFIG_SINCE: version when allow-preconfig was added for this command.
    // Uses the explicit added version from the range, or falls back to the
    // command's own since version if preconfig was present from the start.
    let preconfig_since_const = if let Some(ref s) = ta.allow_preconfig.added {
        let (maj, min, pat) = parse_version(s);
        quote! { const PRECONFIG_SINCE: Option<(u32, u32, u32)> = Some((#maj, #min, #pat)); }
    } else if preconfig {
        // Present from the start — use the command's own since version
        if let Some(ref s) = ta.since {
            let (maj, min, pat) = parse_version(s);
            quote! { const PRECONFIG_SINCE: Option<(u32, u32, u32)> = Some((#maj, #min, #pat)); }
        } else {
            quote! { const PRECONFIG_SINCE: Option<(u32, u32, u32)> = Some((0, 0, 0)); }
        }
    } else {
        quote! { const PRECONFIG_SINCE: Option<(u32, u32, u32)> = None; }
    };

    let preconfig_impl = if preconfig {
        quote! {
            #[allow(deprecated)]
            impl qapi::PreconfigSafe for #name {}
        }
    } else {
        quote! {}
    };

    Ok(quote! {
        #[allow(deprecated)]
        impl qapi::QapiCommand for #name {
            type Returns = #returns;
            const NAME: &'static str = #wn;
            const ALLOW_OOB: bool = #oob;
            const ALLOW_PRECONFIG: bool = #preconfig;
            const SUCCESS_RESPONSE: bool = #success;
            const UNSTABLE: bool = #unstable;
            #since_const
            #until_const
            #preconfig_since_const
        }

        #preconfig_impl
    })
}

fn gen_event_impl(name: &syn::Ident, wn: &str) -> TokenStream2 {
    quote! {
        #[allow(deprecated)]
        impl qapi::QapiEvent for #name {
            const NAME: &'static str = #wn;
        }
    }
}
