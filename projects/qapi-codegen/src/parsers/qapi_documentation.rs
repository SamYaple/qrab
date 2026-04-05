use super::common::{qtag};
use super::parser_version;
use nom::bytes::complete::{is_a, tag, tag_no_case, take_until, take_while1};
use nom::character::complete::{line_ending, multispace0, not_line_ending, space0};
use nom::combinator::{not, opt, peek, recognize};
use nom::error::{Error, ErrorKind};
use nom::multi::{many0, many1};
use nom::sequence::{delimited, preceded, terminated};
use nom::IResult;
use nom::Parser;

// ─── Shared helpers ────────────────────────────────────────────────

fn not_name_break(c: char) -> bool {
    c != ':' && c != '\n' && c != '\r' && c != ' '
}

fn take_line_start(input: &str) -> IResult<&str, &str> {
    recognize((tag("#"), space0)).parse(input)
}

fn take_line_end(input: &str) -> IResult<&str, &str> {
    recognize((space0, line_ending)).parse(input)
}

fn take_empty_line(input: &str) -> IResult<&str, &str> {
    recognize((take_line_start, take_line_end)).parse(input)
}

fn take_value(input: &str) -> IResult<&str, Vec<&str>> {
    many1(preceded(take_line_start, terminated(not_line_ending, line_ending)))
        .parse(input)
}

fn take_line_continuation(input: &str) -> IResult<&str, &str> {
    preceded(
        take_line_start,
        terminated(
            recognize((
                space0,
                not(peek(tag("@"))),
                not(peek(tag_no_case("since"))),
                not(peek(tag_no_case("returns"))),
                not(peek(tag_no_case("errors"))),
                not(peek(tag_no_case("note"))),
                not(peek(tag_no_case("example"))),
                not(peek(tag_no_case("todo"))),
                not(peek(tag_no_case("features"))),
                not_line_ending,
            )),
            line_ending,
        ),
    )
    .parse(input)
}

fn take_multiline_text(input: &str) -> IResult<&str, &str> {
    recognize(many1(take_line_continuation)).parse(input)
}

fn is_since_without_colon(input: &str) -> IResult<&str, &str> {
    preceded(
        (take_line_start, tag_no_case("since"), many0(is_a(" :"))),
        terminated(not_line_ending, line_ending),
    )
    .parse(input)
}

fn take_name(input: &str) -> IResult<&str, &str> {
    preceded(
        (take_line_start, tag("@")),
        terminated(take_while1(not_name_break), opt(tag(":"))),
    )
    .parse(input)
}

fn take_name_optional_colon(input: &str) -> IResult<&str, &str> {
    preceded(
        (take_line_start, tag("@")),
        take_while1(not_name_break),
    )
    .parse(input)
}

fn take_namedkv<'i>(
    key: &'static str,
) -> impl Parser<&'i str, Output = Vec<&'i str>, Error = Error<&'i str>> {
    preceded(
        (take_line_start, tag("@"), tag(key), tag(":")),
        take_value,
    )
}

fn take_namedkey<'i>(
    key: &'static str,
) -> impl Parser<&'i str, Output = Vec<&'i str>, Error = Error<&'i str>> {
    preceded(
        (take_line_start, tag_no_case(key), opt(tag(":")), space0),
        take_value,
    )
}

pub fn extract_since_from_comment(desc: &Vec<&str>) -> Option<String> {
    for line in desc.iter().rev() {
        let trimmed = line.trim();
        let lower = trimmed.to_lowercase();
        if lower.starts_with("since") || lower.starts_with("since:") {
            let rest = if lower.starts_with("since:") {
                &trimmed[6..]
            } else {
                &trimmed[5..]
            };
            let version = rest.trim().trim_end_matches('.');
            if !version.is_empty() {
                return Some(version.to_string());
            }
        }
    }
    None
}

// ─── QapiDocumentation ────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct QapiDocumentation<'i> {
    pub name: &'i str,
    pub description: Vec<&'i str>,
    pub fields: Vec<(&'i str, Vec<&'i str>)>,
    pub features: Vec<(&'i str, Vec<&'i str>)>,
    pub since: Option<&'i str>,
    pub returns: Vec<&'i str>,
    pub errors: Vec<&'i str>,
    pub notes: Vec<&'i str>,
    pub examples: Vec<&'i str>,
    pub todos: Vec<&'i str>,
}

