use crate::helpers::{kv, qstring, qtag};
use nom::branch::alt;
use nom::combinator::map;
use nom::multi::separated_list1;
use nom::sequence::delimited;
use nom::IResult;

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
        let not_parser = kv(qtag("not"), Self::parse);
        let any_parser = kv(
            qtag("any"),
            delimited(
                qtag("["),
                separated_list1(qtag(","), Self::parse),
                qtag("]"),
            ),
        );
        let all_parser = kv(
            qtag("all"),
            delimited(
                qtag("["),
                separated_list1(qtag(","), Self::parse),
                qtag("]"),
            ),
        );
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
