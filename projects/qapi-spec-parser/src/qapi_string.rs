use crate::helpers::qtag;
use nom::bytes::complete::{tag, take_until};
use nom::combinator::map;
use nom::sequence::delimited;
use nom::IResult;

#[derive(Debug, PartialEq, Clone)]
pub struct QapiString(pub String);
impl QapiString {
    pub fn parse(input: &str) -> IResult<&str, Self> {
        map(
            delimited(
                qtag("'"),
                take_until("'"), // TODO handle escaped '\''?
                tag("'"),
            ),
            |v| Self(v.into()),
        )(input)
    }
}
