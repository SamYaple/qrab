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
        separated_list0(qtag(","), item_parser),
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
        separated_list0(qtag(","), item_parser),
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
