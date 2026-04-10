use super::common::qcomment;
use super::qapi_alternate::QapiAlternate;
use super::qapi_command::QapiCommand;
use super::qapi_documentation::{QapiFreeformDoc, QapiSectionDocumentation};
use super::qapi_enum::QapiEnum;
use super::qapi_event::QapiEvent;
use super::qapi_include::QapiInclude;
use super::qapi_pragma::QapiPragma;
use super::qapi_struct::QapiStruct;
use super::qapi_union::QapiUnion;
use super::vec_newtype;
use nom::IResult;
use nom::Parser;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::multispace1;
use nom::character::complete::{line_ending, space0};
use nom::combinator::recognize;
use nom::multi::many0;
use nom::sequence::terminated;

pub fn take_schema(input: &str) -> IResult<&str, QapiSchema<'_>> {
    QapiSchema::parse(input)
}

#[derive(Debug, Clone)]
pub enum QapiSchemaToken<'i> {
    Alternate(QapiAlternate<'i>),
    Command(QapiCommand<'i>),
    Enum(QapiEnum<'i>),
    Event(QapiEvent<'i>),
    Struct(QapiStruct<'i>),
    Union(QapiUnion<'i>),
    Include(QapiInclude<'i>),
    Pragma(QapiPragma<'i>),
    Doc(QapiSectionDocumentation<'i>),
    FreeformDoc(QapiFreeformDoc<'i>),
    Comment(&'i str),
    EmptyLines,
}

// To preserve the token ordering at this stage, the parsers schema output is a
// Vec<Token> with each token being a parsed element from the qapi spec. At this
// stage, all ordering is preserved and the `&str` references can still be used
// to highlight and debug the original schema file.
#[derive(Debug, Clone)]
pub struct QapiSchema<'i>(pub Vec<QapiSchemaToken<'i>>);
impl<'i> QapiSchema<'i> {
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        Self::parse_v1(input)
    }

    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
        many0(alt((
            QapiAlternate::parse.map(QapiSchemaToken::Alternate),
            QapiCommand::parse.map(QapiSchemaToken::Command),
            QapiEnum::parse.map(QapiSchemaToken::Enum),
            QapiEvent::parse.map(QapiSchemaToken::Event),
            QapiStruct::parse.map(QapiSchemaToken::Struct),
            QapiUnion::parse.map(QapiSchemaToken::Union),
            QapiInclude::parse.map(QapiSchemaToken::Include),
            QapiPragma::parse.map(QapiSchemaToken::Pragma),
            QapiSectionDocumentation::parse.map(QapiSchemaToken::Doc),
            QapiFreeformDoc::parse.map(QapiSchemaToken::FreeformDoc),
            // Bare ## separator (not followed by doc content, common in v2.x)
            bare_doc_separator.map(|_| QapiSchemaToken::EmptyLines),
            qcomment.map(QapiSchemaToken::Comment),
            multispace1.map(|_| QapiSchemaToken::EmptyLines),
        )))
        .map(Self)
        .parse(input)
    }
}

/// Matches a bare `##\n` line that acts as a separator (not a doc block).
/// Matches a bare `##` line (with optional trailing whitespace) that acts as a separator.
fn bare_doc_separator(input: &str) -> IResult<&str, &str> {
    recognize(terminated((space0, tag("##"), space0), line_ending)).parse(input)
}

vec_newtype!(QapiSchema, QapiSchemaToken<'i>);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_input() {
        let (rem, s) = QapiSchema::parse("").unwrap();
        assert_eq!(rem, "");
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn test_single_enum() {
        let input = "{ 'enum': 'MyEnum', 'data': ['val-a', 'val-b'] }\n";
        let (rem, s) = QapiSchema::parse(input).unwrap();
        assert_eq!(rem, "");
        assert!(s.iter().any(|t| matches!(t, QapiSchemaToken::Enum(_))));
    }

    #[test]
    fn test_single_struct() {
        let input = "{ 'struct': 'MyStruct', 'data': { 'name': 'str' } }\n";
        let (rem, s) = QapiSchema::parse(input).unwrap();
        assert_eq!(rem, "");
        assert!(s.iter().any(|t| matches!(t, QapiSchemaToken::Struct(_))));
    }

    #[test]
    fn test_single_command() {
        let input = "{ 'command': 'my-cmd', 'data': { 'arg': 'str' } }\n";
        let (rem, s) = QapiSchema::parse(input).unwrap();
        assert_eq!(rem, "");
        assert!(s.iter().any(|t| matches!(t, QapiSchemaToken::Command(_))));
    }

    #[test]
    fn test_include() {
        let input = "{ 'include': 'other.json' }\n";
        let (rem, s) = QapiSchema::parse(input).unwrap();
        assert_eq!(rem, "");
        assert!(s.iter().any(|t| matches!(t, QapiSchemaToken::Include(_))));
    }

    #[test]
    fn test_comment_and_whitespace() {
        // Comments before a definition get absorbed by clean_lines during parsing.
        // A standalone comment only appears as a Comment token when not adjacent to a definition.
        let input = "# a comment\n{ 'enum': 'E', 'data': ['a'] }\n";
        let (rem, s) = QapiSchema::parse(input).unwrap();
        assert_eq!(rem, "");
        let has_enum = s.iter().any(|t| matches!(t, QapiSchemaToken::Enum(_)));
        assert!(has_enum);
    }

    #[test]
    fn test_multi_entry_schema() {
        let input = concat!(
            "{ 'enum': 'Status', 'data': ['ok', 'error'] }\n",
            "{ 'struct': 'Info', 'data': { 'name': 'str' } }\n",
            "{ 'command': 'get-info', 'returns': 'Info' }\n",
            "{ 'event': 'STATUS_CHANGE', 'data': { 'status': 'Status' } }\n",
        );
        let (rem, s) = QapiSchema::parse(input).unwrap();
        assert_eq!(rem, "");
        let type_count = s
            .iter()
            .filter(|t| !matches!(t, QapiSchemaToken::EmptyLines | QapiSchemaToken::Comment(_)))
            .count();
        assert_eq!(type_count, 4);
    }
}

#[cfg(test)]
mod freeform_schema_tests {
    use super::*;
    use crate::parsers::set_parser_version;

    #[test]
    fn test_freeform_doc_direct() {
        set_parser_version((99, 0, 0));
        // Test QapiFreeformDoc directly on the input
        let input = "##\n# title\n# body\n##\n";
        let result = QapiFreeformDoc::parse(input);
        assert!(
            result.is_ok(),
            "Direct FreeformDoc parse failed: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_freeform_doc_in_schema() {
        set_parser_version((99, 0, 0));
        let input = "##\n# title\n# body\n##\n{ 'include': 'other.json' }\n";
        let (rem, s) = QapiSchema::parse(input).unwrap();
        assert_eq!(
            rem,
            "",
            "Unparsed: {:?}",
            &rem[..std::cmp::min(rem.len(), 100)]
        );
        let has_freeform = s
            .iter()
            .any(|t| matches!(t, QapiSchemaToken::FreeformDoc(_)));
        assert!(has_freeform, "Should have freeform doc");
        let has_include = s.iter().any(|t| matches!(t, QapiSchemaToken::Include(_)));
        assert!(has_include, "Should have include");
    }
}
