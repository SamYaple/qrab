use super::{qstring, take_dict, vec_newtype};
use crate::QapiMember;
use nom::branch::alt;
use nom::IResult;
use nom::Parser;

pub fn take_members(input: &str) -> IResult<&str, QapiMembers<'_>> {
    QapiMembers::parse(input)
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct QapiMembers<'i>(pub Vec<QapiMember<'i>>);
impl<'i> QapiMembers<'i> {
    /// MEMBERS = { MEMBER, ... }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        take_dict(QapiMember::parse).map(|v| Self(v)).parse(input)
    }
}
vec_newtype!(QapiMembers, QapiMember<'i>);

pub fn take_members_or_ref(input: &str) -> IResult<&str, MembersOrRef<'_>> {
    MembersOrRef::parse(input)
}

#[derive(Debug, Clone, PartialEq)]
pub enum MembersOrRef<'i> {
    Unset,
    Ref(&'i str),
    Members(QapiMembers<'i>),
}

impl<'i> MembersOrRef<'i> {
    /// STRING | MEMBERS
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        alt((
            qstring.map(|v| Self::Ref(v)),
            take_members.map(|v| Self::Members(v)),
        ))
        .parse(input)
    }
}

impl Default for MembersOrRef<'_> {
    fn default() -> Self {
        Self::Unset
    }
}
