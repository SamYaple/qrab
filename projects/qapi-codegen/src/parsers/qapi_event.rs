use super::common::{qbool, qstring, take_dict, take_kv};
use super::qapi_cond::QapiCond;
use super::qapi_cond::take_cond;
use super::qapi_documentation::QapiDocumentation;
use super::qapi_features::QapiFeatures;
use super::qapi_features::take_features;
use super::qapi_members::MembersOrRef;
use super::qapi_members::take_members_or_ref;
use nom::IResult;
use nom::Parser;
use nom::branch::alt;
use nom::combinator::opt;
use nom::error::{Error, ErrorKind};

pub fn take_event(input: &str) -> IResult<&str, QapiEvent<'_>> {
    QapiEvent::parse(input)
}

#[derive(Debug, Clone, Default)]
pub struct QapiEvent<'i> {
    pub name: &'i str,
    pub data: Option<MembersOrRef<'i>>,
    pub boxed: Option<&'i str>,
    pub r#if: Option<QapiCond<'i>>,
    pub features: Option<QapiFeatures<'i>>,
    pub doc: Option<QapiDocumentation<'i>>,
}

impl<'i> QapiEvent<'i> {
    /// EVENT = { 'event': STRING,
    ///           (
    ///           '*data': ( MEMBERS | STRING ),
    ///           |
    ///           'data': STRING,
    ///           'boxed': true,
    ///           )
    ///           '*if': COND,
    ///           '*features': FEATURES }
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
            take_kv("event", qstring).map(|v| s.name = v),
            take_kv("boxed", qbool).map(|v| s.boxed = Some(v)),
            take_cond.map(|v| s.r#if = Some(v)),
            take_features.map(|v| s.features = Some(v)),
            take_kv("data", take_members_or_ref).map(|v| s.data = Some(v)),
        )))
        .parse(input)?;
        if s.name.is_empty() {
            return Err(nom::Err::Error(Error::new(start, ErrorKind::Tag)));
        }
        // if boxed is set to true, `data` is no optional. I do no account for this at all.
        // the qapi spec needs to be correct here, data being equal to None is allowed for us.
        Ok((input, s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_INPUTS: [&str; 1] = [r#"{ 'event': 'BLOCK_IMAGE_CORRUPTED',
  'data': { 'device'     : 'str',
            '*node-name' : 'str',
            'msg'        : 'str',
            '*offset'    : 'int',
            '*size'      : 'int',
            'fatal'      : 'bool' } }"#];

    #[test]
    fn test_valid() {
        for input in VALID_INPUTS {
            let result = QapiEvent::parse(input);
            match result {
                Ok((remaining, _)) => {
                    assert_eq!(remaining, "");
                }
                _ => panic!("Failed to parse: ```\n{input}\n```"),
            }
        }
    }
}
