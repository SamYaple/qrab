use nom::bytes::complete::tag;
use nom::character::complete::{line_ending, not_line_ending, space0, multispace0};
use nom::combinator::{map, not, peek, recognize, opt};
use nom::multi::{many0, many1};
use nom::sequence::{delimited, tuple, preceded};
use nom::branch::alt;
use nom::bytes::complete::take_until;
use nom::IResult;
use std::collections::HashMap;

#[derive(Debug)]
pub struct QapiDocumentation {
    name: String,
    description: Option<String>,
    fields: HashMap<String, String>,
    since: Option<String>,
}

#[derive(Debug)]
enum ParserKey {
    Name(String),
    Description(String),
    Since(String),
    Field((String, String)),
    Empty,
}

fn take_name(input: &str) -> IResult<&str, &str> {
    delimited(tag("@"), take_until(":"), tag(":"))(input)
}

fn take_empty(input: &str) -> IResult<&str, &str> {
    recognize(tuple((multispace0, not(peek(tag("##"))), tag("#"), space0)))(input)
}


impl QapiDocumentation {
    pub fn parse(input: &str) -> IResult<&str, Self> {
        let description_parser = many1(delimited(take_empty, preceded(tuple((not(peek(tag("##"))), not(peek(take_name)))), not_line_ending), line_ending));
        // since parser
        // field parser

        let parsers = alt((
            map(take_name, |v: &str| ParserKey::Name(v.into())),
            map(description_parser, |v: Vec<&str>| {
                let mut description = String::new();
                for l in v {
                    if description.len() != 0 {
                        description.push_str(" ");
                    }
                    description.push_str(l);
                }
                ParserKey::Description(description)
            }),
            // since parser
            // field parser
            map(take_empty, |_| ParserKey::Empty),
        ));
        delimited(
            tuple((tag("##"), take_empty)),
            map(many1(parsers), |tokens| {
                dbg![&tokens];
                let mut name = None;
                let mut description = None;
                let mut since = None;
                let mut fields = HashMap::new();
                for token in tokens {
                    match token {
                        ParserKey::Name(v) => name = Some(v),
                        ParserKey::Description(v) => description = Some(v),
                        ParserKey::Since(v) => since = Some(v),
                        ParserKey::Field(v) => { fields.insert(v.0, v.1); },
                        ParserKey::Empty => {},
                    }
                }
                let name = name.expect("No name token found");
                Self {
                    name,
                    description,
                    fields,
                    since,
                }
            }),
            tag("##"),
        )(input)
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
