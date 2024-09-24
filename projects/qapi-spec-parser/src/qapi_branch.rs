use crate::helpers::{qstring, qtag, take_dict};
use crate::{take_cond, take_type_ref};
use crate::{QapiCond, QapiTypeRef};
use nom::branch::alt;
use nom::combinator::map;
use nom::sequence::{terminated, tuple};
use nom::IResult;

pub fn take_branch(input: &str) -> IResult<&str, QapiBranch<'_>> {
    QapiBranch::parse(input)
}

#[derive(Debug, Clone, Default)]
pub struct QapiBranch<'i> {
    name: Option<&'i str>,
    r#type: Option<QapiTypeRef<'i>>,
    r#if: Option<QapiCond<'i>>,
}

impl<'i> QapiBranch<'i> {
    /// BRANCH = STRING : TYPE-REF
    ///        | STRING : { 'type': TYPE-REF, '*if': COND }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        alt((Self::simple_parser, Self::complex_parser))(input)
    }

    /// STRING : TYPE-REF
    fn simple_parser(input: &'i str) -> IResult<&'i str, Self> {
        let mut s = Self::default();
        let name_parser = map(terminated(qstring, qtag(":")), |v| s.name = Some(v));
        let type_parser = map(QapiTypeRef::parse, |v| s.r#type = Some(v));
        let (input, _) = tuple((name_parser, type_parser))(input)?;
        Ok((input, s))
    }

    /// STRING : { 'type': TYPE-REF, '*if': COND }
    fn complex_parser(input: &'i str) -> IResult<&'i str, Self> {
        let mut s = Self::default();
        let name_parser = map(terminated(qstring, qtag(":")), |v| s.name = Some(v));
        let type_parser = map(take_type_ref, |v| s.r#type = Some(v));
        let cond_parser = map(take_cond, |v| s.r#if = Some(v));
        let (input, _) = tuple((name_parser, take_dict(alt((type_parser, cond_parser)))))(input)?;
        Ok((input, s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_INPUTS: [&str; 4] = [
        "'name':'type'",
        "'name':['type']",
        "'name':{'if':'CONFIG_OPTION', 'type': ['sometype']}",
        " 'name' : { 'if' : 'CONFIG_OPTION' , 'type' : 'sometype' }",
    ];

    #[test]
    fn test_valid() {
        for input in VALID_INPUTS {
            let result = QapiBranch::parse(input);
            match result {
                Ok((remaining, _)) => {
                    assert_eq!(remaining, "");
                }
                _ => panic!("Failed to parse: ```\n{input}\n```"),
            }
        }
    }
}
