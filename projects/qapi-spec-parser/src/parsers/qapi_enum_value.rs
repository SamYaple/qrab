use crate::helpers::{qstring, take_dict, take_kv};
use crate::{take_cond, take_features};
use crate::{QapiCond, QapiFeatures};
use nom::branch::alt;
use nom::combinator::map;
use nom::error::{Error, ErrorKind};
use nom::IResult;

pub fn take_enum_value(input: &str) -> IResult<&str, QapiEnumValue<'_>> {
    QapiEnumValue::parse(input)
}

#[derive(Debug, Clone, Default)]
pub struct QapiEnumValue<'i> {
    pub name: &'i str,
    pub r#if: Option<QapiCond<'i>>,
    pub features: Option<QapiFeatures<'i>>,
}

impl<'i> QapiEnumValue<'i> {
    /// ENUM-VALUE = STRING
    ///            | { 'name': STRING,
    ///                '*if': COND,
    ///                '*features': FEATURES }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        let start = input;
        let (input, enumvalue) = alt((Self::simple_parser, Self::complex_parser))(input)?;
        if enumvalue.name == "" {
            return Err(nom::Err::Error(Error::new(start, ErrorKind::Tag)));
        }
        Ok((input, enumvalue))
    }

    /// STRING
    fn simple_parser(input: &'i str) -> IResult<&'i str, Self> {
        let mut s = Self::default();
        let (input, _) = map(qstring, |v| s.name = v)(input)?;
        Ok((input, s))
    }

    /// { 'name': STRING,
    ///   '*if': COND,
    ///   '*features': FEATURES }
    fn complex_parser(input: &'i str) -> IResult<&'i str, Self> {
        let mut s = Self::default();
        let (input, _) = take_dict(alt((
            map(take_kv("name", qstring), |v| s.name = v),
            map(take_features, |v| s.features = Some(v)),
            map(take_cond, |v| s.r#if = Some(v)),
        )))(input)?;
        Ok((input, s))
    }
}
