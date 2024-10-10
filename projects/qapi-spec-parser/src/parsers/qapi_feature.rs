use crate::helpers::{qstring, take_dict, take_kv};
use crate::{take_cond, QapiCond};
use nom::branch::alt;
use nom::combinator::map;
use nom::IResult;
use nom::error::{Error, ErrorKind};

pub fn take_feature(input: &str) -> IResult<&str, QapiFeature<'_>> {
    QapiFeature::parse(input)
}

#[derive(Debug, Clone, Default)]
pub struct QapiFeature<'i> {
    pub name: &'i str,
    pub r#if: Option<QapiCond<'i>>,
}

impl<'i> QapiFeature<'i> {
    /// FEATURE = STRING
    ///         | { 'name': STRING, '*if': COND }
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
    use nom::Err;

    #[test]
    fn test_string_parser() {
        let input = r#"'feature_name'"#;
        let expected = QapiFeature {
            name: "feature_name",
            r#if: None,
        };
        let (remaining, result) = QapiFeature::string_parser(input).unwrap();
        assert_eq!(remaining, "");
        assert_eq!(result.name, expected.name);
        assert_eq!(result.r#if, expected.r#if);
    }

    #[test]
    fn test_dict_parser() {
        let input = r#"{ 'name': 'feature_name', 'if': 'condition' }"#;
        let expected = QapiFeature {
            name: "feature_name",
            r#if: Some(QapiCond::ConfigName("condition")),
        };
        let (remaining, result) = QapiFeature::dict_parser(input).unwrap();
        assert_eq!(remaining, "");
        assert_eq!(result.name, expected.name);
        assert_eq!(result.r#if.unwrap(), expected.r#if.unwrap());
    }

    #[test]
    fn test_dict_parser_minimal() {
        let input = r#"{'name':'feature_name'}"#;
        let expected = QapiFeature {
            name: "feature_name",
            r#if: None,
        };
        let (remaining, result) = QapiFeature::dict_parser(input).unwrap();
        assert_eq!(remaining, "");
        assert_eq!(result.name, expected.name);
        assert_eq!(result.r#if, expected.r#if);
    }

    #[test]
    fn test_invalid_input() {
        let invalid_inputs = vec![
            r#"invalid_input"#, // missing wrapping single quotes
            r#"'invalid_input"#, // missing trailing single quote
            r#""invalid_input""#, // incorrect double quotes
            r#"{'name': "invalid_input"}"#, // incorrect double quotes
            r#"{'name': 'invalid_input',}"#, // trailing comma
            r#"{'name': 'invalid_input', 'if': CONFIG_NAME}"#, // missing wrapping single quotes
            r#"{'name': 'invalid_input', 'if': 'CONFIG_NAME',}"#, // trailing comma
            r#"{'name': 'invalid_input', 'if': 'CONFIG_NAME', 'other': 'valid'}"#, // unknown field
            r#"{'if': 'CONFIG_NAME'}"#, // missing name field
        ];
        for input in invalid_inputs {
            let result = take_feature(input);
            assert!(result.is_err());
        }
    }
}
