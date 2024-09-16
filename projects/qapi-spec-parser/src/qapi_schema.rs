use crate::helpers::qcomment;
use crate::{
    QapiAlternate, QapiCommand, QapiDocumentation, QapiEnum, QapiEvent, QapiInclude, QapiPragma,
    QapiStruct, QapiUnion,
};
use anyhow::Result;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::multispace1;
use nom::combinator::map;
use nom::multi::many0;
use nom::IResult;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum ParserKey {
    Struct(QapiStruct),
    Enum(QapiEnum),
    Documentation(QapiDocumentation),
    Alternate(QapiAlternate),
    Pragma(QapiPragma),
    Include(QapiInclude),
    Union(QapiUnion),
    Event(QapiEvent),
    Command(QapiCommand),
    Comment(String),
    Empty,
}

#[derive(Debug)]
pub struct QapiSchema {
    pub structs: Vec<QapiStruct>,
    pub documentations: Vec<QapiDocumentation>,
    pub enums: Vec<QapiEnum>,
    pub alternates: Vec<QapiAlternate>,
    pub pragmas: Vec<QapiPragma>,
    pub includes: Vec<QapiInclude>,
    pub unions: Vec<QapiUnion>,
    pub events: Vec<QapiEvent>,
    pub commands: Vec<QapiCommand>,
}

impl QapiSchema {
    pub fn parse(input: &str) -> IResult<&str, Self> {
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
    pub fn flatten(schemas: Vec<QapiSchema>) -> Self {
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

fn read_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}

pub fn walk_schema(
    path: &Path,
    visited: &mut std::collections::HashSet<PathBuf>,
) -> Result<QapiSchema> {
    if !visited.insert(path.to_path_buf()) {
        let (_, schema) = QapiSchema::parse("").unwrap();
        return Ok(schema);
    }

    let schema_string = read_file(path)?;
    let (remaining, schema) = QapiSchema::parse(&schema_string).unwrap();
    if remaining.len() != 0 {
        dbg![remaining];
        todo! {"No errors, but input remains"};
    }

    let includes = schema.includes.clone();
    let mut schemas = vec![schema];
    for include in includes {
        let full_path = path.parent().unwrap().join(include.0 .0.clone());
        let new_schema = walk_schema(&full_path, visited)?;
        schemas.push(new_schema);
    }

    Ok(QapiSchema::flatten(schemas))
}
