use crate::helpers::cl;
use nom::bytes::complete::{tag, take_until};
use nom::combinator::map;
use nom::sequence::{delimited, tuple};
use nom::IResult;

#[derive(Debug, PartialEq)]
pub struct QapiString(pub String);
impl QapiString {
    pub fn parse(input: &str) -> IResult<&str, Self> {
        map(
            delimited(
                tuple((cl, tag("'"))),
                take_until("'"), // TODO handle escaped '\''?
                tuple((tag("'"), cl)),
            ),
            |v| Self(v.into()),
        )(input)
    }
}
