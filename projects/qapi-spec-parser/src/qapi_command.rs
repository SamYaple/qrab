use crate::helpers::{kv, qtag};
use crate::{QapiBool, QapiCond, QapiFeatures, QapiMembers, QapiString, QapiTypeRef};
use nom::branch::alt;
use nom::combinator::map;
use nom::multi::separated_list1;
use nom::sequence::delimited;
use nom::IResult;

enum ParserKey {
    Name(QapiString),
    Data(QapiCommandData),
    If(QapiCond),
    Features(QapiFeatures),
    Boxed(QapiBool),
    Returns(QapiTypeRef),
    SuccessResponse(QapiBool),
    Gen(QapiBool),
    AllowOob(QapiBool),
    AllowPreconfig(QapiBool),
    Coroutine(QapiBool),
}

#[derive(Debug)]
enum QapiCommandData {
    Ref(QapiString),
    Members(QapiMembers),
}

#[derive(Debug)]
pub struct QapiCommand {
    name: QapiString,
    data: Option<QapiCommandData>,
    boxed: Option<QapiBool>,
    r#if: Option<QapiCond>,
    features: Option<QapiFeatures>,
    returns: Option<QapiTypeRef>,
    success_response: Option<QapiBool>,
    gen: Option<QapiBool>,
    allow_oob: Option<QapiBool>,
    allow_preconfig: Option<QapiBool>,
    coroutine: Option<QapiBool>,
}

impl QapiCommand {
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
    pub fn parse(input: &str) -> IResult<&str, Self> {
        let boxed_parser = map(kv(qtag("boxed"), QapiBool::parse), |v| ParserKey::Boxed(v));
        let cond_parser = map(kv(qtag("if"), QapiCond::parse), |v| ParserKey::If(v));
        let features_parser = map(kv(qtag("features"), QapiFeatures::parse), |v| {
            ParserKey::Features(v)
        });
        let name_parser = map(kv(qtag("command"), QapiString::parse), |v| {
            ParserKey::Name(v)
        });
        let data_parser = map(
            kv(
                qtag("data"),
                alt((
                    map(QapiString::parse, |v| QapiCommandData::Ref(v)),
                    map(QapiMembers::parse, |v| QapiCommandData::Members(v)),
                )),
            ),
            |v| ParserKey::Data(v),
        );
        let returns_parser = map(kv(qtag("returns"), QapiTypeRef::parse), |v| {
            ParserKey::Returns(v)
        });
        let success_response_parser = map(kv(qtag("success-response"), QapiBool::parse), |v| {
            ParserKey::SuccessResponse(v)
        });
        let gen_parser = map(kv(qtag("gen"), QapiBool::parse), |v| ParserKey::Gen(v));
        let allow_oob_parser = map(kv(qtag("allow-oob"), QapiBool::parse), |v| {
            ParserKey::AllowOob(v)
        });
        let allow_preconfig_parser = map(kv(qtag("allow-preconfig"), QapiBool::parse), |v| {
            ParserKey::AllowPreconfig(v)
        });
        let coroutine_parser = map(kv(qtag("coroutine"), QapiBool::parse), |v| {
            ParserKey::Coroutine(v)
        });

        let parsers = alt((
            data_parser,
            cond_parser,
            features_parser,
            name_parser,
            boxed_parser,
            returns_parser,
            success_response_parser,
            gen_parser,
            allow_oob_parser,
            allow_preconfig_parser,
            coroutine_parser,
        ));
        delimited(
            qtag("{"),
            map(separated_list1(qtag(","), parsers), |tokens| {
                let mut r#if = None;
                let mut data = None;
                let mut features = None;
                let mut boxed = None;
                let mut name = None;
                let mut returns = None;
                let mut success_response = None;
                let mut gen = None;
                let mut allow_oob = None;
                let mut allow_preconfig = None;
                let mut coroutine = None;
                for i in tokens {
                    match i {
                        ParserKey::If(v) => r#if = Some(v),
                        ParserKey::Boxed(v) => boxed = Some(v),
                        ParserKey::Data(v) => data = Some(v),
                        ParserKey::Name(v) => name = Some(v),
                        ParserKey::Features(v) => features = Some(v),
                        ParserKey::Returns(v) => returns = Some(v),
                        ParserKey::SuccessResponse(v) => success_response = Some(v),
                        ParserKey::Gen(v) => gen = Some(v),
                        ParserKey::AllowOob(v) => allow_oob = Some(v),
                        ParserKey::AllowPreconfig(v) => allow_preconfig = Some(v),
                        ParserKey::Coroutine(v) => coroutine = Some(v),
                    }
                }
                let name = name.expect("struct is a required key");
                // TODO This is a validation check, not a parsing check
                //if let Some(ref b) = boxed {
                //    if b.0 && data.is_none() {
                //        // TODO Proper parser error returns, but not now...
                //        panic!("data is a required key");
                //    }
                //}
                Self {
                    name,
                    r#if,
                    features,
                    data,
                    boxed,
                    returns,
                    success_response,
                    gen,
                    allow_oob,
                    allow_preconfig,
                    coroutine,
                }
            }),
            qtag("}"),
        )(input)
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