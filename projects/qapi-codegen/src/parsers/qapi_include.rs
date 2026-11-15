use super::common::QapiString;
use super::common::{qstring, qtag, take_kv};
use nom::IResult;
use nom::Parser;
use nom::sequence::delimited;

pub fn take_include(input: &str) -> IResult<&str, QapiInclude<'_>> {
    QapiInclude::parse(input)
}

#[derive(Debug, Clone)]
pub struct QapiInclude<'i>(pub QapiString<'i>);
impl<'i> QapiInclude<'i> {
    /// INCLUDE = { 'include': STRING }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        Self::parse_v1(input)
    }

    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
        delimited(qtag("{"), take_kv("include", qstring).map(Self), qtag("}")).parse(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_INPUTS: [&str; 3] = [
        "{'include':'path/to/file.json'}",
        "{ 'include' : '../file.json' }",
        r#"{ # comment
            'include' : # comment
            '../file.json' # comemnt
        }"#,
    ];

    #[test]
    fn test_valid() {
        for input in VALID_INPUTS {
            let result = QapiInclude::parse(input);
            match result {
                Ok((remaining, _)) => {
                    assert_eq!(remaining, "");
                }
                _ => panic!("Failed to parse: ```\n{input}\n```"),
            }
        }
    }
}
