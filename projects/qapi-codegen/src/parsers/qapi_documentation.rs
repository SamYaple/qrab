use super::common::{qstring, qtag, take_kv};
use nom::branch::alt;
use nom::bytes::complete::{is_a, tag, tag_no_case, take_until, take_while1};
use nom::character::complete::{line_ending, multispace0, not_line_ending, space0};
use nom::combinator::{not, opt, peek, recognize};
use nom::error::Error;
use nom::multi::{many0, many1};
use nom::sequence::{delimited, preceded, terminated};
use nom::IResult;
use nom::Parser;

pub fn extract_since_from_comment(desc: &Vec<&str>) -> Option<String> {
    todo!("extract_since_from_comment — Phase 8")
}

#[derive(Debug, Clone)]
pub struct QapiDocumentation<'i> {
    pub name: &'i str,
    pub description: Vec<&'i str>,
    pub fields: Vec<(&'i str, Vec<&'i str>)>,
    pub features: Vec<(&'i str, Vec<&'i str>)>,
    pub since: Option<&'i str>,
    pub returns: Vec<&'i str>,
    pub errors: Vec<&'i str>,
    pub notes: Vec<&'i str>,
    pub examples: Vec<&'i str>,
    pub todos: Vec<&'i str>,
}

impl<'i> QapiDocumentation<'i> {
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        Self::parse_v1(input)
    }

    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
        todo!("QapiDocumentation::parse_v1 — Phase 8")
    }
}

#[derive(Debug, Clone)]
pub struct QapiSectionDocumentation<'i> {
    pub name: &'i str,
    pub description: Vec<&'i str>,
}

impl<'i> QapiSectionDocumentation<'i> {
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        Self::parse_v1(input)
    }

    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
        todo!("QapiSectionDocumentation::parse_v1 — Phase 8")
    }
}

#[derive(Debug, Clone)]
pub struct QapiFreeformDoc<'i> {
    pub text: &'i str,
}

impl<'i> QapiFreeformDoc<'i> {
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        Self::parse_v1(input)
    }

    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
        todo!("QapiFreeformDoc::parse_v1 — Phase 8")
    }
}
