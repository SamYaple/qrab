use crate::helpers::qcomment;
use crate::{
    QapiAlternate, QapiCommand, QapiEnum, QapiEvent, QapiInclude, QapiPragma,
    QapiSectionDocumentation, QapiStruct, QapiUnion,
};
use nom::branch::alt;
use nom::character::complete::multispace1;
use nom::combinator::map;
use nom::multi::many0;
use nom::IResult;

#[derive(Debug, Clone)]
pub enum QapiSchema<'i> {
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

impl<'i> QapiSchema<'i> {
    pub fn parse(input: &'i str) -> IResult<&'i str, Vec<Self>> {
        many0(alt((
            map(QapiAlternate::parse, |v| QapiSchema::Alternate(v)),
            map(QapiCommand::parse, |v| QapiSchema::Command(v)),
            map(QapiEnum::parse, |v| QapiSchema::Enum(v)),
            map(QapiEvent::parse, |v| QapiSchema::Event(v)),
            map(QapiStruct::parse, |v| QapiSchema::Struct(v)),
            map(QapiUnion::parse, |v| QapiSchema::Union(v)),
            map(QapiInclude::parse, |v| QapiSchema::Include(v)),
            map(QapiPragma::parse, |v| QapiSchema::Pragma(v)),
            map(QapiSectionDocumentation::parse, |v| QapiSchema::Doc(v)),
            map(qcomment, |v| QapiSchema::Comment(v)),
            map(multispace1, |_| QapiSchema::EmptyLines),
        )))(input)
    }
}
