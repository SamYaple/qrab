use crate::helpers::qtag;
use nom::bytes::complete::{tag, take_until};
use nom::combinator::map;
use nom::sequence::delimited;
use nom::IResult;

#[derive(Debug, PartialEq, Clone)]
pub struct QapiString<'i>(pub &'i str);
impl<'i> QapiString<'i> {
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        map(delimited(qtag("'"), take_until("'"), tag("'")), |v| Self(v))(input)
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
