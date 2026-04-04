//! Schema preprocessing: normalizes older QEMU QAPI schema formats into
//! the canonical modern format expected by the strict parser.
//!
//! The parser is strict and only accepts the modern format (v10.2.0 style).
//! All older schema quirks are resolved here before parsing.

use regex::Regex;

pub type Version = (u32, u32, u32);

/// Minimal preprocessing: only simple syntactic fixes that don't change semantics.
/// Used when the parser handles format differences via ParserConfig.
pub fn minimal_preprocess(content: &str) -> String {
    let mut content = content.to_string();
    // Fix malformed doc end marker (typo in v2.0.0-v2.7.0): ### → ##
    content = content.replace("\n###\n", "\n##\n");
    // Normalize spacing around colons: 'key' : → 'key':
    content = fix_key_colon_spacing(&content);
    // Insert missing commas between dict entries separated by newlines
    content = fix_missing_commas(&content);
    // Remove trailing commas before ] or }
    content = fix_trailing_commas(&content);
    content
}

/// Preprocess schema content, normalizing old formats to canonical (modern) form.
/// The version parameter controls which transforms are applied.
pub fn preprocess(content: &str, version: Version) -> String {
    let mut content = content.to_string();

    // Strip inline comments first (before any other processing).
    // This removes file headers, standalone comments, and inline comments
    // in JSON data blocks that would confuse later transforms.
    content = strip_inline_comments(&content);

    // Syntactic normalization first (spacing, commas) — later transforms depend on this
    content = fix_key_colon_spacing(&content);
    content = fix_missing_commas(&content);
    content = fix_trailing_commas(&content);

    // Structural transforms (order matters)
    if version < (2, 4, 0) {
        content = fix_inline_structs(&content);
    }
    if version < (2, 4, 0) {
        content = fix_type_to_struct(&content);
    }
    if version < (2, 4, 0) {
        content = fix_empty_discriminator(&content);
    }
    if version < (6, 2, 0) {
        content = fix_simple_unions(&content);
    }
    if version < (6, 0, 0) {
        content = fix_old_pragma_keys(&content);
    }
    if version < (2, 4, 0) {
        content = fix_gen_string_to_bool(&content);
    }

    // Documentation processing:
    // 1. Convert RST section headers (*** or === decorations)
    // 2. Strip all doc blocks down to essential info (name + since)
    content = fix_rst_section_headers(&content);
    content = simplify_all_doc_blocks(&content);

    content
}

