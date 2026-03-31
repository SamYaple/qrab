use super::{qstring, qtag, take_dict};
use crate::{take_cond, take_type_ref};
use crate::{QapiCond, QapiTypeRef};
use nom::branch::alt;
use nom::error::{Error, ErrorKind};
use nom::sequence::terminated;
use nom::IResult;
use nom::Parser;

pub fn take_alternative(input: &str) -> IResult<&str, QapiAlternative<'_>> {
    QapiAlternative::parse(input)
}

#[derive(Debug, Clone, Default)]
pub struct QapiAlternative<'i> {
    pub name: &'i str,
    pub r#if: Option<QapiCond<'i>>,
    pub r#type: QapiTypeRef<'i>,
}

impl<'i> QapiAlternative<'i> {
    /// ALTERNATIVE = STRING : STRING
    ///             | STRING : { 'type': STRING, '*if': COND }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        let start = input;
        let (input, alternative) = alt((Self::simple_parser, Self::dict_parser)).parse(input)?;
        if alternative.name.is_empty() || QapiTypeRef::Unset == alternative.r#type {
            return Err(nom::Err::Error(Error::new(start, ErrorKind::Tag)));
        }
        Ok((input, alternative))
    }

    /// STRING : STRING
    fn simple_parser(input: &'i str) -> IResult<&'i str, Self> {
        let mut s = Self::default();
        let name_parser = terminated(qstring, qtag(":")).map(|v| s.name = v);
        let type_parser = QapiTypeRef::parse.map(|v| s.r#type = v);
        let (input, _) = (name_parser, type_parser).parse(input)?;
        Ok((input, s))
    }

    /// STRING : { 'type': STRING, '*if': COND }
    fn dict_parser(input: &'i str) -> IResult<&'i str, Self> {
        let mut s = Self::default();
        let name_parser = terminated(qstring, qtag(":")).map(|v| s.name = v);
        let type_parser = take_type_ref.map(|v| s.r#type = v);
        let cond_parser = take_cond.map(|v| s.r#if = Some(v));
        let (input, _) = (name_parser, take_dict(alt((type_parser, cond_parser)))).parse(input)?;
        Ok((input, s))
    }
}
