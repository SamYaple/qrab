use nom::bytes::complete::{tag, take_until};
use nom::character::complete::multispace0;
use nom::combinator::{opt, recognize};
use nom::multi::many1;
use nom::sequence::{delimited, preceded, terminated, tuple};
use nom::IResult;

pub(crate) fn take_comment(input: &str) -> IResult<&str, &str> {
    delimited(tag("#"), take_until("\n"), multispace0)(input)
}

pub(crate) fn cl(input: &str) -> IResult<&str, &str> {
    recognize(tuple((multispace0, opt(take_comment), multispace0)))(input)
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

pub(crate) fn dict_parser<'input, I, O>(
    item_parser: I,
) -> impl FnMut(&'input str) -> IResult<&'input str, O>
where
    I: FnMut(&'input str) -> IResult<&'input str, O>,
{
    array_parser(tag("{"), item_parser, tag("}"))
}

pub(crate) fn list_parser<'input, I, O>(
    item_parser: I,
) -> impl FnMut(&'input str) -> IResult<&'input str, Vec<O>>
where
    I: FnMut(&'input str) -> IResult<&'input str, O>,
{
    array_parser(
        tag("["),
        many1(terminated(item_parser, tuple((cl, opt(tag(",")))))),
        tag("]"),
    )
}

pub(crate) fn kv_parser<'input, I1, I2, O1, O2>(
    key_parser: I1,
    value_parser: I2,
) -> impl FnMut(&'input str) -> IResult<&'input str, O2>
where
    I1: FnMut(&'input str) -> IResult<&'input str, O1>,
    I2: FnMut(&'input str) -> IResult<&'input str, O2>,
{
    delimited(
        tuple((cl, tag("'"), key_parser, tag("'"), cl, tag(":"), cl)),
        value_parser,
        cl,
    )
}