/// Simplify ALL doc blocks to just the essential info: name + since.
///
/// Replaces complex doc blocks like:
/// ```text
/// ##
/// # @TypeName:
/// #
/// # Long description with various formats...
/// # @field: description
/// # Returns: something
/// # Since: 2.0
/// ##
/// ```
/// With:
/// ```text
/// ##
/// # @TypeName:
/// #
/// # Since: 2.0
/// ##
/// ```
///
/// This avoids the need to handle every doc format quirk across all QEMU versions.
pub fn simplify_all_doc_blocks(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut result = String::new();
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim();

        // Handle shorthand format: ## @Name or ## Name (name on same line as ##)
        if trimmed.starts_with("## ") && trimmed.len() > 3 {
            let rest = &trimmed[3..];
            if rest.starts_with('@') || rest.starts_with(|c: char| c.is_alphabetic()) {
                // Find closing ## and extract since
                let _start_i = i;
                i += 1;
                let mut since_line = None;
                while i < lines.len() && lines[i].trim() != "##" {
                    let t = lines[i].trim();
                    if let Some(r) = t.strip_prefix("# Since:") {
                        since_line = Some(format!("# Since:{}", r));
                    } else if let Some(r) = t.strip_prefix("# Since ").filter(|r| r.starts_with(|c: char| c.is_ascii_digit())) {
                        since_line = Some(format!("# Since: {}", r));
                    }
                    i += 1;
                }
                if i < lines.len() { i += 1; } // skip closing ##

                // Normalize name
                let name = if rest.starts_with('@') {
                    let n = rest.trim_end_matches(':').trim();
                    format!("# @{}:", &n[1..])
                } else {
                    let n = rest.split(':').next().unwrap_or(rest).trim();
                    format!("# @{}:", n)
                };

                result.push_str("##\n");
                result.push_str(&name);
                result.push('\n');
                result.push_str("#\n");
                if let Some(since) = since_line {
                    result.push_str(&since);
                    result.push('\n');
                }
                result.push_str("##\n");
                continue;
            }
        }

        // Look for opening ## followed by # @Name: or # = Section
        if trimmed == "##" && i + 1 < lines.len() {
            // Scan forward to find the doc block content and closing ##
            let _open_idx = i;
            i += 1;

            // Skip empty # lines
            while i < lines.len() && lines[i].trim() == "#" {
                i += 1;
            }

            // Check if this is a name line (# @Name:) or section (# = Title) or shorthand (## @Name)
            let name_line = if i < lines.len() {
                let t = lines[i].trim();
                if t.starts_with("# @") || t.starts_with("# =") {
                    let l = lines[i];
                    i += 1;
                    Some(l)
                } else if t.starts_with("#") && !t.starts_with("##") {
                    // Might be a name without @ prefix (old format): `# Name:`
                    // Or a description line after shorthand `## @Name`
                    let l = lines[i];
                    i += 1;
                    Some(l)
                } else {
                    None
                }
            } else {
                None
            };

            // Find the closing ## and extract Since: if present
            let mut since_line = None;
            let mut returns_line = None;
            while i < lines.len() && lines[i].trim() != "##" {
                let t = lines[i].trim();
                // Look for Since: line
                if let Some(rest) = t.strip_prefix("# Since:") {
                    since_line = Some(format!("# Since:{}", rest));
                } else if let Some(rest) = t.strip_prefix("# Since ") {
                    // Old format: # Since 0.14.0
                    if rest.starts_with(|c: char| c.is_ascii_digit()) {
                        since_line = Some(format!("# Since: {}", rest));
                    }
                }
                // Look for Returns: line (single-line only)
                if t.starts_with("# Returns:") && t.len() > 11 {
                    returns_line = Some(lines[i].to_string());
                }
                i += 1;
            }

            // Now i points to closing ## (or end of file)
            if i < lines.len() {
                i += 1; // skip closing ##
            }

            // Emit simplified doc block
            if let Some(name) = name_line {
                let name_trimmed = name.trim();
                // Ensure name has @ and : 
                let fixed_name = if name_trimmed.starts_with("# =") {
                    // Section doc
                    name_trimmed.to_string()
                } else if name_trimmed.starts_with("# @") {
                    // Already has @, ensure it has :
                    if !name_trimmed.contains(':') {
                        format!("{}:", name_trimmed)
                    } else {
                        name_trimmed.to_string()
                    }
                } else if let Some(rest) = name_trimmed.strip_prefix("# ") {
                    // No @, add it and ensure :
                    let name_part = rest.split(':').next().unwrap_or(rest).trim();
                    if name_part.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
                        format!("# @{}:", name_part)
                    } else {
                        // Not a type name, might be a description — skip this doc block
                        continue;
                    }
                } else {
                    continue;
                };

                result.push_str("##\n");
                result.push_str(&fixed_name);
                result.push('\n');
                result.push_str("#\n");
                if let Some(since) = since_line {
                    result.push_str(&since);
                    result.push('\n');
                }
                if let Some(returns) = returns_line {
                    result.push_str(&returns);
                    result.push('\n');
                }
                result.push_str("##\n");
            }
            // If no name line, skip the entire doc block (orphaned/section)
        } else {
            result.push_str(lines[i]);
            result.push('\n');
            i += 1;
        }
    }

    result
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
    // Allow optional spaces before the colon
    let re = Regex::new(r"\{\s*'type'\s*:\s*'([A-Z])").unwrap();
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
    let name_re = Regex::new(r"'union':\s*'([^']+)'").unwrap();
    let branch_re = Regex::new(r"'([^']+)'\s*:").unwrap();

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

