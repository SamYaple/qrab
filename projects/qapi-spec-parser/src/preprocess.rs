//! Schema preprocessing: normalizes older QEMU QAPI schema formats into
//! the canonical modern format expected by the strict parser.
//!
//! The parser is strict and only accepts the modern format (v10.2.0 style).
//! All older schema quirks are resolved here before parsing.

use regex::Regex;

pub type Version = (u32, u32, u32);

/// Preprocess schema content, normalizing old formats to canonical (modern) form.
/// The version parameter controls which transforms are applied.
pub fn preprocess(content: &str, version: Version) -> String {
    let mut content = content.to_string();

    // Structural transforms first (order matters)
    if version < (2, 4, 0) {
        content = fix_inline_structs(&content);
    }
    if version < (2, 4, 0) {
        content = fix_type_to_struct(&content);
    }
    if version < (2, 1, 0) {
        content = fix_empty_discriminator(&content);
    }
    if version < (6, 2, 0) {
        content = fix_simple_unions(&content);
    }
    if version < (5, 0, 0) {
        content = fix_old_pragma_keys(&content);
    }
    if version < (2, 4, 0) {
        content = fix_gen_string_to_bool(&content);
    }

    // Syntactic fixes (safe to apply to all versions)
    content = fix_missing_commas(&content);
    content = fix_trailing_commas(&content);

    // Documentation fixes
    content = fix_doc_blank_lines(&content);
    content = fix_doc_leading_whitespace(&content);
    content = fix_doc_since_colon(&content);
    content = fix_doc_name_colon(&content);
    if version < (2, 0, 0) {
        content = fix_doc_shorthand(&content);
    }
    content = fix_orphaned_doc_markers(&content);

    content
}

/// Replace known inline anonymous structs with named type references.
///
/// Only 4 cases exist across all QEMU versions (v1.0–v2.3.0):
/// - VersionInfo.qemu: {'major': 'int', 'minor': 'int', 'micro': 'int'}
/// - PciDeviceInfo.class_info: {'*desc': 'str', 'class': 'int'}
/// - PciDeviceInfo.id: {'device': 'int', 'vendor': 'int'}
/// - PciBridgeInfo.bus: {nested struct with multiple fields}
pub fn fix_inline_structs(content: &str) -> String {
    let mut result = content.to_string();

    // VersionInfo.qemu → VersionInfoQemu
    // Match: 'qemu': {'major': 'int', 'minor': 'int', 'micro': 'int'}
    let re = Regex::new(r"'qemu':\s*\{\s*'major':\s*'int',\s*'minor':\s*'int',\s*'micro':\s*'int'\s*\}").unwrap();
    if re.is_match(&result) {
        result = re.replace(&result, "'qemu': 'VersionInfoQemu'").to_string();
        // Insert the struct definition before VersionInfo
        let vi_re = Regex::new(r"(\{\s*'(?:type|struct)':\s*'VersionInfo')").unwrap();
        result = vi_re.replace(&result, "{ 'struct': 'VersionInfoQemu', 'data': {'major': 'int', 'minor': 'int', 'micro': 'int'} }\n$1").to_string();
    }

    // PciDeviceInfo.class_info → PciDeviceClass
    let re = Regex::new(r"'class_info':\s*\{\s*'\*desc':\s*'str',\s*'class':\s*'int'\s*\}").unwrap();
    if re.is_match(&result) {
        result = re.replace(&result, "'class_info': 'PciDeviceClass'").to_string();
        let pdi_re = Regex::new(r"(\{\s*'(?:type|struct)':\s*'PciDeviceInfo')").unwrap();
        result = pdi_re.replace(&result, "{ 'struct': 'PciDeviceClass', 'data': {'*desc': 'str', 'class': 'int'} }\n$1").to_string();
    }

    // PciDeviceInfo.id → PciDeviceId
    let re = Regex::new(r"'id':\s*\{\s*'device':\s*'int',\s*'vendor':\s*'int'\s*\}").unwrap();
    if re.is_match(&result) {
        result = re.replace(&result, "'id': 'PciDeviceId'").to_string();
        // Insert before PciDeviceClass (which we just inserted before PciDeviceInfo)
        let pdc_re = Regex::new(r"(\{\s*'struct':\s*'PciDeviceClass')").unwrap();
        if pdc_re.is_match(&result) {
            result = pdc_re.replace(&result, "{ 'struct': 'PciDeviceId', 'data': {'device': 'int', 'vendor': 'int'} }\n$1").to_string();
        } else {
            let pdi_re = Regex::new(r"(\{\s*'(?:type|struct)':\s*'PciDeviceInfo')").unwrap();
            result = pdi_re.replace(&result, "{ 'struct': 'PciDeviceId', 'data': {'device': 'int', 'vendor': 'int'} }\n$1").to_string();
        }
    }

    // PciBridgeInfo.bus → PciBusInfo (this is a multi-field inline struct)
    let re = Regex::new(
        r"'bus':\s*\{\s*'number':\s*'int',\s*'secondary':\s*'int',\s*'subordinate':\s*'int',\s*'io_range':\s*'PciMemoryRange',\s*'memory_range':\s*'PciMemoryRange',\s*'prefetchable_range':\s*'PciMemoryRange'\s*\}"
    ).unwrap();
    if re.is_match(&result) {
        result = re.replace(&result, "'bus': 'PciBusInfo'").to_string();
        let pbi_re = Regex::new(r"(\{\s*'(?:type|struct)':\s*'PciBridgeInfo')").unwrap();
        result = pbi_re.replace(&result, "{ 'struct': 'PciBusInfo', 'data': {'number': 'int', 'secondary': 'int', 'subordinate': 'int', 'io_range': 'PciMemoryRange', 'memory_range': 'PciMemoryRange', 'prefetchable_range': 'PciMemoryRange'} }\n$1").to_string();
    }

    result
}

