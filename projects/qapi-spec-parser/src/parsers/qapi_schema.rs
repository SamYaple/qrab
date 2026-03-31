use super::{qcomment, vec_newtype};
use crate::{
    QapiAlternate, QapiCommand, QapiEnum, QapiEvent, QapiInclude, QapiPragma,
    QapiSectionDocumentation, QapiStruct, QapiUnion,
};
use nom::branch::alt;
use nom::character::complete::multispace1;
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
            multispace1.map(|_| QapiSchemaToken::EmptyLines),
        )))
        .map(|tokens| Self(tokens))
        .parse(input)
    }
}

vec_newtype!(QapiSchema, QapiSchemaToken<'i>);
