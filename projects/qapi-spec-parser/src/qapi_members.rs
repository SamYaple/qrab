use crate::helpers::take_dict;
use crate::QapiMember;
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