/// Replace `{ 'type':` with `{ 'struct':` for struct definitions (v1.0–v2.3.0).
pub fn fix_type_to_struct(content: &str) -> String {
    // Only replace when 'type' is the first key in a top-level definition dict
    // Pattern: { 'type': 'SomeName' (start of a definition)
    let re = Regex::new(r"\{\s*'type':\s*'([A-Z])").unwrap();
    re.replace_all(content, "{ 'struct': '$1").to_string()
}

/// Convert `'discriminator': {}` alternate syntax to `'alternate'` keyword.
/// Used in v1.7.0–v2.0.0 for what later became 'alternate'.
pub fn fix_empty_discriminator(content: &str) -> String {
    // { 'union': 'Name', 'discriminator': {}, 'data': ... }
    // → { 'alternate': 'Name', 'data': ... }
    let re = Regex::new(
        r"\{\s*'union':\s*'([^']+)',\s*'discriminator':\s*\{\s*\},\s*'data'"
    ).unwrap();
    re.replace_all(content, "{ 'alternate': '$1', 'data'").to_string()
}

/// Convert simple unions (no discriminator/base) to flat union format.
///
/// A simple union: `{ 'union': 'X', 'data': { 'a': 'TypeA', 'b': 'TypeB' } }`
/// Becomes:
/// ```text
/// { 'enum': 'XKind', 'data': ['a', 'b'] }
/// { 'union': 'X', 'base': { 'type': 'XKind' }, 'discriminator': 'type',
///   'data': { 'a': 'TypeA', 'b': 'TypeB' } }
/// ```
pub fn fix_simple_unions(content: &str) -> String {
    let mut result = String::new();
    let mut remaining = content;

    while let Some(idx) = remaining.find("'union'") {
        // Find the opening brace before 'union'
        let before = &remaining[..idx];
        let brace_start = match before.rfind('{') {
            Some(pos) => pos,
            None => { result.push_str(&remaining[..idx + 7]); remaining = &remaining[idx + 7..]; continue; }
        };

        // Find the matching closing brace using brace counting
        let block = &remaining[brace_start..];
        let block_end = match find_matching_brace(block) {
            Some(end) => end,
            None => { result.push_str(&remaining[..idx + 7]); remaining = &remaining[idx + 7..]; continue; }
        };
        let full_block = &remaining[brace_start..brace_start + block_end + 1];

        // Skip if it has discriminator or base (it's already a flat union)
        if full_block.contains("'discriminator'") || full_block.contains("'base'") {
            result.push_str(&remaining[..brace_start + block_end + 1]);
            remaining = &remaining[brace_start + block_end + 1..];
            continue;
        }

        // Extract the union name
        let name_re = Regex::new(r"'union':\s*'([^']+)'").unwrap();
        let name = match name_re.captures(full_block) {
            Some(cap) => cap.get(1).unwrap().as_str().to_string(),
            None => { result.push_str(&remaining[..brace_start + block_end + 1]); remaining = &remaining[brace_start + block_end + 1..]; continue; }
        };

        // Extract branch names from 'data': { ... }
        let data_idx = match full_block.find("'data'") {
            Some(idx) => idx,
            None => { result.push_str(&remaining[..brace_start + block_end + 1]); remaining = &remaining[brace_start + block_end + 1..]; continue; }
        };
        // Find the data dict opening brace
        let data_brace = match full_block[data_idx..].find('{') {
            Some(pos) => data_idx + pos,
            None => { result.push_str(&remaining[..brace_start + block_end + 1]); remaining = &remaining[brace_start + block_end + 1..]; continue; }
        };
        let data_end = match find_matching_brace(&full_block[data_brace..]) {
            Some(end) => data_brace + end,
            None => { result.push_str(&remaining[..brace_start + block_end + 1]); remaining = &remaining[brace_start + block_end + 1..]; continue; }
        };
        let data_content = &full_block[data_brace + 1..data_end];

        let branch_re = Regex::new(r"'([^']+)'\s*:").unwrap();
        let branches: Vec<String> = branch_re.captures_iter(data_content)
            .map(|c| c.get(1).unwrap().as_str().to_string())
            .collect();

        if branches.is_empty() {
            result.push_str(&remaining[..brace_start + block_end + 1]);
            remaining = &remaining[brace_start + block_end + 1..];
            continue;
        }

        // Build the enum definition
        let enum_name = format!("{}Kind", name);
        let enum_variants: Vec<String> = branches.iter().map(|b| format!("'{}'", b)).collect();
        let enum_def = format!(
            "{{ 'enum': '{}', 'data': [{}] }}\n",
            enum_name,
            enum_variants.join(", ")
        );

        // Build the modified union with base + discriminator
        let data_part = &full_block[data_idx..=data_end];
        let modified_union = format!(
            "{{ 'union': '{}', 'base': {{ 'type': '{}' }}, 'discriminator': 'type', {} }}",
            name, enum_name, data_part
        );

        // Emit everything before the block, then the enum + modified union
        result.push_str(&remaining[..brace_start]);
        result.push_str(&enum_def);
        result.push_str(&modified_union);
        remaining = &remaining[brace_start + block_end + 1..];
    }

    result.push_str(remaining);
    result
}