/// Normalize spacing around colons in dict keys: `'key' :` → `'key':`.
/// Some QEMU schemas use extra spaces before colons.
pub fn fix_key_colon_spacing(content: &str) -> String {
    let re = Regex::new(r"'(\w[\w-]*)'\s+:").unwrap();
    re.replace_all(content, "'$1':").to_string()
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
        } else if in_doc_block && (trimmed.is_empty() || trimmed == "#" || trimmed.trim_start_matches('#').trim().is_empty()) {
            // Blank line or whitespace-only doc line inside doc block → normalize to '#'
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

/// Strip leading whitespace before `##` markers and `#` doc lines inside doc blocks.
/// Some versions (e.g., v7.0.0 ui.json) have entire doc blocks indented.
pub fn fix_doc_leading_whitespace(content: &str) -> String {
    let mut result = String::new();
    let mut in_doc_block = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "##" {
            in_doc_block = !in_doc_block;
            result.push_str("##\n");
        } else if in_doc_block && trimmed.starts_with('#') {
            // Strip leading whitespace from doc lines inside ## blocks
            result.push_str(trimmed);
            result.push('\n');
        } else {
            if in_doc_block && !trimmed.starts_with('#') {
                in_doc_block = false;
            }
            result.push_str(line);
            result.push('\n');
        }
    }

    result
}

/// Convert indented `Since:` inside field descriptions to plain text.
/// Prevents the doc parser from treating them as top-level Since directives.
/// `#              Since: 1.1` → `#              (Since 1.1)`
pub fn fix_doc_indented_since(content: &str) -> String {
    // Match `#` followed by substantial whitespace (4+) then `Since:` — these are indented
    let re = Regex::new(r"(?m)^(#\s{4,})Since:\s*(\d+\.\d+\S*)").unwrap();
    re.replace_all(content, "${1}(Since $2)").to_string()
}

/// Add colon after `Since` when followed by a version number.
/// `# Since X.Y` → `# Since: X.Y`
pub fn fix_doc_since_colon(content: &str) -> String {
    let re = Regex::new(r"(?m)^(#\s*)Since\s+(\d+\.\d+)").unwrap();
    re.replace_all(content, "${1}Since: $2").to_string()
}

/// Add `@` prefix to doc block names that are missing it.
/// `##\n# LostTickPolicy:` → `##\n# @LostTickPolicy:`
/// Only applies to names that appear right after `##` (type/command names).
pub fn fix_doc_name_at_prefix(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut result = String::new();
    for i in 0..lines.len() {
        if i > 0 && lines[i - 1].trim() == "##" {
            // This line follows ##. Check if it's `# Name:` without @
            let trimmed = lines[i].trim();
            if let Some(rest) = trimmed.strip_prefix("# ") {
                if !rest.starts_with('@') && !rest.starts_with('=') {
                    // Check if it looks like a type/command name followed by colon
                    if let Some(colon_pos) = rest.find(':') {
                        let name = &rest[..colon_pos];
                        if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
                            result.push_str(&format!("# @{}:{}\n", name, &rest[colon_pos + 1..]));
                            continue;
                        }
                    }
                }
            }
        }
        result.push_str(lines[i]);
        result.push('\n');
    }
    result
}

/// Add colon after `@Name` in doc blocks when missing.
/// `# @SomeName\n` → `# @SomeName:\n`
/// `# @query-migrate\n` → `# @query-migrate:\n`
pub fn fix_doc_name_colon(content: &str) -> String {
    // Match `# @Name` at end of line (no colon after name)
    // Name can contain hyphens and underscores (e.g., query-migrate, block_resize)
    let re = Regex::new(r"(?m)^(#\s*@[\w-]+)\s*$").unwrap();
    re.replace_all(content, "$1:").to_string()
}

