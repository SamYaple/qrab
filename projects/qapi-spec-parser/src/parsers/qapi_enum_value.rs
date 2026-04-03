use super::{qstring, take_dict, take_kv};
use crate::{take_cond, take_features};
use crate::{QapiCond, QapiFeatures};
use nom::branch::alt;
use nom::error::{Error, ErrorKind};
use nom::IResult;
use nom::Parser;

pub fn take_enum_value(input: &str) -> IResult<&str, QapiEnumValue<'_>> {
    QapiEnumValue::parse(input)
}

#[derive(Debug, Clone, Default)]
pub struct QapiEnumValue<'i> {
    pub name: &'i str,
    pub r#if: Option<QapiCond<'i>>,
    pub features: Option<QapiFeatures<'i>>,
}

impl<'i> QapiEnumValue<'i> {
    /// ENUM-VALUE = STRING
    ///            | { 'name': STRING,
    ///                '*if': COND,
    ///                '*features': FEATURES }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        let start = input;
        let (input, enumvalue) = alt((Self::simple_parser, Self::complex_parser)).parse(input)?;
        if enumvalue.name.is_empty() {
            return Err(nom::Err::Error(Error::new(start, ErrorKind::Tag)));
        }
        Ok((input, enumvalue))
    }

    /// STRING
    fn simple_parser(input: &'i str) -> IResult<&'i str, Self> {
        let mut s = Self::default();
        let (input, _) = qstring.map(|v| s.name = v).parse(input)?;
        Ok((input, s))
    }

    /// { 'name': STRING,
    ///   '*if': COND,
    ///   '*features': FEATURES }
    fn complex_parser(input: &'i str) -> IResult<&'i str, Self> {
        let mut s = Self::default();
        let (input, _) = take_dict(alt((
            take_kv("name", qstring).map(|v| s.name = v),
            take_features.map(|v| s.features = Some(v)),
            take_cond.map(|v| s.r#if = Some(v)),
        )))
        .parse(input)?;
        Ok((input, s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_string() {
        let (rem, v) = QapiEnumValue::parse("'my-value'").unwrap();
        assert_eq!(rem, "");
        assert_eq!(v.name, "my-value");
        assert!(v.r#if.is_none());
        assert!(v.features.is_none());
    }

    #[test]
    fn test_dict_name_only() {
        let (rem, v) = QapiEnumValue::parse("{ 'name': 'my-value' }").unwrap();
        assert_eq!(rem, "");
        assert_eq!(v.name, "my-value");
        assert!(v.r#if.is_none());
        assert!(v.features.is_none());
    }

    #[test]
    fn test_dict_with_condition() {
        let input = "{ 'name': 'my-value', 'if': 'CONFIG_X' }";
        let (rem, v) = QapiEnumValue::parse(input).unwrap();
        assert_eq!(rem, "");
        assert_eq!(v.name, "my-value");
        assert!(v.r#if.is_some());
    }

    #[test]
    fn test_dict_with_features() {
        let input = "{ 'name': 'my-value', 'features': ['deprecated'] }";
        let (rem, v) = QapiEnumValue::parse(input).unwrap();
        assert_eq!(rem, "");
        assert_eq!(v.name, "my-value");
        assert!(v.features.is_some());
    }

    #[test]
    fn test_dict_with_all_fields() {
        let input = "{ 'name': 'my-value', 'if': 'CONFIG_X', 'features': ['deprecated'] }";
        let (rem, v) = QapiEnumValue::parse(input).unwrap();
        assert_eq!(rem, "");
        assert_eq!(v.name, "my-value");
        assert!(v.r#if.is_some());
        assert!(v.features.is_some());
    }

    #[test]
    fn test_empty_name_fails() {
        assert!(QapiEnumValue::parse("''").is_err());
    }

    #[test]
    fn test_missing_name_in_dict_fails() {
        assert!(QapiEnumValue::parse("{ 'if': 'CONFIG_X' }").is_err());
    }

    #[test]
    fn test_invalid_input_fails() {
        assert!(QapiEnumValue::parse("not_valid").is_err());
    }

    #[test]
    fn test_unknown_field_fails() {
        assert!(QapiEnumValue::parse("{ 'name': 'val', 'unknown': 'x' }").is_err());
    }
}
