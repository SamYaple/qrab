use crate::helpers::{kv, list, qtag};
use crate::QapiString;
use nom::branch::alt;
use nom::combinator::map;
use nom::sequence::delimited;
use nom::IResult;

#[derive(Debug)]
pub enum QapiCond {
    All(Vec<QapiCond>),
    Any(Vec<QapiCond>),
    Not(Box<QapiCond>),
    Value(QapiString),
}

impl QapiCond {
    /// COND = STRING
    ///      | { 'all: [ COND, ... ] }
    ///      | { 'any: [ COND, ... ] }
    ///      | { 'not': COND }
    pub fn parse(input: &str) -> IResult<&str, Self> {
        let value_parser = QapiString::parse;
        let not_parser = kv(qtag("not"), Self::parse);
        let any_parser = kv(qtag("any"), list(Self::parse));
        let all_parser = kv(qtag("all"), list(Self::parse));
        alt((
            map(value_parser, |v| Self::Value(v)),
            delimited(
                qtag("{"),
                alt((
                    map(all_parser, |v| Self::All(v)),
                    map(any_parser, |v| Self::Any(v)),
                    map(not_parser, |v| Self::Not(Box::new(v))),
                )),
                qtag("}"),
            ),
        ))(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_INPUTS: [&str; 9] = [
        "'CONFIG_OPTION'",
        "{'not':'CONFIG_OPTION'}",
        "{'any':['CONFIG_OPTION']}",
        "{'any':['CONFIG_OPTION','CONFIG_OPTION2']}",
        "{'all':['CONFIG_OPTION','CONFIG_OPTION2']}",
        " 'CONFIG_OPTION'",
        "{ 'not' : 'CONFIG_OPTION' }",
        "{ 'any' : [ 'CONFIG_OPTION' , 'CONFIG_OPTION2' ] }",
        "{ 'all' : [ 'CONFIG_OPTION' , 'CONFIG_OPTION2' ] }",
    ];

    #[test]
    fn test_qapi_cond_valid() {
        for input in VALID_INPUTS {
            let result = QapiCond::parse(input);
            match result {
                Ok((remaining, _)) => {
                    assert_eq!(remaining, "");
                }
                _ => panic!("Failed to parse: ```\n{input}\n```"),
            }
        }
    }
}
