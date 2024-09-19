use crate::helpers::{dict, kv, qstring, qtag};
use crate::{QapiCond, QapiFeatures};
use nom::branch::alt;
use nom::combinator::map;
use nom::multi::separated_list1;
use nom::sequence::delimited;
use nom::IResult;

enum EnumParserToken<'i> {
    Name(&'i str),
    Prefix(&'i str),
    Data(Vec<QapiEnumValue<'i>>),
    If(QapiCond<'i>),
    Features(QapiFeatures<'i>),
}

#[derive(Debug, Clone)]
pub struct QapiEnum<'i> {
    name: &'i str,
    data: Vec<QapiEnumValue<'i>>,
    r#if: Option<QapiCond<'i>>,
    prefix: Option<&'i str>,
    features: Option<QapiFeatures<'i>>,
}

impl<'i> QapiEnum<'i> {
    /// ENUM = { 'enum': STRING,
    ///          'data': [ ENUM-VALUE, ... ],
    ///          '*prefix': STRING,
    ///          '*if': COND,
    ///          '*features': FEATURES }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        let name_parser = map(kv(qtag("enum"), qstring), |v| EnumParserToken::Name(v));
        let prefix_parser = map(kv(qtag("prefix"), qstring), |v| EnumParserToken::Prefix(v));
        let type_parser = map(
            kv(
                qtag("data"),
                delimited(
                    qtag("["),
                    separated_list1(qtag(","), QapiEnumValue::parse),
                    qtag("]"),
                ),
            ),
            |v| EnumParserToken::Data(v),
        );
        let cond_parser = map(kv(qtag("if"), QapiCond::parse), |v| EnumParserToken::If(v));
        let features_parser = map(kv(qtag("features"), QapiFeatures::parse), |v| {
            EnumParserToken::Features(v)
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
                    EnumParserToken::If(v) => r#if = Some(v),
                    EnumParserToken::Data(v) => data = Some(v),
                    EnumParserToken::Name(v) => name = Some(v),
                    EnumParserToken::Prefix(v) => prefix = Some(v),
                    EnumParserToken::Features(v) => features = Some(v),
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

enum EnumValueParserToken<'i> {
    Name(&'i str),
    If(QapiCond<'i>),
    Features(QapiFeatures<'i>),
}

#[derive(Debug, Clone)]
pub struct QapiEnumValue<'i> {
    name: &'i str,
    r#if: Option<QapiCond<'i>>,
    features: Option<QapiFeatures<'i>>,
}

impl<'i> QapiEnumValue<'i> {
    /// ENUM-VALUE = STRING
    ///            | { 'name': STRING,
    ///                '*if': COND,
    ///                '*features': FEATURES }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        let name_parser = map(kv(qtag("name"), qstring), |v| EnumValueParserToken::Name(v));
        let cond_parser = map(kv(qtag("if"), QapiCond::parse), |v| {
            EnumValueParserToken::If(v)
        });
        let features_parser = map(kv(qtag("features"), QapiFeatures::parse), |v| {
            EnumValueParserToken::Features(v)
        });

        let simple_parser = qstring;
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
                        EnumValueParserToken::If(v) => r#if = Some(v),
                        EnumValueParserToken::Name(v) => name = Some(v),
                        EnumValueParserToken::Features(v) => features = Some(v),
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
