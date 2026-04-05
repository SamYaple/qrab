use super::common::{qbool, qstring, take_dict, take_kv};
use super::qapi_cond::{take_cond, QapiCond};
use super::qapi_documentation::QapiDocumentation;
use super::qapi_features::{take_features, QapiFeatures};
use super::qapi_members::{take_members_or_ref, MembersOrRef};
use super::qapi_type_ref::QapiTypeRef;
use nom::branch::alt;
use nom::combinator::opt;
use nom::error::{Error, ErrorKind};
use nom::IResult;
use nom::Parser;

pub fn take_command(input: &str) -> IResult<&str, QapiCommand<'_>> {
    QapiCommand::parse(input)
}

#[derive(Debug, Clone, Default)]
pub struct QapiCommand<'i> {
    pub name: &'i str,
    pub data: MembersOrRef<'i>,
    pub returns: Option<QapiTypeRef<'i>>,
    pub r#gen: bool,
    pub allow_oob: bool,
    pub allow_preconfig: bool,
    pub coroutine: bool,
    pub r#if: Option<QapiCond<'i>>,
    pub features: Option<QapiFeatures<'i>>,
    pub doc: Option<QapiDocumentation<'i>>,
    pub success_response: bool,
}

impl<'i> QapiCommand<'i> {
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        Self::parse_v1(input)
    }

    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
        todo!("qapi_command::parse_v1 — Phase 4")
    }
}
