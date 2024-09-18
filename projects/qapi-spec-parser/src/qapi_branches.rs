use crate::helpers::{dict, kv, qtag};
use crate::{QapiCond, QapiString, QapiTypeRef};
use nom::branch::alt;
use nom::combinator::map;
use nom::multi::separated_list1;
use nom::sequence::{delimited, terminated};
use nom::IResult;

enum ParserKey<'input> {
    Type(QapiTypeRef<'input>),
    If(QapiCond<'input>),
}

#[derive(Debug, Clone)]
pub struct QapiBranch<'input> {
    name: QapiString<'input>,
    r#type: QapiTypeRef<'input>,
    r#if: Option<QapiCond<'input>>,
}

impl<'input> QapiBranch<'input> {
    /// BRANCH = STRING : TYPE-REF
    ///        | STRING : { 'type': TYPE-REF, '*if': COND }
    pub fn parse(input: &'input str) -> IResult<&'input str, Self> {
        let (input, name) = terminated(QapiString::parse, qtag(":"))(input)?;

        let type_parser = map(kv(qtag("type"), QapiTypeRef::parse), |v| ParserKey::Type(v));
        let cond_parser = map(kv(qtag("if"), QapiCond::parse), |v| ParserKey::If(v));

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
                        ParserKey::If(v) => r#if = Some(v),
                        ParserKey::Type(v) => r#type = Some(v),
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
pub struct QapiBranches<'input>(Vec<QapiBranch<'input>>);
impl<'input> QapiBranches<'input> {
    /// BRANCHES = { BRANCH, ... }
    pub fn parse(input: &'input str) -> IResult<&'input str, Self> {
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
