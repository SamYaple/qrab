use crate::helpers::{kv, qtag};
use crate::{QapiCond, QapiFeatures, QapiMembers, QapiString};
use nom::branch::alt;
use nom::combinator::map;
use nom::multi::separated_list1;
use nom::sequence::delimited;
use nom::IResult;

enum ParserToken<'input> {
    Name(QapiString<'input>),
    Data(QapiMembers<'input>),
    Base(QapiString<'input>),
    If(QapiCond<'input>),
    Features(QapiFeatures<'input>),
}

#[derive(Debug, Clone)]
pub struct QapiStruct<'input> {
    name: QapiString<'input>,
    data: QapiMembers<'input>,
    base: Option<QapiString<'input>>,
    r#if: Option<QapiCond<'input>>,
    features: Option<QapiFeatures<'input>>,
}

impl<'input> QapiStruct<'input> {
    /// STRUCT = { 'struct': STRING,
    ///            'data': MEMBERS,
    ///            '*base': STRING,
    ///            '*if': COND,
    ///            '*features': FEATURES }
    pub fn parse(input: &'input str) -> IResult<&'input str, Self> {
        let cond_parser = map(kv(qtag("if"), QapiCond::parse), |v| ParserToken::If(v));
        let features_parser = map(kv(qtag("features"), QapiFeatures::parse), |v| {
            ParserToken::Features(v)
        });
        let name_parser = map(kv(qtag("struct"), QapiString::parse), |v| {
            ParserToken::Name(v)
        });
        let data_parser = map(kv(qtag("data"), QapiMembers::parse), |v| {
            ParserToken::Data(v)
        });
        let base_parser = map(kv(qtag("base"), QapiString::parse), |v| {
            ParserToken::Base(v)
        });

        let parsers = alt((
            data_parser,
            cond_parser,
            features_parser,
            name_parser,
            base_parser,
        ));
        delimited(
            qtag("{"),
            map(separated_list1(qtag(","), parsers), |tokens| {
                let mut r#if = None;
                let mut data = None;
                let mut features = None;
                let mut base = None;
                let mut name = None;
                for i in tokens {
                    match i {
                        ParserToken::If(v) => r#if = Some(v),
                        ParserToken::Base(v) => base = Some(v),
                        ParserToken::Data(v) => data = Some(v),
                        ParserToken::Name(v) => name = Some(v),
                        ParserToken::Features(v) => features = Some(v),
                    }
                }
                let name = name.expect("struct is a required key");
                let data = data.expect("data is a required key");
                Self {
                    name,
                    r#if,
                    features,
                    data,
                    base,
                }
            }),
            qtag("}"),
        )(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_INPUTS: [&str; 5] = [
        "{'struct':'SOMENAME','data':{'membername':'membertype'}}",
        "{ 'struct': 'SOMENAME', 'data': {'membername':{'if':'CONFIG_OPTION', 'type': ['sometype'], 'features': ['yes']}}}",
        "{'struct': 'SOMENAME', 'data':{'membername':'membertype'}, 'if':{'not':'CONFIG_VALUE'}}",
        "{'struct': 'SOMENAME', 'data':{'membername':'membertype'}, 'base':'SOMETHING'}",
        "{'struct': 'SOMENAME', 'data':{'membername':{'type':'membertype'}}}",
    ];

    #[test]
    fn test_valid() {
        for input in VALID_INPUTS {
            let result = QapiStruct::parse(input);
            match result {
                Ok((remaining, _)) => {
                    assert_eq!(remaining, "");
                }
                _ => panic!("Failed to parse: ```\n{input}\n```"),
            }
        }
    }
}
