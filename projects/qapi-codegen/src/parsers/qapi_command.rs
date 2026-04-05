use super::common::{qbool, qstring, take_dict, take_kv};
use super::qapi_cond::{take_cond, QapiCond};
use super::qapi_documentation::QapiDocumentation;
use super::qapi_features::{take_features, QapiFeatures};
use super::qapi_members::{take_members_or_ref, MembersOrRef};
use super::qapi_type_ref::QapiTypeRef;
use super::parser_version;
use nom::branch::alt;
use nom::combinator::opt;
use nom::error::{Error, ErrorKind};
use nom::IResult;
use nom::Parser;

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
    pub r#gen: Option<&'i str>,
    pub allow_oob: Option<&'i str>,
    pub allow_preconfig: Option<&'i str>,
    pub coroutine: Option<&'i str>,
    pub doc: Option<QapiDocumentation<'i>>,
}

impl<'i> QapiCommand<'i> {
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        let v = parser_version();
        if v < (2, 4, 0) {
            Self::parse_v1(input)
        } else {
            Self::parse_v2(input)
        }
    }

    /// v1 (QEMU v1.0–v2.3.0): accepts `'gen': 'no'`/`'gen': 'yes'` (string booleans).
    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
        Self::parse_common(input, true)
    }

    /// v2 (QEMU v2.4.0+): only `'gen': false`/`'gen': true` (real booleans).
    pub fn parse_v2(input: &'i str) -> IResult<&'i str, Self> {
        Self::parse_common(input, false)
    }

    fn parse_common(input: &'i str, gen_string_as_bool: bool) -> IResult<&'i str, Self> {
        let (input, doc) = opt(QapiDocumentation::parse).parse(input)?;
        let mut s = Self {
            doc,
            ..Default::default()
        };
        let start = input;
        let (input, _) = take_dict(alt((
            take_kv("command", qstring).map(|v| s.name = v),
            take_kv("boxed", qbool).map(|v| s.boxed = Some(v)),
            take_cond.map(|v| s.r#if = Some(v)),
            take_features.map(|v| s.features = Some(v)),
            take_kv("returns", QapiTypeRef::parse).map(|v| s.returns = Some(v)),
            take_kv("success-response", qbool).map(|v| s.success_response = Some(v)),
            take_kv("gen", move |input| {
                if gen_string_as_bool {
                    alt((qbool, qstring)).parse(input)
                } else {
                    qbool(input)
                }
            }).map(|v| s.r#gen = Some(v)),
            take_kv("allow-oob", qbool).map(|v| s.allow_oob = Some(v)),
            take_kv("coroutine", qbool).map(|v| s.coroutine = Some(v)),
            take_kv("allow-preconfig", qbool).map(|v| s.allow_preconfig = Some(v)),
            take_kv("data", take_members_or_ref).map(|v| s.data = Some(v)),
        )))
        .parse(input)?;
        if s.name.is_empty() {
            return Err(nom::Err::Error(Error::new(start, ErrorKind::Tag)));
        }
        Ok((input, s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::set_parser_version;

    #[test]
    fn test_valid_v2() {
        set_parser_version((10, 2, 0));
        let input = r#"{ 'command': 'block-set-write-threshold',
  'data': { 'node-name': 'str', 'write-threshold': 'uint64' },
  'allow-preconfig': true }"#;
        let (rem, cmd) = QapiCommand::parse(input).unwrap();
        assert_eq!(rem, "");
        assert_eq!(cmd.name, "block-set-write-threshold");
    }

    #[test]
    fn test_gen_string_accepted_in_v1() {
        set_parser_version((1, 1, 0));
        let input = "{ 'command': 'query-name', 'returns': 'NameInfo', 'gen': 'no' }";
        let (rem, cmd) = QapiCommand::parse(input).unwrap();
        assert_eq!(rem, "");
        assert_eq!(cmd.name, "query-name");
        assert_eq!(cmd.r#gen, Some("no"));
    }

    #[test]
    fn test_gen_string_rejected_in_v2() {
        set_parser_version((10, 2, 0));
        let input = "{ 'command': 'test', 'gen': 'no' }";
        let result = QapiCommand::parse(input);
        assert!(result.is_err(), "'gen': 'no' should be rejected in v2");
    }

    #[test]
    fn test_gen_bool_accepted_in_v2() {
        set_parser_version((10, 2, 0));
        let input = "{ 'command': 'test', 'gen': false }";
        let (rem, cmd) = QapiCommand::parse(input).unwrap();
        assert_eq!(rem, "");
        assert_eq!(cmd.r#gen, Some("false"));
    }
}
