use crate::helpers::{qstring, take_dict};
use crate::QapiMember;
use nom::branch::alt;
use nom::combinator::map;
use nom::IResult;

pub fn take_members(input: &str) -> IResult<&str, QapiMembers<'_>> {
    QapiMembers::parse(input)
}

#[derive(Debug, Clone, Default)]
pub struct QapiMembers<'i>(pub Vec<QapiMember<'i>>);
impl<'i> QapiMembers<'i> {
    /// MEMBERS = { MEMBER, ... }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        map(take_dict(QapiMember::parse), |v| Self(v))(input)
    }
}

pub fn take_members_or_ref(input: &str) -> IResult<&str, MembersOrRef<'_>> {
    MembersOrRef::parse(input)
}

#[derive(Debug, Clone)]
pub enum MembersOrRef<'i> {
    Ref(&'i str),
    Members(QapiMembers<'i>),
}
impl<'i> MembersOrRef<'i> {
    /// STRING | MEMBERS
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        alt((
            map(qstring, |v| Self::Ref(v)),
            map(take_members, |v| Self::Members(v)),
        ))(input)
    }
}
