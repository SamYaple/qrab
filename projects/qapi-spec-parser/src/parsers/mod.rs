pub mod qapi_alternate;
pub mod qapi_alternative;
pub mod qapi_alternatives;
pub mod qapi_branch;
pub mod qapi_branches;
pub mod qapi_variant;
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

use std::cell::RefCell;

/// Configuration for version-specific parser behavior.
///
/// The parser is strict by default (modern QEMU format). Each flag enables
/// tolerance for a specific older format quirk. Use `ParserConfig::for_version()`
/// to get the right config for a specific QEMU version.
#[derive(Debug, Clone)]
pub struct ParserConfig {
    /// Accept `'type'` as an alias for `'struct'` (v1.0–v2.3.0)
    pub type_as_struct: bool,
    /// Accept simple unions without `'discriminator'`/`'base'` (v1.0–v6.1.0)
    pub simple_unions: bool,
    /// Accept `'discriminator': {}` as alternate syntax (v1.7.0–v2.0.0)
    pub empty_discriminator_as_alternate: bool,
    /// Accept old pragma keys: `'returns-whitelist'`, `'name-case-whitelist'` (v2.10.0–v4.2.0)
    pub old_pragma_keys: bool,
    /// Accept `'gen': 'no'`/`'gen': 'yes'` as booleans (v1.0–v2.3.0)
    pub gen_string_as_bool: bool,
    /// Accept doc blocks starting with `# @Name` without preceding `##` (v1.0–v2.x)
    pub bare_doc_blocks: bool,
    /// Accept `# @Name` without trailing colon (v1.0–v2.x)
    pub optional_doc_name_colon: bool,
    /// Accept inline anonymous structs as member types (v1.0–v2.3.0)
    pub inline_member_structs: bool,
    /// Accept blank lines inside `##...##` doc blocks (v6.0.0+)
    pub doc_blank_lines: bool,
    /// Accept leading whitespace before `##` markers (v6.2.0+)
    pub doc_leading_whitespace: bool,
    /// Accept trailing commas in lists and dicts
    pub trailing_commas: bool,
}

impl Default for ParserConfig {
    fn default() -> Self {
        Self::modern()
    }
}

impl ParserConfig {
    /// Strict modern parser — only accepts the canonical format (v10.2.0+).
    pub fn modern() -> Self {
        Self {
            type_as_struct: false,
            simple_unions: false,
            empty_discriminator_as_alternate: false,
            old_pragma_keys: false,
            gen_string_as_bool: false,
            bare_doc_blocks: false,
            optional_doc_name_colon: false,
            inline_member_structs: false,
            doc_blank_lines: false,
            doc_leading_whitespace: false,
            trailing_commas: true, // harmless leniency, always on
        }
    }

    /// Parser config for a specific QEMU version.
    pub fn for_version(v: (u32, u32, u32)) -> Self {
        Self {
            type_as_struct: v < (2, 4, 0),
            simple_unions: v < (6, 2, 0),
            empty_discriminator_as_alternate: v < (2, 1, 0),
            old_pragma_keys: v < (5, 0, 0),
            gen_string_as_bool: v < (2, 4, 0),
            bare_doc_blocks: v < (2, 8, 0),
            optional_doc_name_colon: v < (2, 4, 0),
            inline_member_structs: v < (2, 4, 0),
            doc_blank_lines: true, // various versions have this
            doc_leading_whitespace: true, // various versions have this
            trailing_commas: true,
        }
    }
}

thread_local! {
    static PARSER_CONFIG: RefCell<ParserConfig> = RefCell::new(ParserConfig::modern());
}

/// Set the thread-local parser config.
pub fn set_parser_config(config: ParserConfig) {
    PARSER_CONFIG.with(|c| *c.borrow_mut() = config);
}

/// Access the current parser config.
pub fn with_parser_config<T>(f: impl FnOnce(&ParserConfig) -> T) -> T {
    PARSER_CONFIG.with(|c| f(&c.borrow()))
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

use nom::branch::alt;
use nom::bytes::complete::{tag, take_until};
use nom::character::complete::{line_ending, multispace0, not_line_ending, space0};
use nom::combinator::{not, opt, peek, recognize};
use nom::error::Error;
use nom::multi::{many1, separated_list0};
use nom::sequence::{delimited, preceded};
use nom::IResult;
use nom::Parser;

pub(crate) fn qcomment(input: &str) -> IResult<&str, &str> {
    delimited(
        (tag("#"), not(peek(tag("#"))), space0),
        not_line_ending,
        line_ending,
    )
    .parse(input)
}

pub(crate) fn clean_lines(input: &str) -> IResult<&str, &str> {
    recognize((
        multispace0,
        opt((many1((multispace0, qcomment)), multispace0)),
    ))
    .parse(input)
}

pub(crate) fn qtag<'i>(
    t: &'static str,
) -> impl Parser<&'i str, Output = &'i str, Error = Error<&'i str>> {
    preceded(clean_lines, tag(t))
}

pub(crate) fn qstring(input: &str) -> IResult<&str, &str> {
    delimited(qtag("'"), take_until("'"), tag("'")).parse(input)
}

pub(crate) fn qbool(input: &str) -> IResult<&str, &str> {
    alt((qtag("true"), qtag("false"))).parse(input)
}

pub(crate) fn take_list<'i, I, O>(
    item_parser: I,
) -> impl Parser<&'i str, Output = Vec<O>, Error = Error<&'i str>>
where
    I: Parser<&'i str, Output = O, Error = Error<&'i str>>,
{
    delimited(
        qtag("["),
        (separated_list0(qtag(","), item_parser), opt(qtag(","))).map(|(v, _)| v),
        qtag("]"),
    )
}

pub(crate) fn take_dict<'i, I, O>(
    item_parser: I,
) -> impl Parser<&'i str, Output = Vec<O>, Error = Error<&'i str>>
where
    I: Parser<&'i str, Output = O, Error = Error<&'i str>>,
{
    delimited(
        qtag("{"),
        (separated_list0(qtag(","), item_parser), opt(qtag(","))).map(|(v, _)| v),
        qtag("}"),
    )
}

pub(crate) fn take_kv<'i, I, O>(
    key: &'static str,
    value_parser: I,
) -> impl Parser<&'i str, Output = O, Error = Error<&'i str>>
where
    I: Parser<&'i str, Output = O, Error = Error<&'i str>>,
{
    delimited(
        (qtag("'"), tag(key), qtag("'"), qtag(":")),
        value_parser,
        clean_lines,
    )
}
