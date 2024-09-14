use crate::helpers::{dict, kv, qtag};
use crate::{QapiCond, QapiFeatures, QapiString};
use nom::branch::alt;
use nom::combinator::map;
use nom::multi::separated_list1;
use nom::sequence::delimited;
use nom::IResult;

enum EnumParserKey {
    Name(QapiString),
    Prefix(QapiString),
    Data(Vec<QapiEnumValue>),
    If(QapiCond),
    Features(QapiFeatures),
}

#[derive(Debug)]
pub struct QapiEnum {
    name: QapiString,
    data: Vec<QapiEnumValue>,
    r#if: Option<QapiCond>,
    prefix: Option<QapiString>,
    features: Option<QapiFeatures>,
}

impl QapiEnum {
    /// ENUM = { 'enum': STRING,
    ///          'data': [ ENUM-VALUE, ... ],
    ///          '*prefix': STRING,
    ///          '*if': COND,
    ///          '*features': FEATURES }
    pub fn parse(input: &str) -> IResult<&str, Self> {
        let name_parser = map(kv(qtag("enum"), QapiString::parse), |v| {
            EnumParserKey::Name(v)
        });
        let prefix_parser = map(kv(qtag("prefix"), QapiString::parse), |v| {
            EnumParserKey::Prefix(v)
        });
        let type_parser = map(
            kv(
                qtag("data"),
                delimited(
                    qtag("["),
                    separated_list1(qtag(","), QapiEnumValue::parse),
                    qtag("]"),
                ),
            ),
            |v| EnumParserKey::Data(v),
        );
        let cond_parser = map(kv(qtag("if"), QapiCond::parse), |v| EnumParserKey::If(v));
        let features_parser = map(kv(qtag("features"), QapiFeatures::parse), |v| {
            EnumParserKey::Features(v)
        });

        let token_parser = dict(alt((
            type_parser,
            cond_parser,
            features_parser,
            name_parser,
            prefix_parser,
        )));
        let (input, members) = map(token_parser, |tokens| {
            let mut r#if = None;
            let mut prefix = None;
            let mut name = None;
            let mut data = None;
            let mut features = None;
            for i in tokens {
                match i {
                    EnumParserKey::If(v) => r#if = Some(v),
                    EnumParserKey::Data(v) => data = Some(v),
                    EnumParserKey::Name(v) => name = Some(v),
                    EnumParserKey::Prefix(v) => prefix = Some(v),
                    EnumParserKey::Features(v) => features = Some(v),
                }
            }
            let name = name.expect("enum is a required key");
            let data = data.expect("data is a required key");
            Self {
                r#if,
                features,
                prefix,
                name,
                data,
            }
        })(input)?;
        Ok((input, members))
    }
}

enum EnumValueParserKey {
    Name(QapiString),
    If(QapiCond),
    Features(QapiFeatures),
}

#[derive(Debug)]
pub struct QapiEnumValue {
    name: QapiString,
    r#if: Option<QapiCond>,
    features: Option<QapiFeatures>,
}

impl QapiEnumValue {
    /// ENUM-VALUE = STRING
    ///            | { 'name': STRING,
    ///                '*if': COND,
    ///                '*features': FEATURES }
    pub fn parse(input: &str) -> IResult<&str, Self> {
        let name_parser = map(kv(qtag("name"), QapiString::parse), |v| {
            EnumValueParserKey::Name(v)
        });
        let cond_parser = map(kv(qtag("if"), QapiCond::parse), |v| {
            EnumValueParserKey::If(v)
        });
        let features_parser = map(kv(qtag("features"), QapiFeatures::parse), |v| {
            EnumValueParserKey::Features(v)
        });

        let simple_parser = QapiString::parse;
        let complex_parser = dict(alt((name_parser, cond_parser, features_parser)));
        let (input, members) = alt((
            map(simple_parser, |name| Self {
                name,
                r#if: None,
                features: None,
            }),
            map(complex_parser, |tokens| {
                let mut name = None;
                let mut r#if = None;
                let mut features = None;
                for i in tokens {
                    match i {
                        EnumValueParserKey::If(v) => r#if = Some(v),
                        EnumValueParserKey::Name(v) => name = Some(v),
                        EnumValueParserKey::Features(v) => features = Some(v),
                    }
                }
                let name = name.expect("name is a required key");
                Self {
                    name,
                    r#if,
                    features,
                }
            }),
        ))(input)?;
        Ok((input, members))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_INPUTS: [&str; 1] = [
        "{ 'enum': 'Qcow2CompressionType', 'data': [ 'zlib', { 'name': 'zstd', 'if': 'CONFIG_ZSTD' } ] }",
    ];

    #[test]
    fn test_valid() {
        for input in VALID_INPUTS {
            let result = QapiEnum::parse(input);
            match result {
                Ok((remaining, _)) => {
                    assert_eq!(remaining, "");
                }
                _ => panic!("Failed to parse: ```\n{input}\n```"),
            }
        }
    }
}
