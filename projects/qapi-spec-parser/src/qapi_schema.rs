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
pub enum ParserToken<'input> {
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
                map(QapiDocumentation::parse, |v| ParserToken::Documentation(v)),
                map(QapiStruct::parse, |v| ParserToken::Struct(v)),
                map(QapiEnum::parse, |v| ParserToken::Enum(v)),
                map(QapiAlternate::parse, |v| ParserToken::Alternate(v)),
                map(QapiPragma::parse, |v| ParserToken::Pragma(v)),
                map(QapiInclude::parse, |v| ParserToken::Include(v)),
                map(QapiUnion::parse, |v| ParserToken::Union(v)),
                map(QapiEvent::parse, |v| ParserToken::Event(v)),
                map(QapiCommand::parse, |v| ParserToken::Command(v)),
                map(qcomment, |v| ParserToken::Comment(v.into())),
                map(multispace1, |_| ParserToken::Empty),
                // This was a documentation block starter that gets ignored by
                // qcomment
                map(tag("##"), |_| ParserToken::Empty),
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
                        ParserToken::Struct(v) => structs.push(v),
                        ParserToken::Documentation(v) => documentations.push(v),
                        ParserToken::Alternate(v) => alternates.push(v),
                        ParserToken::Pragma(v) => pragmas.push(v),
                        ParserToken::Include(v) => includes.push(v),
                        ParserToken::Union(v) => unions.push(v),
                        ParserToken::Event(v) => events.push(v),
                        ParserToken::Command(v) => commands.push(v),
                        ParserToken::Enum(v) => enums.push(v),
                        ParserToken::Comment(v) => {
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
                        ParserToken::Empty => {}
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
