// TODO: Refactor/comment/tests this code.
// BUG: sometimes struct documentation will get consumed into an option doc
use super::with_parser_config;
use nom::branch::alt;
use nom::bytes::complete::{is_a, tag, tag_no_case, take_until, take_while1};
use nom::character::complete::{line_ending, multispace0, not_line_ending, space0};
use nom::combinator::{not, opt, peek, recognize};
use nom::error::Error;
use nom::multi::{many0, many1};
use nom::sequence::{delimited, preceded, terminated};
use nom::IResult;
use nom::Parser;

fn take_until_since(input: &str) -> IResult<&str, &str> {
    preceded(
        many1((
            alt((take_until("since"), take_until("Since"))),
            tag_no_case("since"),
            many0(is_a(" :")),
        )),
        recognize(many1(is_a("0123456789."))),
    )
    .parse(input)
}

pub fn extract_since_from_comment(desc: &Vec<&str>) -> Option<String> {
    let input = desc.join(" ");
    let (_, strmatch) = opt(take_until_since).parse(&input).unwrap();
    if let Some(s) = strmatch {
        return Some(s.to_string());
    }
    None
}

pub(crate) fn trim_docstr(s: &str) -> &str {
    s.trim_matches(|c: char| c == '#' || c.is_whitespace())
}

pub(crate) fn docstr_to_str(input: &str) -> Vec<&str> {
    let mut output = Vec::new();
    for line in input.lines() {
        output.push(trim_docstr(line).trim_end());
    }
    output
}

fn not_name_break(c: char) -> bool {
    c != ':' && !c.is_ascii_whitespace()
}

fn take_name(input: &str) -> IResult<&str, &str> {
    if with_parser_config(|c| c.optional_doc_name_colon) {
        // Accept `# @Name` without colon (old format)
        delimited(
            (take_line_start, tag("@")),
            take_while1(not_name_break),
            (space0, opt(tag(":"))),
        )
        .parse(input)
    } else {
        delimited(
            (take_line_start, tag("@")),
            take_while1(not_name_break),
            (space0, tag(":")),
        )
        .parse(input)
    }
}

fn take_namedkv<'i>(
    key: &'static str,
) -> impl Parser<&'i str, Output = &'i str, Error = Error<&'i str>> {
    preceded(
        take_namedkey(key),
        recognize((
            not_line_ending,
            line_ending,
            opt(take_multiline_text),
        )),
    )
}

// Case insensitive match for a line like: ```# {key}:```
fn take_namedkey<'i>(
    key: &'static str,
) -> impl Parser<&'i str, Output = &'i str, Error = Error<&'i str>> {
    delimited(
        take_line_start,
        tag_no_case(key),
        (space0, tag(":"), space0),
    )
}

fn take_line_start(input: &str) -> IResult<&str, &str> {
    if with_parser_config(|c| c.doc_leading_whitespace) {
        recognize((space0, tag("#"), space0)).parse(input)
    } else {
        recognize((tag("#"), space0)).parse(input)
    }
}

fn take_line_end(input: &str) -> IResult<&str, &str> {
    recognize((space0, line_ending)).parse(input)
}

fn take_empty_line(input: &str) -> IResult<&str, &str> {
    if with_parser_config(|c| c.doc_blank_lines) {
        // Accept both `# ` empty lines and completely blank lines inside doc blocks
        alt((
            recognize((take_line_start, take_line_end)),
            recognize((space0, line_ending)),
        )).parse(input)
    } else {
        recognize((take_line_start, take_line_end)).parse(input)
    }
}

fn take_value(input: &str) -> IResult<&str, Vec<&str>> {
    let (input, description) = recognize((
        not_line_ending,
        line_ending,
        opt(take_multiline_text),
    ))
    .parse(input)?;
    Ok((input, docstr_to_str(description)))
}

/// Check for `Since X.Y` without colon (only a stop condition, doesn't consume)
fn is_since_without_colon(input: &str) -> IResult<&str, &str> {
    if !with_parser_config(|c| c.optional_doc_name_colon) {
        return Err(nom::Err::Error(Error::new(input, nom::error::ErrorKind::Tag)));
    }
    recognize((
        take_line_start,
        tag_no_case("since"),
        space0,
        take_while1(|c: char| c.is_ascii_digit() || c == '.'),
    )).parse(input)
}

