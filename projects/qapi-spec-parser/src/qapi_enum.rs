use crate::helpers::{qstring, take_dict, take_kv, take_list};
use crate::{take_cond, take_enum_value, take_features};
use crate::{QapiCond, QapiDocumentation, QapiEnumValue, QapiFeatures};
use nom::branch::alt;
use nom::combinator::{map, opt};
use nom::IResult;

pub fn take_enum(input: &str) -> IResult<&str, QapiEnum<'_>> {
    QapiEnum::parse(input)
}

#[derive(Debug, Clone, Default)]
pub struct QapiEnum<'i> {
    name: Option<&'i str>,
    data: Vec<QapiEnumValue<'i>>,
    r#if: Option<QapiCond<'i>>,
    prefix: Option<&'i str>,
    features: Option<QapiFeatures<'i>>,
}

impl<'i> QapiEnum<'i> {
    /// ENUM = { 'enum': STRING,
    ///          'data': [ ENUM-VALUE, ... ],
    ///          '*prefix': STRING,
    ///          '*if': COND,
    ///          '*features': FEATURES }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        let (input, doc) = opt(QapiDocumentation::parse)(input)?;
        let mut s = Self::default();
        let (input, _) = take_dict(alt((
            map(take_kv("enum", qstring), |v| s.name = Some(v)),
            map(take_kv("prefix", qstring), |v| s.prefix = Some(v)),
            map(take_kv("data", take_list(take_enum_value)), |v| s.data = v),
            map(take_cond, |v| s.r#if = Some(v)),
            map(take_features, |v| s.features = Some(v)),
        )))(input)?;
        Ok((input, s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_INPUTS: [&str; 1] = [
        "{ 'enum': 'Qcow2CompressionType', 'data': [ 'zlib', { 'name': 'zstd', 'if': 'CONFIG_ZSTD' } ] }",
    ];

    #[test]
    fn test_valid() {
        for input in VALID_INPUTS {
            let result = QapiEnum::parse(input);
            match result {
                Ok((remaining, _)) => {
                    assert_eq!(remaining, "");
                }
                _ => panic!("Failed to parse: ```\n{input}\n```"),
            }
        }
    }
}
