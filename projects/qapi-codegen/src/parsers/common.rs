//! Shared nom parser helpers used across all parser versions.

use nom::IResult;
use nom::Parser;
use nom::branch::alt;
use nom::bytes::complete::{escaped, tag, take_till, take_while, take_while1};
use nom::character::complete::space0;
use nom::combinator::{eof, not, opt, peek, recognize};
use nom::error::{Error, ErrorKind};
use nom::multi::{many1, separated_list0};
use nom::sequence::{delimited, preceded};
use std::collections::HashSet;

pub fn qcomment(input: &str) -> IResult<&str, &str> {
    delimited(
        (tag("#"), not(peek(tag("#"))), space0),
        not_line_ending,
        alt((line_ending, eof)),
    )
    .parse(input)
}

// QEMU's Python lexer uses str.isspace(), including the four C0 separators.
pub(crate) fn is_whitespace(c: char) -> bool {
    c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}')
}

/// QEMU reads schema files with Python's universal newline handling.
pub fn line_ending(input: &str) -> IResult<&str, &str> {
    alt((tag("\r\n"), tag("\n"), tag("\r"))).parse(input)
}

pub fn not_line_ending(input: &str) -> IResult<&str, &str> {
    take_till(|c| c == '\r' || c == '\n').parse(input)
}

pub(crate) fn lines(mut input: &str) -> impl Iterator<Item = &str> {
    std::iter::from_fn(move || {
        if input.is_empty() {
            return None;
        }
        let end = input.find(['\r', '\n']).unwrap_or(input.len());
        let line = &input[..end];
        input = &input[end..];
        if input.starts_with("\r\n") {
            input = &input[2..];
        } else if !input.is_empty() {
            input = &input[1..];
        }
        Some(line)
    })
}

pub fn whitespace0(input: &str) -> IResult<&str, &str> {
    take_while(is_whitespace).parse(input)
}

pub fn whitespace1(input: &str) -> IResult<&str, &str> {
    take_while1(is_whitespace).parse(input)
}

pub fn clean_lines(input: &str) -> IResult<&str, &str> {
    recognize((
        whitespace0,
        opt((many1((whitespace0, qcomment)), whitespace0)),
    ))
    .parse(input)
}

pub fn qtag<'i>(t: &'static str) -> impl Parser<&'i str, Output = &'i str, Error = Error<&'i str>> {
    preceded(clean_lines, tag(t))
}

/// Borrow raw string contents; QAPI permits only printable ASCII and `\\` escapes.
pub fn qstring(input: &str) -> IResult<&str, &str> {
    delimited(
        qtag("'"),
        opt(escaped(
            take_while1(|c: char| (c.is_ascii_graphic() && c != '\\' && c != '\'') || c == ' '),
            '\\',
            tag("\\"),
        ))
        .map(|value| value.unwrap_or("")),
        tag("'"),
    )
    .parse(input)
}

pub fn qbool(input: &str) -> IResult<&str, &str> {
    alt((qtag("true"), qtag("false"))).parse(input)
}

/// Modern QAPI uses JSON separators. Retain the permissive legacy grammar
/// only when parsing pre-2.4 schemas.
pub fn take_list<'i, I, O>(
    item_parser: I,
) -> impl Parser<&'i str, Output = Vec<O>, Error = Error<&'i str>>
where
    I: Parser<&'i str, Output = O, Error = Error<&'i str>>,
{
    delimited(qtag("["), separated_items(item_parser), qtag("]"))
}

/// Parse a dict without silently overwriting duplicate keys.
pub fn take_dict<'i, I, O>(
    mut item_parser: I,
) -> impl Parser<&'i str, Output = Vec<O>, Error = Error<&'i str>>
where
    I: Parser<&'i str, Output = O, Error = Error<&'i str>>,
{
    move |input| {
        let mut keys = HashSet::new();
        let mut entry = |input| {
            let (_, key) = qstring(input)?;
            if !keys.insert(key) {
                return Err(nom::Err::Failure(Error::new(input, ErrorKind::Verify)));
            }
            item_parser.parse(input)
        };
        delimited(qtag("{"), separated_items(&mut entry), qtag("}")).parse(input)
    }
}

fn separated_items<'i, I, O>(
    mut item_parser: I,
) -> impl Parser<&'i str, Output = Vec<O>, Error = Error<&'i str>>
where
    I: Parser<&'i str, Output = O, Error = Error<&'i str>>,
{
    move |input| {
        if super::parser_version() < (2, 4, 0) {
            separated_list_flexible(|i| item_parser.parse(i)).parse(input)
        } else {
            separated_list0(qtag(","), |i| item_parser.parse(i)).parse(input)
        }
    }
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
                    if remaining.len() == after_ws.len() {
                        return Err(nom::Err::Error(Error::new(after_ws, ErrorKind::Many0)));
                    }
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

#[cfg(test)]
mod tests {
    use crate::{parse_schema, set_parser_version};

    #[test]
    fn strings_follow_qemu_lexical_rules() {
        use super::qstring;
        for input in ["''", "'name with spaces'", r"'path\\name'"] {
            assert_eq!(qstring(input).unwrap().0, "");
        }
        for input in [
            "'line\nbreak'",
            "'tab\there'",
            "'snowman ☃'",
            r"'bad\n'",
            r"'bad\'quote'",
        ] {
            assert!(qstring(input).is_err(), "accepted {input}");
        }
    }

    #[test]
    fn modern_separators_match_qemu() {
        set_parser_version((11, 1, 3));
        for input in [
            "{ 'enum': 'E', 'data': ['a' 'b'] }",
            "{ 'enum': 'E', 'data': ['a',] }",
            "{ 'enum': 'E' 'data': ['a'] }",
            "{ 'enum': 'E', 'data': ['a'], }",
            "{ 'struct': 'S', 'data': { 'x': 'str', 'x': 'int' } }",
            "{ 'enum': 'E', 'enum': 'F', 'data': [] }",
        ] {
            assert!(
                parse_schema(input).is_err(),
                "Accepted invalid QAPI: {input}"
            );
        }
        parse_schema("{ 'enum': 'E', 'data': ['a', 'b'] }").unwrap();
    }

    #[test]
    fn dictionary_keywords_are_exact_strings() {
        set_parser_version((11, 1, 3));
        for input in [
            "{ 'enum ': 'E', 'data': ['a'] }",
            "{ 'enum# comment\n': 'E', 'data': ['a'] }",
            "{ 'enum': 'E', 'data ': ['a'] }",
        ] {
            assert!(parse_schema(input).is_err(), "accepted {input}");
        }
        parse_schema("{ 'enum' # comment\n : 'E', 'data': ['a'] }").unwrap();
    }

    #[test]
    fn historical_separator_compatibility() {
        set_parser_version((1, 0, 0));
        parse_schema("{ 'enum': 'E' 'data': ['a' 'b',], }").unwrap();
    }

    #[test]
    fn comments_can_end_at_eof() {
        parse_schema("# final comment").unwrap();
        parse_schema("{ 'enum': 'E', 'data': ['a'] } # final comment").unwrap();
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
        (qtag("'"), tag(key), tag("'"), qtag(":")),
        value_parser,
        clean_lines,
    )
}
