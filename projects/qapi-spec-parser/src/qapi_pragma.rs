use crate::helpers::{kv, qtag};
use crate::{QapiBool, QapiString};
use nom::branch::alt;
use nom::combinator::map;
use nom::multi::separated_list0;
use nom::multi::separated_list1;
use nom::sequence::{delimited, tuple};
use nom::IResult;

enum ParserKey {
    DocRequired(QapiBool),
    CommandReturnsExceptions(Vec<QapiString>),
    CommandNameExceptions(Vec<QapiString>),
    DocumentationExceptions(Vec<QapiString>),
    MemberNameExceptions(Vec<QapiString>),
}

#[derive(Debug)]
pub struct QapiPragma {
    doc_required: Option<QapiBool>,
    command_name_exceptions: Option<Vec<QapiString>>,
    command_returns_exceptions: Option<Vec<QapiString>>,
    documentation_exceptions: Option<Vec<QapiString>>,
    member_name_exceptions: Option<Vec<QapiString>>,
}

impl QapiPragma {
    /// PRAGMA = { 'pragma': {
    ///            '*doc-required': BOOL,
    ///            '*command-name-exceptions': [ STRING, ... ],
    ///            '*command-returns-exceptions': [ STRING, ... ],
    ///            '*documentation-exceptions': [ STRING, ... ],
    ///            '*member-name-exceptions': [ STRING, ... ] } }
    pub fn parse(input: &str) -> IResult<&str, Self> {
        let doc_required_parser = map(kv(qtag("doc-required"), QapiBool::parse), |v| {
            ParserKey::DocRequired(v)
        });
        let command_name_exceptions_parser = map(
            kv(
                qtag("command-name-exceptions"),
                delimited(
                    qtag("["),
                    separated_list0(qtag(","), QapiString::parse),
                    qtag("]"),
                ),
            ),
            |v| ParserKey::CommandNameExceptions(v),
        );
        let command_returns_exceptions_parser = map(
            kv(
                qtag("command-returns-exceptions"),
                delimited(
                    qtag("["),
                    separated_list0(qtag(","), QapiString::parse),
                    qtag("]"),
                ),
            ),
            |v| ParserKey::CommandReturnsExceptions(v),
        );
        let documentation_exceptions_parser = map(
            kv(
                qtag("documentation-exceptions"),
                delimited(
                    qtag("["),
                    separated_list0(qtag(","), QapiString::parse),
                    qtag("]"),
                ),
            ),
            |v| ParserKey::DocumentationExceptions(v),
        );
        let member_name_exceptions_parser = map(
            kv(
                qtag("member-name-exceptions"),
                delimited(
                    qtag("["),
                    separated_list0(qtag(","), QapiString::parse),
                    qtag("]"),
                ),
            ),
            |v| ParserKey::MemberNameExceptions(v),
        );

        let parsers = alt((
            doc_required_parser,
            command_name_exceptions_parser,
            command_returns_exceptions_parser,
            documentation_exceptions_parser,
            member_name_exceptions_parser,
        ));
        delimited(
            tuple((qtag("{"), qtag("'pragma'"), qtag(":"), qtag("{"))),
            map(separated_list1(qtag(","), parsers), |tokens| {
                let mut doc_required = None;
                let mut command_name_exceptions = None;
                let mut command_returns_exceptions = None;
                let mut documentation_exceptions = None;
                let mut member_name_exceptions = None;
                for i in tokens {
                    match i {
                        ParserKey::DocRequired(v) => doc_required = Some(v),
                        ParserKey::CommandReturnsExceptions(v) => {
                            command_returns_exceptions = Some(v)
                        }
                        ParserKey::CommandNameExceptions(v) => command_name_exceptions = Some(v),
                        ParserKey::DocumentationExceptions(v) => documentation_exceptions = Some(v),
                        ParserKey::MemberNameExceptions(v) => member_name_exceptions = Some(v),
                    }
                }
                Self {
                    doc_required,
                    command_name_exceptions,
                    command_returns_exceptions,
                    documentation_exceptions,
                    member_name_exceptions,
                }
            }),
            tuple((qtag("}"), qtag("}"))),
        )(input)
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
}