use crate::helpers::{qstring, qtag, take_dict};
use crate::{take_cond, take_type_ref};
use crate::{QapiCond, QapiTypeRef};
use nom::branch::alt;
use nom::combinator::map;
use nom::sequence::{terminated, tuple};
use nom::IResult;

pub fn take_alternative(input: &str) -> IResult<&str, QapiAlternative<'_>> {
    QapiAlternative::parse(input)
}

#[derive(Debug, Clone, Default)]
pub struct QapiAlternative<'i> {
    pub name: Option<&'i str>,
    pub r#if: Option<QapiCond<'i>>,
    pub r#type: Option<QapiTypeRef<'i>>,
}

impl<'i> QapiAlternative<'i> {
    /// ALTERNATIVE = STRING : STRING
    ///             | STRING : { 'type': STRING, '*if': COND }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        alt((Self::simple_parser, Self::complex_parser))(input)
    }

    /// STRING : STRING
    fn simple_parser(input: &'i str) -> IResult<&'i str, Self> {
        let mut s = Self::default();
        let name_parser = map(terminated(qstring, qtag(":")), |v| s.name = Some(v));
        let type_parser = map(QapiTypeRef::parse, |v| s.r#type = Some(v));
        let (input, _) = tuple((name_parser, type_parser))(input)?;
        Ok((input, s))
    }

    /// STRING : { 'type': STRING, '*if': COND }
    fn complex_parser(input: &'i str) -> IResult<&'i str, Self> {
        let mut s = Self::default();
        let name_parser = map(terminated(qstring, qtag(":")), |v| s.name = Some(v));
        let type_parser = map(take_type_ref, |v| s.r#type = Some(v));
        let cond_parser = map(take_cond, |v| s.r#if = Some(v));
        let (input, _) = tuple((name_parser, take_dict(alt((type_parser, cond_parser)))))(input)?;
        Ok((input, s))
    }
}
