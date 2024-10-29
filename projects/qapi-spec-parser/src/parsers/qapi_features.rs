use super::{take_kv, take_list};
use crate::QapiFeature;
use nom::combinator::map;
use nom::IResult;
use std::ops::{Deref, DerefMut};

/// Convenience wrapper for taking a list of features
pub fn take_features(input: &str) -> IResult<&str, QapiFeatures<'_>> {
    take_kv("features", QapiFeatures::parse)(input)
}

/// Holds a list of `QapiFeature`
#[derive(Debug, Clone, PartialEq)]
pub struct QapiFeatures<'i>(pub Vec<QapiFeature<'i>>);
impl<'i> QapiFeatures<'i> {
    /// Parses a `FEATURES` as defined in the QAPI spec:
    ///     FEATURES = [ FEATURE, ... ]
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        map(take_list(QapiFeature::parse), |v| Self(v))(input)
    }
}

impl<'i> Deref for QapiFeatures<'i> {
    type Target = Vec<QapiFeature<'i>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'i> DerefMut for QapiFeatures<'i> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'i> IntoIterator for QapiFeatures<'i> {
    type Item = QapiFeature<'i>;
    type IntoIter = std::vec::IntoIter<Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::QapiCond;

    #[test]
    fn test_parse_features_single_dict() {
        let input = "[{'name':'deprecated'}]";
        let expected = QapiFeatures(vec![QapiFeature {
            name: "deprecated",
            r#if: None,
        }]);
        let (remaining, result) = QapiFeatures::parse(input).expect("Parsing failed");
        assert_eq!(remaining, "");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_features_single_string() {
        let input = "['deprecated']";
        let expected = QapiFeatures(vec![QapiFeature {
            name: "deprecated",
            r#if: None,
        }]);
        let (remaining, result) = QapiFeatures::parse(input).expect("Parsing failed");
        assert_eq!(remaining, "");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_features_with_if() {
        let input = "[{'name':'deprecated','if':'CONFIG_OPTION'}]";
        let expected = QapiFeatures(vec![QapiFeature {
            name: "deprecated",
            r#if: Some(QapiCond::ConfigName("CONFIG_OPTION")),
        }]);
        let (remaining, result) = QapiFeatures::parse(input).expect("Parsing failed");
        assert_eq!(remaining, "");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_features_mixed() {
        let input = "[{'name':'deprecated','if':'CONFIG_OPTION'},'testfeature']";
        let expected = QapiFeatures(vec![
            QapiFeature {
                name: "deprecated",
                r#if: Some(QapiCond::ConfigName("CONFIG_OPTION")),
            },
            QapiFeature {
                name: "testfeature",
                r#if: None,
            },
        ]);
        let (remaining, result) = QapiFeatures::parse(input).expect("Parsing failed");
        assert_eq!(remaining, "");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_feature_dict() {
        let input = "{'name':'deprecated'}";
        let expected = QapiFeature {
            name: "deprecated",
            r#if: None,
        };
        let (remaining, result) = QapiFeature::parse(input).expect("Parsing failed");
        assert_eq!(remaining, "");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_feature_string() {
        let input = "'deprecated'";
        let expected = QapiFeature {
            name: "deprecated",
            r#if: None,
        };
        let (remaining, result) = QapiFeature::parse(input).expect("Parsing failed");
        assert_eq!(remaining, "");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_feature_with_if() {
        let input = "{'name':'deprecated','if':'CONFIG_OPTION'}";
        let expected = QapiFeature {
            name: "deprecated",
            r#if: Some(QapiCond::ConfigName("CONFIG_OPTION")),
        };
        let (remaining, result) = QapiFeature::parse(input).expect("Parsing failed");
        assert_eq!(remaining, "");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_features_invalid() {
        let invalid_inputs = [
            "",                                              // Empty input
            "[",                                             // Incomplete list
            "[{'name':'deprecated'}",                        // Missing closing bracket
            "['deprecated'",                                 // Missing closing quote
            "[{'name':'deprecated','if':}]",                 // Missing condition value
            "[{'name':'deprecated','unknown':'value'}]",     // Unknown field
            "[{'name':'deprecated','if':'CONFIG_OPTION',}]", // Trailing comma
            "[{'if':'CONFIG_OPTION'}]",                      // Missing 'name' field
        ];
        for input in invalid_inputs.iter() {
            let result = QapiFeatures::parse(input);
            assert!(
                result.is_err(),
                "Expected parsing to fail for input: {}",
                input
            );
        }
    }

    #[test]
    fn test_parse_feature_invalid() {
        let invalid_inputs = [
            "",                                            // Empty input
            "{'name':'deprecated'",                        // Missing closing brace
            "'deprecated",                                 // Missing closing quote
            "{'name':'deprecated','if':}",                 // Missing condition value
            "{'name':'deprecated','unknown':'value'}",     // Unknown field
            "{'if':'CONFIG_OPTION'}",                      // Missing 'name' field
            "{'name':'deprecated',}",                      // Trailing comma
            "{'name':'deprecated','if':'CONFIG_OPTION',}", // Trailing comma
        ];
        for input in invalid_inputs.iter() {
            let result = QapiFeature::parse(input);
            assert!(
                result.is_err(),
                "Expected parsing to fail for input: {}",
                input
            );
        }
    }
}
