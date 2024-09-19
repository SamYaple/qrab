use crate::helpers::qtag;
use crate::QapiString;
use nom::branch::alt;
use nom::combinator::map;
use nom::sequence::delimited;
use nom::IResult;

#[derive(Debug, Clone)]
pub enum QapiTypeRef<'i> {
    Weak(QapiString<'i>),
    WeakArray(QapiString<'i>),
}

impl<'i> QapiTypeRef<'i> {
    /// TYPE-REF = STRING | ARRAY-TYPE
    /// ARRAY-TYPE = [ STRING ]
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
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

    #[test]
    fn test_valid() {
        for input in VALID_INPUTS {
            let result = QapiTypeRef::parse(input);
            match result {
                Ok((remaining, _)) => {
                    assert_eq!(remaining, "");
                }
                _ => panic!("Failed to parse: ```\n{input}\n```"),
            }
        }
    }

    const VALID_INPUTS_ARRAY: [&str; 5] = [
        "['ARRAYSTRING']",
        "['ARRAYSTRING' ]",
        "[ 'ARRAYSTRING' ]",
        "[ 'ARRAYSTRING']",
        r#"[ # some qapi comment
            'ARRAYSTRING' # another comment with a ]
        ]"#,
    ];

    #[test]
    fn test_array_valid() {
        for input in VALID_INPUTS_ARRAY {
            let result = QapiTypeRef::parse(input);
            match result {
                Ok((remaining, _)) => {
                    assert_eq!(remaining, "");
                }
                _ => panic!("Failed to parse: ```\n{input}\n```"),
            }
        }
    }
}
