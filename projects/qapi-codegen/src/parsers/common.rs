//! Shared nom parser helpers used across all parser versions.

use nom::branch::alt;
use nom::bytes::complete::{tag, take_until};
use nom::character::complete::{line_ending, multispace0, not_line_ending, space0};
use nom::combinator::{not, opt, peek, recognize};
use nom::error::Error;
use nom::multi::{many1, separated_list0};
use nom::sequence::{delimited, preceded};
use nom::IResult;
use nom::Parser;

pub fn qcomment(input: &str) -> IResult<&str, &str> {
    delimited(
        (tag("#"), not(peek(tag("#"))), space0),
        not_line_ending,
        line_ending,
    )
    .parse(input)
}

pub fn clean_lines(input: &str) -> IResult<&str, &str> {
    recognize((
        multispace0,
        opt((many1((multispace0, qcomment)), multispace0)),
    ))
    .parse(input)
}

pub fn qtag<'i>(
    t: &'static str,
) -> impl Parser<&'i str, Output = &'i str, Error = Error<&'i str>> {
    preceded(clean_lines, tag(t))
}

pub fn qstring(input: &str) -> IResult<&str, &str> {
    delimited(qtag("'"), take_until("'"), tag("'")).parse(input)
}

pub fn qbool(input: &str) -> IResult<&str, &str> {
    alt((qtag("true"), qtag("false"))).parse(input)
}

pub fn take_list<'i, I, O>(
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

pub fn take_dict<'i, I, O>(
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

pub fn take_kv<'i, I, O>(
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
