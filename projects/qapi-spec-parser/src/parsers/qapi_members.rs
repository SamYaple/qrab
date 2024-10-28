use super::{qstring, take_dict};
use crate::QapiMember;
use nom::branch::alt;
use nom::combinator::map;
use nom::IResult;
use std::ops::{Deref, DerefMut};

pub fn take_members(input: &str) -> IResult<&str, QapiMembers<'_>> {
    QapiMembers::parse(input)
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct QapiMembers<'i>(pub Vec<QapiMember<'i>>);
impl<'i> QapiMembers<'i> {
    /// MEMBERS = { MEMBER, ... }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        map(take_dict(QapiMember::parse), |v| Self(v))(input)
    }
}

impl<'i> Deref for QapiMembers<'i> {
    type Target = Vec<QapiMember<'i>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'i> DerefMut for QapiMembers<'i> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'i> IntoIterator for QapiMembers<'i> {
    type Item = QapiMember<'i>;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

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
            map(qstring, |v| Self::Ref(v)),
            map(take_members, |v| Self::Members(v)),
        ))(input)
    }
}

impl Default for MembersOrRef<'_> {
    fn default() -> Self {
        Self::Unset
    }
}
