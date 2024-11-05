use super::{qbool, qstring, take_dict, take_kv};
use crate::{take_cond, take_features, take_members_or_ref};
use crate::{MembersOrRef, QapiCond, QapiDocumentation, QapiFeatures, QapiTypeRef};
use nom::branch::alt;
use nom::combinator::{map, opt};
use nom::error::{Error, ErrorKind};
use nom::IResult;

pub fn take_command(input: &str) -> IResult<&str, QapiCommand<'_>> {
    QapiCommand::parse(input)
}

#[derive(Debug, Clone, Default)]
pub struct QapiCommand<'i> {
    pub name: &'i str,
    pub data: Option<MembersOrRef<'i>>,
    pub boxed: Option<&'i str>,
    pub r#if: Option<QapiCond<'i>>,
    pub features: Option<QapiFeatures<'i>>,
    pub returns: Option<QapiTypeRef<'i>>,
    pub success_response: Option<&'i str>,
    pub gen: Option<&'i str>,
    pub allow_oob: Option<&'i str>,
    pub allow_preconfig: Option<&'i str>,
    pub coroutine: Option<&'i str>,
    pub doc: Option<QapiDocumentation<'i>>,
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
        let mut s = Self {
            doc,
            ..Default::default()
        };
        let start = input;
        let (input, _) = take_dict(alt((
            map(take_kv("command", qstring), |v| s.name = v),
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
