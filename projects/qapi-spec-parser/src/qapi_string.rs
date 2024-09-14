use crate::helpers::qtag;
use nom::bytes::complete::{tag, take_until};
use nom::combinator::map;
use nom::sequence::delimited;
use nom::IResult;

#[derive(Debug, PartialEq, Clone)]
pub struct QapiString(pub String);
impl QapiString {
    pub fn parse(input: &str) -> IResult<&str, Self> {
        map(
            delimited(
                qtag("'"),
                take_until("'"), // TODO handle escaped '\''?
                tag("'"),
            ),
            |v| Self(v.into()),
        )(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_INPUTS: [&str; 3] = [
        "'name'",
        "  'name'",
        r#" # some comment
        'name'"#,
    ];

    #[test]
    fn test_valid() {
        for input in VALID_INPUTS {
            let result = QapiString::parse(input);
            match result {
                Ok((remaining, _)) => {
                    assert_eq!(remaining, "");
                }
                _ => panic!("Failed to parse: ```\n{input}\n```"),
            }
        }
    }
}