/// Insert `##` opening marker before `# @Name:` when it's missing.
/// Some older schemas have doc blocks without the `##` opener.
pub fn fix_doc_missing_open_marker(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut result = String::new();
    let mut in_doc_block = false;

    for i in 0..lines.len() {
        let trimmed = lines[i].trim();

        if trimmed == "##" {
            in_doc_block = !in_doc_block;
        }

        // Check if this line starts a doc block (# @Name:) without a preceding ##
        if !in_doc_block && trimmed.starts_with("# @") {
            // Check if we should insert ##
            let prev = if i > 0 { lines[i - 1].trim() } else { "" };
            if prev != "##" {
                result.push_str("##\n");
                in_doc_block = true;
            }
        }

        result.push_str(lines[i]);
        result.push('\n');
    }

    result
}

/// Transform shorthand doc format to standard multi-line form.
/// `## @Name` → `##\n# @Name:\n`
pub fn fix_doc_shorthand(content: &str) -> String {
    let re = Regex::new(r"(?m)^##\s+@([\w-]+)\s*:?\s*$").unwrap();
    re.replace_all(content, "##\n# @$1:").to_string()
}

/// Convert reStructuredText-style section headers to the `# = Title` format.
///
/// v10.2.0+ uses:
/// ```text
/// # ****
/// # Title
/// # ****
/// ```
/// This converts it to: `# = Title`
pub fn fix_rst_section_headers(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut result = String::new();
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim();
        // Check for `# ****` or `# ====` pattern (decoration line)
        if i + 2 < lines.len()
            && is_rst_decoration(trimmed)
            && is_rst_decoration(lines[i + 2].trim())
        {
            let title = lines[i + 1].trim();
            // The title should be a `# SomeText` line
            if title.starts_with("# ") {
                let title_text = &title[2..];
                result.push_str(&format!("# = {}\n", title_text));
                i += 3; // Skip decoration, title, decoration
                continue;
            }
        }
        // Also handle single decoration line after title
        if i + 1 < lines.len()
            && trimmed.starts_with("# ")
            && !is_rst_decoration(trimmed)
            && is_rst_decoration(lines[i + 1].trim())
        {
            // Title followed by decoration — convert to = Title
            let title_text = &trimmed[2..];
            result.push_str(&format!("# = {}\n", title_text));
            i += 2; // Skip title + decoration
            continue;
        }
        result.push_str(lines[i]);
        result.push('\n');
        i += 1;
    }

    result
}

/// Check if a line is an RST decoration line (e.g., `# ****` or `# ====`)
fn is_rst_decoration(line: &str) -> bool {
    if !line.starts_with("# ") {
        return false;
    }
    let rest = line[2..].trim();
    rest.len() >= 3 && rest.chars().all(|c| c == '*' || c == '=' || c == '-' || c == '~')
}

/// Simplify section documentation blocks by removing rich content
/// (qmp-examples, admonitions, etc.) that the strict parser can't handle.
/// Section docs use `# = Title` and contain free-form reST content.
/// We keep just the title and a minimal description.
pub fn simplify_section_docs(content: &str) -> String {
    let mut result = String::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        // Look for `##` opening of a section doc
        if lines[i].trim() == "##" && i + 1 < lines.len() {
            // Check if this is a section doc (next non-empty line starts with `# =`)
            let mut j = i + 1;
            while j < lines.len() && lines[j].trim() == "#" {
                j += 1;
            }
            if j < lines.len() && lines[j].trim().starts_with("# =") {
                // This is a section doc. Find the closing ##.
                let title_line = lines[j];
                let mut end = j + 1;
                while end < lines.len() && lines[end].trim() != "##" {
                    end += 1;
                }
                if end < lines.len() {
                    // Emit simplified section doc: just ## title ##
                    result.push_str("##\n");
                    result.push_str(title_line);
                    result.push('\n');
                    result.push_str("##\n");
                    i = end + 1;
                    continue;
                }
            }
        }
        result.push_str(lines[i]);
        result.push('\n');
        i += 1;
    }

    result
}

