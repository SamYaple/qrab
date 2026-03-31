use super::{qstring, take_dict, take_kv};
use crate::{take_alternatives, take_cond, take_features};
use crate::{QapiAlternatives, QapiCond, QapiDocumentation, QapiFeatures};
use nom::branch::alt;
use nom::combinator::opt;
use nom::error::{Error, ErrorKind};
use nom::IResult;
use nom::Parser;

pub fn take_alternate(input: &str) -> IResult<&str, QapiAlternate<'_>> {
    QapiAlternate::parse(input)
}

#[derive(Debug, Clone, Default)]
pub struct QapiAlternate<'i> {
    pub name: &'i str,
    pub data: QapiAlternatives<'i>,
    pub r#if: Option<QapiCond<'i>>,
    pub features: Option<QapiFeatures<'i>>,
    pub doc: Option<QapiDocumentation<'i>>,
}

impl<'i> QapiAlternate<'i> {
    /// ALTERNATE = { 'alternate': STRING,
    ///               'data': ALTERNATIVES,
    ///               '*if': COND,
    ///               '*features': FEATURES }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        let (input, doc) = opt(QapiDocumentation::parse).parse(input)?;
        let mut s = Self {
            doc,
            ..Default::default()
        };
        let start = input;
        let (input, _) = take_dict(alt((
            take_kv("alternate", qstring).map(|v| s.name = v),
            take_alternatives.map(|v| s.data = v),
            take_cond.map(|v| s.r#if = Some(v)),
            take_features.map(|v| s.features = Some(v)),
        )))
        .parse(input)?;
        if s.name.is_empty() || s.data.is_empty() {
            return Err(nom::Err::Error(Error::new(start, ErrorKind::Tag)));
        }
        Ok((input, s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_INPUTS: [&str; 5] = [
        "{'alternate':'SOMENAME','data':{'membername':'membertype'}}",
        "{ 'alternate': 'SOMENAME', 'data': {'membername':{'if':'CONFIG_OPTION', 'type': ['sometype']}}, 'features': ['yes']}",
        "{'alternate': 'SOMENAME', 'data':{'membername':'membertype'}, 'if':{'not':'CONFIG_VALUE'}}",
        "{'alternate': 'SOMENAME', 'data':{'membername':'membertype'}}",
        "{'alternate': 'SOMENAME', 'data':{'membername':{'type':'membertype'}}}",
    ];

    #[test]
    fn test_valid() {
        for input in VALID_INPUTS {
            let result = QapiAlternate::parse(input);
            match result {
                Ok((remaining, _)) => {
                    assert_eq!(remaining, "");
                }
                _ => panic!("Failed to parse: ```\n{input}\n```"),
            }
        }
    }
}
