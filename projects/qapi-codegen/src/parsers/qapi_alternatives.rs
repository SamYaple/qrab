use super::common::{take_dict, take_kv};
use super::vec_newtype;
use super::qapi_alternative::take_alternative;
use super::qapi_alternative::QapiAlternative;
use nom::IResult;
use nom::Parser;

pub fn take_alternatives(input: &str) -> IResult<&str, QapiAlternatives<'_>> {
    take_kv("data", QapiAlternatives::parse).parse(input)
}

#[derive(Debug, Clone, Default)]
pub struct QapiAlternatives<'i>(pub Vec<QapiAlternative<'i>>);
impl<'i> QapiAlternatives<'i> {
    /// ALTERNATIVES = { ALTERNATIVE, ... }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        Self::parse_v1(input)
    }

    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
        take_dict(take_alternative).map(Self).parse(input)
    }
}
vec_newtype!(QapiAlternatives, QapiAlternative<'i>);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::qapi_type_ref::QapiTypeRef;

    #[test]
    fn test_empty_alternatives() {
        let (rem, a) = QapiAlternatives::parse("{}").unwrap();
        assert_eq!(rem, "");
        assert_eq!(a.len(), 0);
    }

    #[test]
    fn test_single_alternative() {
        let (rem, a) = QapiAlternatives::parse("{ 'str-val': 'str' }").unwrap();
        assert_eq!(rem, "");
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].name, "str-val");
        assert_eq!(a[0].r#type, QapiTypeRef::Ref("str"));
    }

    #[test]
    fn test_multiple_alternatives() {
        let input = "{ 'str-val': 'str', 'int-val': 'int', 'array-val': ['bool'] }";
        let (rem, a) = QapiAlternatives::parse(input).unwrap();
        assert_eq!(rem, "");
        assert_eq!(a.len(), 3);
        assert_eq!(a[2].r#type, QapiTypeRef::ArrayRef("bool"));
    }

    #[test]
    fn test_alternative_with_condition() {
        let input = "{ 'data': { 'type': 'str', 'if': 'CONFIG_X' } }";
        let (rem, a) = QapiAlternatives::parse(input).unwrap();
        assert_eq!(rem, "");
        assert_eq!(a.len(), 1);
        assert!(a[0].r#if.is_some());
    }

    #[test]
    fn test_invalid_input() {
        assert!(QapiAlternatives::parse("not_valid").is_err());
    }
}
