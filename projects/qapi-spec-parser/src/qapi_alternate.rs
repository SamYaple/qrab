use crate::helpers::{dict, kv, qtag};
use crate::{QapiCond, QapiFeatures, QapiString, QapiTypeRef};
use nom::branch::alt;
use nom::combinator::map;
use nom::multi::separated_list1;
use nom::sequence::{delimited, terminated};
use nom::IResult;

enum AlternateParserKey<'input> {
    Name(QapiString<'input>),
    Data(QapiAlternatives<'input>),
    If(QapiCond<'input>),
    Features(QapiFeatures<'input>),
}

#[derive(Debug, Clone)]
pub struct QapiAlternate<'input> {
    name: QapiString<'input>,
    data: QapiAlternatives<'input>,
    r#if: Option<QapiCond<'input>>,
    features: Option<QapiFeatures<'input>>,
}

impl<'input> QapiAlternate<'input> {
    /// ALTERNATE = { 'alternate': STRING,
    ///               'data': ALTERNATIVES,
    ///               '*if': COND,
    ///               '*features': FEATURES }
    pub fn parse(input: &'input str) -> IResult<&'input str, Self> {
        let cond_parser = map(kv(qtag("if"), QapiCond::parse), |v| {
            AlternateParserKey::If(v)
        });
        let features_parser = map(kv(qtag("features"), QapiFeatures::parse), |v| {
            AlternateParserKey::Features(v)
        });
        let name_parser = map(kv(qtag("alternate"), QapiString::parse), |v| {
            AlternateParserKey::Name(v)
        });
        let data_parser = map(kv(qtag("data"), QapiAlternatives::parse), |v| {
            AlternateParserKey::Data(v)
        });

        let parsers = alt((data_parser, cond_parser, features_parser, name_parser));
        delimited(
            qtag("{"),
            map(separated_list1(qtag(","), parsers), |tokens| {
                let mut r#if = None;
                let mut data = None;
                let mut features = None;
                let mut name = None;
                for i in tokens {
                    match i {
                        AlternateParserKey::If(v) => r#if = Some(v),
                        AlternateParserKey::Data(v) => data = Some(v),
                        AlternateParserKey::Name(v) => name = Some(v),
                        AlternateParserKey::Features(v) => features = Some(v),
                    }
                }
                let name = name.expect("alternate is a required key");
                let data = data.expect("data is a required key");
                Self {
                    name,
                    r#if,
                    features,
                    data,
                }
            }),
            qtag("}"),
        )(input)
    }
}

enum AlternativeParserKey<'input> {
    Type(QapiTypeRef<'input>),
    If(QapiCond<'input>),
}

#[derive(Debug, Clone)]
pub struct QapiAlternative<'input> {
    pub name: QapiString<'input>,
    pub r#type: QapiTypeRef<'input>,
    pub r#if: Option<QapiCond<'input>>,
}

impl<'input> QapiAlternative<'input> {
    /// ALTERNATIVE = STRING : STRING
    ///             | STRING : { 'type': STRING, '*if': COND }
    pub fn parse(input: &'input str) -> IResult<&'input str, Self> {
        let (input, name) = terminated(QapiString::parse, qtag(":"))(input)?;

        let type_parser = map(kv(qtag("type"), QapiTypeRef::parse), |v| {
            AlternativeParserKey::Type(v)
        });
        let cond_parser = map(kv(qtag("if"), QapiCond::parse), |v| {
            AlternativeParserKey::If(v)
        });

        let simple_parser = QapiTypeRef::parse;
        let complex_parser = dict(alt((type_parser, cond_parser)));
        let (input, members) = alt((
            map(simple_parser, |r#type| Self {
                name: name.clone(),
                r#type,
                r#if: None,
            }),
            map(complex_parser, |tokens| {
                let mut r#if = None;
                let mut r#type = None;
                for i in tokens {
                    match i {
                        AlternativeParserKey::If(v) => r#if = Some(v),
                        AlternativeParserKey::Type(v) => r#type = Some(v),
                    }
                }
                let r#type = r#type.expect("type is a required key");
                Self {
                    name: name.clone(),
                    r#if,
                    r#type,
                }
            }),
        ))(input)?;
        Ok((input, members))
    }
}

#[derive(Debug, Clone)]
pub struct QapiAlternatives<'input>(pub Vec<QapiAlternative<'input>>);
impl<'input> QapiAlternatives<'input> {
    /// ALTERNATIVES = { ALTERNATIVE, ... }
    pub fn parse(input: &'input str) -> IResult<&'input str, Self> {
        map(
            delimited(
                qtag("{"),
                separated_list1(qtag(","), QapiAlternative::parse),
                qtag("}"),
            ),
            |v| Self(v),
        )(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_INPUTS: [&str; 5] = [
        "{'alternate':'SOMENAME','data':{'membername':'membertype'}}",
        "{ 'alternate': 'SOMENAME', 'data': {'membername':{'if':'CONFIG_OPTION', 'type': ['sometype']}}, 'features': ['yes']}",
        "{'alternate': 'SOMENAME', 'data':{'membername':'membertype'}, 'if':{'not':'CONFIG_VALUE'}}",
        "{'alternate': 'SOMENAME', 'data':{'membername':'membertype'}}",
        "{'alternate': 'SOMENAME', 'data':{'membername':{'type':'membertype'}}}",
    ];

    #[test]
    fn test_valid() {
        for input in VALID_INPUTS {
            let result = QapiAlternate::parse(input);
            match result {
                Ok((remaining, _)) => {
                    assert_eq!(remaining, "");
                }
                _ => panic!("Failed to parse: ```\n{input}\n```"),
            }
        }
    }
}
