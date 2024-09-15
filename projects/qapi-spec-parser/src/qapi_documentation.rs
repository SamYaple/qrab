use crate::{QapiCond, QapiFeatures, QapiMembers, QapiString};
use nom::bytes::complete::tag;
use std::collections::HashMap;
use nom::branch::alt;
use nom::combinator::{recognize, map};
use nom::multi::{separated_list1, many1};
use nom::sequence::{tuple, delimited};
use nom::character::complete::multispace0;
use nom::character::complete::{line_ending, not_line_ending};
use nom::IResult;

//enum ParserKey {
//    Name(String),
//    Data(HashMap<String, String>),
//    Description(String),
//}

#[derive(Debug)]
pub struct QapiDocumentation {
    name: String,
    description: Option<String>,
    fields: HashMap<String, String>,
}

impl QapiDocumentation {
    pub fn parse(input: &str) -> IResult<&str, Self> {
        //let data_parser = map(kv(qtag("data"), QapiMembers::parse), |v| ParserKey::Data(v));
        //let name_parser = map(kv(qtag("name"), QapiString::parse), |v| ParserKey::Name(v));
        //let description_parser = map(kv(qtag("description"), QapiString::parse), |v| ParserKey::Description(v));

        //let parsers = alt((
        //    data_parser,
        //    name_parser,
        //    description_parser,
        //));
        delimited(
            tag("##"),
            map(recognize(many1(delimited(tuple((multispace0, tag("#"))), not_line_ending, line_ending))), |v: &str| Self { name: v.into(), description: None, fields: HashMap::new() }),
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
