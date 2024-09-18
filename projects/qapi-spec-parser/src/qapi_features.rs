use crate::helpers::{dict, kv, qtag};
use crate::{QapiCond, QapiString};
use nom::branch::alt;
use nom::combinator::map;
use nom::multi::separated_list1;
use nom::sequence::delimited;

use nom::IResult;

enum ParserKey<'input> {
    Name(QapiString<'input>),
    If(QapiCond<'input>),
}

#[derive(Debug, Clone)]
pub struct QapiFeature<'input> {
    name: QapiString<'input>,
    r#if: Option<QapiCond<'input>>,
}

impl<'input> QapiFeature<'input> {
    /// FEATURE = STRING
    ///         | { 'name': STRING, '*if': COND }
    pub fn parse(input: &'input str) -> IResult<&'input str, Self> {
        let simple_parser = QapiString::parse;

        let name_parser = map(kv(qtag("name"), QapiString::parse), |v| {
            ParserKey::Name(v.into())
        });
        let cond_parser = map(kv(qtag("if"), QapiCond::parse), |v| ParserKey::If(v));
        let complex_parser = dict(alt((name_parser, cond_parser)));
        alt((
            map(simple_parser, |v| Self {
                name: v.into(),
                r#if: None,
            }),
            map(complex_parser, |tokens| {
                let mut name = None;
                let mut r#if = None;
                for i in tokens {
                    match i {
                        ParserKey::Name(v) => name = Some(v),
                        ParserKey::If(v) => r#if = Some(v),
                    }
                }
                let name = name.expect("name is a required key");
                Self { name, r#if }
            }),
        ))(input)
    }
}

#[derive(Debug, Clone)]
pub struct QapiFeatures<'input>(Vec<QapiFeature<'input>>);
impl<'input> QapiFeatures<'input> {
    /// FEATURES = [ FEATURE, ... ]
    pub fn parse(input: &'input str) -> IResult<&'input str, Self> {
        map(
            delimited(
                qtag("["),
                separated_list1(qtag(","), QapiFeature::parse),
                qtag("]"),
            ),
            |v| Self(v),
        )(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_INPUTS_FEATURES: [&str; 4] = [
        "[{'name':'deprecated'}]",
        "['deprecated']",
        "[{'name':'deprecated','if':'CONFIG_OPTION'}]",
        "[{'name':'deprecated','if':'CONFIG_OPTION'},'testfeature']",
    ];

    #[test]
    fn test_features_valid() {
        for input in VALID_INPUTS_FEATURES {
            let result = QapiFeatures::parse(input);
            match result {
                Ok((remaining, _)) => {
                    assert_eq!(remaining, "");
                }
                _ => panic!("Failed to parse: ```\n{input}\n```"),
            }
        }
    }
    const VALID_INPUTS_FEATURE: [&str; 3] = [
        "{'name':'deprecated'}",
        "'deprecated'",
        "{'name':'deprecated','if':'CONFIG_OPTION'}",
    ];

    #[test]
    fn test_feature_valid() {
        for input in VALID_INPUTS_FEATURE {
            let result = QapiFeature::parse(input);
            match result {
                Ok((remaining, _)) => {
                    assert_eq!(remaining, "");
                }
                _ => panic!("Failed to parse: ```\n{input}\n```"),
            }
        }
    }
}
