use super::common::{qstring, qtag, take_kv, take_list};
use nom::IResult;
use nom::Parser;
use nom::branch::alt;
use nom::combinator::verify;
use nom::sequence::delimited;

/// Parse an `if` key and its nested configuration expression.
pub fn take_cond(input: &str) -> IResult<&str, QapiCond<'_>> {
    take_kv("if", QapiCond::parse).parse(input)
}

/// Holds conditional information about an option, feature, or field.
#[derive(Debug, Clone, PartialEq)]
pub enum QapiCond<'i> {
    All(Vec<QapiCond<'i>>),
    Any(Vec<QapiCond<'i>>),
    Not(Box<QapiCond<'i>>),
    ConfigName(&'i str),
}

impl<'i> QapiCond<'i> {
    /// Parses a `COND` as defined in the QAPI spec:
    ///     COND = STRING
    ///          | { 'all': [ COND, ... ] }
    ///          | { 'any': [ COND, ... ] }
    ///          | { 'not': COND }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        if super::parser_version() < (6, 2, 0) {
            return alt((
                verify(qstring, |name: &str| !name.trim().is_empty()).map(Self::ConfigName),
                verify(
                    take_list(verify(qstring, |name: &str| !name.trim().is_empty())),
                    |v: &Vec<_>| !v.is_empty(),
                )
                .map(|names| Self::All(names.into_iter().map(Self::ConfigName).collect())),
            ))
            .parse(input);
        }
        Self::parse_v1(input)
    }

    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
        let not_parser = take_kv("not", Self::parse);
        let any_parser = take_kv(
            "any",
            verify(take_list(Self::parse), |v: &Vec<_>| !v.is_empty()),
        );
        let all_parser = take_kv(
            "all",
            verify(take_list(Self::parse), |v: &Vec<_>| !v.is_empty()),
        );
        alt((
            verify(qstring, |name: &str| {
                name.as_bytes().first().is_some_and(u8::is_ascii_uppercase)
                    && name
                        .bytes()
                        .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
            })
            .map(Self::ConfigName),
            delimited(
                qtag("{"),
                alt((
                    all_parser.map(Self::All),
                    any_parser.map(Self::Any),
                    not_parser.map(|v| Self::Not(Box::new(v))),
                )),
                qtag("}"),
            ),
        ))
        .parse(input)
    }
}

