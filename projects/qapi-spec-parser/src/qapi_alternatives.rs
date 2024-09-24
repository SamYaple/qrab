use crate::helpers::{qtag, take_kv};
use crate::take_alternative;
use crate::QapiAlternative;
use nom::combinator::map;
use nom::multi::separated_list1;
use nom::sequence::delimited;
use nom::IResult;

pub fn take_alternatives(input: &str) -> IResult<&str, QapiAlternatives<'_>> {
    take_kv("data", QapiAlternatives::parse)(input)
}

#[derive(Debug, Clone)]
pub struct QapiAlternatives<'i>(pub Vec<QapiAlternative<'i>>);
impl<'i> QapiAlternatives<'i> {
    /// ALTERNATIVES = { ALTERNATIVE, ... }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        map(
            delimited(
                qtag("{"),
                separated_list1(qtag(","), take_alternative),
                qtag("}"),
            ),
            |v| Self(v),
        )(input)
    }
}
