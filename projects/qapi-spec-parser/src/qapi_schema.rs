use crate::helpers::qcomment;
use crate::{
    QapiAlternate, QapiCommand, QapiEnum, QapiEvent, QapiInclude, QapiPragma, QapiStruct, QapiUnion,
};
use nom::branch::alt;
use nom::character::complete::multispace1;
use nom::combinator::map;
use nom::multi::many1;
use nom::IResult;

#[derive(Debug)]
pub enum QapiSchema {
    Struct(QapiStruct),
    Enum(QapiEnum),
    Alternate(QapiAlternate),
    Pragma(QapiPragma),
    Include(QapiInclude),
    Union(QapiUnion),
    Event(QapiEvent),
    Command(QapiCommand),
    Comment(String),
    Empty,
}

impl QapiSchema {
    pub fn parse(input: &str) -> IResult<&str, Vec<Self>> {
        many1(alt((
            map(QapiStruct::parse, |v| Self::Struct(v)),
            map(QapiEnum::parse, |v| Self::Enum(v)),
            map(QapiAlternate::parse, |v| Self::Alternate(v)),
            map(QapiPragma::parse, |v| Self::Pragma(v)),
            map(QapiInclude::parse, |v| Self::Include(v)),
            map(QapiUnion::parse, |v| Self::Union(v)),
            map(QapiEvent::parse, |v| Self::Event(v)),
            map(QapiCommand::parse, |v| Self::Command(v)),
            map(qcomment, |v| Self::Comment(v.into())),
            map(multispace1, |_| Self::Empty),
        )))(input)
    }
}
