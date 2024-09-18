use crate::helpers::{dict, kv, qtag};
use crate::{QapiCond, QapiFeatures, QapiString, QapiTypeRef};
use nom::branch::alt;
use nom::combinator::map;
use nom::multi::separated_list0;
use nom::sequence::{delimited, terminated};
use nom::IResult;

enum ParserToken<'input> {
    Type(QapiTypeRef<'input>),
    If(QapiCond<'input>),
    Features(QapiFeatures<'input>),
}

#[derive(Debug, Clone)]
pub struct QapiMember<'input> {
    name: QapiString<'input>,
    r#type: QapiTypeRef<'input>,
    r#if: Option<QapiCond<'input>>,
    features: Option<QapiFeatures<'input>>,
}

impl<'input> QapiMember<'input> {
    /// MEMBER = STRING : TYPE-REF
    ///        | STRING : { 'type': TYPE-REF,
    ///                     '*if': COND,
    ///                     '*features': FEATURES }
    pub fn parse(input: &'input str) -> IResult<&'input str, Self> {
        let (input, name) = terminated(QapiString::parse, qtag(":"))(input)?;

        let type_parser = map(kv(qtag("type"), QapiTypeRef::parse), |v| {
            ParserToken::Type(v)
        });
        let cond_parser = map(kv(qtag("if"), QapiCond::parse), |v| ParserToken::If(v));
        let features_parser = map(kv(qtag("features"), QapiFeatures::parse), |v| {
            ParserToken::Features(v)
        });

        let simple_parser = QapiTypeRef::parse;
        let complex_parser = dict(alt((type_parser, cond_parser, features_parser)));
        let (input, members) = alt((
            map(simple_parser, |r#type| Self {
                name: name.clone(),
                r#type,
                r#if: None,
                features: None,
            }),
            map(complex_parser, |tokens| {
                let mut r#if = None;
                let mut r#type = None;
                let mut features = None;
                for i in tokens {
                    match i {
                        ParserToken::If(v) => r#if = Some(v),
                        ParserToken::Type(v) => r#type = Some(v),
                        ParserToken::Features(v) => features = Some(v),
                    }
                }
                let r#type = r#type.expect("type is a required key");
                Self {
                    name: name.clone(),
                    r#if,
                    features,
                    r#type,
                }
            }),
        ))(input)?;
        Ok((input, members))
    }
}

#[derive(Debug, Clone)]
pub struct QapiMembers<'input>(Vec<QapiMember<'input>>);
impl<'input> QapiMembers<'input> {
    /// MEMBERS = { MEMBER, ... }
    pub fn parse(input: &'input str) -> IResult<&'input str, Self> {
        map(
            delimited(
                qtag("{"),
                separated_list0(qtag(","), QapiMember::parse),
                qtag("}"),
            ),
            |v| Self(v),
        )(input)
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
