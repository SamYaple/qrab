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
pub enum QapiSchema<'input> {
    Struct(QapiStruct<'input>),
    Enum(QapiEnum<'input>),
    Documentation(QapiDocumentation<'input>),
    SectionDocumentation(QapiSectionDocumentation<'input>),
    Alternate(QapiAlternate<'input>),
    Pragma(QapiPragma<'input>),
    Include(QapiInclude<'input>),
    Union(QapiUnion<'input>),
    Event(QapiEvent<'input>),
    Command(QapiCommand<'input>),
    Comment(&'input str),
    EmptyLines,
}

impl<'input> QapiSchema<'input> {
    pub fn parse(input: &'input str) -> IResult<&'input str, Vec<Self>> {
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
        // QapiSchema::Comment(v) => {
        //     // Discarding known strings and eprinting everything
        //     // else for debug and such
        //     match v.trim() {
        //         "" |
        //         "-*- Mode: Python -*-" |
        //         "-*- mode: python -*-" |
        //         "*-*- Mode: Python -*-*" |
        //         "vim: filetype=python" => {},
        //         // Ignore floating dev comment
        //         "Note: This type is deprecated in favor of SocketAddress.  The" |
        //         "difference between SocketAddressLegacy and SocketAddress is that the" |
        //         "latter has fewer ``{}`` on the wire." => {},
        //         // Ignore floating dev comment
        //         "If we need to add block driver specific parameters for" |
        //         "LUKS in future, then we'll subclass QCryptoBlockInfoLUKS" |
        //         "to define a ImageInfoSpecificLUKS" => {},
        //         // Ignore various license string bits
        //         "SPDX-License-Identifier: GPL-2.0-or-later" |
        //         "See the COPYING file in the top-level directory." |
        //         "This work is licensed under the terms of the GNU GPL, version 2 or later." => {},
        //         v if v.starts_with("Copyright") => {},
        //         v if v.starts_with("Authors") => {},
        //         v if v.starts_with("Markus") => {},
        //         // All bytes in all schema files parse as of 9.1.0 so this should
        //         // only trigger on newer schemas
        //         _ => eprintln!("WARNING: unused comment string ```{v}```"),
        //     }
        // }
    }
}
