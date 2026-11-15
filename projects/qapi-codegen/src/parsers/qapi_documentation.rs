use super::{
    common::{line_ending, lines, not_line_ending},
    parser_version,
};
use nom::IResult;
use nom::Parser;
use nom::bytes::complete::{tag, tag_no_case};
use nom::character::complete::space0;
use nom::combinator::{not, opt, peek, recognize};
use nom::error::{Error, ErrorKind};
use nom::multi::{many0, many1};
use nom::sequence::{preceded, terminated};

// ─── Shared helpers ────────────────────────────────────────────────

fn not_name_break(c: char) -> bool {
    c.is_alphanumeric() || c == '-' || c == '_' || c == '.'
}

fn take_line_start(input: &str) -> IResult<&str, &str> {
    // Accept optional leading whitespace before # (some versions indent doc blocks)
    recognize((space0, tag("#"), not(peek(tag("#"))), space0)).parse(input)
}

fn take_line_end(input: &str) -> IResult<&str, &str> {
    recognize((space0, line_ending)).parse(input)
}

fn take_empty_line(input: &str) -> IResult<&str, &str> {
    recognize((take_line_start, take_line_end)).parse(input)
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

#[derive(Debug, Clone, Default)]
pub struct QapiDocumentation<'i> {
    pub name: &'i str,
    pub description: Vec<&'i str>,
    /// Ordered prose and sections, excluding member descriptions and Since metadata.
    pub body: Vec<&'i str>,
    pub fields: Vec<(&'i str, Vec<&'i str>)>,
    pub features: Vec<(&'i str, Vec<&'i str>)>,
    pub since: Option<&'i str>,
    pub returns: Vec<&'i str>,
    pub errors: Vec<&'i str>,
    pub notes: Vec<&'i str>,
    pub examples: Vec<&'i str>,
    pub todos: Vec<&'i str>,
}

fn documentation_line(input: &str) -> IResult<&str, &str> {
    let (input, _) = (space0, tag("#"), not(peek(tag("#"))), opt(tag(" "))).parse(input)?;
    terminated(not_line_ending, line_ending).parse(input)
}

fn documentation_end(input: &str) -> IResult<&str, &str> {
    recognize((
        space0,
        tag("##"),
        space0,
        peek(nom::branch::alt((line_ending, nom::combinator::eof))),
    ))
    .parse(input)
}

