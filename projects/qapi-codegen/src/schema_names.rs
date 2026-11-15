//! Version-specific QName syntax, independent of casing/pragma style rules.
use anyhow::{Result, ensure};

pub(crate) fn c_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub(crate) fn validate(name: &str, enum_member: bool) -> Result<()> {
    let version = crate::parser_version();
    if version < (2, 4, 0) {
        return Ok(());
    }
    let legacy = version < (2, 6, 0);
    let reserved = version >= (2, 5, 0)
        && !(legacy && enum_member)
        && (name.starts_with("q_") || name.starts_with("q-"));
    let valid = if legacy {
        let first = enum_member
            || name
                .as_bytes()
                .first()
                .is_some_and(|b| b.is_ascii_alphabetic() || *b == b'_');
        first
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
    } else {
        let stem = if let Some(rest) = name.strip_prefix("__") {
            rest.split_once('_').and_then(|(prefix, stem)| {
                (!prefix.is_empty()
                    && prefix
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-')))
                .then_some(stem)
            })
        } else {
            Some(name)
        };
        let digit = enum_member && name.as_bytes().first().is_some_and(u8::is_ascii_digit);
        stem.is_some_and(|stem| {
            stem.as_bytes()
                .first()
                .is_some_and(|b| b.is_ascii_alphabetic() || digit && b.is_ascii_digit())
                && stem
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
        })
    };
    ensure!(valid && !reserved, "Invalid QAPI name '{name}'");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qname_rules_follow_historical_namespace_changes() {
        for (version, name, enumeration, valid) in [
            ((2, 4, 0), "_private", false, true),
            ((2, 4, 0), "dot.name", false, true),
            ((2, 4, 0), "q-private", false, true),
            ((2, 5, 0), "q-private", false, false),
            ((2, 5, 0), "q-private", true, true),
            ((2, 5, 0), "", true, true),
            ((2, 6, 0), "_private", false, false),
            ((2, 6, 0), "dot.name", false, false),
            ((2, 6, 0), "q-private", true, false),
            ((2, 6, 0), "", true, false),
        ] {
            crate::set_parser_version(version);
            assert_eq!(
                validate(name, enumeration).is_ok(),
                valid,
                "{version:?} {name}"
            );
        }
    }

    #[test]
    fn downstream_prefixes_and_numeric_enum_members_remain_valid() {
        crate::set_parser_version((11, 1, 3));
        for name in [
            "__org.qemu_Name",
            "__org.qemu_x-name",
            "__org.qemu_q-name",
            "x-name",
            "Self",
        ] {
            validate(name, false).unwrap();
        }
        validate("1-name", true).unwrap();
        assert!(validate("1-name", false).is_err());
        assert!(validate("__org.qemu_1", true).is_err());
        assert!(validate("___Name", false).is_err());
    }
}
