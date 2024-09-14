use crate::helpers::qtag;
use crate::QapiString;
use nom::branch::alt;
use nom::combinator::map;
use nom::sequence::delimited;
use nom::IResult;

#[derive(Debug)]
pub enum QapiTypeRef {
    Weak(QapiString),
    WeakArray(QapiString),
}

impl QapiTypeRef {
    /// TYPE-REF = STRING | ARRAY-TYPE
    /// ARRAY-TYPE = [ STRING ]
    pub fn parse(input: &str) -> IResult<&str, Self> {
        let weak_parser = QapiString::parse;
        let array_parser = delimited(qtag("["), weak_parser, qtag("]"));
        alt((
            map(weak_parser, |v| Self::Weak(v)),
            map(array_parser, |v| Self::WeakArray(v)),
        ))(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_INPUTS: [&str; 3] = [
        "'STRING'",
        " 'STRING'",
        r#"
        # some precomment
        'STRING'"#,
    ];
    const VALID_INPUTS_ARRAY: [&str; 5] = [
        "['ARRAYSTRING']",
        "['ARRAYSTRING' ]",
        "[ 'ARRAYSTRING' ]",
        "[ 'ARRAYSTRING']",
        r#"[ # some qapi comment 
            'ARRAYSTRING' # another comment with a ]
            ]"#,
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
                    assert_eq!(weak_str, QapiString("STRING".into()));
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
                    assert_eq!(weak_array_str, QapiString("ARRAYSTRING".into()));
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
