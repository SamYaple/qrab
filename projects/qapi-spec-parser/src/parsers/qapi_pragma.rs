use super::{qbool, qstring, qtag, take_kv, take_list};
use nom::branch::alt;
use nom::multi::separated_list1;
use nom::sequence::delimited;
use nom::IResult;
use nom::Parser;

pub fn take_pragma(input: &str) -> IResult<&str, QapiPragma<'_>> {
    QapiPragma::parse(input)
}

enum ParserToken<'i> {
    DocRequired(&'i str),
    CommandReturnsExceptions(Vec<&'i str>),
    CommandNameExceptions(Vec<&'i str>),
    DocumentationExceptions(Vec<&'i str>),
    MemberNameExceptions(Vec<&'i str>),
}

#[derive(Debug, Clone)]
pub struct QapiPragma<'i> {
    _doc_required: Option<&'i str>,
    _command_name_exceptions: Option<Vec<&'i str>>,
    _command_returns_exceptions: Option<Vec<&'i str>>,
    _documentation_exceptions: Option<Vec<&'i str>>,
    _member_name_exceptions: Option<Vec<&'i str>>,
}

impl<'i> QapiPragma<'i> {
    /// PRAGMA = { 'pragma': {
    ///            '*doc-required': BOOL,
    ///            '*command-name-exceptions': [ STRING, ... ],
    ///            '*command-returns-exceptions': [ STRING, ... ],
    ///            '*documentation-exceptions': [ STRING, ... ],
    ///            '*member-name-exceptions': [ STRING, ... ] } }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        let doc_required_parser = take_kv("doc-required", qbool).map(|v| {
            ParserToken::DocRequired(v)
        });
        let command_name_exceptions_parser =
            take_kv("command-name-exceptions", take_list(qstring))
                .map(ParserToken::CommandNameExceptions);
        let command_returns_exceptions_parser =
            take_kv("command-returns-exceptions", take_list(qstring))
                .map(ParserToken::CommandReturnsExceptions);
        let documentation_exceptions_parser =
            take_kv("documentation-exceptions", take_list(qstring))
                .map(ParserToken::DocumentationExceptions);
        let member_name_exceptions_parser =
            take_kv("member-name-exceptions", take_list(qstring))
                .map(ParserToken::MemberNameExceptions);

        // Old pragma keys (v2.10.0–v4.2.0) — treated as aliases
        let returns_whitelist_parser =
            take_kv("returns-whitelist", take_list(qstring))
                .map(ParserToken::CommandReturnsExceptions);
        let name_case_whitelist_parser =
            take_kv("name-case-whitelist", take_list(qstring))
                .map(ParserToken::MemberNameExceptions);

        let parsers = alt((
            doc_required_parser,
            command_name_exceptions_parser,
            command_returns_exceptions_parser,
            documentation_exceptions_parser,
            member_name_exceptions_parser,
            returns_whitelist_parser,
            name_case_whitelist_parser,
        ));
        delimited(
            (qtag("{"), qtag("'pragma'"), qtag(":"), qtag("{")),
            separated_list1(qtag(","), parsers).map(|tokens| {
                let mut doc_required = None;
                let mut command_name_exceptions = None;
                let mut command_returns_exceptions = None;
                let mut documentation_exceptions = None;
                let mut member_name_exceptions = None;
                for i in tokens {
                    match i {
                        ParserToken::DocRequired(v) => doc_required = Some(v),
                        ParserToken::CommandReturnsExceptions(v) => {
                            command_returns_exceptions = Some(v)
                        }
                        ParserToken::CommandNameExceptions(v) => command_name_exceptions = Some(v),
                        ParserToken::DocumentationExceptions(v) => {
                            documentation_exceptions = Some(v)
                        }
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
            }),
            (qtag("}"), qtag("}")),
        )
        .parse(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_INPUTS: [&str; 2] = [
        "{ 'pragma': { 'doc-required': true } }",
        r#"{ 'pragma': {
            # Command names containing '_'
            'command-name-exceptions': [
                'add_client',
                'system_reset',
                'system_wakeup' ],
            # Commands allowed to return a non-dictionary
            'command-returns-exceptions': [
                'human-monitor-command',
                'query-tpm-types',
                'ringbuf-read' ],
            # Types, commands, and events with undocumented members / arguments:
            'documentation-exceptions': [
                'AbortWrapper',
                'query-rocker',
                'query-rocker-ports' ],
            # Externally visible types whose member names may use uppercase
            'member-name-exceptions': [     # visible in:
                'ACPISlotType',             # query-acpi-ospm-status
                'AcpiTableOptions',         # -acpitable
                'VncClientInfo',            # query-vnc, query-vnc-servers, ...
                'X86CPURegister32'          # qom-get of x86 CPU properties
            ] } }"#,
    ];

    #[test]
    fn test_valid() {
        for input in VALID_INPUTS {
            let result = QapiPragma::parse(input);
            match result {
                Ok((remaining, _)) => {
                    assert_eq!(remaining, "");
                }
                _ => panic!("Failed to parse: ```\n{input}\n```"),
            }
        }
    }

    #[test]
    fn test_old_pragma_keys() {
        let input = "{ 'pragma': { 'returns-whitelist': ['human-monitor-command'], 'name-case-whitelist': ['ACPISlotType'] } }";
        let result = QapiPragma::parse(input);
        match result {
            Ok((remaining, _)) => assert_eq!(remaining, ""),
            Err(e) => panic!("Old pragma keys should be accepted: {}", e),
        }
    }
}
