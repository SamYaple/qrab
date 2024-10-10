use crate::helpers::{take_kv, take_list};
use crate::QapiFeature;
use nom::combinator::map;
use nom::IResult;

pub fn take_features(input: &str) -> IResult<&str, QapiFeatures<'_>> {
    take_kv("features", QapiFeatures::parse)(input)
}

#[derive(Debug, Clone)]
pub struct QapiFeatures<'i>(pub Vec<QapiFeature<'i>>);
impl<'i> QapiFeatures<'i> {
    /// FEATURES = [ FEATURE, ... ]
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        map(take_list(QapiFeature::parse), |v| Self(v))(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_INPUTS_FEATURES: [&str; 4] = [
        "[{'name':'deprecated'}]",
        "['deprecated']",
        "[{'name':'deprecated','if':'CONFIG_OPTION'}]",
        "[{'name':'deprecated','if':'CONFIG_OPTION'},'testfeature']",
    ];

    #[test]
    fn test_features_valid() {
        for input in VALID_INPUTS_FEATURES {
            let result = QapiFeatures::parse(input);
            match result {
                Ok((remaining, _)) => {
                    assert_eq!(remaining, "");
                }
                _ => panic!("Failed to parse: ```\n{input}\n```"),
            }
        }
    }
    const VALID_INPUTS_FEATURE: [&str; 3] = [
        "{'name':'deprecated'}",
        "'deprecated'",
        "{'name':'deprecated','if':'CONFIG_OPTION'}",
    ];

    #[test]
    fn test_feature_valid() {
        for input in VALID_INPUTS_FEATURE {
            let result = QapiFeature::parse(input);
            match result {
                Ok((remaining, _)) => {
                    assert_eq!(remaining, "");
                }
                _ => panic!("Failed to parse: ```\n{input}\n```"),
            }
        }
    }
}
