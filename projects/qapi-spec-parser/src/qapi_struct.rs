use crate::helpers::{qstring, take_dict, take_kv};
use crate::{take_cond, take_features, take_members};
use crate::{QapiCond, QapiFeatures, QapiMembers};
use nom::branch::alt;
use nom::combinator::map;
use nom::IResult;

pub fn take_struct(input: &str) -> IResult<&str, QapiStruct<'_>> {
    QapiStruct::parse(input)
}

#[derive(Debug, Clone, Default)]
pub struct QapiStruct<'i> {
    name: Option<&'i str>,
    data: Option<QapiMembers<'i>>,
    base: Option<&'i str>,
    r#if: Option<QapiCond<'i>>,
    features: Option<QapiFeatures<'i>>,
}

impl<'i> QapiStruct<'i> {
    /// STRUCT = { 'struct': STRING,
    ///            'data': MEMBERS,
    ///            '*base': STRING,
    ///            '*if': COND,
    ///            '*features': FEATURES }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        let mut s = Self::default();
        let (input, _) = take_dict(alt((
            map(take_kv("struct", qstring), |v| s.name = Some(v)),
            map(take_kv("base", qstring), |v| s.base = Some(v)),
            map(take_cond, |v| s.r#if = Some(v)),
            map(take_features, |v| s.features = Some(v)),
            map(take_kv("data", take_members), |v| s.data = Some(v)),
        )))(input)?;
        Ok((input, s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_INPUTS: [&str; 5] = [
        "{'struct':'SOMENAME','data':{'membername':'membertype'}}",
        "{ 'struct': 'SOMENAME', 'data': {'membername':{'if':'CONFIG_OPTION', 'type': ['sometype'], 'features': ['yes']}}}",
        "{'struct': 'SOMENAME', 'data':{'membername':'membertype'}, 'if':{'not':'CONFIG_VALUE'}}",
        "{'struct': 'SOMENAME', 'data':{'membername':'membertype'}, 'base':'SOMETHING'}",
        "{'struct': 'SOMENAME', 'data':{'membername':{'type':'membertype'}}}",
    ];

    #[test]
    fn test_valid() {
        for input in VALID_INPUTS {
            let result = QapiStruct::parse(input);
            match result {
                Ok((remaining, _)) => {
                    assert_eq!(remaining, "");
                }
                _ => panic!("Failed to parse: ```\n{input}\n```"),
            }
        }
    }
}