impl<'i> std::fmt::Display for QapiCond<'i> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Any(conds) => {
                write!(f, "(")?;
                let mut conds_iter = conds.iter();
                if let Some(first_cond) = conds_iter.next() {
                    write!(f, "{}", first_cond)?;
                    for cond in conds_iter {
                        write!(f, " || {}", cond)?;
                    }
                }
                write!(f, ")")
            }
            Self::All(conds) => {
                write!(f, "(")?;
                let mut conds_iter = conds.iter();
                if let Some(first_cond) = conds_iter.next() {
                    write!(f, "{}", first_cond)?;
                    for cond in conds_iter {
                        write!(f, " && {}", cond)?;
                    }
                }
                write!(f, ")")
            }
            Self::Not(cond) => {
                write!(f, "!{}", cond)
            }
            Self::ConfigName(name) => {
                write!(f, "{}", name)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_c_expressions_and_condition_lists() {
        crate::set_parser_version((6, 1, 0));
        let (_, value) = QapiCond::parse("'defined(CONFIG_SPICE)'").unwrap();
        assert_eq!(value.to_string(), "defined(CONFIG_SPICE)");
        let (remaining, value) =
            QapiCond::parse("['defined(CONFIG_A)', 'defined(CONFIG_B)']").unwrap();
        assert!(remaining.is_empty());
        assert_eq!(
            value.to_string(),
            "(defined(CONFIG_A) && defined(CONFIG_B))"
        );
        crate::set_parser_version((6, 2, 0));
        assert!(QapiCond::parse("'defined(CONFIG_SPICE)'").is_err());
        assert!(QapiCond::parse("['CONFIG_A']").is_err());
    }

    #[test]
    fn test_parse_with_whitespace() {
        crate::set_parser_version((11, 1, 3));
        let inputs = [
            " 'CONFIG_OPTION'",
            "{ 'not' : 'CONFIG_OPTION' }",
            "{ 'any' : [ 'CONFIG_OPTION' , 'CONFIG_OPTION2' ] }",
            "{ 'all' : [ 'CONFIG_OPTION' , 'CONFIG_OPTION2' ] }",
        ];
        for input in inputs.iter() {
            let (remaining, parsed) = QapiCond::parse(input).expect("Parsing failed");
            assert_eq!(remaining, "");
            assert!(matches!(
                parsed,
                QapiCond::ConfigName(_) | QapiCond::Not(_) | QapiCond::Any(_) | QapiCond::All(_)
            ));
        }
    }

    #[test]
    fn test_parse_invalid_inputs() {
        crate::set_parser_version((11, 1, 3));
        let invalid_inputs = [
            "'invalid'",
            "''",
            "'1CONFIG'",
            "{'all':[]}",
            "{'any':[]}",
            "",                              // Empty input
            "'CONFIG_OPTION",                // Missing closing quote
            "{'unknown':['CONFIG_OPTION']}", // Unknown keyword
            "{'any':}",                      // Missing list
            "{'not':}",                      // Missing condition
        ];
        for input in invalid_inputs.iter() {
            let result = QapiCond::parse(input);
            assert!(
                result.is_err(),
                "Expected parsing to fail for input: {}",
                input
            );
        }
    }

    #[test]
    fn test_parse_config_name() {
        crate::set_parser_version((11, 1, 3));
        let input = "'CONFIG_OPTION'";
        let expected = QapiCond::ConfigName("CONFIG_OPTION");
        let (remaining, parsed) = QapiCond::parse(input).expect("Parsing failed");
        assert_eq!(remaining, "");
        assert_eq!(parsed, expected);
    }

    #[test]
    fn test_parse_not_condition() {
        crate::set_parser_version((11, 1, 3));
        let input = "{'not':'CONFIG_OPTION'}";
        let expected = QapiCond::Not(Box::new(QapiCond::ConfigName("CONFIG_OPTION")));
        let (remaining, parsed) = QapiCond::parse(input).expect("Parsing failed");
        assert_eq!(remaining, "");
        assert_eq!(parsed, expected);
    }

    #[test]
    fn test_parse_any_condition_single() {
        crate::set_parser_version((11, 1, 3));
        let input = "{'any':['CONFIG_OPTION']}";
        let expected = QapiCond::Any(vec![QapiCond::ConfigName("CONFIG_OPTION")]);
        let (remaining, parsed) = QapiCond::parse(input).expect("Parsing failed");
        assert_eq!(remaining, "");
        assert_eq!(parsed, expected);
    }

    #[test]
    fn test_parse_any_condition_multiple() {
        crate::set_parser_version((11, 1, 3));
        let input = "{'any':['CONFIG_OPTION','CONFIG_OPTION2']}";
        let expected = QapiCond::Any(vec![
            QapiCond::ConfigName("CONFIG_OPTION"),
            QapiCond::ConfigName("CONFIG_OPTION2"),
        ]);
        let (remaining, parsed) = QapiCond::parse(input).expect("Parsing failed");
        assert_eq!(remaining, "");
        assert_eq!(parsed, expected);
    }

    #[test]
    fn test_parse_all_condition() {
        crate::set_parser_version((11, 1, 3));
        let input = "{'all':['CONFIG_OPTION','CONFIG_OPTION2']}";
        let expected = QapiCond::All(vec![
            QapiCond::ConfigName("CONFIG_OPTION"),
            QapiCond::ConfigName("CONFIG_OPTION2"),
        ]);
        let (remaining, parsed) = QapiCond::parse(input).expect("Parsing failed");
        assert_eq!(remaining, "");
        assert_eq!(parsed, expected);
    }

    #[test]
    fn test_display_config_name() {
        let cond = QapiCond::ConfigName("CONFIG_OPTION");
        assert_eq!(cond.to_string(), "CONFIG_OPTION");
    }

    #[test]
    fn test_display_not_condition() {
        let cond = QapiCond::Not(Box::new(QapiCond::ConfigName("CONFIG_OPTION")));
        assert_eq!(cond.to_string(), "!CONFIG_OPTION");
    }

    #[test]
    fn test_display_any_condition() {
        let cond = QapiCond::Any(vec![
            QapiCond::ConfigName("CONFIG_OPTION1"),
            QapiCond::ConfigName("CONFIG_OPTION2"),
        ]);
        assert_eq!(cond.to_string(), "(CONFIG_OPTION1 || CONFIG_OPTION2)");
    }

    #[test]
    fn test_display_all_condition() {
        let cond = QapiCond::All(vec![
            QapiCond::ConfigName("CONFIG_OPTION1"),
            QapiCond::ConfigName("CONFIG_OPTION2"),
        ]);
        assert_eq!(cond.to_string(), "(CONFIG_OPTION1 && CONFIG_OPTION2)");
    }

    #[test]
    fn test_display_complex_nested_conditions() {
        let cond = QapiCond::Any(vec![
            QapiCond::ConfigName("CONFIG_OPTION1"),
            QapiCond::Not(Box::new(QapiCond::All(vec![
                QapiCond::ConfigName("CONFIG_OPTION2"),
                QapiCond::ConfigName("CONFIG_OPTION3"),
            ]))),
        ]);
        assert_eq!(
            cond.to_string(),
            "(CONFIG_OPTION1 || !(CONFIG_OPTION2 && CONFIG_OPTION3))"
        );
    }
}
