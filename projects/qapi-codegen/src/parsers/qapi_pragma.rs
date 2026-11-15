use super::common::QapiString;
use super::common::{qbool, qstring, take_dict, take_kv, take_list};
use super::parser_version;
use nom::IResult;
use nom::Parser;
use nom::branch::alt;

pub fn take_pragma(input: &str) -> IResult<&str, QapiPragma<'_>> {
    QapiPragma::parse(input)
}

enum ParserToken<'i> {
    DocRequired(&'i str),
    CommandReturnsExceptions(Vec<QapiString<'i>>),
    CommandNameExceptions(Vec<QapiString<'i>>),
    DocumentationExceptions(Vec<QapiString<'i>>),
    MemberNameExceptions(Vec<QapiString<'i>>),
}

#[derive(Debug, Clone)]
pub struct QapiPragma<'i> {
    _doc_required: Option<&'i str>,
    _command_name_exceptions: Option<Vec<QapiString<'i>>>,
    _command_returns_exceptions: Option<Vec<QapiString<'i>>>,
    _documentation_exceptions: Option<Vec<QapiString<'i>>>,
    _member_name_exceptions: Option<Vec<QapiString<'i>>>,
}

impl<'i> QapiPragma<'i> {
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        let v = parser_version();
        if v < (6, 0, 0) {
            Self::parse_v1(input)
        } else {
            Self::parse_v2(input)
        }
    }

    /// v1 (QEMU v2.10.0–v5.2.0): old keys `returns-whitelist`, `name-case-whitelist`.
    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
        Self::parse_common(input, true)
    }

    /// v2 (QEMU v5.0.0+): new keys `command-returns-exceptions`, `member-name-exceptions`.
    pub fn parse_v2(input: &'i str) -> IResult<&'i str, Self> {
        Self::parse_common(input, false)
    }

    fn parse_common(input: &'i str, old_keys: bool) -> IResult<&'i str, Self> {
        let doc_required_parser = take_kv("doc-required", qbool).map(ParserToken::DocRequired);
        let command_name_exceptions_parser = take_kv("command-name-exceptions", take_list(qstring))
            .map(ParserToken::CommandNameExceptions);
        let command_returns_exceptions_parser =
            take_kv("command-returns-exceptions", take_list(qstring))
                .map(ParserToken::CommandReturnsExceptions);
        let documentation_exceptions_parser =
            take_kv("documentation-exceptions", take_list(qstring))
                .map(ParserToken::DocumentationExceptions);
        let member_name_exceptions_parser = take_kv("member-name-exceptions", take_list(qstring))
            .map(ParserToken::MemberNameExceptions);

        if old_keys {
            let returns_whitelist_parser = take_kv("returns-whitelist", take_list(qstring))
                .map(ParserToken::CommandReturnsExceptions);
            let name_case_whitelist_parser = take_kv("name-case-whitelist", take_list(qstring))
                .map(ParserToken::MemberNameExceptions);

            take_dict(take_kv(
                "pragma",
                take_dict(alt((
                    doc_required_parser,
                    command_name_exceptions_parser,
                    command_returns_exceptions_parser,
                    documentation_exceptions_parser,
                    member_name_exceptions_parser,
                    returns_whitelist_parser,
                    name_case_whitelist_parser,
                )))
                .map(Self::from_tokens),
            ))
            .map_opt(|mut values| values.pop())
            .parse(input)
        } else {
            take_dict(take_kv(
                "pragma",
                take_dict(alt((
                    doc_required_parser,
                    command_name_exceptions_parser,
                    command_returns_exceptions_parser,
                    documentation_exceptions_parser,
                    member_name_exceptions_parser,
                )))
                .map(Self::from_tokens),
            ))
            .map_opt(|mut values| values.pop())
            .parse(input)
        }
    }

    fn from_tokens(tokens: Vec<ParserToken<'i>>) -> Self {
        let mut doc_required = None;
        let mut command_name_exceptions = None;
        let mut command_returns_exceptions = None;
        let mut documentation_exceptions = None;
        let mut member_name_exceptions = None;
        for i in tokens {
            match i {
                ParserToken::DocRequired(v) => doc_required = Some(v),
                ParserToken::CommandReturnsExceptions(v) => command_returns_exceptions = Some(v),
                ParserToken::CommandNameExceptions(v) => command_name_exceptions = Some(v),
                ParserToken::DocumentationExceptions(v) => documentation_exceptions = Some(v),
                ParserToken::MemberNameExceptions(v) => member_name_exceptions = Some(v),
            }
        }
        Self {
            _doc_required: doc_required,
            _command_name_exceptions: command_name_exceptions,
            _command_returns_exceptions: command_returns_exceptions,
            _documentation_exceptions: documentation_exceptions,
            _member_name_exceptions: member_name_exceptions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::set_parser_version;

    #[test]
    fn empty_pragma_is_valid_but_missing_pragma_is_not() {
        set_parser_version((11, 1, 3));
        assert_eq!(QapiPragma::parse("{ 'pragma': {} }").unwrap().0, "");
        assert!(QapiPragma::parse("{}").is_err());
    }

    #[test]
    fn duplicate_pragma_keys_are_rejected() {
        set_parser_version((11, 1, 3));
        for input in [
            "{ 'pragma': { 'doc-required': true, 'doc-required': false } }",
            "{ 'pragma': {}, 'pragma': {} }",
            "{ 'pragma': { 'member-name-exceptions': [], 'member-name-exceptions': ['X'] } }",
        ] {
            assert!(QapiPragma::parse(input).is_err(), "accepted {input}");
        }
    }

    #[test]
    fn test_valid_v2() {
        set_parser_version((10, 2, 0));
        let input = r#"{ 'pragma': {
            'command-name-exceptions': ['add_client'],
            'command-returns-exceptions': ['human-monitor-command'],
            'documentation-exceptions': ['AbortWrapper'],
            'member-name-exceptions': ['ACPISlotType'] } }"#;
        let (rem, _) = QapiPragma::parse(input).unwrap();
        assert_eq!(rem, "");
    }

    #[test]
    fn test_old_keys_accepted_in_v1() {
        set_parser_version((3, 0, 0));
        let input = "{ 'pragma': { 'returns-whitelist': ['human-monitor-command'], 'name-case-whitelist': ['ACPISlotType'] } }";
        let (rem, _) = QapiPragma::parse(input).unwrap();
        assert_eq!(rem, "");
    }

    #[test]
    fn test_old_keys_rejected_in_v2() {
        set_parser_version((10, 2, 0));
        let input = "{ 'pragma': { 'returns-whitelist': ['test'] } }";
        let result = QapiPragma::parse(input);
        assert!(result.is_err(), "Old pragma keys should be rejected in v2");
    }

    #[test]
    fn test_doc_required() {
        set_parser_version((10, 2, 0));
        let input = "{ 'pragma': { 'doc-required': true } }";
        let (rem, _) = QapiPragma::parse(input).unwrap();
        assert_eq!(rem, "");
    }
}
