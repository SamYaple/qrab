use super::{qcomment, vec_newtype};
use crate::{
    QapiAlternate, QapiCommand, QapiEnum, QapiEvent, QapiInclude, QapiPragma,
    QapiSectionDocumentation, QapiStruct, QapiUnion,
};
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::{line_ending, multispace1, space0};
use nom::combinator::recognize;
use nom::multi::many0;
use nom::IResult;
use nom::Parser;

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
        many0(alt((
            QapiAlternate::parse.map(|v| QapiSchemaToken::Alternate(v)),
            QapiCommand::parse.map(|v| QapiSchemaToken::Command(v)),
            QapiEnum::parse.map(|v| QapiSchemaToken::Enum(v)),
            QapiEvent::parse.map(|v| QapiSchemaToken::Event(v)),
            QapiStruct::parse.map(|v| QapiSchemaToken::Struct(v)),
            QapiUnion::parse.map(|v| QapiSchemaToken::Union(v)),
            QapiInclude::parse.map(|v| QapiSchemaToken::Include(v)),
            QapiPragma::parse.map(|v| QapiSchemaToken::Pragma(v)),
            QapiSectionDocumentation::parse.map(|v| QapiSchemaToken::Doc(v)),
            qcomment.map(|v| QapiSchemaToken::Comment(v)),
            // Bare `##` line — orphaned doc end marker in older schemas
            recognize((space0, tag("##"), line_ending)).map(|v| QapiSchemaToken::Comment(v)),
            multispace1.map(|_| QapiSchemaToken::EmptyLines),
        )))
        .map(|tokens| Self(tokens))
        .parse(input)
    }
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
        let type_count = s.iter().filter(|t| !matches!(t, QapiSchemaToken::EmptyLines | QapiSchemaToken::Comment(_))).count();
        assert_eq!(type_count, 4);
    }
}
