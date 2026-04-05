use super::common::{qstring, qtag, take_dict, take_kv};
use super::qapi_branches::{take_branches, QapiBranches};
use super::qapi_cond::{take_cond, QapiCond};
use super::qapi_documentation::QapiDocumentation;
use super::qapi_features::{take_features, QapiFeatures};
use super::qapi_members::{take_members_or_ref, MembersOrRef};
use nom::branch::alt;
use nom::combinator::opt;
use nom::error::{Error, ErrorKind};
use nom::sequence::delimited;
use nom::IResult;
use nom::Parser;

pub fn take_union(input: &str) -> IResult<&str, QapiUnion<'_>> {
    QapiUnion::parse(input)
}

#[derive(Debug, Clone, Default)]
pub struct QapiUnion<'i> {
    pub name: &'i str,
    pub data: QapiBranches<'i>,
    pub base: MembersOrRef<'i>,
    /// None for simple unions (v1.x-v6.1.0)
    pub discriminator: Option<&'i str>,
    pub r#if: Option<QapiCond<'i>>,
    pub features: Option<QapiFeatures<'i>>,
    pub doc: Option<QapiDocumentation<'i>>,
}

impl<'i> QapiUnion<'i> {
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        Self::parse_v1(input)
    }

    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
        todo!("qapi_union::parse_v1 — Phase 7")
    }
}
