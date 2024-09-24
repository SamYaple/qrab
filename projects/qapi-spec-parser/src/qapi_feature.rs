use crate::helpers::{qstring, take_dict, take_kv};
use crate::take_cond;
use crate::QapiCond;
use nom::branch::alt;
use nom::combinator::map;
use nom::IResult;

pub fn take_feature(input: &str) -> IResult<&str, QapiFeature<'_>> {
    QapiFeature::parse(input)
}

#[derive(Debug, Clone, Default)]
pub struct QapiFeature<'i> {
    name: Option<&'i str>,
    r#if: Option<QapiCond<'i>>,
}

impl<'i> QapiFeature<'i> {
    /// FEATURE = STRING
    ///         | { 'name': STRING, '*if': COND }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        alt((Self::simple_parser, Self::complex_parser))(input)
    }

    /// STRING
    fn simple_parser(input: &'i str) -> IResult<&'i str, Self> {
        let s = Self::default();
        let (input, _) = map(qstring, |v| s.name)(input)?;
        Ok((input, s))
    }

    /// { 'name': STRING, '*if': COND }
    fn complex_parser(input: &'i str) -> IResult<&'i str, Self> {
        let mut s = Self::default();
        let name_parser = map(take_kv("name", qstring), |v| s.name = Some(v));
        let cond_parser = map(take_cond, |v| s.r#if = Some(v));
        let (input, _) = take_dict(alt((name_parser, cond_parser)))(input)?;
        Ok((input, s))
    }
}
