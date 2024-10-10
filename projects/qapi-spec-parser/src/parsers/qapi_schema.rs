use crate::helpers::qcomment;
use crate::{
    QapiAlternate, QapiCommand, QapiEnum, QapiEvent, QapiInclude, QapiPragma,
    QapiSectionDocumentation, QapiStruct, QapiUnion,
};
use nom::branch::alt;
use nom::character::complete::multispace1;
use nom::combinator::{all_consuming, map};
use nom::multi::many0;
use nom::IResult;

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

#[derive(Debug)]
pub struct QapiSchema<'i>(pub Vec<QapiSchemaToken<'i>>);
impl<'i> QapiSchema<'i> {
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        map(
            many0(alt((
                map(QapiAlternate::parse, |v| QapiSchemaToken::Alternate(v)),
                map(QapiCommand::parse, |v| QapiSchemaToken::Command(v)),
                map(QapiEnum::parse, |v| QapiSchemaToken::Enum(v)),
                map(QapiEvent::parse, |v| QapiSchemaToken::Event(v)),
                map(QapiStruct::parse, |v| QapiSchemaToken::Struct(v)),
                map(QapiUnion::parse, |v| QapiSchemaToken::Union(v)),
                map(QapiInclude::parse, |v| QapiSchemaToken::Include(v)),
                map(QapiPragma::parse, |v| QapiSchemaToken::Pragma(v)),
                map(QapiSectionDocumentation::parse, |v| QapiSchemaToken::Doc(v)),
                map(qcomment, |v| QapiSchemaToken::Comment(v)),
                map(multispace1, |_| QapiSchemaToken::EmptyLines),
            ))),
            |tokens| Self(tokens),
        )(input)
    }
}
