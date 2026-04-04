use super::{qstring, take_dict, take_kv, with_parser_config};
use crate::{take_cond, take_features, take_members};
use crate::{QapiCond, QapiDocumentation, QapiFeatures, QapiMembers};
use nom::branch::alt;
use nom::combinator::opt;
use nom::error::{Error, ErrorKind};
use nom::IResult;
use nom::Parser;

/// Parser that accepts `'struct': STRING` or `'type': STRING` (when enabled).
fn take_struct_name(input: &str) -> IResult<&str, &str> {
    if let Ok(result) = take_kv("struct", qstring).parse(input) {
        return Ok(result);
    }
    if with_parser_config(|c| c.type_as_struct) {
        return take_kv("type", qstring).parse(input);
    }
    Err(nom::Err::Error(Error::new(input, ErrorKind::Tag)))
}

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
        let (input, doc) = opt(QapiDocumentation::parse).parse(input)?;
        let mut s = Self {
            doc,
            ..Default::default()
        };
        let start = input;
        let (input, _) = take_dict(alt((
            take_struct_name.map(|v| s.name = v),
            take_kv("base", qstring).map(|v| s.base = Some(v)),
            take_cond.map(|v| s.r#if = Some(v)),
            take_features.map(|v| s.features = Some(v)),
            take_kv("data", take_members).map(|v| s.data = v),
        )))
        .parse(input)?;
        if s.name.is_empty() {
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

    #[test]
    fn test_type_as_struct_enabled() {
        use crate::parsers::{ParserConfig, set_parser_config};
        set_parser_config(ParserConfig::for_version((2, 0, 0)));
        let input = "{ 'type': 'NameInfo', 'data': {'*name': 'str'} }";
        let result = QapiStruct::parse(input);
        set_parser_config(ParserConfig::modern());
        match result {
            Ok((remaining, s)) => {
                assert_eq!(remaining, "");
                assert_eq!(s.name, "NameInfo");
            }
            Err(e) => panic!("Failed: {}", e),
        }
    }

    #[test]
    fn test_type_as_struct_rejected_in_modern() {
        use crate::parsers::{ParserConfig, set_parser_config};
        set_parser_config(ParserConfig::modern());
        let input = "{ 'type': 'NameInfo', 'data': {'*name': 'str'} }";
        let result = QapiStruct::parse(input);
        assert!(result.is_err(), "'type' should be rejected in modern mode");
    }
}