impl<'i> QapiDocumentation<'i> {
    /// Dispatches to the correct versioned parser.
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        let v = parser_version();
        if v < (2, 8, 0) {
            Self::parse_v1(input)
        } else if v < (5, 0, 0) {
            Self::parse_v2(input)
        } else {
            Self::parse_v3(input)
        }
    }

    /// v1 (QEMU v1.0–v2.7.0): `# @Name` with optional colon, standard `##...##` blocks.
    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
        // Try ## opening, or shorthand ## @Name on same line
        let input = if let Ok((input, _)) = space0::<&str, Error<&str>>(input) {
            input
        } else {
            input
        };
        let (input, _) = terminated(tag("##"), take_line_end).parse(input)?;
        let (input, _) = many0(take_empty_line).parse(input)?;
        // Name with optional colon
        let (input, name) = terminated(take_name_optional_colon, (opt(tag(":")), take_line_end)).parse(input)?;
        let (input, _) = many0(take_empty_line).parse(input)?;
        let (input, description_str) = opt(take_multiline_text).parse(input)?;
        let (input, _) = many0(take_empty_line).parse(input)?;

        let mut fields: Vec<(&str, Vec<&str>)> = Vec::new();
        let mut features: Vec<(&str, Vec<&str>)> = Vec::new();
        let mut doc = Self {
            name,
            description: Vec::new(),
            fields: Vec::new(),
            features: Vec::new(),
            since: None,
            returns: Vec::new(),
            errors: Vec::new(),
            notes: Vec::new(),
            examples: Vec::new(),
            todos: Vec::new(),
        };

        // Parse field docs, since, returns, etc.
        let input = Self::parse_body(input, &mut doc)?;

        if let Some(desc) = description_str {
            doc.description = desc.lines().collect();
        }

        // Closing ## (optional for v1)
        let input = if let Ok((input, _)) = tag::<&str, &str, Error<&str>>("##").parse(input) {
            input
        } else {
            input
        };

        Ok((input, doc))
    }

    /// v2 (QEMU v2.8.0–v4.2.0): `# @Name:` with mandatory colon, standard `##...##`.
    pub fn parse_v2(input: &'i str) -> IResult<&'i str, Self> {
        let (input, _) = terminated(tag("##"), take_line_end).parse(input)?;
        let (input, _) = many0(take_empty_line).parse(input)?;
        let (input, name) = terminated(take_name, take_line_end).parse(input)?;
        let (input, _) = many0(take_empty_line).parse(input)?;
        let (input, description_str) = opt(take_multiline_text).parse(input)?;
        let (input, _) = many0(take_empty_line).parse(input)?;

        let mut doc = Self {
            name,
            description: Vec::new(),
            fields: Vec::new(),
            features: Vec::new(),
            since: None,
            returns: Vec::new(),
            errors: Vec::new(),
            notes: Vec::new(),
            examples: Vec::new(),
            todos: Vec::new(),
        };

        let input = Self::parse_body(input, &mut doc)?;

        if let Some(desc) = description_str {
            doc.description = desc.lines().collect();
        }

        let (input, _) = tag("##").parse(input)?;
        Ok((input, doc))
    }

    /// v3 (QEMU v5.0.0+): standard + blank lines allowed, leading whitespace tolerance.
    pub fn parse_v3(input: &'i str) -> IResult<&'i str, Self> {
        // Allow leading whitespace before ##
        let input = input.trim_start();
        let (input, _) = terminated(tag("##"), take_line_end).parse(input)?;
        let (input, _) = many0(take_empty_line).parse(input)?;
        let (input, name) = terminated(take_name, take_line_end).parse(input)?;
        let (input, _) = many0(take_empty_line).parse(input)?;
        let (input, description_str) = opt(take_multiline_text).parse(input)?;
        let (input, _) = many0(take_empty_line).parse(input)?;

        let mut doc = Self {
            name,
            description: Vec::new(),
            fields: Vec::new(),
            features: Vec::new(),
            since: None,
            returns: Vec::new(),
            errors: Vec::new(),
            notes: Vec::new(),
            examples: Vec::new(),
            todos: Vec::new(),
        };

        let input = Self::parse_body(input, &mut doc)?;

        if let Some(desc) = description_str {
            doc.description = desc.lines().collect();
        }

        let (input, _) = tag("##").parse(input)?;
        Ok((input, doc))
    }

    /// Shared: parse fields, since, returns, etc. after the description.
    fn parse_body(mut input: &'i str, doc: &mut Self) -> Result<&'i str, nom::Err<Error<&'i str>>> {
        loop {
            // Try blank line
            if let Ok((next, _)) = take_empty_line(input) {
                input = next;
                continue;
            }
            // Try field: @field-name: description (possibly multi-line)
            if let Ok((next, (name, first_line))) = (|input| {
                let (input, _) = take_line_start(input)?;
                let (input, _) = tag("@").parse(input)?;
                let (input, name) = take_while1(not_name_break).parse(input)?;
                let (input, _) = opt(tag(":")).parse(input)?;
                let (input, _) = space0.parse(input)?;
                let (input, first_line) = not_line_ending.parse(input)?;
                let (input, _) = line_ending.parse(input)?;
                Ok((input, (name, first_line)))
            }).parse(input) {
                let mut field_vals: Vec<&str> = Vec::new();
                if !first_line.trim().is_empty() {
                    field_vals.push(first_line);
                }
                // Consume continuation lines (indented, not starting with @, not a section keyword)
                let mut next = next;
                loop {
                    if let Ok((n, line)) = take_line_continuation(next) {
                        field_vals.push(line);
                        next = n;
                    } else {
                        break;
                    }
                }
                // Skip trailing empty lines
                while let Ok((n, _)) = take_empty_line(next) {
                    next = n;
                }
                doc.fields.push((name, field_vals));
                input = next;
                continue;
            }
            // Try since
            if let Ok((next, since)) = is_since_without_colon(input) {
                doc.since = Some(since.trim().trim_end_matches('.'));
                input = next;
                continue;
            }
            // Try named sections
            if let Ok((next, vals)) = take_namedkey("Since").parse(input) {
                if let Some(v) = vals.first() {
                    doc.since = Some(v.trim().trim_end_matches('.'));
                }
                input = next;
                continue;
            }
            if let Ok((next, vals)) = take_namedkey("Returns").parse(input) {
                doc.returns = vals;
                input = next;
                continue;
            }
            if let Ok((next, vals)) = take_namedkey("Errors").parse(input) {
                doc.errors = vals;
                input = next;
                continue;
            }
            if let Ok((next, vals)) = take_namedkey("Note").parse(input) {
                doc.notes = vals;
                input = next;
                continue;
            }
            if let Ok((next, vals)) = take_namedkey("Notes").parse(input) {
                doc.notes = vals;
                input = next;
                continue;
            }
            if let Ok((next, vals)) = take_namedkey("Example").parse(input) {
                doc.examples = vals;
                input = next;
                continue;
            }
            if let Ok((next, vals)) = take_namedkey("Examples").parse(input) {
                doc.examples = vals;
                input = next;
                continue;
            }
            if let Ok((next, vals)) = take_namedkey("TODO").parse(input) {
                doc.todos = vals;
                input = next;
                continue;
            }
            if let Ok((next, vals)) = take_namedkey("Features").parse(input) {
                doc.features.push(("features", vals));
                input = next;
                continue;
            }
            break;
        }
        Ok(input)
    }
}

// ─── QapiSectionDocumentation ─────────────────────────────────────

#[derive(Debug, Clone)]
pub struct QapiSectionDocumentation<'i> {
    pub name: &'i str,
    pub description: Vec<&'i str>,
}

