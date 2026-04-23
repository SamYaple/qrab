//! IMA policy check (L5 step 7).
//!
//! `/sys/kernel/security/ima/policy` is the live IMA rule set. L5's
//! minimum bar: the file is non-empty and at least one rule uses the
//! `appraise` action — without that, IMA measures but does not refuse
//! tampered files, which fails our integrity model.

use crate::tokens::Token;

pub fn check_ima_policy(raw: &str) -> Result<(), Token> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(Token::FailImaPolicy);
    }
    // IMA rules are whitespace-separated `key=value` pairs with an
    // action keyword at the start. We look for a token that is
    // literally `appraise` (the action) anywhere in the rule set —
    // substring search would wrongly accept a filename like
    // `appraise_notes.txt`.
    let has_appraise = raw
        .split(|c: char| c.is_whitespace())
        .any(|tok| tok == "appraise");
    if has_appraise {
        Ok(())
    } else {
        Err(Token::FailImaPolicy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_with_appraise_rule_passes() {
        let policy = "\
            measure func=BPRM_CHECK\n\
            appraise func=BPRM_CHECK appraise_type=imasig\n\
        ";
        assert!(check_ima_policy(policy).is_ok());
    }

    #[test]
    fn empty_policy_fails() {
        assert_eq!(check_ima_policy(""), Err(Token::FailImaPolicy));
        assert_eq!(check_ima_policy("   \n\t\n"), Err(Token::FailImaPolicy));
    }

    #[test]
    fn measure_only_policy_fails() {
        let policy = "measure func=BPRM_CHECK\nmeasure func=FILE_MMAP\n";
        assert_eq!(check_ima_policy(policy), Err(Token::FailImaPolicy));
    }

    #[test]
    fn substring_false_match_rejected() {
        // `dont_appraise` and a path like `/etc/appraise.conf` should
        // not trick the check into passing.
        let policy = "dont_appraise func=FILE_MMAP fsmagic=0x9fa0\n";
        assert_eq!(check_ima_policy(policy), Err(Token::FailImaPolicy));

        let policy2 = "measure func=FILE_CHECK obj_type=appraisal_data\n";
        assert_eq!(check_ima_policy(policy2), Err(Token::FailImaPolicy));
    }

    #[test]
    fn appraise_as_first_token_of_line_passes() {
        assert!(check_ima_policy("appraise\n").is_ok());
    }
}
