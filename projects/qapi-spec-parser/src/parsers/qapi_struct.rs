use super::{qstring, take_dict, take_kv};
use crate::{take_cond, take_features, take_members};
use crate::{QapiCond, QapiDocumentation, QapiFeatures, QapiMembers};
use nom::branch::alt;
use nom::combinator::{map, opt};
use nom::error::{Error, ErrorKind};
use nom::IResult;

pub fn take_struct(input: &str) -> IResult<&str, QapiStruct<'_>> {
    QapiStruct::parse(input)
}

#[derive(Debug, Clone, Default)]
pub struct QapiStruct<'i> {
    pub name: &'i str,
    pub data: QapiMembers<'i>,
    pub base: Option<&'i str>,
    pub r#if: Option<QapiCond<'i>>,
    pub features: Option<QapiFeatures<'i>>,
    pub doc: Option<QapiDocumentation<'i>>,
}

impl<'i> QapiStruct<'i> {
    /// STRUCT = { 'struct': STRING,
    ///            'data': MEMBERS,
    ///            '*base': STRING,
    ///            '*if': COND,
    ///            '*features': FEATURES }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        let (input, doc) = opt(QapiDocumentation::parse)(input)?;
        let mut s = Self {
            doc,
            ..Default::default()
        };
        let start = input;
        let (input, _) = take_dict(alt((
            map(take_kv("struct", qstring), |v| s.name = v),
            map(take_kv("base", qstring), |v| s.base = Some(v)),
            map(take_cond, |v| s.r#if = Some(v)),
            map(take_features, |v| s.features = Some(v)),
            map(take_kv("data", take_members), |v| s.data = v),
        )))(input)?;
        if s.name == "" {
            return Err(nom::Err::Error(Error::new(start, ErrorKind::Tag)));
        }
        Ok((input, s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_INPUTS: [&str; 6] = [
        "{'struct':'SOMENAME','data':{'membername':'membertype'}}",
        "{ 'struct': 'SOMENAME', 'data': {'membername':{'if':'CONFIG_OPTION', 'type': ['sometype'], 'features': ['yes']}}}",
        "{'struct': 'SOMENAME', 'data':{'membername':'membertype'}, 'if':{'not':'CONFIG_VALUE'}}",
        "{'struct': 'SOMENAME', 'data':{'membername':'membertype'}, 'base':'SOMETHING'}",
        "{'struct': 'SOMENAME', 'data':{'membername':{'type':'membertype'}}}",
        r#"##
# @RbdEncryptionOptionsLUKS:
#
# Since: 6.1
##
{ 'struct': 'RbdEncryptionOptionsLUKS',
  'base': 'RbdEncryptionOptionsLUKSBase',
  'data': { } }"#,
    ];

    #[test]
    fn test_valid() {
        for input in VALID_INPUTS {
            let result = QapiStruct::parse(input);
            dbg![&result];
            match result {
                Ok((remaining, _)) => {
                    assert_eq!(remaining, "");
                }
                _ => panic!("Failed to parse: ```\n{input}\n```"),
            }
        }
    }
}
