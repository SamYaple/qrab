use super::{take_dict, vec_newtype};
use crate::QapiBranch;
use nom::IResult;
use nom::Parser;

pub fn take_branches(input: &str) -> IResult<&str, QapiBranches<'_>> {
    QapiBranches::parse(input)
}

#[derive(Debug, Clone, Default)]
pub struct QapiBranches<'i>(pub Vec<QapiBranch<'i>>);
impl<'i> QapiBranches<'i> {
    /// BRANCHES = { BRANCH, ... }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        take_dict(QapiBranch::parse).map(|v| Self(v)).parse(input)
    }
}
vec_newtype!(QapiBranches, QapiBranch<'i>);
