use super::common::{qstring, qtag};
use super::qapi_members::QapiMembers;
use nom::branch::alt;
use nom::sequence::delimited;
use nom::IResult;
use nom::Parser;

pub fn take_type_ref(input: &str) -> IResult<&str, QapiTypeRef<'_>> {
    QapiTypeRef::parse(input)
}

#[derive(Debug, Clone, PartialEq)]
pub enum QapiTypeRef<'i> {
    Unset,
    Ref(&'i str),
    ArrayRef(&'i str),
    /// Inline anonymous struct definition (v1.0-v2.3.0)
    InlineStruct(QapiMembers<'i>),
}

impl Default for QapiTypeRef<'_> {
    fn default() -> Self {
        Self::Unset
    }
}

impl<'i> QapiTypeRef<'i> {
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        Self::parse_v1(input)
    }

    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
        todo!("qapi_type_ref::parse_v1 — Phase 5")
    }
}
