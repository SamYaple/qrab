use crate::helpers::{clean_line, take_qapi_string};
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::combinator::map;
use nom::sequence::{delimited, tuple};
use nom::IResult;

#[derive(Debug)]
pub enum QapiTypeRef {
    Weak(String),
    WeakArray(String),
}

impl QapiTypeRef {
    /// TYPE-REF = STRING | ARRAY-TYPE
    /// ARRAY-TYPE = [ STRING ]
    pub fn parse(input: &str) -> IResult<&str, Self> {
        let weak_parser = take_qapi_string;
        let weak_array_parser = delimited(
            tuple((clean_line, tag("["))),
            weak_parser,
            tuple((tag("]"), clean_line)),
        );
        alt((
            map(weak_parser, |v| Self::Weak(v.to_string())),
            map(weak_array_parser, |v| Self::WeakArray(v.to_string())),
        ))(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_INPUTS: [&str; 5] = [
        "'STRING'",
        " 'STRING'",
        " 'STRING' ",
        "'STRING' ",
        r#"
        # some precomment
        'STRING' "#,
    ];
    const VALID_INPUTS_ARRAY: [&str; 6] = [
        "['ARRAYSTRING']",
        "['ARRAYSTRING' ]",
        "[ 'ARRAYSTRING' ]",
        "[ 'ARRAYSTRING']",
        "[ 'ARRAYSTRING']  ",
        r#"[ # some qapi comment 
            'ARRAYSTRING' # another comment with a ]
            ]  #final comment
        "#,
    ];
    const INVALID_INPUTS: [&str; 5] = [
        "invalid_input",
        "STRING",
        "[STRING]",
        "['STRING',]",
        "['STRING','STRING']",
    ];

    #[test]
    fn test_qapi_type_ref_weak() {
        for input in VALID_INPUTS {
            let result = QapiTypeRef::parse(input);
            match result {
                Ok((remaining, QapiTypeRef::Weak(weak_str))) => {
                    assert_eq!(weak_str, "STRING".to_string());
                    assert_eq!(remaining, "");
                }
                _ => panic!("Failed to parse weak type"),
            }
        }
    }

    #[test]
    fn test_qapi_type_ref_weak_array() {
        for input in VALID_INPUTS_ARRAY {
            let result = QapiTypeRef::parse(input);
            match result {
                Ok((remaining, QapiTypeRef::WeakArray(weak_array_str))) => {
                    assert_eq!(weak_array_str, "ARRAYSTRING".to_string());
                    assert_eq!(remaining, "");
                }
                _ => panic!("Failed to parse weak array type"),
            }
        }
    }

    #[test]
    fn test_qapi_type_ref_invalid() {
        for input in INVALID_INPUTS {
            let result = QapiTypeRef::parse(input);
            assert!(result.is_err());
        }
    }
}
