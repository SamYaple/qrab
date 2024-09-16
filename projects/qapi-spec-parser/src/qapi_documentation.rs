use nom::bytes::complete::tag;
use nom::character::complete::{line_ending, not_line_ending, space0};
use nom::combinator::{map, not, peek, recognize};
use nom::multi::many1;
use nom::sequence::{delimited, tuple};
use nom::IResult;
use std::collections::HashMap;

#[derive(Debug)]
pub struct QapiDocumentation {
    name: String,
    description: Option<String>,
    fields: HashMap<String, String>,
}

impl QapiDocumentation {
    pub fn parse(input: &str) -> IResult<&str, Self> {
        delimited(
            tuple((tag("##"), space0, line_ending)),
            map(
                recognize(many1(delimited(
                    tuple((not(peek(tag("##"))), tag("#"))),
                    not_line_ending,
                    line_ending,
                ))),
                |v: &str| Self {
                    name: v.into(),
                    description: None,
                    fields: HashMap::new(),
                },
            ),
            tag("##"),
        )(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_INPUTS: [&str; 3] = [
        r#"##
# @RbdAuthMode:
#
# Since: 3.0
##"#,
        r#"##
# @IscsiHeaderDigest:
#
# An enumeration of header digests supported by libiscsi
#
# Since: 2.9
##"#,
        r#"##
# @BlockdevOptionsIscsi:
#
# Driver specific block device options for iscsi
#
# @transport: The iscsi transport type
#
# @portal: The address of the iscsi portal
#
# @target: The target iqn name
#
# @lun: LUN to connect to.  Defaults to 0.
#
# @user: User name to log in with.  If omitted, no CHAP authentication
#     is performed.
#
# @password-secret: The ID of a QCryptoSecret object providing the
#     password for the login.  This option is required if @user is
#     specified.
#
# @initiator-name: The iqn name we want to identify to the target as.
#     If this option is not specified, an initiator name is generated
#     automatically.
#
# @header-digest: The desired header digest.  Defaults to none-crc32c.
#
# @timeout: Timeout in seconds after which a request will timeout.  0
#     means no timeout and is the default.
#
# Since: 2.9
##"#,
    ];

    #[test]
    fn test_valid() {
        for input in VALID_INPUTS {
            let result = QapiDocumentation::parse(input);
            match result {
                Ok((remaining, _)) => {
                    assert_eq!(remaining, "");
                }
                _ => panic!("Failed to parse: ```\n{input}\n```"),
            }
        }
    }
}
