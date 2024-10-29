use super::{qstring, take_dict, take_kv};
use crate::{take_cond, QapiCond};
use nom::branch::alt;
use nom::combinator::map;
use nom::error::{Error, ErrorKind};
use nom::IResult;

/// Convenience wrapper around the QapiFeature parser
pub fn take_feature(input: &str) -> IResult<&str, QapiFeature<'_>> {
    QapiFeature::parse(input)
}

/// Holds information on a specific Feature in a single option
#[derive(Debug, Clone, Default, PartialEq)]
pub struct QapiFeature<'i> {
    pub name: &'i str,
    pub r#if: Option<QapiCond<'i>>,
}

impl<'i> QapiFeature<'i> {
    /// Parses a `FEATURE` as defined in the QAPI spec
    ///     FEATURE = STRING
    ///             | { 'name': STRING, '*if': COND }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        let start = input;
        let (input, feature) = alt((Self::string_parser, Self::dict_parser))(input)?;
        if feature.name == "" {
            return Err(nom::Err::Error(Error::new(start, ErrorKind::Tag)));
        }
        Ok((input, feature))
    }

    /// STRING
    fn string_parser(input: &'i str) -> IResult<&'i str, Self> {
        let mut s = Self::default();
        let (input, _) = map(qstring, |v| s.name = v)(input)?;
        Ok((input, s))
    }

    /// { 'name': STRING, '*if': COND }
    fn dict_parser(input: &'i str) -> IResult<&'i str, Self> {
        let mut s = Self::default();
        let name_parser = map(take_kv("name", qstring), |v| s.name = v);
        let cond_parser = map(take_cond, |v| s.r#if = Some(v));
        let (input, _) = take_dict(alt((name_parser, cond_parser)))(input)?;
        Ok((input, s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_string_feature() {
        let input = "'feature_name'";
        let expected = QapiFeature {
            name: "feature_name",
            r#if: None,
        };
        let (remaining, result) = QapiFeature::parse(input).expect("Parsing failed");
        assert_eq!(remaining, "");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_dict_feature_minimal() {
        let input = "{'name':'feature_name'}";
        let expected = QapiFeature {
            name: "feature_name",
            r#if: None,
        };
        let (remaining, result) = QapiFeature::parse(input).expect("Parsing failed");
        assert_eq!(remaining, "");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_dict_feature_with_if() {
        let input = "{ 'name': 'feature_name', 'if': 'condition' }";
        let expected = QapiFeature {
            name: "feature_name",
            r#if: Some(QapiCond::ConfigName("condition")),
        };
        let (remaining, result) = QapiFeature::parse(input).expect("Parsing failed");
        assert_eq!(remaining, "");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_dict_feature_with_complex_if() {
        let input = "{ 'name': 'feature_name', 'if': { 'not': 'CONFIG_CONDITION' } }";
        let expected = QapiFeature {
            name: "feature_name",
            r#if: Some(QapiCond::Not(Box::new(QapiCond::ConfigName(
                "CONFIG_CONDITION",
            )))),
        };
        let (remaining, result) = QapiFeature::parse(input).expect("Parsing failed");
        assert_eq!(remaining, "");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_dict_feature_with_any_if() {
        let input = "{ 'name': 'feature_name', 'if': { 'any': [ 'CONFIG_A', 'CONFIG_B' ] } }";
        let expected = QapiFeature {
            name: "feature_name",
            r#if: Some(QapiCond::Any(vec![
                QapiCond::ConfigName("CONFIG_A"),
                QapiCond::ConfigName("CONFIG_B"),
            ])),
        };
        let (remaining, result) = QapiFeature::parse(input).expect("Parsing failed");
        assert_eq!(remaining, "");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_dict_feature_with_all_if() {
        let input = "{ 'name': 'feature_name', 'if': { 'all': [ 'CONFIG_A', 'CONFIG_B' ] } }";
        let expected = QapiFeature {
            name: "feature_name",
            r#if: Some(QapiCond::All(vec![
                QapiCond::ConfigName("CONFIG_A"),
                QapiCond::ConfigName("CONFIG_B"),
            ])),
        };
        let (remaining, result) = QapiFeature::parse(input).expect("Parsing failed");
        assert_eq!(remaining, "");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_feature_with_nested_conditions() {
        let input =
            "{ 'name': 'feature_name', 'if': { 'all': [ 'CONFIG_A', { 'not': 'CONFIG_B' } ] } }";
        let expected = QapiFeature {
            name: "feature_name",
            r#if: Some(QapiCond::All(vec![
                QapiCond::ConfigName("CONFIG_A"),
                QapiCond::Not(Box::new(QapiCond::ConfigName("CONFIG_B"))),
            ])),
        };
        let (remaining, result) = QapiFeature::parse(input).expect("Parsing failed");
        assert_eq!(remaining, "");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_feature_with_whitespace() {
        let input = " { 'name' : 'feature_name' , 'if' : 'condition' } ";
        let expected = QapiFeature {
            name: "feature_name",
            r#if: Some(QapiCond::ConfigName("condition")),
        };
        let (remaining, result) = QapiFeature::parse(input).expect("Parsing failed");
        assert_eq!(remaining.trim(), "");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_invalid_missing_name() {
        let input = "{'if':'condition'}";
        let result = QapiFeature::parse(input);
        assert!(
            result.is_err(),
            "Expected parsing to fail due to missing 'name' field"
        );
    }

    #[test]
    fn test_parse_invalid_unknown_field() {
        let input = "{'name':'feature_name','unknown':'value'}";
        let result = QapiFeature::parse(input);
        assert!(
            result.is_err(),
            "Expected parsing to fail due to unknown field"
        );
    }

    #[test]
    fn test_parse_invalid_unexpected_format() {
        let input = "invalid_input";
        let result = QapiFeature::parse(input);
        assert!(
            result.is_err(),
            "Expected parsing to fail due to invalid format"
        );
    }

    #[test]
    fn test_parse_invalid_incorrect_quotes() {
        let inputs = [
            "\"feature_name\"",
            "{'name':\"feature_name\"}",
            "{'name':'feature_name', 'if':\"condition\"}",
        ];
        for input in inputs.iter() {
            let result = QapiFeature::parse(input);
            assert!(
                result.is_err(),
                "Expected parsing to fail for input: {}",
                input
            );
        }
    }

    #[test]
    fn test_parse_invalid_missing_quotes() {
        let inputs = [
            "'feature_name",
            "{'name':'feature_name}",
            "{'name':'feature_name', 'if':'condition}",
        ];
        for input in inputs.iter() {
            let result = QapiFeature::parse(input);
            assert!(
                result.is_err(),
                "Expected parsing to fail due to missing closing quote"
            );
        }
    }

    #[test]
    fn test_parse_invalid_trailing_comma() {
        let inputs = [
            "{'name':'feature_name',}",
            "{'name':'feature_name', 'if':'condition',}",
        ];
        for input in inputs.iter() {
            let result = QapiFeature::parse(input);
            assert!(
                result.is_err(),
                "Expected parsing to fail due to trailing comma"
            );
        }
    }

    #[test]
    fn test_parse_invalid_missing_wrapping_quotes() {
        let inputs = [
            "{'name':feature_name}",
            "{'name':'feature_name', 'if':condition}",
        ];
        for input in inputs.iter() {
            let result = QapiFeature::parse(input);
            assert!(
                result.is_err(),
                "Expected parsing to fail due to missing wrapping quotes"
            );
        }
    }

    #[test]
    fn test_parse_invalid_extra_fields() {
        let input = "{'name':'feature_name', 'if':'condition', 'extra':'value'}";
        let result = QapiFeature::parse(input);
        assert!(
            result.is_err(),
            "Expected parsing to fail due to extra unknown fields"
        );
    }
}
