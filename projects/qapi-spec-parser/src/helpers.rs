use nom::bytes::complete::{tag, take_until};
use nom::character::complete::multispace0;
use nom::combinator::{opt, recognize};
use nom::sequence::{delimited, preceded, tuple};
use nom::IResult;

pub(crate) fn take_comment(input: &str) -> IResult<&str, &str> {
    preceded(tag("#"), take_until("\n"))(input)
}

pub(crate) fn clean_line(input: &str) -> IResult<&str, &str> {
    recognize(tuple((multispace0, opt(take_comment), multispace0)))(input)
}

pub(crate) fn take_qapi_string(input: &str) -> IResult<&str, &str> {
    delimited(
        tuple((clean_line, tag("'"))),
        take_until("'"),
        tuple((tag("'"), clean_line)),
    )(input)
}
