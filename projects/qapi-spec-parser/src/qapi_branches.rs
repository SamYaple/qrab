use crate::helpers::{dict, kv, qtag};
use crate::{QapiCond, QapiString, QapiTypeRef};
use nom::branch::alt;
use nom::combinator::map;
use nom::multi::separated_list1;
use nom::sequence::{delimited, terminated};
use nom::IResult;

enum ParserToken<'i> {
    Type(QapiTypeRef<'i>),
    If(QapiCond<'i>),
}

#[derive(Debug, Clone)]
pub struct QapiBranch<'i> {
    name: QapiString<'i>,
    r#type: QapiTypeRef<'i>,
    r#if: Option<QapiCond<'i>>,
}

impl<'i> QapiBranch<'i> {
    /// BRANCH = STRING : TYPE-REF
    ///        | STRING : { 'type': TYPE-REF, '*if': COND }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        let (input, name) = terminated(QapiString::parse, qtag(":"))(input)?;

        let type_parser = map(kv(qtag("type"), QapiTypeRef::parse), |v| {
            ParserToken::Type(v)
        });
        let cond_parser = map(kv(qtag("if"), QapiCond::parse), |v| ParserToken::If(v));

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
                        ParserToken::If(v) => r#if = Some(v),
                        ParserToken::Type(v) => r#type = Some(v),
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
pub struct QapiBranches<'i>(Vec<QapiBranch<'i>>);
impl<'i> QapiBranches<'i> {
    /// BRANCHES = { BRANCH, ... }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        map(
            delimited(
                qtag("{"),
                separated_list1(qtag(","), QapiBranch::parse),
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