/// Find the position of the matching closing brace for the opening brace at position 0.
fn find_matching_brace(s: &str) -> Option<usize> {
    let mut depth = 0;
    let mut in_string = false;
    for (i, c) in s.char_indices() {
        if in_string {
            if c == '\'' { in_string = false; }
            continue;
        }
        match c {
            '\'' => in_string = true,
            '{' => depth += 1,
            '}' => { depth -= 1; if depth == 0 { return Some(i); } }
            _ => {}
        }
    }
    None
}

/// Replace old pragma key names with modern equivalents.
pub fn fix_old_pragma_keys(content: &str) -> String {
    content
        .replace("'returns-whitelist'", "'command-returns-exceptions'")
        .replace("'name-case-whitelist'", "'member-name-exceptions'")
}

/// Replace `'gen': 'no'` with `'gen': false` and `'gen': 'yes'` with `'gen': true`.
pub fn fix_gen_string_to_bool(content: &str) -> String {
    content
        .replace("'gen': 'no'", "'gen': false")
        .replace("'gen': 'yes'", "'gen': true")
}

/// Insert missing commas between dict entries separated by newlines.
///
/// Pattern: `'value'\n  'key':` → `'value',\n  'key':`
/// This handles schemas where dict entries are separated by newlines without commas.
pub fn fix_missing_commas(content: &str) -> String {
    let re = Regex::new(r"'([^']*)'\s*\n(\s*'[^']+'\s*:)").unwrap();
    re.replace_all(content, "'$1',\n$2").to_string()
}

/// Remove trailing commas before closing brackets/braces.
pub fn fix_trailing_commas(content: &str) -> String {
    let re = Regex::new(r",(\s*[}\]])").unwrap();
    re.replace_all(content, "$1").to_string()
}

