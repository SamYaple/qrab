//! Schema preprocessing: minimal syntactic fixes applied before parsing.
//!
//! The parser handles version-specific format differences via ParserConfig.
//! Preprocessing only fixes small syntactic issues that are clearer as
//! simple string replacements than parser modifications.

pub type Version = (u32, u32, u32);

/// Apply minimal preprocessing fixes to schema content.
/// Only small syntactic fixes — no structural transformations.
pub fn minimal_preprocess(content: &str) -> String {
    let mut content = content.to_string();
    // Fix malformed doc end marker (typo in v2.0.0-v2.7.0): ### → ##
    content = content.replace("\n###\n", "\n##\n");
    // Fix shorthand doc format: `## @Name` → `##\n# @Name:`
    // Used in v2.4.0-v2.7.0 for a few types (ACPISlotType, ACPIOSTInfo)
    content = fix_doc_shorthand(&content);
    // Strip leading whitespace from # and ## doc lines (some versions indent entire doc blocks)
    content = strip_doc_leading_whitespace(&content);
    // Normalize spacing around colons: 'key' : → 'key':
    content = fix_key_colon_spacing(&content);
    // Insert missing commas between dict entries separated by newlines
    content = fix_missing_commas(&content);
    // Remove trailing commas before ] or }
    content = fix_trailing_commas(&content);
    content
}

/// Strip leading whitespace from lines that start with `#` or `##`.
/// Some QEMU schema versions (v3.0.0+) indent entire doc blocks.
fn strip_doc_leading_whitespace(content: &str) -> String {
    let mut result: String = content
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with('#') {
                trimmed
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    // Preserve trailing newline if original had one
    if content.ends_with('\n') && !result.ends_with('\n') {
        result.push('\n');
    }
    result
}

/// Normalize spacing around colons in dict entries: `'key' :` → `'key':`.
pub fn fix_key_colon_spacing(content: &str) -> String {
    let re = regex::Regex::new(r"'(\s+):").unwrap();
    re.replace_all(content, "':").to_string()
}

/// Insert missing commas between dict entries separated by newlines.
/// Pattern: a value at end of line followed by a key on the next line without a comma.
pub fn fix_missing_commas(content: &str) -> String {
    let re = regex::Regex::new(r"(?m)(')\s*$
(\s*')").unwrap();
    re.replace_all(content, "$1,\n$2").to_string()
}

/// Remove trailing commas before `]` or `}`.
pub fn fix_trailing_commas(content: &str) -> String {
    let re = regex::Regex::new(r",\s*([}\]])").unwrap();
    re.replace_all(content, "$1").to_string()
}

/// Fix shorthand doc format: `## @Name` on a single line → `##\n# @Name:`.
pub fn fix_doc_shorthand(content: &str) -> String {
    let re = regex::Regex::new(r"(?m)^##\s+@(\w[\w-]*)(.*)$").unwrap();
    re.replace_all(content, |caps: &regex::Captures| {
        let name = &caps[1];
        let rest = caps[2].trim();
        if rest.is_empty() {
            format!("##\n# @{}:", name)
        } else {
            format!("##\n# @{}:\n# {}", name, rest)
        }
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fix_missing_commas() {
        let input = "{ 'enum': 'DataFormat'\n  'data': ['utf8'] }";
        let result = fix_missing_commas(input);
        assert!(result.contains("'DataFormat',\n  'data'"), "Should insert comma: {}", result);
    }

    #[test]
    fn test_fix_missing_commas_no_false_positive() {
        let input = "{ 'enum': 'Test',\n  'data': ['a'] }";
        let result = fix_missing_commas(input);
        assert_eq!(result, input, "Should not modify already-comma'd input");
    }

    #[test]
    fn test_fix_trailing_commas() {
        let input = "['foo', 'bar',]";
        let result = fix_trailing_commas(input);
        assert_eq!(result, "['foo', 'bar']");
    }

    #[test]
    fn test_fix_doc_shorthand() {
        let input = "## @ACPISlotType\n# doc text";
        let result = fix_doc_shorthand(input);
        assert!(result.starts_with("##\n# @ACPISlotType:"), "Got: {}", result);
    }

    #[test]
    fn test_fix_key_colon_spacing() {
        let input = "'key' : 'value'";
        let result = fix_key_colon_spacing(input);
        assert_eq!(result, "'key': 'value'");
    }

    #[test]
    fn test_strip_doc_leading_whitespace() {
        let input = "  ## indented\n  # doc line\nnot indented";
        let result = strip_doc_leading_whitespace(input);
        assert_eq!(result, "## indented\n# doc line\nnot indented");
    }

    #[test]
    fn test_minimal_preprocess() {
        let input = "{ 'enum': 'Test'\n  'data': ['a',] }";
        let result = minimal_preprocess(input);
        assert!(result.contains("'Test',\n"), "Should fix missing comma");
        assert!(result.contains("['a']"), "Should fix trailing comma: {}", result);
    }

    #[test]
    fn test_minimal_preprocess_no_structural_changes() {
        // Ensure minimal_preprocess does NOT do structural changes
        let input = "{ 'type': 'Foo', 'data': {} }";
        let result = minimal_preprocess(input);
        assert!(result.contains("'type'"), "Should NOT convert type to struct");
    }

    #[test]
    fn test_triple_hash_fix() {
        let input = "some text\n###\nmore text";
        let result = minimal_preprocess(input);
        assert!(result.contains("\n##\n"), "Should fix ### to ##");
    }
}
