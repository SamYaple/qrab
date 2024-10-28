use super::{take_dict, take_kv};
use crate::take_alternative;
use crate::QapiAlternative;
use nom::combinator::map;
use nom::IResult;
use std::ops::{Deref, DerefMut};

pub fn take_alternatives(input: &str) -> IResult<&str, QapiAlternatives<'_>> {
    take_kv("data", QapiAlternatives::parse)(input)
}

#[derive(Debug, Clone, Default)]
pub struct QapiAlternatives<'i>(pub Vec<QapiAlternative<'i>>);
impl<'i> QapiAlternatives<'i> {
    /// ALTERNATIVES = { ALTERNATIVE, ... }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        map(take_dict(take_alternative), |v| Self(v))(input)
    }
}

impl<'i> Deref for QapiAlternatives<'i> {
    type Target = Vec<QapiAlternative<'i>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'i> DerefMut for QapiAlternatives<'i> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'i> IntoIterator for QapiAlternatives<'i> {
    type Item = QapiAlternative<'i>;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
