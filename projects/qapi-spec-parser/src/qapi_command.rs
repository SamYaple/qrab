use crate::helpers::{qbool, qstring, take_dict, take_kv};
use crate::{take_cond, take_features, take_members};
use crate::{QapiCond, QapiDocumentation, QapiFeatures, QapiMembers, QapiTypeRef};
use nom::branch::alt;
use nom::combinator::{map, opt};
use nom::IResult;

pub fn take_command(input: &str) -> IResult<&str, QapiCommand<'_>> {
    QapiCommand::parse(input)
}

#[derive(Debug, Clone)]
enum QapiCommandData<'i> {
    Ref(&'i str),
    Members(QapiMembers<'i>),
}

#[derive(Debug, Clone, Default)]
pub struct QapiCommand<'i> {
    name: Option<&'i str>,
    data: Option<QapiCommandData<'i>>,
    boxed: Option<&'i str>,
    r#if: Option<QapiCond<'i>>,
    features: Option<QapiFeatures<'i>>,
    returns: Option<QapiTypeRef<'i>>,
    success_response: Option<&'i str>,
    gen: Option<&'i str>,
    allow_oob: Option<&'i str>,
    allow_preconfig: Option<&'i str>,
    coroutine: Option<&'i str>,
}

impl<'i> QapiCommand<'i> {
    /// COMMAND = { 'command': STRING,
    ///             (
    ///             '*data': ( MEMBERS | STRING ),
    ///             |
    ///             'data': STRING,
    ///             'boxed': true,
    ///             )
    ///             '*returns': TYPE-REF,
    ///             '*success-response': false,
    ///             '*gen': false,
    ///             '*allow-oob': true,
    ///             '*allow-preconfig': true,
    ///             '*coroutine': true,
    ///             '*if': COND,
    ///             '*features': FEATURES }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        let (input, doc) = opt(QapiDocumentation::parse)(input)?;
        let mut s = Self::default();
        let (input, _) = take_dict(alt((
            map(take_kv("command", qstring), |v| s.name = Some(v)),
            map(take_kv("boxed", qbool), |v| s.boxed = Some(v)),
            map(take_cond, |v| s.r#if = Some(v)),
            map(take_features, |v| s.features = Some(v)),
            map(take_kv("returns", QapiTypeRef::parse), |v| {
                s.returns = Some(v)
            }),
            map(take_kv("success-response", qbool), |v| {
                s.success_response = Some(v)
            }),
            map(take_kv("gen", qbool), |v| s.gen = Some(v)),
            map(take_kv("allow-oob", qbool), |v| s.allow_oob = Some(v)),
            map(take_kv("coroutine", qbool), |v| s.coroutine = Some(v)),
            map(take_kv("allow-preconfig", qbool), |v| {
                s.allow_preconfig = Some(v)
            }),
            map(
                take_kv(
                    "data",
                    alt((
                        map(qstring, |v| QapiCommandData::Ref(v)),
                        map(take_members, |v| QapiCommandData::Members(v)),
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

    const VALID_INPUTS: [&str; 1] = [r#"{ 'command': 'block-set-write-threshold',
  'data': { 'node-name': 'str', 'write-threshold': 'uint64' },
  'allow-preconfig': true }"#];

    #[test]
    fn test_valid() {
        for input in VALID_INPUTS {
            let result = QapiCommand::parse(input);
            match result {
                Ok((remaining, _)) => {
                    assert_eq!(remaining, "");
                }
                _ => panic!("Failed to parse: ```\n{input}\n```"),
            }
        }
    }
}
