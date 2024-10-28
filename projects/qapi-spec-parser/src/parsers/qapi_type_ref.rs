use super::{qstring, qtag, take_kv};
use nom::branch::alt;
use nom::combinator::map;
use nom::sequence::delimited;
use nom::IResult;

pub fn take_type_ref(input: &str) -> IResult<&str, QapiTypeRef<'_>> {
    take_kv("type", QapiTypeRef::parse)(input)
}

#[derive(Debug, Clone, PartialEq)]
pub enum QapiTypeRef<'i> {
    Unset,
    Ref(&'i str),
    ArrayRef(&'i str),
}

impl Default for QapiTypeRef<'_> {
    fn default() -> Self {
        Self::Unset
    }
}

impl<'i> QapiTypeRef<'i> {
    /// TYPE-REF = STRING | ARRAY-TYPE
    /// ARRAY-TYPE = [ STRING ]
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        let array_parser = delimited(qtag("["), qstring, qtag("]"));
        alt((
            map(qstring, |v| Self::Ref(v)),
            map(array_parser, |v| Self::ArrayRef(v)),
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
