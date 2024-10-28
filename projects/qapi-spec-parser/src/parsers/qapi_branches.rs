use super::take_dict;
use crate::QapiBranch;
use nom::combinator::map;
use nom::IResult;
use std::ops::{Deref, DerefMut};

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

impl<'i> Deref for QapiBranches<'i> {
    type Target = Vec<QapiBranch<'i>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'i> DerefMut for QapiBranches<'i> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'i> IntoIterator for QapiBranches<'i> {
    type Item = QapiBranch<'i>;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
