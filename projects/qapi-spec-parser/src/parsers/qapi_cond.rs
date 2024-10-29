use super::{qstring, qtag, take_kv, take_list};
use nom::branch::alt;
use nom::combinator::map;
use nom::sequence::delimited;
use nom::IResult;

/// Convenience wrapper for taking a condition keypair.
/// Examples:
///   - `'if': CONFIG_OPT1`
///   - `'if': {'not': CONFIG_OPT1}`
///   - `'if': {'all': [CONFIG_OPT1, CONFIG_OPT2}`
///   - `'if': {'any': [CONFIG_OPT1, CONFIG_OPT2}`
pub fn take_cond(input: &str) -> IResult<&str, QapiCond<'_>> {
    take_kv("if", QapiCond::parse)(input)
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
    ///          | { 'all: [ COND, ... ] }
    ///          | { 'any: [ COND, ... ] }
    ///          | { 'not': COND }
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        let not_parser = take_kv("not", Self::parse);
        let any_parser = take_kv("any", take_list(Self::parse));
        let all_parser = take_kv("all", take_list(Self::parse));
        alt((
            map(qstring, |v| Self::ConfigName(v)),
            delimited(
                qtag("{"),
                alt((
                    map(all_parser, |v| Self::All(v)),
                    map(any_parser, |v| Self::Any(v)),
                    map(not_parser, |v| Self::Not(Box::new(v))),
                )),
                qtag("}"),
            ),
        ))(input)
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
    fn test_parse_with_whitespace() {
        let inputs = [
            " 'CONFIG_OPTION'",
            "{ 'not' : 'CONFIG_OPTION' }",
            "{ 'any' : [ 'CONFIG_OPTION' , 'CONFIG_OPTION2' ] }",
            "{ 'all' : [ 'CONFIG_OPTION' , 'CONFIG_OPTION2' ] }",
        ];
        for input in inputs.iter() {
            let (remaining, parsed) = QapiCond::parse(input).expect("Parsing failed");
            assert_eq!(remaining, "");
            assert!(matches!(parsed, QapiCond::ConfigName(_) | QapiCond::Not(_) | QapiCond::Any(_) | QapiCond::All(_)));
        }
    }

    #[test]
    fn test_parse_invalid_inputs() {
        let invalid_inputs = [
            "", // Empty input
            "'CONFIG_OPTION", // Missing closing quote
            "{'unknown':['CONFIG_OPTION']}", // Unknown keyword
            "{'any':}", // Missing list
            "{'not':}", // Missing condition
            "{'any':['CONFIG_OPTION',]}", // Trailing comma
        ];
        for input in invalid_inputs.iter() {
            let result = QapiCond::parse(input);
            assert!(result.is_err(), "Expected parsing to fail for input: {}", input);
        }
    }

    #[test]
    fn test_parse_config_name() {
        let input = "'CONFIG_OPTION'";
        let expected = QapiCond::ConfigName("CONFIG_OPTION");
        let (remaining, parsed) = QapiCond::parse(input).expect("Parsing failed");
        assert_eq!(remaining, "");
        assert_eq!(parsed, expected);
    }

    #[test]
    fn test_parse_not_condition() {
        let input = "{'not':'CONFIG_OPTION'}";
        let expected = QapiCond::Not(Box::new(QapiCond::ConfigName("CONFIG_OPTION")));
        let (remaining, parsed) = QapiCond::parse(input).expect("Parsing failed");
        assert_eq!(remaining, "");
        assert_eq!(parsed, expected);
    }

    #[test]
    fn test_parse_any_condition_single() {
        let input = "{'any':['CONFIG_OPTION']}";
        let expected = QapiCond::Any(vec![QapiCond::ConfigName("CONFIG_OPTION")]);
        let (remaining, parsed) = QapiCond::parse(input).expect("Parsing failed");
        assert_eq!(remaining, "");
        assert_eq!(parsed, expected);
    }

    #[test]
    fn test_parse_any_condition_multiple() {
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
