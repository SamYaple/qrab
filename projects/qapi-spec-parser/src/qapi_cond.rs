use crate::helpers::{qstring, qtag, take_kv, take_list};
use nom::branch::alt;
use nom::combinator::map;
use nom::sequence::delimited;
use nom::IResult;

pub fn take_cond(input: &str) -> IResult<&str, QapiCond<'_>> {
    take_kv("if", QapiCond::parse)(input)
}

#[derive(Debug, Clone)]
pub enum QapiCond<'i> {
    All(Vec<QapiCond<'i>>),
    Any(Vec<QapiCond<'i>>),
    Not(Box<QapiCond<'i>>),
    ConfigName(&'i str),
}

impl<'i> QapiCond<'i> {
    /// COND = STRING
    ///      | { 'all: [ COND, ... ] }
    ///      | { 'any: [ COND, ... ] }
    ///      | { 'not': COND }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        let not_parser = take_kv("not", Self::parse);
        let any_parser = take_kv("any", take_list(Self::parse));
        let all_parser = take_kv("all", take_list(Self::parse));
        alt((
            map(qstring, |v| Self::ConfigName(v)),
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
    fn test_valid() {
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
