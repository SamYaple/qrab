use super::common::QapiString;
use super::common::{qstring, qtag, take_dict, take_kv};
use super::parser_version;
use super::qapi_branches::{QapiBranches, take_branches};
use super::qapi_cond::{QapiCond, take_cond};
use super::qapi_documentation::QapiDocumentation;
use super::qapi_features::{QapiFeatures, take_features};
use super::qapi_members::{MembersOrRef, take_members_or_ref};
use nom::IResult;
use nom::Parser;
use nom::branch::alt;
use nom::combinator::opt;
use nom::error::{Error, ErrorKind};

pub fn take_union(input: &str) -> IResult<&str, QapiUnion<'_>> {
    QapiUnion::parse(input)
}

#[derive(Debug, Clone, Default)]
pub struct QapiUnion<'i> {
    pub name: QapiString<'i>,
    pub data: QapiBranches<'i>,
    pub base: MembersOrRef<'i>,
    /// Named for flat unions; absent for simple unions and legacy alternates.
    pub discriminator: Option<QapiString<'i>>,
    /// An empty discriminator object denoted an untagged alternate.
    pub legacy_alternate: bool,
    pub r#if: Option<QapiCond<'i>>,
    pub features: Option<QapiFeatures<'i>>,
    pub doc: Option<QapiDocumentation<'i>>,
}

