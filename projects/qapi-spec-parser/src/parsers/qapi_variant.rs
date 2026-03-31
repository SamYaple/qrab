use super::{qstring, qtag, take_dict};
use crate::{take_cond, take_type_ref};
use crate::{QapiCond, QapiTypeRef};
use nom::branch::alt;
use nom::error::{Error, ErrorKind};
use nom::sequence::terminated;
use nom::IResult;
use nom::Parser;

pub fn take_variant(input: &str) -> IResult<&str, QapiVariant<'_>> {
    QapiVariant::parse(input)
}

/// A named type reference with an optional condition.
/// Used for both union branches and alternate alternatives in the QAPI schema.
#[derive(Debug, Clone, Default)]
pub struct QapiVariant<'i> {
    pub name: &'i str,
    pub r#type: QapiTypeRef<'i>,
    pub r#if: Option<QapiCond<'i>>,
}

impl<'i> QapiVariant<'i> {
    /// STRING : TYPE-REF
    /// | STRING : { 'type': TYPE-REF, '*if': COND }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        let start = input;
        let (input, variant) = alt((Self::simple_parser, Self::complex_parser)).parse(input)?;
        if variant.name.is_empty() || variant.r#type == QapiTypeRef::Unset {
            return Err(nom::Err::Error(Error::new(start, ErrorKind::Tag)));
        }
        Ok((input, variant))
    }

    /// STRING : TYPE-REF
    fn simple_parser(input: &'i str) -> IResult<&'i str, Self> {
        let mut s = Self::default();
        let name_parser = terminated(qstring, qtag(":")).map(|v| s.name = v);
        let type_parser = QapiTypeRef::parse.map(|v| s.r#type = v);
        let (input, _) = (name_parser, type_parser).parse(input)?;
        Ok((input, s))
    }

    /// STRING : { 'type': TYPE-REF, '*if': COND }
    fn complex_parser(input: &'i str) -> IResult<&'i str, Self> {
        let mut s = Self::default();
        let name_parser = terminated(qstring, qtag(":")).map(|v| s.name = v);
        let type_parser = take_type_ref.map(|v| s.r#type = v);
        let cond_parser = take_cond.map(|v| s.r#if = Some(v));
        let (input, _) = (name_parser, take_dict(alt((type_parser, cond_parser)))).parse(input)?;
        Ok((input, s))
    }
}
