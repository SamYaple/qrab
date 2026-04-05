use super::common::{qstring, take_dict, take_kv, take_list};
use nom::IResult;
use nom::Parser;

pub fn take_pragma(input: &str) -> IResult<&str, QapiPragma<'_>> {
    QapiPragma::parse(input)
}

#[derive(Debug, Clone, Default)]
pub struct QapiPragma<'i> {
    pub doc_required: bool,
    pub command_returns_exceptions: Vec<&'i str>,
    pub member_name_exceptions: Vec<&'i str>,
    pub documentation_exceptions: Vec<&'i str>,
}

impl<'i> QapiPragma<'i> {
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        Self::parse_v1(input)
    }

    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
        todo!("qapi_pragma::parse_v1 — Phase 6")
    }
}
