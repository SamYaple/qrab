use crate::helpers::{qstring, take_dict, take_kv, take_list};
use crate::QapiCond;
use nom::branch::alt;
use nom::combinator::map;

use nom::IResult;

enum ParserToken<'i> {
    Name(&'i str),
    Cond(QapiCond<'i>),
}

#[derive(Debug, Clone)]
pub struct QapiFeature<'i> {
    name: &'i str,
    cond: Option<QapiCond<'i>>,
}

impl<'i> QapiFeature<'i> {
    /// FEATURE = STRING
    ///         | { 'name': STRING, '*if': COND }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        let name_parser = map(take_kv("name", qstring), |v| ParserToken::Name(v));
        let cond_parser = map(take_kv("if", QapiCond::parse), |v| ParserToken::Cond(v));
        let dict_parser = take_dict(alt((name_parser, cond_parser)));
        let (input, (name, cond)) = alt((
            map(qstring, |name| (Some(name), None)),
            map(dict_parser, |tokens| {
                let mut name = None;
                let mut cond = None;
                for i in tokens {
                    match i {
                        ParserToken::Name(v) => name = Some(v),
                        ParserToken::Cond(v) => cond = Some(v),
                    }
                }
                (name, cond)
            }),
        ))(input)?;
        if name.is_none() {
            todo! {"missing 'name' key, but this should be a nom error not a crash"};
        }
        let name = name.unwrap();
        Ok((input, Self { name, cond }))
    }
}

#[derive(Debug, Clone)]
pub struct QapiFeatures<'i>(Vec<QapiFeature<'i>>);
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
