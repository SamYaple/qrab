use crate::helpers::{kv, qtag};
use crate::{QapiBool, QapiString};
use nom::branch::alt;
use nom::combinator::map;
use nom::multi::separated_list1;
use nom::sequence::{delimited, tuple};
use nom::IResult;

enum ParserToken<'input> {
    DocRequired(QapiBool),
    CommandReturnsExceptions(Vec<QapiString<'input>>),
    CommandNameExceptions(Vec<QapiString<'input>>),
    DocumentationExceptions(Vec<QapiString<'input>>),
    MemberNameExceptions(Vec<QapiString<'input>>),
}

#[derive(Debug, Clone)]
pub struct QapiPragma<'input> {
    doc_required: Option<QapiBool>,
    command_name_exceptions: Option<Vec<QapiString<'input>>>,
    command_returns_exceptions: Option<Vec<QapiString<'input>>>,
    documentation_exceptions: Option<Vec<QapiString<'input>>>,
    member_name_exceptions: Option<Vec<QapiString<'input>>>,
}

impl<'input> QapiPragma<'input> {
    /// PRAGMA = { 'pragma': {
    ///            '*doc-required': BOOL,
    ///            '*command-name-exceptions': [ STRING, ... ],
    ///            '*command-returns-exceptions': [ STRING, ... ],
    ///            '*documentation-exceptions': [ STRING, ... ],
    ///            '*member-name-exceptions': [ STRING, ... ] } }
    pub fn parse(input: &'input str) -> IResult<&'input str, Self> {
        let doc_required_parser = map(kv(qtag("doc-required"), QapiBool::parse), |v| {
            ParserToken::DocRequired(v)
        });
        let command_name_exceptions_parser = map(
            kv(
                qtag("command-name-exceptions"),
                delimited(
                    qtag("["),
                    separated_list1(qtag(","), QapiString::parse),
                    qtag("]"),
                ),
            ),
            |v| ParserToken::CommandNameExceptions(v),
        );
        let command_returns_exceptions_parser = map(
            kv(
                qtag("command-returns-exceptions"),
                delimited(
                    qtag("["),
                    separated_list1(qtag(","), QapiString::parse),
                    qtag("]"),
                ),
            ),
            |v| ParserToken::CommandReturnsExceptions(v),
        );
        let documentation_exceptions_parser = map(
            kv(
                qtag("documentation-exceptions"),
                delimited(
                    qtag("["),
                    separated_list1(qtag(","), QapiString::parse),
                    qtag("]"),
                ),
            ),
            |v| ParserToken::DocumentationExceptions(v),
        );
        let member_name_exceptions_parser = map(
            kv(
                qtag("member-name-exceptions"),
                delimited(
                    qtag("["),
                    separated_list1(qtag(","), QapiString::parse),
                    qtag("]"),
                ),
            ),
            |v| ParserToken::MemberNameExceptions(v),
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
