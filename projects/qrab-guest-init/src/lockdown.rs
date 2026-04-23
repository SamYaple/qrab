//! Parse `/sys/kernel/security/lockdown` and enforce L5's minimum
//! lockdown level.
//!
//! Kernel format:
//! > `none [integrity] confidentiality`
//!
//! Exactly one entry is bracketed; that is the active mode. L5 requires
//! `integrity` or stronger (`confidentiality`).

use crate::tokens::Token;

/// Lockdown modes in order of increasing restriction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Mode {
    None,
    Integrity,
    Confidentiality,
}

/// Extract the bracketed mode from one lockdown file line. Returns
/// `None` if no bracketed token is found or the token is unknown.
pub fn parse_lockdown_line(line: &str) -> Option<Mode> {
    for tok in line.split_whitespace() {
        let Some(stripped) = tok.strip_prefix('[').and_then(|s| s.strip_suffix(']')) else {
            continue;
        };
        return match stripped {
            "none" => Some(Mode::None),
            "integrity" => Some(Mode::Integrity),
            "confidentiality" => Some(Mode::Confidentiality),
            _ => None,
        };
    }
    None
}

/// L5 enforcement: mode must be at least `Integrity`.
pub fn check_lockdown(raw: &str) -> Result<(), Token> {
    // The file is one line plus trailing newline in every kernel since
    // the Lockdown LSM landed. Be liberal on leading/trailing whitespace
    // but stop at the first non-empty line.
    let line = raw.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    match parse_lockdown_line(line) {
        Some(m) if m >= Mode::Integrity => Ok(()),
        _ => Err(Token::FailLockdown),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_integrity_selected() {
        assert_eq!(
            parse_lockdown_line("none [integrity] confidentiality"),
            Some(Mode::Integrity),
        );
    }

    #[test]
    fn parses_confidentiality_selected() {
        assert_eq!(
            parse_lockdown_line("none integrity [confidentiality]"),
            Some(Mode::Confidentiality),
        );
    }

    #[test]
    fn parses_none_selected() {
        assert_eq!(
            parse_lockdown_line("[none] integrity confidentiality"),
            Some(Mode::None),
        );
    }

    #[test]
    fn unbracketed_line_is_none() {
        assert_eq!(parse_lockdown_line("none integrity confidentiality"), None);
    }

    #[test]
    fn unknown_mode_is_none() {
        assert_eq!(parse_lockdown_line("[supermode]"), None);
    }

    #[test]
    fn check_integrity_passes() {
        assert!(check_lockdown("none [integrity] confidentiality\n").is_ok());
    }

    #[test]
    fn check_confidentiality_passes() {
        assert!(check_lockdown("none integrity [confidentiality]\n").is_ok());
    }

    #[test]
    fn check_none_fails() {
        assert_eq!(
            check_lockdown("[none] integrity confidentiality\n"),
            Err(Token::FailLockdown),
        );
    }

    #[test]
    fn check_empty_file_fails() {
        assert_eq!(check_lockdown(""), Err(Token::FailLockdown));
    }

    #[test]
    fn check_unbracketed_file_fails() {
        // Pathological: a caller has somehow populated the file with a
        // mode-token but no brackets. Treated as unparseable.
        assert_eq!(
            check_lockdown("integrity\n"),
            Err(Token::FailLockdown),
        );
    }

    #[test]
    fn ordering_is_monotonic() {
        assert!(Mode::Integrity > Mode::None);
        assert!(Mode::Confidentiality > Mode::Integrity);
    }
}
