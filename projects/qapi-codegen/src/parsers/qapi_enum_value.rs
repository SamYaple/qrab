use super::common::QapiString;
use super::common::{qstring, take_dict, take_kv};
use super::qapi_cond::QapiCond;
use super::qapi_cond::take_cond;
use super::qapi_features::QapiFeatures;
use super::qapi_features::take_features;
use nom::IResult;
use nom::Parser;
use nom::branch::alt;
use nom::error::{Error, ErrorKind};

pub fn take_enum_value(input: &str) -> IResult<&str, QapiEnumValue<'_>> {
    QapiEnumValue::parse(input)
}

#[derive(Debug, Clone, Default)]
pub struct QapiEnumValue<'i> {
    pub name: QapiString<'i>,
    pub r#if: Option<QapiCond<'i>>,
    pub features: Option<QapiFeatures<'i>>,
}

impl<'i> QapiEnumValue<'i> {
    /// ENUM-VALUE = STRING
    ///            | { 'name': STRING,
    ///                '*if': COND,
    ///                '*features': FEATURES }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        Self::parse_v1(input)
    }

    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
        let start = input;
        let (input, (enumvalue, simple)) = alt((
            Self::simple_parser.map(|value| (value, true)),
            Self::complex_parser.map(|value| (value, false)),
        ))
        .parse(input)?;
        if enumvalue.name.is_empty() && (!simple || super::parser_version() >= (2, 6, 0)) {
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
        crate::set_parser_version((11, 1, 3));
        assert!(QapiEnumValue::parse("''").is_err());
    }

    #[test]
    fn legacy_empty_enum_values_do_not_allow_missing_dictionary_names() {
        crate::set_parser_version((2, 5, 0));
        assert_eq!(QapiEnumValue::parse("''").unwrap().1.name, "");
        assert!(QapiEnumValue::parse("{}").is_err());
        crate::set_parser_version((2, 6, 0));
        assert!(QapiEnumValue::parse("''").is_err());
        crate::set_parser_version((11, 1, 3));
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
