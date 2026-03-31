use super::{take_dict, take_kv, vec_newtype};
use crate::take_alternative;
use crate::QapiAlternative;
use nom::IResult;
use nom::Parser;

pub fn take_alternatives(input: &str) -> IResult<&str, QapiAlternatives<'_>> {
    take_kv("data", QapiAlternatives::parse).parse(input)
}

#[derive(Debug, Clone, Default)]
pub struct QapiAlternatives<'i>(pub Vec<QapiAlternative<'i>>);
impl<'i> QapiAlternatives<'i> {
    /// ALTERNATIVES = { ALTERNATIVE, ... }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        take_dict(take_alternative).map(|v| Self(v)).parse(input)
    }
}
vec_newtype!(QapiAlternatives, QapiAlternative<'i>);
