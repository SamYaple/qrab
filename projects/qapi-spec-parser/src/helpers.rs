use nom::bytes::complete::{tag, take_until};
use nom::character::complete::multispace0;
use nom::combinator::{opt, recognize};
use nom::multi::many1;
use nom::sequence::{delimited, preceded, terminated, tuple};
use nom::IResult;

pub(crate) fn take_comment(input: &str) -> IResult<&str, &str> {
    preceded(tuple((multispace0, tag("#"))), take_until("\n"))(input)
}

pub(crate) fn cl(input: &str) -> IResult<&str, &str> {
    recognize(tuple((opt(take_comment), multispace0)))(input)
}

pub(crate) fn qtag<'input>(
    t: &'static str,
) -> impl FnMut(&'input str) -> IResult<&'input str, &'input str> {
    preceded(cl, tag(t))
}

pub(crate) fn array_parser<'input, I1, I2, I3, O1, O2, O3>(
    begin_delim: I1,
    item_parser: I2,
    end_delim: I3,
) -> impl FnMut(&'input str) -> IResult<&'input str, O2>
where
    I1: FnMut(&'input str) -> IResult<&'input str, O1>,
    I2: FnMut(&'input str) -> IResult<&'input str, O2>,
    I3: FnMut(&'input str) -> IResult<&'input str, O3>,
{
    delimited(
        tuple((cl, begin_delim)),
        preceded(cl, item_parser),
        tuple((cl, end_delim)),
    )
}

pub(crate) fn dict<'input, I, O>(
    item_parser: I,
) -> impl FnMut(&'input str) -> IResult<&'input str, Vec<O>>
where
    I: FnMut(&'input str) -> IResult<&'input str, O>,
{
    array_parser(
        qtag("{"),
        many1(terminated(item_parser, opt(qtag(",")))),
        qtag("}"),
    )
}

pub(crate) fn list<'input, I, O>(
    item_parser: I,
) -> impl FnMut(&'input str) -> IResult<&'input str, Vec<O>>
where
    I: FnMut(&'input str) -> IResult<&'input str, O>,
{
    array_parser(
        qtag("["),
        many1(terminated(item_parser, opt(qtag(",")))),
        qtag("]"),
    )
}
pub(crate) fn kv<'input, I1, I2, O1, O2>(
    key_parser: I1,
    value_parser: I2,
) -> impl FnMut(&'input str) -> IResult<&'input str, O2>
where
    I1: FnMut(&'input str) -> IResult<&'input str, O1>,
    I2: FnMut(&'input str) -> IResult<&'input str, O2>,
{
    delimited(
        tuple((qtag("'"), key_parser, qtag("'"), qtag(":"))),
        value_parser,
        cl,
    )
}
