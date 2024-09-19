use crate::helpers::qcomment;
use crate::{
    QapiAlternate, QapiCommand, QapiDocumentation, QapiEnum, QapiEvent, QapiInclude, QapiPragma,
    QapiSectionDocumentation, QapiStruct, QapiUnion,
};
use nom::branch::alt;
use nom::character::complete::multispace1;
use nom::combinator::map;
use nom::multi::many0;
use nom::IResult;

#[derive(Debug, Clone)]
pub enum QapiSchema<'i> {
    Struct(QapiStruct<'i>),
    Enum(QapiEnum<'i>),
    Documentation(QapiDocumentation<'i>),
    SectionDocumentation(QapiSectionDocumentation<'i>),
    Alternate(QapiAlternate<'i>),
    Pragma(QapiPragma<'i>),
    Include(QapiInclude<'i>),
    Union(QapiUnion<'i>),
    Event(QapiEvent<'i>),
    Command(QapiCommand<'i>),
    Comment(&'i str),
    EmptyLines,
}

impl<'i> QapiSchema<'i> {
    pub fn parse(input: &'i str) -> IResult<&'i str, Vec<Self>> {
        many0(alt((
            map(QapiSectionDocumentation::parse, |v| {
                QapiSchema::SectionDocumentation(v)
            }),
            map(QapiDocumentation::parse, |v| QapiSchema::Documentation(v)),
            map(QapiStruct::parse, |v| QapiSchema::Struct(v)),
            map(QapiEnum::parse, |v| QapiSchema::Enum(v)),
            map(QapiAlternate::parse, |v| QapiSchema::Alternate(v)),
            map(QapiPragma::parse, |v| QapiSchema::Pragma(v)),
            map(QapiInclude::parse, |v| QapiSchema::Include(v)),
            map(QapiUnion::parse, |v| QapiSchema::Union(v)),
            map(QapiEvent::parse, |v| QapiSchema::Event(v)),
            map(QapiCommand::parse, |v| QapiSchema::Command(v)),
            map(qcomment, |v| QapiSchema::Comment(v)),
            map(multispace1, |_| QapiSchema::EmptyLines),
        )))(input)
    }
}
