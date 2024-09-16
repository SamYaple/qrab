use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::bytes::complete::take_until;
use nom::character::complete::{line_ending, not_line_ending, space0};
use nom::combinator::{map, not, opt, recognize};
use nom::multi::many0;
use nom::sequence::{delimited, preceded, tuple};
use nom::IResult;
use std::collections::HashMap;

#[derive(Debug)]
pub struct QapiDocumentation {
    name: String,
    description: Option<String>,
    fields: HashMap<String, String>,
    since: Option<String>,
    returns: Option<String>,
}

#[derive(Debug)]
enum ParserKey {
    Since(String),
    Returns(String),
    Field((String, String)),
    Empty,
}

fn take_name(input: &str) -> IResult<&str, &str> {
    delimited(
        tuple((tag("#"), space0, tag("@"))),
        take_until(":"),
        tag(":"),
    )(input)
}

fn take_empty(input: &str) -> IResult<&str, &str> {
    recognize(many0(tuple((opt(tag("#")), space0, line_ending))))(input)
}

impl QapiDocumentation {
    pub fn parse(input: &str) -> IResult<&str, Self> {
        let since_parser = delimited(
            take_empty,
            preceded(
                tuple((
                    tag("#"),
                    space0,
                    alt((tag("Since:"), tag("since:"))),
                    space0,
                )),
                not_line_ending,
            ),
            take_empty,
        );
        // field parser

        let parsers = alt((
            map(since_parser, |v: &str| ParserKey::Since(v.into())),
            // field parser
        ));

        let (input, _) = tag("##")(input)?;
        let (input, _) = take_empty(input)?;
        let (input, name) = take_name(input)?;
        let (input, _) = take_empty(input)?;
        let (input, description) = many0(preceded(
            tuple((
                take_empty,
                tag("#"),
                not(tag("#")),
                space0,
                not(alt((tag("@"), tag("Returns"), tag("Since"), tag("since")))),
            )),
            not_line_ending,
        ))(input)?;
        let (input, _) = take_empty(input)?;
        let (input, tokens) = many0(parsers)(input)?;
        let (input, _) = take_empty(input)?;
        let (input, _) = tag("##")(input)?;

        let name = name.into();
        let mut since = None;
        let mut returns = None;
        let mut fields = HashMap::new();
        for token in tokens {
            match token {
                ParserKey::Since(v) => since = Some(v),
                ParserKey::Returns(v) => returns = Some(v),
                ParserKey::Field(v) => {
                    fields.insert(v.0, v.1);
                }
                ParserKey::Empty => {}
            }
        }
        let description = {
            let mut cc = String::new();
            for line in description {
                if cc.len() != 0 {
                    cc.push_str(" ");
                }
                cc.push_str(line);
            }
            Some(cc)
        };
        Ok((
            input,
            Self {
                name,
                description,
                returns,
                fields,
                since,
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_INPUTS: [&str; 4] = [
        r#"##
# @RbdAuthMode:
#
# blah blah
#
##"#,
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
