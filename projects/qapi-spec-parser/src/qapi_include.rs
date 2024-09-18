use crate::helpers::{kv, qtag};
use crate::QapiString;
use nom::combinator::map;
use nom::sequence::delimited;
use nom::IResult;

#[derive(Debug, Clone)]
pub struct QapiInclude<'input>(pub QapiString<'input>);
impl<'input> QapiInclude<'input> {
    /// INCLUDE = { 'include': STRING }
    pub fn parse(input: &'input str) -> IResult<&'input str, Self> {
        delimited(
            qtag("{"),
            map(kv(qtag("include"), QapiString::parse), |v| Self(v)),
            qtag("}"),
        )(input)
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
