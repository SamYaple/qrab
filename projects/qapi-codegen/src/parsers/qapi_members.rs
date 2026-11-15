use super::common::QapiString;
use super::common::{qstring, take_dict};
use super::qapi_member::QapiMember;
use super::vec_newtype;
use nom::IResult;
use nom::Parser;
use nom::branch::alt;
use nom::combinator::verify;
use std::collections::HashSet;

pub fn take_members(input: &str) -> IResult<&str, QapiMembers<'_>> {
    QapiMembers::parse(input)
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct QapiMembers<'i>(pub Vec<QapiMember<'i>>);
impl<'i> QapiMembers<'i> {
    /// MEMBERS = { MEMBER, ... }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        Self::parse_v1(input)
    }

    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
        verify(
            take_dict(QapiMember::parse),
            |members: &Vec<QapiMember<'i>>| {
                let mut names = HashSet::new();
                members
                    .iter()
                    .all(|member| names.insert(member.name.as_ref()))
            },
        )
        .map(Self)
        .parse(input)
    }
}
vec_newtype!(QapiMembers, QapiMember<'i>);

pub fn take_members_or_ref(input: &str) -> IResult<&str, MembersOrRef<'_>> {
    MembersOrRef::parse(input)
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum MembersOrRef<'i> {
    #[default]
    Unset,
    Ref(QapiString<'i>),
    Members(QapiMembers<'i>),
}

impl<'i> MembersOrRef<'i> {
    /// STRING | MEMBERS
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        Self::parse_v1(input)
    }

    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
        alt((qstring.map(Self::Ref), take_members.map(Self::Members))).parse(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::qapi_type_ref::QapiTypeRef;

    #[test]
    fn optional_marker_does_not_hide_empty_or_duplicate_member_names() {
        for input in ["{ '*': 'str' }", "{ 'name': 'str', '*name': 'int' }"] {
            assert!(QapiMembers::parse(input).is_err(), "accepted {input}");
        }
    }

    #[test]
    fn test_empty_members() {
        let (rem, m) = QapiMembers::parse("{}").unwrap();
        assert_eq!(rem, "");
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn test_single_member() {
        let (rem, m) = QapiMembers::parse("{ 'name': 'str' }").unwrap();
        assert_eq!(rem, "");
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].name, "name");
        assert_eq!(m[0].r#type, QapiTypeRef::Ref("str".into()));
    }

    #[test]
    fn test_multiple_members() {
        let input = "{ 'name': 'str', 'count': 'int', '*optional-field': 'bool' }";
        let (rem, m) = QapiMembers::parse(input).unwrap();
        assert_eq!(rem, "");
        assert_eq!(m.len(), 3);
        assert!(!m[0].optional);
        assert!(!m[1].optional);
        assert!(m[2].optional);
        assert_eq!(m[2].name, "optional-field");
    }

    #[test]
    fn test_members_or_ref_string() {
        let (rem, mor) = MembersOrRef::parse("'SomeStruct'").unwrap();
        assert_eq!(rem, "");
        assert_eq!(mor, MembersOrRef::Ref("SomeStruct".into()));
    }

    #[test]
    fn test_members_or_ref_members() {
        let (rem, mor) = MembersOrRef::parse("{ 'field': 'str' }").unwrap();
        assert_eq!(rem, "");
        match mor {
            MembersOrRef::Members(m) => assert_eq!(m.len(), 1),
            _ => panic!("expected Members variant"),
        }
    }

    #[test]
    fn test_invalid_members() {
        assert!(QapiMembers::parse("not_valid").is_err());
    }
}
