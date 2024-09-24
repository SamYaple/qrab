use crate::helpers::{qbool, qstring, take_dict, take_kv};
use crate::{take_cond, take_features, take_members};
use crate::{QapiCond, QapiFeatures, QapiMembers};
use nom::branch::alt;
use nom::combinator::map;
use nom::IResult;

pub fn take_event(input: &str) -> IResult<&str, QapiEvent<'_>> {
    QapiEvent::parse(input)
}

#[derive(Debug, Clone)]
enum QapiEventData<'i> {
    Ref(&'i str),
    Members(QapiMembers<'i>),
}

#[derive(Debug, Clone, Default)]
pub struct QapiEvent<'i> {
    name: Option<&'i str>,
    data: Option<QapiEventData<'i>>,
    boxed: Option<&'i str>,
    r#if: Option<QapiCond<'i>>,
    features: Option<QapiFeatures<'i>>,
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
        let mut s = Self::default();
        let (input, _) = take_dict(alt((
            map(take_kv("event", qstring), |v| s.name = Some(v)),
            map(take_kv("boxed", qbool), |v| s.boxed = Some(v)),
            map(take_cond, |v| s.r#if = Some(v)),
            map(take_features, |v| s.features = Some(v)),
            map(
                take_kv(
                    "data",
                    alt((
                        map(qstring, |v| QapiEventData::Ref(v)),
                        map(take_members, |v| QapiEventData::Members(v)),
                    )),
                ),
                |v| s.data = Some(v),
            ),
        )))(input)?;
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
