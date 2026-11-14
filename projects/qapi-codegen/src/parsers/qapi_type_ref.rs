use super::common::{qstring, qtag};
use super::parser_version;
use super::qapi_members::QapiMembers;
use nom::IResult;
use nom::Parser;
use nom::branch::alt;
use nom::sequence::delimited;

pub fn take_type_ref(input: &str) -> IResult<&str, QapiTypeRef<'_>> {
    use super::common::take_kv;
    take_kv("type", QapiTypeRef::parse).parse(input)
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum QapiTypeRef<'i> {
    #[default]
    Unset,
    Ref(&'i str),
    ArrayRef(&'i str),
    /// Inline anonymous struct definition (v1.0–v2.3.0)
    InlineStruct(QapiMembers<'i>),
}

impl<'i> QapiTypeRef<'i> {
    /// Dispatches to the correct versioned parser.
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        let v = parser_version();
        if v < (2, 4, 0) {
            Self::parse_v1(input)
        } else {
            Self::parse_v2(input)
        }
    }

    /// v1 (QEMU v1.0–v2.3.0): accepts string, array, or inline struct dict.
    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
        use super::qapi_members::take_members;
        let array_parser = delimited(qtag("["), qstring, qtag("]"));
        alt((
            qstring.map(Self::Ref),
            array_parser.map(Self::ArrayRef),
            take_members.map(Self::InlineStruct),
        ))
        .parse(input)
    }

    /// v2 (QEMU v2.4.0+): only string or array refs.
    pub fn parse_v2(input: &'i str) -> IResult<&'i str, Self> {
        let array_parser = delimited(qtag("["), qstring, qtag("]"));
        alt((qstring.map(Self::Ref), array_parser.map(Self::ArrayRef))).parse(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::set_parser_version;

    #[test]
    fn test_string_ref() {
        set_parser_version((10, 2, 0));
        let (rem, parsed) = QapiTypeRef::parse("'str'").unwrap();
        assert_eq!(rem, "");
        assert_eq!(parsed, QapiTypeRef::Ref("str"));
    }

    #[test]
    fn test_array_ref() {
        set_parser_version((10, 2, 0));
        let (rem, parsed) = QapiTypeRef::parse("['int']").unwrap();
        assert_eq!(rem, "");
        assert_eq!(parsed, QapiTypeRef::ArrayRef("int"));
    }

    #[test]
    fn test_inline_struct_not_parsed_in_v2() {
        set_parser_version((10, 2, 0));
        let input = "{ 'major': 'int', 'minor': 'int' }";
        let result = QapiTypeRef::parse(input);
        assert!(result.is_err(), "v2 should not accept inline structs");
    }

    #[test]
    fn test_inline_struct_parsed_in_v1() {
        set_parser_version((2, 0, 0));
        let input = "{ 'major': 'int', 'minor': 'int' }";
        let result = QapiTypeRef::parse(input);
        assert!(result.is_ok(), "v1 should accept inline structs");
        let (rem, parsed) = result.unwrap();
        assert_eq!(rem, "");
        match parsed {
            QapiTypeRef::InlineStruct(members) => {
                assert_eq!(members.len(), 2);
            }
            _ => panic!("Expected InlineStruct"),
        }
    }
}