impl<'i> QapiDocumentation<'i> {
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        if parser_version() < (2, 9, 0) {
            Self::parse_v1(input)
        } else if parser_version() < (5, 0, 0) {
            Self::parse_v2(input)
        } else {
            Self::parse_v3(input)
        }
    }

    /// Legacy headers permit an omitted colon and `## @Name` shorthand.
    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
        Self::parse_common(input, true)
    }

    pub fn parse_v2(input: &'i str) -> IResult<&'i str, Self> {
        Self::parse_common(input, false)
    }

    pub fn parse_v3(input: &'i str) -> IResult<&'i str, Self> {
        Self::parse_common(input, false)
    }

    fn parse_common(input: &'i str, legacy: bool) -> IResult<&'i str, Self> {
        let start = input;
        let (input, _) = (space0, tag("##")).parse(input)?;
        let (mut input, opening) = terminated(not_line_ending, line_ending).parse(input)?;
        let header = if legacy && opening.trim_start().starts_with('@') {
            opening.trim()
        } else {
            if !opening.trim().is_empty() {
                return Err(nom::Err::Error(Error::new(start, ErrorKind::Tag)));
            }
            loop {
                let (next, _) = many0((space0, line_ending)).parse(input)?;
                input = next;
                let (next, line) = documentation_line(input)?;
                input = next;
                if !line.trim().is_empty() {
                    break line.trim();
                }
            }
        };
        let name = header
            .strip_prefix('@')
            .and_then(|name| name.strip_suffix(':').or_else(|| legacy.then_some(name)))
            .filter(|name| !name.is_empty() && name.chars().all(not_name_break))
            .ok_or_else(|| nom::Err::Error(Error::new(start, ErrorKind::Tag)))?;
        let mut lines = Vec::new();
        loop {
            let end = if legacy {
                recognize((
                    space0,
                    tag("##"),
                    nom::bytes::complete::take_while(|c| c == '#'),
                    space0,
                    peek(nom::branch::alt((line_ending, nom::combinator::eof))),
                ))
                .parse(input)
            } else {
                documentation_end(input)
            };
            if let Ok((next, _)) = end {
                let mut doc = Self {
                    name,
                    ..Self::default()
                };
                doc.parse_lines(&lines, legacy);
                return Ok((next, doc));
            }
            if let Ok((next, _)) = (space0::<&str, Error<&str>>, line_ending).parse(input) {
                input = next;
                continue;
            }
            match documentation_line(input) {
                Ok((next, line)) => {
                    lines.push(line);
                    input = next;
                }
                Err(nom::Err::Error(_)) if legacy => {
                    let mut doc = Self {
                        name,
                        ..Self::default()
                    };
                    doc.parse_lines(&lines, legacy);
                    return Ok((input, doc));
                }
                Err(error) => return Err(error),
            }
        }
    }

    fn parse_lines(&mut self, lines: &[&'i str], legacy: bool) {
        #[derive(Clone, Copy)]
        enum Section {
            Description,
            Field(usize),
            Feature(usize),
            Returns,
            Errors,
            Notes,
            Examples,
            Todos,
        }
        fn section_lines<'a, 'i>(
            doc: &'a mut QapiDocumentation<'i>,
            section: Section,
        ) -> &'a mut Vec<&'i str> {
            match section {
                Section::Description => &mut doc.description,
                Section::Field(index) => &mut doc.fields[index].1,
                Section::Feature(index) => &mut doc.features[index].1,
                Section::Returns => &mut doc.returns,
                Section::Errors => &mut doc.errors,
                Section::Notes => &mut doc.notes,
                Section::Examples => &mut doc.examples,
                Section::Todos => &mut doc.todos,
            }
        }
        let mut section = Section::Description;
        let mut features = false;
        let mut previous_blank = true;
        for &line in lines {
            let indented = line.starts_with([' ', '\t']);
            if !indented {
                if let Some((name, value)) = line
                    .strip_prefix('@')
                    .and_then(|text| text.split_once(':'))
                    .filter(|(name, _)| !name.is_empty() && name.chars().all(not_name_break))
                {
                    let target = if features {
                        &mut self.features
                    } else {
                        &mut self.fields
                    };
                    target.push((name, vec![value.trim_start()]));
                    if features {
                        self.body.push(line);
                    }
                    section = if features {
                        Section::Feature(target.len() - 1)
                    } else {
                        Section::Field(target.len() - 1)
                    };
                    previous_blank = false;
                    continue;
                }
                if let Some((name, value)) = line
                    .split_once(':')
                    .filter(|(_, value)| !value.starts_with(':'))
                {
                    let next = match name.to_ascii_lowercase().as_str() {
                        "returns" => Some(Section::Returns),
                        "errors" => Some(Section::Errors),
                        "note" | "notes" => Some(Section::Notes),
                        "example" | "examples" => Some(Section::Examples),
                        "todo" => Some(Section::Todos),
                        "features" if value.trim().is_empty() => {
                            self.body.push(line);
                            features = true;
                            section = Section::Description;
                            previous_blank = true;
                            continue;
                        }
                        "since" => {
                            self.since = Some(value.trim().trim_end_matches('.'));
                            features = false;
                            section = Section::Description;
                            previous_blank = true;
                            continue;
                        }
                        _ => None,
                    };
                    if let Some(next) = next {
                        self.body.push(line);
                        section = next;
                        features = false;
                        section_lines(self, section).push(value.trim_start());
                        previous_blank = false;
                        continue;
                    }
                }
                if legacy
                    && line
                        .get(..5)
                        .is_some_and(|word| word.eq_ignore_ascii_case("since"))
                    && line.as_bytes().get(5).is_some_and(u8::is_ascii_whitespace)
                {
                    self.since = Some(line[5..].trim().trim_end_matches('.'));
                    section = Section::Description;
                    features = false;
                    previous_blank = true;
                    continue;
                }
                if previous_blank
                    && !line.is_empty()
                    && (matches!(section, Section::Field(_) | Section::Feature(_))
                        || (!legacy && !matches!(section, Section::Description)))
                {
                    section = Section::Description;
                    features = false;
                }
            }
            if !matches!(section, Section::Field(_)) {
                self.body.push(line);
            }
            match section {
                Section::Description => self.description.push(line),
                Section::Field(index) => self.fields[index].1.push(line),
                Section::Feature(index) => self.features[index].1.push(line),
                _ => section_lines(self, section).push(line),
            }
            previous_blank = line.trim().is_empty();
        }
        fn trim_lines(lines: &mut Vec<&str>) {
            while lines.first().is_some_and(|line| line.trim().is_empty()) {
                lines.remove(0);
            }
            while lines.last().is_some_and(|line| line.trim().is_empty()) {
                lines.pop();
            }
        }
        for lines in [
            &mut self.body,
            &mut self.description,
            &mut self.returns,
            &mut self.errors,
            &mut self.notes,
            &mut self.examples,
            &mut self.todos,
        ] {
            trim_lines(lines);
        }
        for (_, lines) in self.fields.iter_mut().chain(self.features.iter_mut()) {
            trim_lines(lines);
        }
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
        if v < (2, 9, 0) {
            Self::parse_v1(input)
        } else if v < (5, 0, 0) {
            Self::parse_v2(input)
        } else {
            Self::parse_v3(input)
        }
    }

    /// v1 (QEMU v1.0–v2.8.0)
    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
        Self::parse_common(input, false)
    }

    /// v2 (QEMU v2.9.0–v4.2.0)
    pub fn parse_v2(input: &'i str) -> IResult<&'i str, Self> {
        Self::parse_common(input, false)
    }

    /// v3 (QEMU v5.0.0+)
    pub fn parse_v3(input: &'i str) -> IResult<&'i str, Self> {
        Self::parse_common(input, true)
    }

    fn parse_common(input: &'i str, _allow_leading_ws: bool) -> IResult<&'i str, Self> {
        // Always accept optional leading whitespace (harmless)
        let (input, _) = terminated(preceded(space0, tag("##")), take_line_end).parse(input)?;
        let (input, _) = many0(take_empty_line).parse(input)?;
        // Section docs start with "= Section Name" or similar
        let (input, name) = preceded(
            take_line_start,
            terminated(
                preceded(tag("="), preceded(space0, not_line_ending)),
                line_ending,
            ),
        )
        .parse(input)?;
        let (input, _) = many0(take_empty_line).parse(input)?;
        let (input, desc_str) = opt(take_multiline_text).parse(input)?;
        let (input, _) = many0(take_empty_line).parse(input)?;
        let (input, _) = preceded(space0, tag("##")).parse(input)?;

        let description = desc_str.map(|s| lines(s).collect()).unwrap_or_default();

        Ok((
            input,
            Self {
                name: name.trim(),
                description,
            },
        ))
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
        if v < (2, 9, 0) {
            Self::parse_v1(input)
        } else if v < (5, 0, 0) {
            Self::parse_v2(input)
        } else {
            Self::parse_v3(input)
        }
    }

    /// v1 (QEMU v1.0–v2.8.0)
    pub fn parse_v1(input: &'i str) -> IResult<&'i str, Self> {
        Self::parse_common(input, false)
    }

    /// v2 (QEMU v2.9.0–v4.2.0)
    pub fn parse_v2(input: &'i str) -> IResult<&'i str, Self> {
        Self::parse_common(input, false)
    }

    /// v3 (QEMU v5.0.0+)
    pub fn parse_v3(input: &'i str) -> IResult<&'i str, Self> {
        Self::parse_common(input, true)
    }

    fn parse_common(input: &'i str, _allow_leading_ws: bool) -> IResult<&'i str, Self> {
        // Always accept optional leading whitespace (harmless)
        let start = input;
        let (input, _) = terminated(preceded(space0, tag("##")), take_line_end).parse(input)?;
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
        if check_input.trim_start_matches(['#', ' ']).starts_with('@') {
            return Err(nom::Err::Error(Error::new(start, ErrorKind::Tag)));
        }
        // Consume lines starting with # until ##
        let mut input = input;
        loop {
            if let Ok((next, _)) = preceded(
                space0::<&str, Error<&str>>,
                tag::<&str, &str, Error<&str>>("##"),
            )
            .parse(input)
            {
                let consumed = &start[..start.len() - next.len()];
                return Ok((next, Self { text: consumed }));
            }
            if let Ok((next, _)) = take_empty_line(input) {
                input = next;
                continue;
            }
            if let Ok((next, _)) =
                preceded(take_line_start, terminated(not_line_ending, line_ending)).parse(input)
            {
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
    fn legacy_docs_allow_bare_blank_lines_and_triple_hash_terminators() {
        set_parser_version((1, 6, 0));
        let input = "##\n# @Example\n# first paragraph\n\n# continuation\n###\n{ 'type': 'Example', 'data': {} }";
        let (remaining, doc) = QapiDocumentation::parse(input).unwrap();
        assert!(remaining.starts_with("\n{ 'type'"));
        assert_eq!(doc.description, ["first paragraph", "continuation"]);
    }

    #[test]
    fn block_boundaries_sections_and_literal_indentation_are_preserved() {
        set_parser_version((11, 1, 3));
        let input = concat!(
            "##\n# @Example:\n#\n# # Heading\n#\n",
            "# @member: first line\n#     indented continuation\n#\n",
            "# Features:\n# @flag: feature text\n#\n",
            "# Returns: inline result\n#\n# Errors: failure details\n#\n",
            "# Example::\n#\n#     # literal comment\n#     @not-a-field: value\n#\n",
            "# Since: 11.0\n##\n{ 'struct': 'Example', 'data': {} }",
        );
        let (remaining, doc) = QapiDocumentation::parse(input).unwrap();
        assert!(remaining.starts_with("\n{ 'struct'"));
        assert_eq!(doc.fields.len(), 1);
        assert_eq!(doc.fields[0].0, "member");
        assert_eq!(doc.fields[0].1, ["first line", "    indented continuation"]);
        assert_eq!(doc.features, [("flag", vec!["feature text"])]);
        assert_eq!(doc.returns, ["inline result"]);
        assert_eq!(doc.errors, ["failure details"]);
        assert_eq!(doc.since, Some("11.0"));
        assert!(doc.body.contains(&"# Heading"));
        assert!(doc.body.contains(&"    # literal comment"));
        assert!(doc.body.contains(&"    @not-a-field: value"));
        assert!(!doc.body.iter().any(|line| line.starts_with("@member:")));
    }

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
    fn test_doc_v1_shorthand() {
        set_parser_version((2, 5, 0));
        let input = "## @ACPISlotType\n#\n# @DIMM: memory slot\n#\n# Since: 2.1\n##";
        let result = QapiDocumentation::parse(input);
        match result {
            Ok((rem, doc)) => {
                assert_eq!(rem, "");
                assert_eq!(doc.name, "ACPISlotType");
                assert!(!doc.fields.is_empty(), "Should have field @DIMM");
            }
            Err(e) => panic!("Doc v1 shorthand parse failed: {}", e),
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

    #[test]
    fn test_doc_v2_indented_block() {
        set_parser_version((3, 0, 0));
        let input = " ##\n # @DisplayGLMode:\n #\n # Display OpenGL mode.\n #\n # @off: Disable OpenGL (default).\n #\n # Since: 3.0\n #\n ##";
        let result = QapiDocumentation::parse(input);
        match result {
            Ok((rem, doc)) => {
                assert_eq!(rem, "");
                assert_eq!(doc.name, "DisplayGLMode");
                assert!(!doc.fields.is_empty(), "Should have field @off");
            }
            Err(e) => panic!("Indented doc block should parse: {}", e),
        }
    }
}
