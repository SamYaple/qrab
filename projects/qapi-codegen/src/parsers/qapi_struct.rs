use super::common::{qstring, take_dict, take_kv};
use super::parser_version;
use super::qapi_cond::{QapiCond, take_cond};
use super::qapi_documentation::QapiDocumentation;
use super::qapi_features::{QapiFeatures, take_features};
use super::qapi_members::{QapiMembers, take_members};
use nom::IResult;
use nom::Parser;
use nom::branch::alt;
use nom::combinator::opt;
use nom::error::{Error, ErrorKind};

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
    /// Dispatches to the correct versioned parser.
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        let v = parser_version();
        if v < (2, 4, 0) {
            Self::parse_v1(input)
        } else {
            Self::parse_v2(input)
        }
    }

    /// v1 (QEMU v1.0–v2.3.0): accepts `'type'` or `'struct'` keyword.
    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
        let (input, doc) = opt(QapiDocumentation::parse).parse(input)?;
        let mut s = Self {
            doc,
            ..Default::default()
        };
        let start = input;
        let take_name = alt((take_kv("struct", qstring), take_kv("type", qstring)));
        let (input, _) = take_dict(alt((
            take_name.map(|v| s.name = v),
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

    /// v2 (QEMU v2.4.0+): only `'struct'` keyword.
    pub fn parse_v2(input: &'i str) -> IResult<&'i str, Self> {
        let (input, doc) = opt(QapiDocumentation::parse).parse(input)?;
        let mut s = Self {
            doc,
            ..Default::default()
        };
        let start = input;
        let (input, _) = take_dict(alt((
            take_kv("struct", qstring).map(|v| s.name = v),
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
    use crate::parsers::set_parser_version;

    const VALID_INPUTS: [&str; 4] = [
        "{'struct':'SOMENAME','data':{'membername':'membertype'}}",
        "{ 'struct': 'SOMENAME', 'data': {'membername':{'if':'CONFIG_OPTION', 'type': ['sometype'], 'features': ['yes']}}}",
        "{'struct': 'SOMENAME', 'data':{'membername':'membertype'}, 'if':{'not':'CONFIG_VALUE'}}",
        "{'struct': 'SOMENAME', 'data':{'membername':'membertype'}, 'base':'SOMETHING'}",
    ];

    #[test]
    fn test_valid_v2() {
        set_parser_version((10, 2, 0));
        for input in VALID_INPUTS {
            let result = QapiStruct::parse(input);
            match result {
                Ok((remaining, _)) => assert_eq!(remaining, ""),
                _ => panic!("Failed to parse: ```\n{input}\n```"),
            }
        }
    }

    #[test]
    fn test_type_keyword_accepted_in_v1() {
        set_parser_version((2, 0, 0));
        let input = "{ 'type': 'NameInfo', 'data': {'*name': 'str'} }";
        let result = QapiStruct::parse(input);
        match result {
            Ok((remaining, s)) => {
                assert_eq!(remaining, "");
                assert_eq!(s.name, "NameInfo");
            }
            Err(e) => panic!("v1 should accept 'type': {}", e),
        }
    }

    #[test]
    fn test_struct_keyword_also_accepted_in_v1() {
        set_parser_version((2, 0, 0));
        let input = "{ 'struct': 'NameInfo', 'data': {'*name': 'str'} }";
        let result = QapiStruct::parse(input);
        match result {
            Ok((remaining, s)) => {
                assert_eq!(remaining, "");
                assert_eq!(s.name, "NameInfo");
            }
            Err(e) => panic!("v1 should also accept 'struct': {}", e),
        }
    }

    #[test]
    fn test_type_keyword_rejected_in_v2() {
        set_parser_version((10, 2, 0));
        let input = "{ 'type': 'NameInfo', 'data': {'*name': 'str'} }";
        let result = QapiStruct::parse(input);
        assert!(result.is_err(), "'type' should be rejected in v2 (modern)");
    }
}
