use crate::helpers::{qstring, qtag, take_kv};
use crate::{take_alternatives, take_cond, take_features};
use crate::{QapiAlternatives, QapiCond, QapiFeatures};
use nom::branch::alt;
use nom::combinator::map;
use nom::multi::separated_list1;
use nom::sequence::delimited;
use nom::IResult;

pub fn take_alternate(input: &str) -> IResult<&str, QapiAlternate<'_>> {
    QapiAlternate::parse(input)
}

#[derive(Debug, Clone, Default)]
pub struct QapiAlternate<'i> {
    pub name: Option<&'i str>,
    pub data: Option<QapiAlternatives<'i>>,
    pub r#if: Option<QapiCond<'i>>,
    pub features: Option<QapiFeatures<'i>>,
}

impl<'i> QapiAlternate<'i> {
    /// ALTERNATE = { 'alternate': STRING,
    ///               'data': ALTERNATIVES,
    ///               '*if': COND,
    ///               '*features': FEATURES }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        let mut s = Self::default();
        let (input, _) = delimited(
            qtag("{"),
            separated_list1(
                qtag(","),
                alt((
                    map(take_kv("alternate", qstring), |v| {
                        s.name = Some(v);
                    }),
                    map(take_alternatives, |v| {
                        s.data = Some(v);
                    }),
                    map(take_cond, |v| {
                        s.r#if = Some(v);
                    }),
                    map(take_features, |v| {
                        s.features = Some(v);
                    }),
                )),
            ),
            qtag("}"),
        )(input)?;
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
            dbg![&result];
            match result {
                Ok((remaining, _)) => {
                    assert_eq!(remaining, "");
                }
                _ => panic!("Failed to parse: ```\n{input}\n```"),
            }
        }
    }
}
