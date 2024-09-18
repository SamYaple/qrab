use crate::helpers::qcomment;
use crate::{
    QapiAlternate, QapiCommand, QapiDocumentation, QapiEnum, QapiEvent, QapiInclude, QapiPragma,
    QapiStruct, QapiUnion,
};
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::multispace1;
use nom::combinator::map;
use nom::multi::many0;
use nom::IResult;

#[derive(Debug)]
pub enum ParserKey<'input> {
    Struct(QapiStruct<'input>),
    Enum(QapiEnum<'input>),
    Documentation(QapiDocumentation<'input>),
    Alternate(QapiAlternate<'input>),
    Pragma(QapiPragma<'input>),
    Include(QapiInclude<'input>),
    Union(QapiUnion<'input>),
    Event(QapiEvent<'input>),
    Command(QapiCommand<'input>),
    Comment(String),
    Empty,
}

#[derive(Debug, Clone)]
pub struct QapiSchema<'input> {
    pub structs: Vec<QapiStruct<'input>>,
    pub documentations: Vec<QapiDocumentation<'input>>,
    pub enums: Vec<QapiEnum<'input>>,
    pub alternates: Vec<QapiAlternate<'input>>,
    pub pragmas: Vec<QapiPragma<'input>>,
    pub includes: Vec<QapiInclude<'input>>,
    pub unions: Vec<QapiUnion<'input>>,
    pub events: Vec<QapiEvent<'input>>,
    pub commands: Vec<QapiCommand<'input>>,
}

impl<'input> QapiSchema<'input> {
    pub fn parse(input: &'input str) -> IResult<&'input str, Self> {
        map(
            many0(alt((
                map(QapiDocumentation::parse, |v| ParserKey::Documentation(v)),
                map(QapiStruct::parse, |v| ParserKey::Struct(v)),
                map(QapiEnum::parse, |v| ParserKey::Enum(v)),
                map(QapiAlternate::parse, |v| ParserKey::Alternate(v)),
                map(QapiPragma::parse, |v| ParserKey::Pragma(v)),
                map(QapiInclude::parse, |v| ParserKey::Include(v)),
                map(QapiUnion::parse, |v| ParserKey::Union(v)),
                map(QapiEvent::parse, |v| ParserKey::Event(v)),
                map(QapiCommand::parse, |v| ParserKey::Command(v)),
                map(qcomment, |v| ParserKey::Comment(v.into())),
                map(multispace1, |_| ParserKey::Empty),
                // This was a documentation block starter that gets ignored by
                // qcomment
                map(tag("##"), |_| ParserKey::Empty),
            ))),
            |tokens| {
                let mut structs = vec![];
                let mut enums = vec![];
                let mut documentations = vec![];
                let mut alternates = vec![];
                let mut pragmas = vec![];
                let mut includes = vec![];
                let mut unions = vec![];
                let mut events = vec![];
                let mut commands = vec![];
                for token in tokens {
                    match token {
                        ParserKey::Struct(v) => structs.push(v),
                        ParserKey::Documentation(v) => documentations.push(v),
                        ParserKey::Alternate(v) => alternates.push(v),
                        ParserKey::Pragma(v) => pragmas.push(v),
                        ParserKey::Include(v) => includes.push(v),
                        ParserKey::Union(v) => unions.push(v),
                        ParserKey::Event(v) => events.push(v),
                        ParserKey::Command(v) => commands.push(v),
                        ParserKey::Enum(v) => enums.push(v),
                        ParserKey::Comment(v) => {
                            // Discarding known strings and eprinting everything
                            // else for debug and such
                            match v.trim() {
                                "" |
                                "-*- Mode: Python -*-" |
                                "-*- mode: python -*-" |
                                "*-*- Mode: Python -*-*" |
                                "vim: filetype=python" |
                                "SPDX-License-Identifier: GPL-2.0-or-later" |
                                "See the COPYING file in the top-level directory." |
                                "This work is licensed under the terms of the GNU GPL, version 2 or later." => {},
                                v if v.starts_with("Copyright") => {},
                                _ => eprintln!("DEBUG: unused comment string ```{v}"),
                            }
                        }
                        ParserKey::Empty => {}
                    }
                }
                Self {
                    structs,
                    enums,
                    documentations,
                    alternates,
                    pragmas,
                    includes,
                    unions,
                    events,
                    commands,
                }
            },
        )(input)
    }
    pub fn flatten(schemas: Vec<QapiSchema<'input>>) -> Self {
        let mut structs = vec![];
        let mut enums = vec![];
        let mut documentations = vec![];
        let mut alternates = vec![];
        let mut pragmas = vec![];
        let mut includes = vec![];
        let mut unions = vec![];
        let mut events = vec![];
        let mut commands = vec![];

        for schema in schemas {
            structs.extend(schema.structs);
            enums.extend(schema.enums);
            documentations.extend(schema.documentations);
            alternates.extend(schema.alternates);
            pragmas.extend(schema.pragmas);
            includes.extend(schema.includes);
            unions.extend(schema.unions);
            events.extend(schema.events);
            commands.extend(schema.commands);
        }

        Self {
            structs,
            enums,
            documentations,
            alternates,
            pragmas,
            includes,
            unions,
            events,
            commands,
        }
    }
}
