use super::common::{qstring, take_dict, take_kv};
use super::qapi_cond::{take_cond, QapiCond};
use super::qapi_documentation::QapiDocumentation;
use super::qapi_features::{take_features, QapiFeatures};
use super::qapi_members::{take_members, QapiMembers};
use nom::branch::alt;
use nom::combinator::opt;
use nom::error::{Error, ErrorKind};
use nom::IResult;
use nom::Parser;

pub fn take_struct(input: &str) -> IResult<&str, QapiStruct<'_>> {
    QapiStruct::parse(input)
}

#[derive(Debug, Clone, Default)]
pub struct QapiStruct<'i> {
    pub name: &'i str,
    pub data: QapiMembers<'i>,
    pub base: Option<&'i str>,
    pub r#if: Option<QapiCond<'i>>,
    pub features: Option<QapiFeatures<'i>>,
    pub doc: Option<QapiDocumentation<'i>>,
}

impl<'i> QapiStruct<'i> {
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        Self::parse_v1(input)
    }

    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
        todo!("qapi_struct::parse_v1 — Phase 3")
    }
}