impl<'i> QapiSectionDocumentation<'i> {
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        let v = parser_version();
        if v < (2, 8, 0) {
            Self::parse_v1(input)
        } else if v < (5, 0, 0) {
            Self::parse_v2(input)
        } else {
            Self::parse_v3(input)
        }
    }

    /// v1 (QEMU v1.0–v2.7.0)
    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
        Self::parse_common(input, false)
    }

    /// v2 (QEMU v2.8.0–v4.2.0)
    pub fn parse_v2(input: &'i str) -> IResult<&'i str, Self> {
        Self::parse_common(input, false)
    }

    /// v3 (QEMU v5.0.0+)
    pub fn parse_v3(input: &'i str) -> IResult<&'i str, Self> {
        Self::parse_common(input, true)
    }

    fn parse_common(input: &'i str, allow_leading_ws: bool) -> IResult<&'i str, Self> {
        let input = if allow_leading_ws {
            input.trim_start()
        } else {
            input
        };
        let (input, _) = terminated(tag("##"), take_line_end).parse(input)?;
        let (input, _) = many0(take_empty_line).parse(input)?;
        // Section docs start with "= Section Name" or similar
        let (input, name) = preceded(
            take_line_start,
            terminated(
                preceded(tag("="), preceded(space0, not_line_ending)),
                line_ending,
            ),
        ).parse(input)?;
        let (input, _) = many0(take_empty_line).parse(input)?;
        let (input, desc_str) = opt(take_multiline_text).parse(input)?;
        let (input, _) = many0(take_empty_line).parse(input)?;
        let (input, _) = tag("##").parse(input)?;

        let description = desc_str
            .map(|s| s.lines().collect())
            .unwrap_or_default();

        Ok((input, Self { name: name.trim(), description }))
    }
}

// ─── QapiFreeformDoc ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct QapiFreeformDoc<'i> {
    pub text: &'i str,
}

impl<'i> QapiFreeformDoc<'i> {
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        let v = parser_version();
        if v < (2, 8, 0) {
            Self::parse_v1(input)
        } else if v < (5, 0, 0) {
            Self::parse_v2(input)
        } else {
            Self::parse_v3(input)
        }
    }

    /// v1 (QEMU v1.0–v2.7.0)
    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
        Self::parse_common(input, false)
    }

    /// v2 (QEMU v2.8.0–v4.2.0)
    pub fn parse_v2(input: &'i str) -> IResult<&'i str, Self> {
        Self::parse_common(input, false)
    }

    /// v3 (QEMU v5.0.0+)
    pub fn parse_v3(input: &'i str) -> IResult<&'i str, Self> {
        Self::parse_common(input, true)
    }

    fn parse_common(input: &'i str, allow_leading_ws: bool) -> IResult<&'i str, Self> {
        let input = if allow_leading_ws {
            input.trim_start()
        } else {
            input
        };
        let start = input;
        let (input, _) = terminated(tag("##"), take_line_end).parse(input)?;
        // Don't match if this is a doc block (starts with # @Name)
        // A freeform doc has NO @Name line — it's just free text
        // Check if this looks like a type doc block (has @Name)
        let check_input = {
            let mut tmp = input;
            while let Ok((next, _)) = take_empty_line(tmp) {
                tmp = next;
            }
            tmp
        };
        if check_input.trim_start_matches(|c: char| c == '#' || c == ' ').starts_with('@') {
            return Err(nom::Err::Error(Error::new(start, ErrorKind::Tag)));
        }
        // Consume lines starting with # until ##
        let mut input = input;
        loop {
            if let Ok((next, _)) = tag::<&str, &str, Error<&str>>("##").parse(input) {
                let consumed = &start[..start.len() - next.len()];
                return Ok((next, Self { text: consumed }));
            }
            if let Ok((next, _)) = take_empty_line(input) {
                input = next;
                continue;
            }
            if let Ok((next, _)) = preceded(take_line_start, terminated(not_line_ending, line_ending)).parse(input) {
                input = next;
                continue;
            }
            return Err(nom::Err::Error(Error::new(start, ErrorKind::Tag)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::set_parser_version;

    #[test]
    fn test_doc_v3_basic() {
        set_parser_version((10, 2, 0));
        let input = "##\n# @StatusInfo:\n#\n# Status info.\n#\n# @running: whether the VM is running\n#\n# Since: 0.14\n##";
        let result = QapiDocumentation::parse(input);
        match result {
            Ok((rem, doc)) => {
                assert_eq!(rem, "");
                assert_eq!(doc.name, "StatusInfo");
                assert!(!doc.fields.is_empty());
            }
            Err(e) => panic!("Doc parse failed: {}", e),
        }
    }

    #[test]
    fn test_doc_v1_optional_colon() {
        set_parser_version((2, 5, 0));
        let input = "##\n# @ErrorClass\n#\n# QEMU error classes\n#\n# Since: 1.2\n##";
        let result = QapiDocumentation::parse(input);
        match result {
            Ok((rem, doc)) => {
                assert_eq!(rem, "");
                assert_eq!(doc.name, "ErrorClass");
            }
            Err(e) => panic!("Doc v1 parse failed: {}", e),
        }
    }

    #[test]
    fn test_freeform_doc() {
        set_parser_version((10, 2, 0));
        let input = "##\n# Some free text\n# More text\n##";
        let result = QapiFreeformDoc::parse(input);
        match result {
            Ok((rem, _)) => assert_eq!(rem, ""),
            Err(e) => panic!("Freeform doc parse failed: {}", e),
        }
    }

    #[test]
    fn test_freeform_not_confused_with_doc() {
        set_parser_version((10, 2, 0));
        // This starts with @Name — should NOT parse as freeform
        let input = "##\n# @SomeName:\n# desc\n##";
        let result = QapiFreeformDoc::parse(input);
        assert!(result.is_err(), "Freeform should reject @Name blocks");
    }
}
