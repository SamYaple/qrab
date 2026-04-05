use super::{qstring, qtag, take_dict, take_kv, with_parser_config};
use crate::{take_branches, take_cond, take_features, take_members_or_ref};
use crate::{MembersOrRef, QapiBranches, QapiCond, QapiDocumentation, QapiFeatures};
use nom::branch::alt;
use nom::combinator::opt;
use nom::error::{Error, ErrorKind};
use nom::sequence::delimited;
use nom::IResult;
use nom::Parser;

pub fn take_union(input: &str) -> IResult<&str, QapiUnion<'_>> {
    QapiUnion::parse(input)
}

#[derive(Debug, Clone, Default)]
pub struct QapiUnion<'i> {
    pub name: &'i str,
    pub data: QapiBranches<'i>,
    pub base: MembersOrRef<'i>,
    /// None for simple unions (v1.x-v6.1.0)
    pub discriminator: Option<&'i str>,
    pub r#if: Option<QapiCond<'i>>,
    pub features: Option<QapiFeatures<'i>>,
    pub doc: Option<QapiDocumentation<'i>>,
}

impl<'i> QapiUnion<'i> {
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        let (input, doc) = opt(QapiDocumentation::parse).parse(input)?;
        let mut s = Self {
            doc,
            ..Default::default()
        };
        let start = input;
        let (input, _) = take_dict(alt((
            take_kv("union", qstring).map(|v| s.name = v),
            take_kv("data", take_branches).map(|v| s.data = v),
            take_kv("discriminator", qstring).map(|v| s.discriminator = Some(v)),
            // Old alternate syntax: 'discriminator': {} (v1.7.0-v2.0.0)
            take_kv("discriminator", delimited(qtag("{"), opt(qstring), qtag("}"))).map(|_| ()),
            take_cond.map(|v| s.r#if = Some(v)),
            take_features.map(|v| s.features = Some(v)),
            take_kv("base", take_members_or_ref).map(|v| s.base = v),
        )))
        .parse(input)?;

        if s.name.is_empty() || s.data.is_empty() {
            return Err(nom::Err::Error(Error::new(start, ErrorKind::Tag)));
        }

        if !with_parser_config(|c| c.simple_unions)
            && (s.discriminator.is_none() || s.base == MembersOrRef::Unset)
        {
            return Err(nom::Err::Error(Error::new(start, ErrorKind::Tag)));
        }
        Ok((input, s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::{ParserConfig, set_parser_config};

    #[test]
    fn test_valid() {
        let inputs = [
            "{'union':'SOMENAME',  'discriminator': 'testdiscrim', 'base': 'testbase', 'data':{'branchname':'branchtype'}}",
            "{'union': 'SOMENAME', 'discriminator': 'testdiscrim', 'data': {'branchname':'branchtype'}, 'base': {'membername':{'if':'CONFIG_OPTION', 'type': ['sometype'], 'features': ['yes']}}}",
        ];
        for input in inputs {
            let result = QapiUnion::parse(input);
            match result {
                Ok((remaining, _)) => assert_eq!(remaining, ""),
                _ => panic!("Failed to parse: {}", input),
            }
        }
    }

    #[test]
    fn test_simple_union() {
        set_parser_config(ParserConfig::for_version((2, 0, 0)));
        let input = "{ 'union': 'BlockdevAction', 'data': { 'blockdev-snapshot-sync': 'BlockdevSnapshot' } }";
        let result = QapiUnion::parse(input);
        set_parser_config(ParserConfig::modern());
        match result {
            Ok((remaining, u)) => {
                assert_eq!(remaining, "");
                assert_eq!(u.name, "BlockdevAction");
                assert!(u.discriminator.is_none());
                assert_eq!(u.base, MembersOrRef::Unset);
            }
            Err(e) => panic!("Simple union should be accepted: {}", e),
        }
    }

    #[test]
    fn test_empty_discriminator() {
        set_parser_config(ParserConfig::for_version((1, 7, 0)));
        let input = "{ 'union': 'BlockdevRef', 'discriminator': {}, 'data': { 'definition': 'BlockdevOptions', 'reference': 'str' } }";
        let result = QapiUnion::parse(input);
        set_parser_config(ParserConfig::modern());
        match result {
            Ok((remaining, u)) => {
                assert_eq!(remaining, "");
                assert_eq!(u.name, "BlockdevRef");
                assert!(u.discriminator.is_none());
            }
            Err(e) => panic!("Empty discriminator should be accepted: {}", e),
        }
    }

    #[test]
    fn test_simple_union_rejected_in_modern() {
        set_parser_config(ParserConfig::modern());
        let input = "{ 'union': 'Test', 'data': { 'a': 'TypeA' } }";
        let result = QapiUnion::parse(input);
        assert!(result.is_err(), "Simple unions should be rejected in modern mode");
    }
}
