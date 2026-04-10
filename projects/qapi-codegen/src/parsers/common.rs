//! Shared nom parser helpers used across all parser versions.

use nom::IResult;
use nom::Parser;
use nom::branch::alt;
use nom::bytes::complete::{tag, take_until};
use nom::character::complete::{line_ending, multispace0, not_line_ending, space0};
use nom::combinator::{not, opt, peek, recognize};
use nom::error::Error;
use nom::multi::many1;
use nom::sequence::{delimited, preceded};

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

pub fn qtag<'i>(t: &'static str) -> impl Parser<&'i str, Output = &'i str, Error = Error<&'i str>> {
    preceded(clean_lines, tag(t))
}

pub fn qstring(input: &str) -> IResult<&str, &str> {
    delimited(qtag("'"), take_until("'"), tag("'")).parse(input)
}

pub fn qbool(input: &str) -> IResult<&str, &str> {
    alt((qtag("true"), qtag("false"))).parse(input)
}

/// Parse a comma-separated list. Commas between items are optional,
/// matching QEMU's Python-based QAPI parser behavior.
/// Trailing commas are also accepted.
pub fn take_list<'i, I, O>(
    item_parser: I,
) -> impl Parser<&'i str, Output = Vec<O>, Error = Error<&'i str>>
where
    I: Parser<&'i str, Output = O, Error = Error<&'i str>>,
{
    delimited(qtag("["), separated_list_flexible(item_parser), qtag("]"))
}

/// Parse a dict of key-value entries. Commas between entries are optional,
/// matching QEMU's Python-based QAPI parser behavior.
/// Trailing commas are also accepted.
pub fn take_dict<'i, I, O>(
    item_parser: I,
) -> impl Parser<&'i str, Output = Vec<O>, Error = Error<&'i str>>
where
    I: Parser<&'i str, Output = O, Error = Error<&'i str>>,
{
    delimited(qtag("{"), separated_list_flexible(item_parser), qtag("}"))
}

/// Parse items separated by optional commas. Accepts:
/// - `a, b, c` (standard)
/// - `a b c` (missing commas)
/// - `a, b, c,` (trailing comma)
fn separated_list_flexible<'i, I, O>(
    mut item_parser: I,
) -> impl Parser<&'i str, Output = Vec<O>, Error = Error<&'i str>>
where
    I: Parser<&'i str, Output = O, Error = Error<&'i str>>,
{
    move |mut input: &'i str| {
        let mut items = Vec::new();
        // Try to parse the first item (with optional leading whitespace via clean_lines)
        let first_input = clean_lines.parse(input).map(|(i, _)| i).unwrap_or(input);
        match item_parser.parse(first_input) {
            Ok((remaining, item)) => {
                items.push(item);
                input = remaining;
            }
            Err(nom::Err::Error(_)) => return Ok((input, items)),
            Err(e) => return Err(e),
        }
        // Parse subsequent items, each optionally preceded by a comma
        loop {
            // Consume optional comma
            let after_comma = match qtag::<'i>(",").parse(input) {
                Ok((remaining, _)) => remaining,
                Err(nom::Err::Error(_)) => input,
                Err(e) => return Err(e),
            };
            // Try to parse next item
            let after_ws = clean_lines
                .parse(after_comma)
                .map(|(i, _)| i)
                .unwrap_or(after_comma);
            match item_parser.parse(after_ws) {
                Ok((remaining, item)) => {
                    items.push(item);
                    input = remaining;
                }
                Err(nom::Err::Error(_)) => {
                    // No more items — if we consumed a comma, that's a trailing comma (ok)
                    return Ok((after_comma, items));
                }
                Err(e) => return Err(e),
            }
        }
    }
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
