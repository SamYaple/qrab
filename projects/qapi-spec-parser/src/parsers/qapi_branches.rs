use crate::helpers::take_dict;
use crate::QapiBranch;
use nom::combinator::map;
use nom::IResult;

pub fn take_branches(input: &str) -> IResult<&str, QapiBranches<'_>> {
    QapiBranches::parse(input)
}

#[derive(Debug, Clone, Default)]
pub struct QapiBranches<'i>(pub Vec<QapiBranch<'i>>);
impl<'i> QapiBranches<'i> {
    /// BRANCHES = { BRANCH, ... }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        map(take_dict(QapiBranch::parse), |v| Self(v))(input)
    }
}
