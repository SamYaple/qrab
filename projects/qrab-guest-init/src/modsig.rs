//! `/sys/module/module/parameters/sig_enforce` check (L5 step 6).
//! The kernel writes exactly `Y\n` when module signature enforcement
//! is on; anything else means an unsigned module can be loaded, which
//! breaks the L4 integrity guarantee.

use crate::tokens::Token;

pub fn check_sig_enforce(raw: &str) -> Result<(), Token> {
    // Kernel returns `Y` or `N`; any other character shape means we're
    // reading a different file than the one we expected, and that's a
    // failure by construction — fail closed.
    match raw.trim() {
        "Y" => Ok(()),
        _ => Err(Token::FailModsig),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enforce_y_passes() {
        assert!(check_sig_enforce("Y\n").is_ok());
        assert!(check_sig_enforce("Y").is_ok());
    }

    #[test]
    fn enforce_n_fails() {
        assert_eq!(check_sig_enforce("N\n"), Err(Token::FailModsig));
    }

    #[test]
    fn empty_or_garbage_fails() {
        assert_eq!(check_sig_enforce(""), Err(Token::FailModsig));
        assert_eq!(check_sig_enforce("yes\n"), Err(Token::FailModsig));
        assert_eq!(check_sig_enforce("1\n"), Err(Token::FailModsig));
    }
}