impl<'i> QapiUnion<'i> {
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        let v = parser_version();
        if v < (1, 7, 0) {
            Self::parse_v1(input)
        } else if v < (6, 2, 0) {
            Self::parse_v2(input)
        } else {
            Self::parse_v3(input)
        }
    }

    /// v1 (QEMU v1.0–v1.6.0): simple unions only, `'discriminator': {}` for alternates.
    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
        let (input, doc) = opt(QapiDocumentation::parse).parse(input)?;
        let mut s = Self {
            doc,
            ..Default::default()
        };
        let start = input;
        let (input, _) = take_dict(alt((
            take_kv("union", qstring).map(|v| s.name = v),
            take_kv("data", take_branches).map(|v| s.data = v),
            // Historical untagged alternate syntax.
            take_kv("discriminator", (qtag("{"), qtag("}"))).map(|_| s.legacy_alternate = true),
            take_cond.map(|v| s.r#if = Some(v)),
            take_features.map(|v| s.features = Some(v)),
            take_kv("base", take_members_or_ref).map(|v| s.base = v),
        )))
        .parse(input)?;
        if s.name.is_empty() || s.data.is_empty() {
            return Err(nom::Err::Error(Error::new(start, ErrorKind::Tag)));
        }
        if s.legacy_alternate && s.base != MembersOrRef::Unset {
            return Err(nom::Err::Error(Error::new(start, ErrorKind::Verify)));
        }
        // v1: simple unions are valid without discriminator/base
        Ok((input, s))
    }

    /// v2 (QEMU v1.7.0–v6.1.0): simple unions + discriminated unions coexist.
    pub fn parse_v2(input: &'i str) -> IResult<&'i str, Self> {
        let (input, doc) = opt(QapiDocumentation::parse).parse(input)?;
        let mut s = Self {
            doc,
            ..Default::default()
        };
        let start = input;
        let mut has_data = false;
        let (input, _) = take_dict(alt((
            take_kv("union", qstring).map(|v| s.name = v),
            take_kv("data", take_branches).map(|v| {
                has_data = true;
                s.data = v;
            }),
            take_kv("discriminator", qstring).map(|v| s.discriminator = Some(v)),
            // Historical untagged alternate syntax.
            take_kv("discriminator", (qtag("{"), qtag("}"))).map(|_| s.legacy_alternate = true),
            take_cond.map(|v| s.r#if = Some(v)),
            take_features.map(|v| s.features = Some(v)),
            take_kv("base", take_members_or_ref).map(|v| s.base = v),
        )))
        .parse(input)?;
        if s.name.is_empty() || !has_data || (s.discriminator.is_none() && s.data.is_empty()) {
            return Err(nom::Err::Error(Error::new(start, ErrorKind::Tag)));
        }
        if s.legacy_alternate && s.base != MembersOrRef::Unset {
            return Err(nom::Err::Error(Error::new(start, ErrorKind::Verify)));
        }
        // v2: both simple (no discriminator) and flat (with discriminator) are valid
        Ok((input, s))
    }

    /// v3 (QEMU v6.2.0+): discriminated (flat) unions only — base + discriminator required.
    pub fn parse_v3(input: &'i str) -> IResult<&'i str, Self> {
        let (input, doc) = opt(QapiDocumentation::parse).parse(input)?;
        let mut s = Self {
            doc,
            ..Default::default()
        };
        let start = input;
        let mut has_data = false;
        let (input, _) = take_dict(alt((
            take_kv("union", qstring).map(|v| s.name = v),
            take_kv("data", take_branches).map(|v| {
                has_data = true;
                s.data = v;
            }),
            take_kv("discriminator", qstring).map(|v| s.discriminator = Some(v)),
            take_cond.map(|v| s.r#if = Some(v)),
            take_features.map(|v| s.features = Some(v)),
            take_kv("base", take_members_or_ref).map(|v| s.base = v),
        )))
        .parse(input)?;
        if s.name.is_empty() || !has_data {
            return Err(nom::Err::Error(Error::new(start, ErrorKind::Tag)));
        }
        // v3: discriminator and base are required
        if s.discriminator.is_none() || s.base == MembersOrRef::Unset {
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
    fn flat_unions_allow_implicit_empty_payloads_but_require_data() {
        for version in [(2, 1, 0), (6, 2, 0), (11, 1, 3)] {
            set_parser_version(version);
            let prefix =
                "{ 'union': 'EmptyPayload', 'base': {'kind': 'Kind'}, 'discriminator': 'kind'";
            assert!(QapiUnion::parse(&format!("{prefix} }}")).is_err());
            let input = format!("{prefix}, 'data': {{}} }}");
            let (remaining, value) = QapiUnion::parse(&input).unwrap();
            assert!(remaining.is_empty());
            assert!(value.data.is_empty());
        }
    }

    #[test]
    fn empty_discriminator_is_distinct_from_simple_union_and_rejects_nonempty_objects() {
        set_parser_version((1, 7, 0));
        let (_, alternate) = QapiUnion::parse(
            "{ 'union': 'Ref', 'discriminator': {}, 'data': {'reference':'str'} }",
        )
        .unwrap();
        assert!(alternate.legacy_alternate);
        let (_, tagged) =
            QapiUnion::parse("{ 'union': 'Ref', 'data': {'reference':'str'} }").unwrap();
        assert!(!tagged.legacy_alternate);
        assert!(
            QapiUnion::parse(
                "{ 'union': 'Ref', 'discriminator': {'ignored'}, 'data': {'reference':'str'} }"
            )
            .is_err()
        );
        assert!(QapiUnion::parse("{ 'union': 'Ref', 'discriminator': {}, 'base': 'Base', 'data': {'reference':'str'} }").is_err());
    }

    #[test]
    fn test_flat_union_v3() {
        set_parser_version((10, 2, 0));
        let input = "{'union':'SOMENAME', 'discriminator': 'testdiscrim', 'base': 'testbase', 'data':{'branchname':'branchtype'}}";
        let (rem, u) = QapiUnion::parse(input).unwrap();
        assert_eq!(rem, "");
        assert_eq!(u.name, "SOMENAME");
        assert_eq!(u.discriminator, Some("testdiscrim".into()));
    }

    #[test]
    fn test_simple_union_rejected_in_v3() {
        set_parser_version((10, 2, 0));
        let input = "{ 'union': 'Test', 'data': { 'a': 'TypeA' } }";
        let result = QapiUnion::parse(input);
        assert!(result.is_err(), "Simple unions should be rejected in v3");
    }

    #[test]
    fn test_simple_union_accepted_in_v2() {
        set_parser_version((2, 0, 0));
        let input = "{ 'union': 'BlockdevAction', 'data': { 'blockdev-snapshot-sync': 'BlockdevSnapshot' } }";
        let (rem, u) = QapiUnion::parse(input).unwrap();
        assert_eq!(rem, "");
        assert_eq!(u.name, "BlockdevAction");
        assert!(u.discriminator.is_none());
        assert_eq!(u.base, MembersOrRef::Unset);
    }

    #[test]
    fn test_empty_discriminator_v1() {
        set_parser_version((1, 5, 0));
        let input = "{ 'union': 'BlockdevRef', 'discriminator': {}, 'data': { 'definition': 'BlockdevOptions', 'reference': 'str' } }";
        let (rem, u) = QapiUnion::parse(input).unwrap();
        assert_eq!(rem, "");
        assert_eq!(u.name, "BlockdevRef");
        assert!(u.discriminator.is_none());
        assert!(u.legacy_alternate);
    }

    #[test]
    fn test_flat_union_accepted_in_v2() {
        set_parser_version((3, 0, 0));
        let input = "{'union': 'TestUnion', 'discriminator': 'type', 'base': 'TestBase', 'data': {'a': 'TypeA'}}";
        let (rem, u) = QapiUnion::parse(input).unwrap();
        assert_eq!(rem, "");
        assert_eq!(u.discriminator, Some("type".into()));
    }
}
