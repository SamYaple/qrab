pub mod common;
pub mod qapi_alternate;
pub mod qapi_alternative;
pub mod qapi_alternatives;
pub mod qapi_branch;
pub mod qapi_branches;
pub mod qapi_command;
pub mod qapi_cond;
pub mod qapi_documentation;
pub mod qapi_enum;
pub mod qapi_enum_value;
pub mod qapi_event;
pub mod qapi_feature;
pub mod qapi_features;
pub mod qapi_include;
pub mod qapi_member;
pub mod qapi_members;
pub mod qapi_pragma;
pub mod qapi_schema;
pub mod qapi_struct;
pub mod qapi_type_ref;
pub mod qapi_union;
pub mod qapi_variant;

use std::cell::Cell;

thread_local! {
    static PARSER_VERSION: Cell<(u32, u32, u32)> = const { Cell::new((99, 0, 0)) };
}

/// Set the QEMU schema version for parser dispatch.
/// Each parser's `parse()` uses this to select the correct versioned parser.
pub fn set_parser_version(v: (u32, u32, u32)) {
    PARSER_VERSION.with(|c| c.set(v));
}

/// Get the current QEMU schema version for parser dispatch.
pub fn parser_version() -> (u32, u32, u32) {
    PARSER_VERSION.with(|c| c.get())
}

/// Generates Deref, DerefMut, and IntoIterator impls for a Vec newtype.
macro_rules! vec_newtype {
    ($name:ident, $inner:ty) => {
        impl<'i> std::ops::Deref for $name<'i> {
            type Target = Vec<$inner>;
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
        impl<'i> std::ops::DerefMut for $name<'i> {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }
        impl<'i> IntoIterator for $name<'i> {
            type Item = $inner;
            type IntoIter = std::vec::IntoIter<Self::Item>;
            fn into_iter(self) -> Self::IntoIter {
                self.0.into_iter()
            }
        }
    };
}
pub(crate) use vec_newtype;
