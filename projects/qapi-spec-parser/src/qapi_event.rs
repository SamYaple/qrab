use crate::helpers::{kv, qtag};
use crate::{QapiBool, QapiCond, QapiFeatures, QapiMembers, QapiString};
use nom::branch::alt;
use nom::combinator::map;
use nom::multi::separated_list1;
use nom::sequence::delimited;
use nom::IResult;

enum ParserKey {
    Name(QapiString),
    Data(QapiEventData),
    If(QapiCond),
    Features(QapiFeatures),
    Boxed(QapiBool),
}

#[derive(Debug)]
enum QapiEventData {
    Ref(QapiString),
    Members(QapiMembers),
}

#[derive(Debug)]
pub struct QapiEvent {
    name: QapiString,
    data: Option<QapiEventData>,
    boxed: Option<QapiBool>,
    r#if: Option<QapiCond>,
    features: Option<QapiFeatures>,
}

impl QapiEvent {
    /// EVENT = { 'event': STRING,
    ///           (
    ///           '*data': ( MEMBERS | STRING ),
    ///           |
    ///           'data': STRING,
    ///           'boxed': true,
    ///           )
    ///           '*if': COND,
    ///           '*features': FEATURES }
    pub fn parse(input: &str) -> IResult<&str, Self> {
        let boxed_parser = map(kv(qtag("boxed"), QapiBool::parse), |v| ParserKey::Boxed(v));
        let cond_parser = map(kv(qtag("if"), QapiCond::parse), |v| ParserKey::If(v));
        let features_parser = map(kv(qtag("features"), QapiFeatures::parse), |v| {
            ParserKey::Features(v)
        });
        let name_parser = map(kv(qtag("event"), QapiString::parse), |v| ParserKey::Name(v));
        let data_parser = map(
            kv(
                qtag("data"),
                alt((
                    map(QapiString::parse, |v| QapiEventData::Ref(v)),
                    map(QapiMembers::parse, |v| QapiEventData::Members(v)),
                )),
            ),
            |v| ParserKey::Data(v),
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
                        ParserKey::If(v) => r#if = Some(v),
                        ParserKey::Boxed(v) => boxed = Some(v),
                        ParserKey::Data(v) => data = Some(v),
                        ParserKey::Name(v) => name = Some(v),
                        ParserKey::Features(v) => features = Some(v),
                    }
                }
                let name = name.expect("struct is a required key");
                if let Some(ref b) = boxed {
                    if b.0 && data.is_none() {
                        // TODO Proper parser error returns, but not now...
                        panic!("data is a required key");
                    }
                }
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
