use crate::helpers::{kv, qbool, qstring, qtag};
use crate::{QapiCond, QapiFeatures, QapiMembers};
use nom::branch::alt;
use nom::combinator::map;
use nom::multi::separated_list1;
use nom::sequence::delimited;
use nom::IResult;

enum ParserToken<'i> {
    Name(&'i str),
    Data(QapiEventData<'i>),
    If(QapiCond<'i>),
    Features(QapiFeatures<'i>),
    Boxed(&'i str),
}

#[derive(Debug, Clone)]
enum QapiEventData<'i> {
    Ref(&'i str),
    Members(QapiMembers<'i>),
}

#[derive(Debug, Clone)]
pub struct QapiEvent<'i> {
    name: &'i str,
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
        let boxed_parser = map(kv(qtag("boxed"), qbool), |v| ParserToken::Boxed(v));
        let cond_parser = map(kv(qtag("if"), QapiCond::parse), |v| ParserToken::If(v));
        let features_parser = map(kv(qtag("features"), QapiFeatures::parse), |v| {
            ParserToken::Features(v)
        });
        let name_parser = map(kv(qtag("event"), qstring), |v| ParserToken::Name(v));
        let data_parser = map(
            kv(
                qtag("data"),
                alt((
                    map(qstring, |v| QapiEventData::Ref(v)),
                    map(QapiMembers::parse, |v| QapiEventData::Members(v)),
                )),
            ),
            |v| ParserToken::Data(v),
        );

        let parsers = alt((
            data_parser,
            cond_parser,
            features_parser,
            name_parser,
            boxed_parser,
        ));
        delimited(
            qtag("{"),
            map(separated_list1(qtag(","), parsers), |tokens| {
                let mut r#if = None;
                let mut data = None;
                let mut features = None;
                let mut boxed = None;
                let mut name = None;
                for i in tokens {
                    match i {
                        ParserToken::If(v) => r#if = Some(v),
                        ParserToken::Boxed(v) => boxed = Some(v),
                        ParserToken::Data(v) => data = Some(v),
                        ParserToken::Name(v) => name = Some(v),
                        ParserToken::Features(v) => features = Some(v),
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
                }
            }),
            qtag("}"),
        )(input)
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
