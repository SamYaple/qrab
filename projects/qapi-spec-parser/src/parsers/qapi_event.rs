use crate::helpers::{qbool, qstring, take_dict, take_kv};
use crate::{take_cond, take_features, take_members_or_ref};
use crate::{MembersOrRef, QapiCond, QapiDocumentation, QapiFeatures};
use nom::branch::alt;
use nom::combinator::{map, opt};
use nom::error::{Error, ErrorKind};
use nom::IResult;

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
        let (input, doc) = opt(QapiDocumentation::parse)(input)?;
        let mut s = Self {
            doc,
            ..Default::default()
        };
        let start = input;
        let (input, _) = take_dict(alt((
            map(take_kv("event", qstring), |v| s.name = v),
            map(take_kv("boxed", qbool), |v| s.boxed = Some(v)),
            map(take_cond, |v| s.r#if = Some(v)),
            map(take_features, |v| s.features = Some(v)),
            map(take_kv("data", take_members_or_ref), |v| s.data = Some(v)),
        )))(input)?;
        if s.name == "" {
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
