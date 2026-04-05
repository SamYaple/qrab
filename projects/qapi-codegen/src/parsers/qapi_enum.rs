use super::common::{qstring, take_dict, take_kv, take_list};
use super::qapi_cond::take_cond;
use super::qapi_enum_value::take_enum_value;
use super::qapi_features::take_features;
use super::qapi_cond::QapiCond;
use super::qapi_documentation::QapiDocumentation;
use super::qapi_enum_value::QapiEnumValue;
use super::qapi_features::QapiFeatures;
use nom::branch::alt;
use nom::combinator::opt;
use nom::error::{Error, ErrorKind};
use nom::IResult;
use nom::Parser;

pub fn take_enum(input: &str) -> IResult<&str, QapiEnum<'_>> {
    QapiEnum::parse(input)
}

#[derive(Debug, Clone, Default)]
pub struct QapiEnum<'i> {
    pub name: &'i str,
    pub data: Vec<QapiEnumValue<'i>>,
    pub r#if: Option<QapiCond<'i>>,
    pub prefix: Option<&'i str>,
    pub features: Option<QapiFeatures<'i>>,
    pub doc: Option<QapiDocumentation<'i>>,
}

impl<'i> QapiEnum<'i> {
    /// ENUM = { 'enum': STRING,
    ///          'data': [ ENUM-VALUE, ... ],
    ///          '*prefix': STRING,
    ///          '*if': COND,
    ///          '*features': FEATURES }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        Self::parse_v1(input)
    }

    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
        let (input, doc) = opt(QapiDocumentation::parse).parse(input)?;
        let mut s = Self {
            doc,
            ..Default::default()
        };
        let start = input;
        let (input, _) = take_dict(alt((
            take_kv("enum", qstring).map(|v| s.name = v),
            take_kv("prefix", qstring).map(|v| s.prefix = Some(v)),
            take_kv("data", take_list(take_enum_value)).map(|v| s.data = v),
            take_cond.map(|v| s.r#if = Some(v)),
            take_features.map(|v| s.features = Some(v)),
        )))
        .parse(input)?;
        if s.name.is_empty() || s.data.is_empty() {
            return Err(nom::Err::Error(Error::new(start, ErrorKind::Tag)));
        }
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
