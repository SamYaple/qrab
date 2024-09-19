use crate::helpers::{kv, qbool, qstring, qtag};
use crate::{QapiCond, QapiFeatures, QapiMembers, QapiTypeRef};
use nom::branch::alt;
use nom::combinator::map;
use nom::multi::separated_list1;
use nom::sequence::delimited;
use nom::IResult;

enum ParserToken<'i> {
    Name(&'i str),
    Data(QapiCommandData<'i>),
    If(QapiCond<'i>),
    Features(QapiFeatures<'i>),
    Boxed(&'i str),
    Returns(QapiTypeRef<'i>),
    SuccessResponse(&'i str),
    Gen(&'i str),
    AllowOob(&'i str),
    AllowPreconfig(&'i str),
    Coroutine(&'i str),
}

#[derive(Debug, Clone)]
enum QapiCommandData<'i> {
    Ref(&'i str),
    Members(QapiMembers<'i>),
}

#[derive(Debug, Clone)]
pub struct QapiCommand<'i> {
    name: &'i str,
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
        let boxed_parser = map(kv(qtag("boxed"), qbool), |v| ParserToken::Boxed(v));
        let cond_parser = map(kv(qtag("if"), QapiCond::parse), |v| ParserToken::If(v));
        let features_parser = map(kv(qtag("features"), QapiFeatures::parse), |v| {
            ParserToken::Features(v)
        });
        let name_parser = map(kv(qtag("command"), qstring), |v| ParserToken::Name(v));
        let data_parser = map(
            kv(
                qtag("data"),
                alt((
                    map(qstring, |v| QapiCommandData::Ref(v)),
                    map(QapiMembers::parse, |v| QapiCommandData::Members(v)),
                )),
            ),
            |v| ParserToken::Data(v),
        );
        let returns_parser = map(kv(qtag("returns"), QapiTypeRef::parse), |v| {
            ParserToken::Returns(v)
        });
        let success_response_parser = map(kv(qtag("success-response"), qbool), |v| {
            ParserToken::SuccessResponse(v)
        });
        let gen_parser = map(kv(qtag("gen"), qbool), |v| ParserToken::Gen(v));
        let allow_oob_parser = map(kv(qtag("allow-oob"), qbool), |v| ParserToken::AllowOob(v));
        let allow_preconfig_parser = map(kv(qtag("allow-preconfig"), qbool), |v| {
            ParserToken::AllowPreconfig(v)
        });
        let coroutine_parser = map(kv(qtag("coroutine"), qbool), |v| ParserToken::Coroutine(v));

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
                        ParserToken::If(v) => r#if = Some(v),
                        ParserToken::Boxed(v) => boxed = Some(v),
                        ParserToken::Data(v) => data = Some(v),
                        ParserToken::Name(v) => name = Some(v),
                        ParserToken::Features(v) => features = Some(v),
                        ParserToken::Returns(v) => returns = Some(v),
                        ParserToken::SuccessResponse(v) => success_response = Some(v),
                        ParserToken::Gen(v) => gen = Some(v),
                        ParserToken::AllowOob(v) => allow_oob = Some(v),
                        ParserToken::AllowPreconfig(v) => allow_preconfig = Some(v),
                        ParserToken::Coroutine(v) => coroutine = Some(v),
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
