use super::{qstring, qtag, take_dict};
use crate::{take_cond, take_features, take_type_ref};
use crate::{QapiCond, QapiFeatures, QapiTypeRef};
use nom::branch::alt;
use nom::combinator::map;
use nom::error::{Error, ErrorKind};
use nom::sequence::{terminated, tuple};
use nom::IResult;

pub fn take_member(input: &str) -> IResult<&str, QapiMember<'_>> {
    QapiMember::parse(input)
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct QapiMember<'i> {
    pub name: &'i str,
    pub optional: bool,
    pub r#type: QapiTypeRef<'i>,
    pub r#if: Option<QapiCond<'i>>,
    pub features: Option<QapiFeatures<'i>>,
}

impl<'i> QapiMember<'i> {
    /// MEMBER = STRING : TYPE-REF
    ///        | STRING : { 'type': TYPE-REF,
    ///                     '*if': COND,
    ///                     '*features': FEATURES }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        let start = input;
        let (input, mut member) = alt((Self::simple_parser, Self::complex_parser))(input)?;
        if member.name == "" || member.r#type == QapiTypeRef::Unset {
            return Err(nom::Err::Error(Error::new(start, ErrorKind::Tag)));
        }
        if member.name.starts_with('*') {
            member.optional = true;
            member.name = &member.name[1..];
        };
        Ok((input, member))
    }

    /// STRING : TYPE-REF
    fn simple_parser(input: &'i str) -> IResult<&'i str, Self> {
        let mut s = Self::default();
        let name_parser = map(terminated(qstring, qtag(":")), |v| s.name = v);
        let type_parser = map(QapiTypeRef::parse, |v| s.r#type = v);
        let (input, _) = tuple((name_parser, type_parser))(input)?;
        Ok((input, s))
    }

    /// STRING : { 'type': TYPE-REF,
    ///            '*if': COND,
    ///            '*features': FEATURES }
    fn complex_parser(input: &'i str) -> IResult<&'i str, Self> {
        let mut s = Self::default();
        let name_parser = map(terminated(qstring, qtag(":")), |v| s.name = v);
        let type_parser = map(take_type_ref, |v| s.r#type = v);
        let cond_parser = map(take_cond, |v| s.r#if = Some(v));
        let features_parser = map(take_features, |v| s.features = Some(v));
        let (input, _) = tuple((
            name_parser,
            take_dict(alt((features_parser, type_parser, cond_parser))),
        ))(input)?;
        Ok((input, s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_INPUTS: [&str; 4] = [
        //"{'name':'type', 'name':{'type':['sometype'], 'if':{'not':'CONFIG_VALUE'}}}",
        "'name':'type'",
        "'name':['type']",
        "'name':{'if':'CONFIG_OPTION', 'type': ['sometype'], 'features': ['yes']}",
        "'name':{'if':'CONFIG_OPTION', 'type': 'sometype', 'features': [{'name':'yes', 'if': 'CONFIG_MAP'}]}",
    ];

    #[test]
    fn test_valid() {
        for input in VALID_INPUTS {
            let result = QapiMember::parse(input);
            match result {
                Ok((remaining, _)) => {
                    assert_eq!(remaining, "");
                }
                _ => panic!("Failed to parse: ```\n{input}\n```"),
            }
        }
    }
}