/// Replace blank lines inside `##...##` doc blocks with `#` (empty doc lines).
pub fn fix_doc_blank_lines(content: &str) -> String {
    let mut result = String::new();
    let mut in_doc_block = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "##" {
            if in_doc_block {
                // Closing ##
                in_doc_block = false;
            } else {
                // Opening ##
                in_doc_block = true;
            }
            result.push_str(line);
            result.push('\n');
        } else if in_doc_block && trimmed.is_empty() {
            // Blank line inside doc block → replace with '#'
            result.push_str("#\n");
        } else {
            if in_doc_block && !trimmed.starts_with('#') {
                // Non-# line inside doc block could mean the doc block was
                // terminated implicitly (some old schemas)
                in_doc_block = false;
            }
            result.push_str(line);
            result.push('\n');
        }
    }

    result
}

/// Strip leading whitespace before `##` markers.
pub fn fix_doc_leading_whitespace(content: &str) -> String {
    let re = Regex::new(r"(?m)^[ \t]+(##)").unwrap();
    re.replace_all(content, "$1").to_string()
}

/// Add colon after `Since` when followed by a version number.
/// `# Since X.Y` → `# Since: X.Y`
pub fn fix_doc_since_colon(content: &str) -> String {
    let re = Regex::new(r"(?m)^(#\s*)Since\s+(\d+\.\d+)").unwrap();
    re.replace_all(content, "${1}Since: $2").to_string()
}

/// Add colon after `@Name` in doc blocks when missing.
/// `# @SomeName\n` → `# @SomeName:\n`
pub fn fix_doc_name_colon(content: &str) -> String {
    // Match `# @Name` at end of line (no colon after name)
    let re = Regex::new(r"(?m)^(#\s*@\w+)\s*$").unwrap();
    re.replace_all(content, "$1:").to_string()
}

/// Transform shorthand doc format to standard multi-line form.
/// `## @Name` → `##\n# @Name:\n`
pub fn fix_doc_shorthand(content: &str) -> String {
    let re = Regex::new(r"(?m)^##\s+@(\w+)\s*:?\s*$").unwrap();
    re.replace_all(content, "##\n# @$1:").to_string()
}

