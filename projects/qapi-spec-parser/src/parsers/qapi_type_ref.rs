use super::{qstring, qtag, take_kv, with_parser_config};
use crate::{take_members, QapiMembers};
use nom::branch::alt;
use nom::sequence::delimited;
use nom::IResult;
use nom::Parser;

pub fn take_type_ref(input: &str) -> IResult<&str, QapiTypeRef<'_>> {
    take_kv("type", QapiTypeRef::parse).parse(input)
}

#[derive(Debug, Clone, PartialEq)]
#[derive(Default)]
pub enum QapiTypeRef<'i> {
    #[default]
    Unset,
    Ref(&'i str),
    ArrayRef(&'i str),
    /// Inline anonymous struct definition (v1.0–v2.3.0)
    InlineStruct(QapiMembers<'i>),
}


impl<'i> QapiTypeRef<'i> {
    /// TYPE-REF = STRING | ARRAY-TYPE | INLINE-STRUCT
    /// ARRAY-TYPE = [ STRING ]
    /// INLINE-STRUCT = { MEMBERS } (only when inline_member_structs config is set)
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        let array_parser = delimited(qtag("["), qstring, qtag("]"));
        if with_parser_config(|c| c.inline_member_structs) {
            alt((
                qstring.map(Self::Ref),
                array_parser.map(Self::ArrayRef),
                take_members.map(Self::InlineStruct),
            ))
            .parse(input)
        } else {
            alt((
                qstring.map(Self::Ref),
                array_parser.map(Self::ArrayRef),
            ))
            .parse(input)
        }
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

    #[test]
    fn test_inline_struct_not_parsed_by_default() {
        // With modern config (default), inline structs should NOT be parsed
        let input = "{ 'major': 'int', 'minor': 'int' }";
        let result = QapiTypeRef::parse(input);
        assert!(result.is_err(), "Modern parser should not accept inline structs");
    }

    #[test]
    fn test_inline_struct_parsed_with_config() {
        use crate::parsers::set_parser_config;
        use crate::ParserConfig;

        // Enable inline struct parsing
        set_parser_config(ParserConfig {
            inline_member_structs: true,
            ..ParserConfig::modern()
        });

        let input = "{ 'major': 'int', 'minor': 'int' }";
        let result = QapiTypeRef::parse(input);
        assert!(result.is_ok(), "Should parse inline struct with config enabled");
        let (remaining, parsed) = result.unwrap();
        assert_eq!(remaining, "");
        match parsed {
            QapiTypeRef::InlineStruct(members) => {
                assert_eq!(members.len(), 2);
            }
            _ => panic!("Expected InlineStruct variant"),
        }

        // Reset to modern config
        set_parser_config(ParserConfig::modern());
    }
}