/// Collapse multi-line `Returns:` doc sections to single lines.
///
/// The strict parser expects `# Returns: value` on one line, but some schemas use:
/// ```text
/// # Returns:
/// #     @TypeName
/// ```
/// or:
/// ```text
/// # Returns:
/// #     TypeName
/// ```
/// This collapses them to `# Returns: @TypeName` / `# Returns: TypeName`.
pub fn fix_doc_returns_multiline(content: &str) -> String {
    let re = Regex::new(r"(?m)^(# Returns:)\s*\n#\s+(@?\w+)\s*$").unwrap();
    re.replace_all(content, "$1 $2").to_string()
}

/// Strip inline `#` comments that appear outside doc blocks.
/// These are bare comments in the JSON-like QAPI schema (not inside `##...##` blocks).
pub fn strip_inline_comments(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut result = String::new();
    let mut in_doc_block = false;

    for line in &lines {
        let trimmed = line.trim();
        if trimmed == "##" {
            in_doc_block = !in_doc_block;
            result.push_str(line);
            result.push('\n');
        } else if !in_doc_block && trimmed.starts_with("## ") {
            // Shorthand doc format (## @Name) — opens a doc block
            in_doc_block = true;
            result.push_str(line);
            result.push('\n');
        } else if !in_doc_block && trimmed.starts_with('#') {
            // This is an inline comment outside a doc block — skip it
        } else {
            result.push_str(line);
            result.push('\n');
        }
    }

    result
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
    fn test_fix_rst_section_headers() {
        let input = "##\n# ************\n# Introduction\n# ************\n#\n# Description\n##\n";
        let output = fix_rst_section_headers(input);
        assert!(output.contains("# = Introduction"), "got: {}", output);
        assert!(!output.contains("****"), "decoration should be removed");
    }

    #[test]
    fn test_minimal_preprocess() {
        // Fixes malformed doc markers
        let input = "##\n# @Name:\n###\n";
        let output = minimal_preprocess(input);
        assert!(output.contains("\n##\n"), "### should become ##");

        // Fixes missing commas
        let input = "{ 'enum': 'Test'\n  'data': [] }";
        let output = minimal_preprocess(input);
        assert!(output.contains("'Test',"), "missing comma inserted");

        // Fixes key colon spacing
        let input = "'key' : 'val'";
        let output = minimal_preprocess(input);
        assert!(output.contains("'key':"), "spacing normalized");

        // Fixes trailing commas
        let input = "['a',]";
        let output = minimal_preprocess(input);
        assert_eq!(output, "['a']");
    }

    #[test]
    fn test_minimal_preprocess_no_structural_changes() {
        // Should NOT change 'type' to 'struct'
        let input = "{ 'type': 'NameInfo', 'data': {} }";
        let output = minimal_preprocess(input);
        assert!(output.contains("'type':"), "should not change type to struct");

        // Should NOT convert simple unions
        let input = "{ 'union': 'X', 'data': { 'a': 'TypeA' } }";
        let output = minimal_preprocess(input);
        assert!(!output.contains("'discriminator'"), "should not add discriminator");
    }

    #[test]
    fn test_preprocess_modern_schema() {
        // Modern schema entries are simplified: doc blocks stripped to name + since
        let input = "##\n# @StatusInfo:\n#\n# Status info\n#\n# @running: true if running\n#\n# Since: 0.14\n##\n{ 'struct': 'StatusInfo', 'data': {'running': 'bool'} }\n";
        let output = preprocess(input, (10, 2, 0));
        assert!(output.contains("@StatusInfo:"), "name preserved, got: {}", output);
        assert!(output.contains("Since: 0.14"), "since preserved, got: {}", output);
        assert!(output.contains("'struct': 'StatusInfo'"), "definition preserved, got: {}", output);
        // Doc details are stripped
        assert!(!output.contains("@running"), "field docs should be stripped, got: {}", output);
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
        // Should have: struct (not type), since preserved, @NameInfo with colon
        assert!(output.contains("'struct': 'NameInfo'"), "type→struct, got: {}", output);
        assert!(output.contains("Since: 0.14.0") || output.contains("Since 0.14.0"), "since, got: {}", output);
        assert!(output.contains("@NameInfo:"), "name colon, got: {}", output);
    }
}
