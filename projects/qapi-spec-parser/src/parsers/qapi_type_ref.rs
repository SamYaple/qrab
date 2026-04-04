use super::{qstring, qtag, take_kv};
use crate::QapiMembers;
use crate::take_members;
use nom::branch::alt;
use nom::sequence::delimited;
use nom::IResult;
use nom::Parser;

pub fn take_type_ref(input: &str) -> IResult<&str, QapiTypeRef<'_>> {
    take_kv("type", QapiTypeRef::parse).parse(input)
}

#[derive(Debug, Clone, PartialEq)]
pub enum QapiTypeRef<'i> {
    Unset,
    Ref(&'i str),
    ArrayRef(&'i str),
    /// Inline anonymous struct (v1.x schemas): `{'field': 'type', ...}`
    InlineStruct(QapiMembers<'i>),
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
            qstring.map(|v| Self::Ref(v)),
            array_parser.map(|v| Self::ArrayRef(v)),
        ))
        .parse(input)
    }

    /// Extended parser that also handles inline anonymous structs (v1.x schemas).
    /// Use this only in contexts where inline structs are valid (member simple parser).
    pub fn parse_with_inline(input: &'i str) -> IResult<&'i str, Self> {
        alt((
            Self::parse,
            take_members.map(|v| Self::InlineStruct(v)),
        ))
        .parse(input)
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