// SUCCESS: on success, this is equivalent to the `take_line_start` parser
// FAILURE: fails when the parser encouters the start of a new section
fn take_line_continuation(input: &str) -> IResult<&str, &str> {
    recognize((
        not(alt((
            take_name,
            take_namedkey("since"),
            take_namedkey("returns"),
            take_namedkey("errors"),
            take_namedkey("features"),
            is_since_without_colon,
        ))),
        take_line_start,
        not(alt((
            tag(".. "), // gen-doc strings like `.. qmp-example::`
            tag("#"),   // we are at the end-of-section marker if this succeeds
        ))),
    ))
    .parse(input)
}

fn take_multiline_text(input: &str) -> IResult<&str, &str> {
    recognize(many1(delimited(
        take_line_continuation,
        not_line_ending,
        line_ending,
    )))
    .parse(input)
}
enum ParserToken<'i> {
    Errors(&'i str),
    Since(&'i str),
    Returns(&'i str),
    Note(&'i str),
    Caution(&'i str),
    QmpExample(&'i str),
    Table(&'i str),
    Admonition(&'i str),
    Features(Vec<(&'i str, Vec<&'i str>)>),
}

#[derive(Debug, Clone)]
pub struct QapiDocumentation<'i> {
    pub name: &'i str,
    pub fields: Vec<(&'i str, Vec<&'i str>)>,
    pub features: Vec<(&'i str, Vec<&'i str>)>,
    pub description: Vec<&'i str>,
    pub errors: Option<&'i str>,
    pub since: Option<&'i str>,
    pub returns: Option<&'i str>,
    pub qmp_examples: Vec<&'i str>,
    pub notes: Vec<&'i str>,
    pub admonitions: Vec<&'i str>,
    pub tables: Vec<&'i str>,
    pub cautions: Vec<&'i str>,
}

impl<'i> QapiDocumentation<'i> {
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        // Consume optional leading whitespace before opening ##
        let input = if with_parser_config(|c| c.doc_leading_whitespace) {
            space0::<&str, Error<&str>>(input).map(|(i, _)| i).unwrap_or(input)
        } else {
            input
        };

        // Try standard ## opening, shorthand ## @Name, or bare doc block
        let (input, shorthand_name) = if let Ok((input, _)) = terminated(tag::<&str, &str, Error<&str>>("##"), take_line_end).parse(input) {
            // Standard opening: ##\n followed by # @Name: on next line
            (input, None)
        } else if with_parser_config(|c| c.bare_doc_blocks) {
            // Shorthand: ## @Name on same line, or bare # @Name without ##
            if let Ok((input, name)) = preceded(
                (tag::<&str, &str, Error<&str>>("##"), space0),
                terminated(
                    delimited(
                        tag("@"),
                        take_while1(not_name_break),
                        (space0, opt(tag(":"))),
                    ),
                    take_line_end,
                ),
            ).parse(input) {
                (input, Some(name))
            } else {
                // Bare doc block: no ## opening, starts directly with # @Name
                let _ = peek(terminated(take_name, take_line_end)).parse(input)?;
                (input, None)
            }
        } else {
            return Err(nom::Err::Error(Error::new(input, nom::error::ErrorKind::Tag)));
        };

        let (input, _) = many0(take_empty_line).parse(input)?;
        let (input, name) = if let Some(n) = shorthand_name {
            (input, n)
        } else {
            terminated(take_name, take_line_end).parse(input)?
        };
        let (input, _) = many0(take_empty_line).parse(input)?;
        let (input, description_str) = opt(take_multiline_text).parse(input)?;
        let (input, _) = many0(take_empty_line).parse(input)?;
        let (input, fields) = many0((take_name, take_value)).parse(input)?;
        let (input, _) = many0(take_empty_line).parse(input)?;

        ////
        // at this point in the parsing, we have to switch to a more dynamic
        // parsing mode. While for the most part the keys are ordered, there are
        // some exceptions. Since there is no convention I can find, we must
        // match them in any order.
        ////

        // all branches of the `alt()` parser need to have the same type, which
        // leads to this `parser.map(|v| ParserToken::Something(v))` pattern.
        // It's not my favorite, but not all return types are `&str`.
        let (input, tokens) = many0(alt((
            // Standard "Since: X.Y" with colon
            take_namedkv("since").map(ParserToken::Since),
            // Old format "Since X.Y" without colon — only version numbers
            (|input| {
                if !with_parser_config(|c| c.optional_doc_name_colon) {
                    return Err(nom::Err::Error(Error::new(input, nom::error::ErrorKind::Tag)));
                }
                let (input, _) = take_line_start(input)?;
                let (input, _) = tag_no_case("since")(input)?;
                let (input, _) = space0(input)?;
                let (input, ver) = recognize(many1(is_a("0123456789."))).parse(input)?;
                let (input, _) = not_line_ending(input)?;
                let (input, _) = line_ending(input)?;
                Ok((input, ver))
            }).map(ParserToken::Since),
            take_namedkv("returns").map(ParserToken::Returns),
            take_namedkv("errors").map(ParserToken::Errors),
            take_namedkv(".. note:").map(ParserToken::Note),
            take_namedkv(".. caution:").map(ParserToken::Caution),
            take_namedkv(".. table:").map(ParserToken::Table),
            take_namedkv(".. qmp-example:").map(|v| {
                ParserToken::QmpExample(v)
            }),
            take_namedkv(".. admonition:").map(|v| {
                ParserToken::Admonition(v)
            }),
            preceded(
                (
                    take_namedkey("features"),
                    take_line_end,
                    many0(take_empty_line),
                ),
                many1((take_name, take_value)),
            )
            .map(ParserToken::Features),
        )))
        .parse(input)?;

        // UPSTREAMFIX: This is because `qga/qapi-schema.json` in the `@GuestNetworkRoute` doc block.
        let (input, _) = multispace0(input)?;
        // Consume optional leading whitespace before closing ##
        let input = if with_parser_config(|c| c.doc_leading_whitespace) {
            space0::<&str, Error<&str>>(input).map(|(i, _)| i).unwrap_or(input)
        } else {
            input
        };
        // Closing ## is required for standard doc blocks, optional for bare doc blocks
        let input = if let Ok((input, _)) = tag::<&str, &str, Error<&str>>("##").parse(input) {
            input
        } else if with_parser_config(|c| c.bare_doc_blocks) {
            input
        } else {
            return Err(nom::Err::Error(Error::new(input, nom::error::ErrorKind::Tag)));
        };

        ////
        // Parsing is now done, we might still return a parsing error after
        // this point, but that will be an issue of a missing field or other
        // required value. The `input` string will no longer be touched though
        ////

        let mut since = None;
        let mut errors = None;
        let mut returns = None;
        let mut features = vec![];
        let mut notes = vec![];
        let mut cautions = vec![];
        let mut tables = vec![];
        let mut qmp_examples = vec![];
        let mut admonitions = vec![];
        for token in tokens {
            match token {
                ParserToken::Since(v) => since = Some(trim_docstr(v)),
                ParserToken::Errors(v) => errors = Some(trim_docstr(v)),
                ParserToken::Returns(v) => returns = Some(trim_docstr(v)),
                ParserToken::Features(v) => features = v,
                ParserToken::Note(v) => notes.push(trim_docstr(v)),
                ParserToken::Caution(v) => cautions.push(trim_docstr(v)),
                ParserToken::Table(v) => tables.push(trim_docstr(v)),
                ParserToken::QmpExample(v) => qmp_examples.push(trim_docstr(v)),
                ParserToken::Admonition(v) => admonitions.push(trim_docstr(v)),
            }
        }
        let name = trim_docstr(name);
        let description = if let Some(d) = description_str {
            docstr_to_str(d)
        } else {
            Vec::new()
        };
        Ok((
            input,
            Self {
                features,
                cautions,
                tables,
                errors,
                notes,
                name,
                description,
                returns,
                fields,
                since,
                qmp_examples,
                admonitions,
            },
        ))
    }
}

#[derive(Debug, Clone)]
pub struct QapiSectionDocumentation<'i> {
    pub name: &'i str,
    pub description: Option<&'i str>,
}

impl<'i> QapiSectionDocumentation<'i> {
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        let input = if with_parser_config(|c| c.doc_leading_whitespace) {
            space0::<&str, Error<&str>>(input).map(|(i, _)| i).unwrap_or(input)
        } else {
            input
        };
        let (input, _) = terminated(tag("##"), take_line_end).parse(input)?;
        let (input, _) = many0(take_empty_line).parse(input)?;
        let (input, name) = delimited(
            take_line_start,
            preceded(tag("="), not_line_ending),
            take_line_end,
        )
        .parse(input)?;
        let (input, mut description) = opt(take_multiline_text).parse(input)?;
        let (input, _) = many0(take_empty_line).parse(input)?;
        let (input, _) = tag("##").parse(input)?;

        let name = trim_docstr(name);
        if let Some(ref mut d) = description {
            *d = trim_docstr(d);
        };
        Ok((input, Self { name, description }))
    }
}

/// A freeform documentation block: ##\n# text...\n##
/// Used for section intros, copyright headers, etc.
/// These don't have a # @Name: line — they're just free text.
#[derive(Debug, Clone)]
pub struct QapiFreeformDoc<'i> {
    pub text: &'i str,
}

impl<'i> QapiFreeformDoc<'i> {
    pub fn parse(input: &'i str) -> IResult<&'i str, Self> {
        let input = if with_parser_config(|c| c.doc_leading_whitespace) {
            space0::<&str, Error<&str>>(input).map(|(i, _)| i).unwrap_or(input)
        } else {
            input
        };
        let start = input;
        let (input, _) = terminated(tag("##"), take_line_end).parse(input)?;
        // Consume lines starting with # until we hit ##
        let (input, _) = many0(alt((
            take_empty_line,
            take_doc_line,
        ))).parse(input)?;
        let (input, _) = tag("##").parse(input)?;

        let consumed = &start[..start.len() - input.len()];
        Ok((input, Self { text: consumed }))
    }
}

/// Parse a single # doc comment line (used within ## blocks).
/// Does NOT match the closing ## marker.
fn take_doc_line(input: &str) -> IResult<&str, &str> {
    // Ensure we don't consume the closing ## marker
    // Don't consume the closing ## (with optional leading whitespace)
    let (input, _) = not(peek(terminated((space0, tag("##")), take_line_end))).parse(input)?;
    delimited(
        take_line_start,
        not_line_ending,
        take_line_end,
    )
    .parse(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_INPUTS: [&str; 12] = [
        r#"##
# @RbdAuthMode:
#
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
        r#"##
# @query-block-jobs:
# 
# Return information about long-running block device operations.
#
# Returns: a list of @BlockJobInfo for each active block job
#
# Since: 1.1
##"#,
        r#"##
# @RunState:
#
# An enumeration of VM run states.
#
# @debug: QEMU is running on a debugger
#
# @finish-migrate: guest is paused to finish the migration process
#
# @inmigrate: guest is paused waiting for an incoming migration.  Note
#     that this state does not tell whether the machine will start at
#     the end of the migration.  This depends on the command-line -S
#     option and any invocation of 'stop' or 'cont' that has happened
#     since QEMU was started.
#
# @internal-error: An internal error that prevents further guest
#     execution has occurred
#
# @io-error: the last IOP has failed and the device is configured to
#     pause on I/O errors
#
# @paused: guest has been paused via the 'stop' command
#
# @postmigrate: guest is paused following a successful 'migrate'
#
# @prelaunch: QEMU was started with -S and guest has not started
#
# @restore-vm: guest is paused to restore VM state
#
# @running: guest is actively running
#
# @save-vm: guest is paused to save the VM state
#
# @shutdown: guest is shut down (and -no-shutdown is in use)
#
# @suspended: guest is suspended (ACPI S3)
#
# @watchdog: the watchdog action is configured to pause and has been
#     triggered
#
# @guest-panicked: guest has been panicked as a result of guest OS
#     panic
#
# @colo: guest is paused to save/restore VM state under colo
#     checkpoint, VM can not get into this state unless colo
#     capability is enabled for migration.  (since 2.8)
##"#,
        r#"##
# @SHUTDOWN:
#
# Emitted when the virtual machine has shut down, indicating that qemu
# is about to exit.
#
# @guest: If true, the shutdown was triggered by a guest request (such
#     as a guest-initiated ACPI shutdown request or other
#     hardware-specific action) rather than a host request (such as
#     sending qemu a SIGINT).  (since 2.10)
#
# @reason: The @ShutdownCause which resulted in the SHUTDOWN.
#     (since 4.0)
#
# .. note:: If the command-line option ``-no-shutdown`` has been
#    specified, qemu will not exit, and a STOP event will eventually
#    follow the SHUTDOWN event.
#
# Since: 0.12
# 
# .. qmp-example::
#           
#     <- { "event": "SHUTDOWN",
#          "data": { "guest": true, "reason": "guest-shutdown" },
#          "timestamp": { "seconds": 1267040730, "microseconds": 682951 } }
##"#,
        r#"##
# @query-command-line-options:
#
# Query command line option schema.
#
# @option: option name
#
# Returns: list of @CommandLineOptionInfo for all options (or for the
#     given @option).
#
# Errors:
#     - if the given @option doesn't exist
#
# Since: 1.5
#
# .. qmp-example::
#
#     -> { "execute": "query-command-line-options",
#          "arguments": { "option": "option-rom" } }
#     <- { "return": [
#             {
#                 "parameters": [
#                     {
#                         "name": "romfile",
#                         "type": "string"
#                     },
#                     {
#                         "name": "bootindex",
#                         "type": "number"
#                     }
#                 ],
#                 "option": "option-rom"
#             }
#          ]
#        }
##"#,
        r#"##
# @migrate:
#
# Migrates the current running guest to another Virtual Machine.
#
# @uri: the Uniform Resource Identifier of the destination VM
#
# @channels: list of migration stream channels with each stream in the
#     list connected to a destination interface endpoint.
#
# @detach: this argument exists only for compatibility reasons and is
#     ignored by QEMU
#
# @resume: resume one paused migration, default "off".  (since 3.0)
#
# Since: 0.14
#
# .. admonition:: Notes
#
#     1. The 'query-migrate' command should be used to check
#        migration's progress and final result (this information is
#        provided by the 'status' member).
#
#     2. All boolean arguments default to false.
#
#     3. The user Monitor's "detach" argument is invalid in QMP and
#        should not be used.
#
#     4. The uri argument should have the Uniform Resource Identifier
#        of default destination VM.  This connection will be bound to
#        default network.
#
#     5. For now, number of migration streams is restricted to one,
#        i.e. number of items in 'channels' list is just 1.
#
#     6. The 'uri' and 'channels' arguments are mutually exclusive;
#        exactly one of the two should be present.
#
# .. qmp-example::
#
#     -> { "execute": "migrate", "arguments": { "uri": "tcp:0:4446" } }
#     <- { "return": {} }
#
##"#,
        r#"##
# @query-commands:
#
# Return a list of supported QMP commands by this server
#
# Returns: A list of @CommandInfo for all supported commands
#
# Since: 0.14
#
# .. qmp-example::
#
#     -> { "execute": "query-commands" }
#     <- {
#          "return":[
#             {
#                "name":"query-balloon"
#             },
#             {
#                "name":"system_powerdown"
#             },
#             ...
#          ]
#        }
#
# This example has been shortened as the real response is too long.
##"#,
        r#"##
# @MigMode:
#
# @normal: the original form of migration.  (since 8.2)
#
# @cpr-reboot: The migrate command stops the VM and saves state to the
#     URI.  After quitting QEMU, the user resumes by running QEMU
#     -incoming.
#
#     This mode allows the user to quit QEMU, optionally update and
#     reboot the OS, and restart QEMU.  If the user reboots, the URI
#     must persist across the reboot, such as by using a file.
#
#     Unlike normal mode, the use of certain local storage options
#     does not block the migration, but the user must not modify the
#     contents of guest block devices between the quit and restart.
#
#     This mode supports VFIO devices provided the user first puts the
#     guest in the suspended runstate, such as by issuing
#     guest-suspend-ram to the QEMU guest agent.
#
#     Best performance is achieved when the memory backend is shared
#     and the @x-ignore-shared migration capability is set, but this
#     is not required.  Further, if the user reboots before restarting
#     such a configuration, the shared memory must persist across the
#     reboot, such as by backing it with a dax device.
#
#     @cpr-reboot may not be used with postcopy, background-snapshot,
#     or COLO.
# 
#     (since 8.2)
##"#,
        r#"##
# @BlockdevOptionsFile:
# 
# Driver specific block device options for the file backend.
#           
# @filename: path to the image file
#           
# @pr-manager: the id for the object that will handle persistent
#     reservations for this device (default: none, forward the
#     commands via SG_IO; since 2.11)
#
# @aio: AIO backend (default: threads) (since: 2.8)
#
# @aio-max-batch: maximum number of requests to batch together into a
#     single submission in the AIO backend.  The smallest value
#     between this and the aio-max-batch value of the IOThread object
#     is chosen.  0 means that the AIO backend will handle it
#     automatically.  (default: 0, since 6.2)
#
# @locking: whether to enable file locking.  If set to 'auto', only
#     enable when Open File Descriptor (OFD) locking API is available
#     (default: auto, since 2.10)
#
# @drop-cache: invalidate page cache during live migration.  This
#     prevents stale data on the migration destination with
#     cache.direct=off.  Currently only supported on Linux hosts.
#     (default: on, since: 4.0)
#
# @x-check-cache-dropped: whether to check that page cache was dropped
#     on live migration.  May cause noticeable delays if the image
#     file is large, do not use in production.  (default: off)
#     (since: 3.0)
#
# Features:
#
# @dynamic-auto-read-only: If present, enabled auto-read-only means
#     that the driver will open the image read-only at first,
#     dynamically reopen the image file read-write when the first
#     writer is attached to the node and reopen read-only when the
#     last writer is detached.  This allows giving QEMU write
#     permissions only on demand when an operation actually needs
#     write access.
#
# @unstable: Member x-check-cache-dropped is meant for debugging.
#
# Since: 2.9
##"#,
        r#"##
# @add-fd:
#
# Add a file descriptor, that was passed via SCM rights, to an fd set.
# 
# @fdset-id: The ID of the fd set to add the file descriptor to.
#
# @opaque: A free-form string that can be used to describe the fd.
#
# Returns:
#     @AddfdInfo 
#
# Errors:
#     - If file descriptor was not received, GenericError
#     - If @fdset-id is a negative value, GenericError
#
# .. note:: The list of fd sets is shared by all monitor connections.
# 
# .. note:: If @fdset-id is not specified, a new fd set will be
#    created.
#
# Since: 1.2
#
# .. qmp-example::
#
#     -> { "execute": "add-fd", "arguments": { "fdset-id": 1 } }
#     <- { "return": { "fdset-id": 1, "fd": 3 } }
##"#,
    ];

    #[test]
    fn test_valid() {
        for input in VALID_INPUTS {
            let result = QapiDocumentation::parse(input);
            match result {
                Ok((remaining, _d)) => {
                    assert_eq!(remaining, "");
                }
                _ => panic!("Failed to parse: ```\n{input}\n```"),
            }
        }
    }

    #[test]
    fn test_doc_preserves_description() {
        let input = "##\n# @RunState:\n#\n# An enumeration of VM run states.\n#\n# @debug: QEMU is running on a debugger\n#\n# Since: 0.14\n##";
        let (_, doc) = QapiDocumentation::parse(input).unwrap();
        assert_eq!(doc.name, "RunState");
        assert!(!doc.description.is_empty(), "description should be preserved");
        assert!(doc.description.iter().any(|d| d.contains("enumeration")));
        assert_eq!(doc.since, Some("0.14"));
        assert!(!doc.fields.is_empty(), "fields should be preserved");
        assert_eq!(doc.fields[0].0, "debug");
    }

    #[test]
    fn test_doc_blank_lines() {
        use crate::parsers::{ParserConfig, set_parser_config};
        set_parser_config(ParserConfig { doc_blank_lines: true, ..ParserConfig::modern() });
        let input = "##\n# @Name:\n\n# description\n#\n# Since: 1.0\n##";
        let result = QapiDocumentation::parse(input);
        set_parser_config(ParserConfig::modern());
        match result {
            Ok((remaining, doc)) => {
                assert_eq!(remaining, "");
                assert_eq!(doc.name, "Name");
            }
            Err(e) => panic!("Blank lines in doc block should be accepted: {}", e),
        }
    }

    #[test]
    fn test_doc_leading_whitespace() {
        use crate::parsers::{ParserConfig, set_parser_config};
        set_parser_config(ParserConfig { doc_leading_whitespace: true, ..ParserConfig::modern() });
        let input = " ##\n# @Name:\n#\n# Since: 1.0\n ##";
        let result = QapiDocumentation::parse(input);
        set_parser_config(ParserConfig::modern());
        match result {
            Ok((remaining, doc)) => {
                assert_eq!(remaining, "");
                assert_eq!(doc.name, "Name");
            }
            Err(e) => panic!("Leading whitespace before ## should be accepted: {}", e),
        }
    }

    #[test]
    fn test_doc_optional_name_colon() {
        use crate::parsers::{ParserConfig, set_parser_config};
        set_parser_config(ParserConfig { optional_doc_name_colon: true, ..ParserConfig::modern() });
        let input = "##\n# @StatusInfo\n#\n# Since: 0.14\n##";
        let result = QapiDocumentation::parse(input);
        set_parser_config(ParserConfig::modern());
        match result {
            Ok((remaining, doc)) => {
                assert_eq!(remaining, "");
                assert_eq!(doc.name, "StatusInfo");
            }
            Err(e) => panic!("Name without colon should be accepted: {}", e),
        }
    }

    #[test]
    fn test_doc_since_without_colon() {
        use crate::parsers::{ParserConfig, set_parser_config};
        set_parser_config(ParserConfig { optional_doc_name_colon: true, ..ParserConfig::modern() });
        let input = "##\n# @StatusInfo:\n#\n# Since 0.14.0\n##";
        let result = QapiDocumentation::parse(input);
        set_parser_config(ParserConfig::modern());
        match result {
            Ok((remaining, doc)) => {
                assert_eq!(remaining, "");
                assert_eq!(doc.name, "StatusInfo");
                assert_eq!(doc.since, Some("0.14.0"));
            }
            Err(e) => panic!("Since without colon should be accepted: {}", e),
        }
    }
}

#[cfg(test)]
mod freeform_tests {
    use super::*;
    use crate::parsers::{ParserConfig, set_parser_config};

    #[test]
    fn test_freeform_doc_simple() {
        set_parser_config(ParserConfig::modern());
        let input = "##\n# some text\n# more text\n##";
        let result = QapiFreeformDoc::parse(input);
        match result {
            Ok((remaining, _doc)) => {
                assert_eq!(remaining, "");
            }
            Err(e) => panic!("Freeform doc should parse: {}", e),
        }
    }

    #[test]
    fn test_freeform_doc_with_empty_lines() {
        set_parser_config(ParserConfig::modern());
        let input = "##\n# title\n#\n# body\n##";
        let result = QapiFreeformDoc::parse(input);
        match result {
            Ok((remaining, _doc)) => {
                assert_eq!(remaining, "");
            }
            Err(e) => panic!("Freeform doc with empty line should parse: {}", e),
        }
    }

    #[test]
    fn test_freeform_intro_block() {
        set_parser_config(ParserConfig::modern());
        let input = "##\n# ************\n# Introduction\n# ************\n#\n# Some description.\n##";
        let result = QapiFreeformDoc::parse(input);
        match result {
            Ok((remaining, _doc)) => {
                assert_eq!(remaining, "");
            }
            Err(e) => panic!("Introduction block should parse: {}", e),
        }
    }
}