/// Remove orphaned `##` markers that aren't part of a doc block.
/// An orphaned `##` is one that appears alone (not as an opening after a blank/non-doc line,
/// and not as a closing after doc content).
pub fn fix_orphaned_doc_markers(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut result = String::new();
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim();
        if trimmed == "##" {
            // Check if this is the START of a doc block: next line should be `# @...`
            // or `# =...` (section doc)
            let next = if i + 1 < lines.len() { lines[i + 1].trim() } else { "" };
            if next.starts_with("# @") || next.starts_with("# =") {
                // Valid opening ##
                result.push_str(lines[i]);
                result.push('\n');
            } else if i > 0 {
                // Check if previous content was doc lines (this is a closing ##)
                let prev = lines[i - 1].trim();
                if prev.starts_with('#') && prev != "##" {
                    // Valid closing ##
                    result.push_str(lines[i]);
                    result.push('\n');
                } else {
                    // Orphaned ## — skip it
                }
            } else {
                // First line is ## with no doc following — orphaned
            }
        } else {
            result.push_str(lines[i]);
            result.push('\n');
        }
        i += 1;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fix_type_to_struct() {
        let input = "{ 'type': 'NameInfo', 'data': {'*name': 'str'} }";
        let output = fix_type_to_struct(input);
        assert!(output.contains("'struct': 'NameInfo'"), "got: {}", output);
        assert!(!output.contains("'type':"), "type should be replaced");
    }

    #[test]
    fn test_fix_type_to_struct_leaves_lowercase() {
        // Should not replace 'type' when value starts with lowercase (it's a field, not a definition)
        let input = "'type': 'string'";
        let output = fix_type_to_struct(input);
        assert_eq!(input, output, "lowercase type values should not be touched");
    }

    #[test]
    fn test_fix_missing_commas() {
        let input = "{ 'enum': 'DataFormat'\n  'data': [ 'utf8', 'base64' ] }";
        let output = fix_missing_commas(input);
        assert!(output.contains("'DataFormat',\n  'data'"), "got: {}", output);
    }

    #[test]
    fn test_fix_missing_commas_no_false_positive() {
        // Already has comma — should not double-comma
        let input = "{ 'enum': 'RunState',\n  'data': [ 'debug' ] }";
        let output = fix_missing_commas(input);
        assert!(!output.contains(",,"), "should not introduce double comma");
    }

    #[test]
    fn test_fix_trailing_commas() {
        assert_eq!(fix_trailing_commas("['foo']"), "['foo']");
        assert_eq!(fix_trailing_commas("['foo' ]"), "['foo' ]");
        assert_eq!(fix_trailing_commas("{'a': 'b'}"), "{'a': 'b'}");

        // With trailing commas
        assert_eq!(fix_trailing_commas("['foo',]"), "['foo']");
        assert_eq!(fix_trailing_commas("{'a': 'b',}"), "{'a': 'b'}");
        assert_eq!(fix_trailing_commas("['foo', 'bar',\n]"), "['foo', 'bar'\n]");
    }

    #[test]
    fn test_fix_old_pragma_keys() {
        let input = "{ 'pragma': { 'returns-whitelist': ['x'], 'name-case-whitelist': ['y'] } }";
        let output = fix_old_pragma_keys(input);
        assert!(output.contains("'command-returns-exceptions'"), "got: {}", output);
        assert!(output.contains("'member-name-exceptions'"), "got: {}", output);
    }

    #[test]
    fn test_fix_gen_string_to_bool() {
        let input = "'gen': 'no' }";
        let output = fix_gen_string_to_bool(input);
        assert_eq!(output, "'gen': false }");

        let input = "'gen': 'yes' }";
        let output = fix_gen_string_to_bool(input);
        assert_eq!(output, "'gen': true }");
    }

    #[test]
    fn test_fix_doc_blank_lines() {
        let input = "##\n# @Name:\n\n# description\n##\n";
        let output = fix_doc_blank_lines(input);
        assert!(output.contains("# @Name:\n#\n# description"), "got: {}", output);
    }

    #[test]
    fn test_fix_doc_leading_whitespace() {
        let input = " ##\n# @Name:\n ##\n";
        let output = fix_doc_leading_whitespace(input);
        assert!(output.starts_with("##\n"), "got: {}", output);
        assert!(output.contains("\n##\n"), "got: {}", output);
    }

    #[test]
    fn test_fix_doc_since_colon() {
        let input = "# Since 0.14.0\n";
        let output = fix_doc_since_colon(input);
        assert_eq!(output, "# Since: 0.14.0\n");

        // Should not modify when colon already present
        let input = "# Since: 2.0\n";
        let output = fix_doc_since_colon(input);
        assert_eq!(output, "# Since: 2.0\n");

        // Should not match prose
        let input = "# Since some limit combinations are illegal\n";
        let output = fix_doc_since_colon(input);
        assert_eq!(output, "# Since some limit combinations are illegal\n");
    }

    #[test]
    fn test_fix_doc_name_colon() {
        let input = "# @ErrorClass\n# Description\n";
        let output = fix_doc_name_colon(input);
        assert!(output.starts_with("# @ErrorClass:\n"), "got: {}", output);

        // Should not modify when colon present
        let input = "# @ErrorClass:\n";
        let output = fix_doc_name_colon(input);
        assert_eq!(output, "# @ErrorClass:\n");
    }

    #[test]
    fn test_fix_doc_shorthand() {
        let input = "## @StatusInfo\n# Some description\n##\n";
        let output = fix_doc_shorthand(input);
        assert!(output.starts_with("##\n# @StatusInfo:\n"), "got: {}", output);
    }

    #[test]
    fn test_fix_empty_discriminator() {
        let input = "{ 'union': 'BlockdevRef', 'discriminator': {}, 'data': { 'definition': 'BlockdevOptions', 'reference': 'str' } }";
        let output = fix_empty_discriminator(input);
        assert!(output.contains("'alternate': 'BlockdevRef'"), "got: {}", output);
        assert!(!output.contains("'union'"), "union should be replaced");
        assert!(!output.contains("'discriminator'"), "discriminator should be removed");
    }

    #[test]
    fn test_fix_simple_unions() {
        let input = "{ 'union': 'BlockdevAction', 'data': { 'snapshot': 'BlockdevSnapshot', 'backup': 'DriveBackup' } }";
        let output = fix_simple_unions(input);

        // Should generate an enum
        assert!(output.contains("'enum': 'BlockdevActionKind'"), "got: {}", output);
        assert!(output.contains("'snapshot'"), "got: {}", output);
        assert!(output.contains("'backup'"), "got: {}", output);

        // Should add discriminator and base
        assert!(output.contains("'discriminator': 'type'"), "got: {}", output);
        assert!(output.contains("'base': { 'type': 'BlockdevActionKind' }"), "got: {}", output);
    }

    #[test]
    fn test_fix_simple_unions_leaves_flat_unions() {
        // Flat union with discriminator — should not be modified
        let input = "{ 'union': 'ImageInfoSpecific', 'base': 'ImageInfoSpecificBase', 'discriminator': 'format', 'data': { 'qcow2': 'QCow2' } }";
        let output = fix_simple_unions(input);
        assert_eq!(input, output, "flat union should not be modified");
    }

    #[test]
    fn test_fix_inline_structs_version_info() {
        let input = "{ 'type': 'VersionInfo', 'data': {'qemu': {'major': 'int', 'minor': 'int', 'micro': 'int'}, 'package': 'str'} }";
        let output = fix_inline_structs(input);
        assert!(output.contains("'struct': 'VersionInfoQemu'"), "got: {}", output);
        assert!(output.contains("'qemu': 'VersionInfoQemu'"), "got: {}", output);
    }

    #[test]
    fn test_fix_inline_structs_pci_device_info() {
        let input = "{ 'type': 'PciDeviceInfo', 'data': {'class_info': {'*desc': 'str', 'class': 'int'}, 'id': {'device': 'int', 'vendor': 'int'}} }";
        let output = fix_inline_structs(input);
        assert!(output.contains("'struct': 'PciDeviceClass'"), "got: {}", output);
        assert!(output.contains("'struct': 'PciDeviceId'"), "got: {}", output);
        assert!(output.contains("'class_info': 'PciDeviceClass'"), "got: {}", output);
        assert!(output.contains("'id': 'PciDeviceId'"), "got: {}", output);
    }

    #[test]
    fn test_fix_inline_structs_pci_bridge_info() {
        let input = "{ 'type': 'PciBridgeInfo', 'data': {'bus': { 'number': 'int', 'secondary': 'int', 'subordinate': 'int', 'io_range': 'PciMemoryRange', 'memory_range': 'PciMemoryRange', 'prefetchable_range': 'PciMemoryRange' }} }";
        let output = fix_inline_structs(input);
        assert!(output.contains("'struct': 'PciBusInfo'"), "got: {}", output);
        assert!(output.contains("'bus': 'PciBusInfo'"), "got: {}", output);
    }

    #[test]
    fn test_fix_orphaned_doc_markers() {
        // Orphaned ## (not followed by doc content, not preceded by doc lines)
        let input = "some code\n##\n{ 'struct': 'Foo' }\n";
        let output = fix_orphaned_doc_markers(input);
        assert!(!output.contains("\n##\n{"), "orphaned ## should be removed, got: {}", output);

        // Valid doc block should be preserved
        let input = "##\n# @Foo:\n# desc\n##\n";
        let output = fix_orphaned_doc_markers(input);
        assert_eq!(input, output, "valid doc block should be preserved");
    }

    #[test]
    fn test_preprocess_modern_schema_unchanged() {
        // A modern schema entry should pass through preprocessing unchanged
        // (except for potential trailing newline normalization)
        let input = "##\n# @StatusInfo:\n#\n# Status info\n#\n# @running: true if running\n#\n# Since: 0.14\n##\n{ 'struct': 'StatusInfo', 'data': {'running': 'bool'} }\n";
        let output = preprocess(input, (10, 2, 0));
        assert_eq!(input, output, "modern schema should not be modified");
    }

    #[test]
    fn test_preprocess_full_pipeline_v1() {
        // A realistic v1.0 style entry
        let input = concat!(
            "## @NameInfo\n",
            "# Guest name information.\n",
            "#\n",
            "# @name: #optional The name\n",
            "#\n",
            "# Since 0.14.0\n",
            "##\n",
            "{ 'type': 'NameInfo', 'data': {'*name': 'str'} }\n",
        );
        let output = preprocess(input, (1, 0, 0));
        // Should have: struct (not type), since with colon, @NameInfo with colon
        assert!(output.contains("'struct': 'NameInfo'"), "type→struct, got: {}", output);
        assert!(output.contains("Since: 0.14.0"), "since colon, got: {}", output);
        assert!(output.contains("@NameInfo:"), "name colon, got: {}", output);
    }
}
