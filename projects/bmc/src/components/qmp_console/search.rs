//! Hand-rolled fuzzy scorer for the command picker.
//!
//! Returns `Some(score)` iff every character of `query` appears in `target`
//! in order (case-insensitive subsequence match), or `None` otherwise. The
//! scoring weights consecutive runs, start-of-target, and start-of-word
//! matches so commonly-typed abbreviations like "qs" match "query-status"
//! ahead of "query-block".
pub fn score(query: &str, target: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    let q: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();
    let t: Vec<char> = target.chars().flat_map(char::to_lowercase).collect();

    let mut total: i32 = 0;
    let mut qi = 0usize;
    let mut prev_matched: Option<usize> = None;

    for (ti, tc) in t.iter().enumerate() {
        if qi >= q.len() {
            break;
        }
        if *tc == q[qi] {
            total += 10;

            if ti == 0 {
                total += 3;
            } else {
                let prev = t[ti - 1];
                if prev == '-' || prev == '_' || prev == ' ' {
                    total += 2;
                }
            }

            if let Some(last) = prev_matched {
                if last + 1 == ti {
                    total += 5;
                } else {
                    total -= (ti - last - 1) as i32;
                }
            }
            prev_matched = Some(ti);
            qi += 1;
        }
    }

    if qi == q.len() {
        Some(total)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::score;

    #[test]
    fn qs_beats_query_block() {
        // `qs` subsequence-matches `query-status` (the `s` comes after
        // `q` in `query-_s_tatus`). `query-block` has no `s` after its
        // `q`, so the scorer returns `None`. `Some > None` in Rust's
        // built-in Ord for Option, so the assertion covers both the
        // happy path and the non-match case.
        assert!(
            score("qs", "query-status") > score("qs", "query-block"),
            "query-status should outrank query-block for query 'qs'"
        );
    }

    #[test]
    fn subsequence_mismatch_returns_none() {
        assert!(score("zzz", "query-status").is_none());
    }

    #[test]
    fn empty_query_matches_all() {
        assert_eq!(score("", "anything"), Some(0));
    }

    #[test]
    fn case_insensitive() {
        assert!(score("QS", "query-status").is_some());
    }
}
