use super::{qbool, qstring, take_dict, take_kv, with_parser_config};
use crate::{take_cond, take_features, take_members_or_ref};
use crate::{MembersOrRef, QapiCond, QapiDocumentation, QapiFeatures, QapiTypeRef};
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
            take_kv("returns", QapiTypeRef::parse).map(|v| {
                s.returns = Some(v)
            }),
            take_kv("success-response", qbool).map(|v| {
                s.success_response = Some(v)
            }),
            take_kv("gen", |input| {
                // Accept 'gen': 'no'/'yes' (string) in older schemas alongside true/false
                if with_parser_config(|c| c.gen_string_as_bool) {
                    nom::branch::alt((qbool, qstring)).parse(input)
                } else {
                    qbool(input)
                }
            }).map(|v| s.r#gen = Some(v)),
            take_kv("allow-oob", qbool).map(|v| s.allow_oob = Some(v)),
            take_kv("coroutine", qbool).map(|v| s.coroutine = Some(v)),
            take_kv("allow-preconfig", qbool).map(|v| {
                s.allow_preconfig = Some(v)
            }),
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

    #[test]
    fn test_gen_string_as_bool() {
        use crate::parsers::{ParserConfig, set_parser_config};
        set_parser_config(ParserConfig::for_version((1, 1, 0)));
        let input = "{ 'command': 'query-name', 'returns': 'NameInfo', 'gen': 'no' }";
        let result = QapiCommand::parse(input);
        set_parser_config(ParserConfig::modern());
        match result {
            Ok((remaining, cmd)) => {
                assert_eq!(remaining, "");
                assert_eq!(cmd.name, "query-name");
                assert_eq!(cmd.r#gen, Some("no"));
            }
            Err(e) => panic!("gen string should be accepted in old mode: {}", e),
        }
    }

    #[test]
    fn test_gen_string_rejected_in_modern() {
        use crate::parsers::{ParserConfig, set_parser_config};
        set_parser_config(ParserConfig::modern());
        let input = "{ 'command': 'test', 'gen': 'no' }";
        let result = QapiCommand::parse(input);
        assert!(result.is_err(), "'gen': 'no' should be rejected in modern mode");
    }
}
