use super::common::take_dict;
use super::qapi_branch::QapiBranch;
use super::vec_newtype;
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
        Self::parse_v1(input)
    }

    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
        take_dict(QapiBranch::parse).map(Self).parse(input)
    }
}
vec_newtype!(QapiBranches, QapiBranch<'i>);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::qapi_type_ref::QapiTypeRef;

    #[test]
    fn test_empty_branches() {
        let (rem, b) = QapiBranches::parse("{}").unwrap();
        assert_eq!(rem, "");
        assert_eq!(b.len(), 0);
    }

    #[test]
    fn test_single_branch() {
        let (rem, b) = QapiBranches::parse("{ 'variant-a': 'TypeA' }").unwrap();
        assert_eq!(rem, "");
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].name, "variant-a");
        assert_eq!(b[0].r#type, QapiTypeRef::Ref("TypeA".into()));
    }

    #[test]
    fn test_multiple_branches() {
        let input = "{ 'a': 'TypeA', 'b': ['TypeB'], 'c': 'TypeC' }";
        let (rem, b) = QapiBranches::parse(input).unwrap();
        assert_eq!(rem, "");
        assert_eq!(b.len(), 3);
        assert_eq!(b[1].r#type, QapiTypeRef::ArrayRef("TypeB".into()));
    }

    #[test]
    fn test_branch_with_condition() {
        let input = "{ 'x': { 'type': 'TypeX', 'if': 'CONFIG_X' } }";
        let (rem, b) = QapiBranches::parse(input).unwrap();
        assert_eq!(rem, "");
        assert_eq!(b.len(), 1);
        assert!(b[0].r#if.is_some());
    }

    #[test]
    fn test_invalid_input() {
        assert!(QapiBranches::parse("not_valid").is_err());
    }
}
