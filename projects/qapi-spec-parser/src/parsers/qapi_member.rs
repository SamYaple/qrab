use crate::helpers::{qstring, qtag, take_dict};
use crate::{take_cond, take_features, take_type_ref};
use crate::{QapiCond, QapiFeatures, QapiTypeRef};
use nom::branch::alt;
use nom::combinator::map;
use nom::sequence::{terminated, tuple};
use nom::IResult;

pub fn take_member(input: &str) -> IResult<&str, QapiMember<'_>> {
    QapiMember::parse(input)
}

#[derive(Debug, Clone, Default)]
pub struct QapiMember<'i> {
    pub name: Option<&'i str>,
    pub optional: bool,
    pub r#type: Option<QapiTypeRef<'i>>,
    pub r#if: Option<QapiCond<'i>>,
    pub features: Option<QapiFeatures<'i>>,
}

impl<'i> QapiMember<'i> {
    /// MEMBER = STRING : TYPE-REF
    ///        | STRING : { 'type': TYPE-REF,
    ///                     '*if': COND,
    ///                     '*features': FEATURES }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        let (input, mut s) = alt((Self::simple_parser, Self::complex_parser))(input)?;
        if let Some(ref mut name) = s.name {
            if name.starts_with('*') {
                s.optional = true;
                s.name = Some(&name[1..])
            };
        }
        Ok((input, s))
    }

    /// STRING : TYPE-REF
    fn simple_parser(input: &'i str) -> IResult<&'i str, Self> {
        let mut s = Self::default();
        let name_parser = map(terminated(qstring, qtag(":")), |v| s.name = Some(v));
        let type_parser = map(QapiTypeRef::parse, |v| s.r#type = Some(v));
        let (input, _) = tuple((name_parser, type_parser))(input)?;
        Ok((input, s))
    }

    /// STRING : { 'type': TYPE-REF,
    ///            '*if': COND,
    ///            '*features': FEATURES }
    fn complex_parser(input: &'i str) -> IResult<&'i str, Self> {
        let mut s = Self::default();
        let name_parser = map(terminated(qstring, qtag(":")), |v| s.name = Some(v));
        let type_parser = map(take_type_ref, |v| s.r#type = Some(v));
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
