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

use qapi_schema::QapiSchema;

use nom::branch::alt;
use nom::bytes::complete::{tag, take_until};
use nom::character::complete::{line_ending, multispace0, not_line_ending, space0};
use nom::combinator::{all_consuming, not, opt, peek, recognize};
use nom::multi::{many1, separated_list0};
use nom::sequence::{delimited, preceded, tuple};
use nom::IResult;

pub(crate) fn qcomment(input: &str) -> IResult<&str, &str> {
    delimited(
        tuple((tag("#"), not(peek(tag("#"))), space0)),
        not_line_ending,
        line_ending,
    )(input)
}

pub(crate) fn clean_lines(input: &str) -> IResult<&str, &str> {
    recognize(tuple((
        multispace0,
        opt(tuple((many1(tuple((multispace0, qcomment))), multispace0))),
    )))(input)
}

pub(crate) fn qtag<'i>(t: &'static str) -> impl FnMut(&'i str) -> IResult<&'i str, &'i str> {
    preceded(clean_lines, tag(t))
}

pub(crate) fn qstring(input: &str) -> IResult<&str, &str> {
    delimited(qtag("'"), take_until("'"), tag("'"))(input)
}

pub(crate) fn qbool(input: &str) -> IResult<&str, &str> {
    alt((qtag("true"), qtag("false")))(input)
}

pub(crate) fn take_list<'i, I, O>(item_parser: I) -> impl FnMut(&'i str) -> IResult<&'i str, Vec<O>>
where
    I: FnMut(&'i str) -> IResult<&'i str, O>,
{
    delimited(
        qtag("["),
        separated_list0(qtag(","), item_parser),
        qtag("]"),
    )
}

pub(crate) fn take_dict<'i, I, O>(item_parser: I) -> impl FnMut(&'i str) -> IResult<&'i str, Vec<O>>
where
    I: FnMut(&'i str) -> IResult<&'i str, O>,
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
) -> impl FnMut(&'i str) -> IResult<&'i str, O>
where
    I: FnMut(&'i str) -> IResult<&'i str, O>,
{
    delimited(
        tuple((qtag("'"), tag(key), qtag("'"), qtag(":"))),
        value_parser,
        clean_lines,
    )
}
