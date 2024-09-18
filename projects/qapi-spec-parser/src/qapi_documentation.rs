use nom::branch::alt;
use nom::bytes::complete::{tag, tag_no_case, take_while1};
use nom::character::complete::{line_ending, multispace0, not_line_ending, space0};
use nom::character::is_space;
use nom::combinator::{map, not, opt, recognize};
use nom::multi::{many0, many1};
use nom::sequence::{delimited, pair, preceded, terminated, tuple};
use nom::IResult;
use std::collections::HashMap;

pub(crate) fn trim_docstring(input: &str) -> String {
    let mut output = String::new();
    for line in input.lines() {
        let trimmed = line.trim_start_matches('#');
        let trimmed = trimmed.trim_end();
        if trimmed.is_empty() {
            output.push('\n');
            continue;
        }
        let trimmed = if trimmed.starts_with(' ') {
            &trimmed[1..]
        } else {
            trimmed
        };
        output.push_str(trimmed);
        output.push('\n');
    }
    output.trim().to_string()
}

fn not_name_break(c: char) -> bool {
    c != ':' && !is_space(c as u8)
}

fn take_name(input: &str) -> IResult<&str, &str> {
    delimited(
        tuple((take_line_start, tag("@"))),
        take_while1(not_name_break),
        tuple((space0, tag(":"))),
    )(input)
}

fn take_namedkv<'input>(
    key: &'static str,
) -> impl FnMut(&'input str) -> IResult<&'input str, &'input str> {
    preceded(
        take_namedkey(key),
        recognize(tuple((
            not_line_ending,
            line_ending,
            opt(take_multiline_text),
        ))),
    )
}

// Case insensitive match for a line like: ```# {key}:```
fn take_namedkey<'input>(
    key: &'static str,
) -> impl FnMut(&'input str) -> IResult<&'input str, &'input str> {
    delimited(
        take_line_start,
        tag_no_case(key),
        tuple((space0, tag(":"), space0)),
    )
}

fn take_line_start(input: &str) -> IResult<&str, &str> {
    recognize(tuple((tag("#"), space0)))(input)
}

fn take_line_end(input: &str) -> IResult<&str, &str> {
    recognize(tuple((space0, line_ending)))(input)
}

fn take_empty_line(input: &str) -> IResult<&str, &str> {
    recognize(tuple((take_line_start, take_line_end)))(input)
}

fn take_value(input: &str) -> IResult<&str, &str> {
    recognize(tuple((
        not_line_ending,
        line_ending,
        opt(take_multiline_text),
    )))(input)
}

// SUCCESS: on success, this is equivalent to the `take_line_start` parser
// FAILURE: fails when the parser encouters the start of a new section
fn take_line_continuation(input: &str) -> IResult<&str, &str> {
    recognize(tuple((
        not(alt((
            take_name,
            take_namedkey("since"),
            take_namedkey("returns"),
            take_namedkey("errors"),
            take_namedkey("features"),
        ))),
        take_line_start,
        not(alt((
            tag(".. "), // gen-doc strings like `.. qmp-example::`
            tag("#"),   // we are at the end-of-section marker if this succeeds
        ))),
    )))(input)
}

fn take_multiline_text(input: &str) -> IResult<&str, &str> {
    recognize(many1(delimited(
        take_line_continuation,
        not_line_ending,
        line_ending,
    )))(input)
}

enum ParserToken<'input> {
    Errors(&'input str),
    Since(&'input str),
    Returns(&'input str),
    Note(&'input str),
    Caution(&'input str),
    QmpExample(&'input str),
    Table(&'input str),
    Admonition(&'input str),
    Features(HashMap<&'input str, &'input str>),
}

#[derive(Debug, Clone)]
pub struct QapiDocumentation<'input> {
    pub name: &'input str,
    pub fields: HashMap<&'input str, &'input str>,
    pub features: HashMap<&'input str, &'input str>,
    pub description: Option<&'input str>,
    pub errors: Option<&'input str>,
    pub since: Option<&'input str>,
    pub returns: Option<&'input str>,
    pub qmp_examples: Vec<&'input str>,
    pub notes: Vec<&'input str>,
    pub admonitions: Vec<&'input str>,
    pub tables: Vec<&'input str>,
    pub cautions: Vec<&'input str>,
}

impl<'input> QapiDocumentation<'input> {
    pub fn parse(input: &'input str) -> IResult<&'input str, Self> {
        let (input, _) = terminated(tag("##"), take_line_end)(input)?;
        let (input, _) = many0(take_empty_line)(input)?;
        let (input, name) = terminated(take_name, take_line_end)(input)?;
        let (input, _) = many0(take_empty_line)(input)?;
        let (input, description) = opt(take_multiline_text)(input)?;
        let (input, _) = many0(take_empty_line)(input)?;
        let (input, fields_vec_tuple) = many0(pair(take_name, take_value))(input)?;
        let (input, _) = many0(take_empty_line)(input)?;

        ////
        // at this point in the parsing, we have to switch to a more dynamic
        // parsing mode. While for the most part the keys are ordered, there are
        // some exceptions. Since there is no convention I can find, we must
        // match them in any order.
        ////

        // all branches of the `alt()` parser need to have the same type, which
        // leads to this `map(take_something, |v| ParserToken::Something(v))`
        // pattern. It's not my favorite, but not all return types are `&str`.
        let (input, tokens) = many0(alt((
            map(take_namedkv("since"), |v| ParserToken::Since(v)),
            map(take_namedkv("returns"), |v| ParserToken::Returns(v)),
            map(take_namedkv("errors"), |v| ParserToken::Errors(v)),
            map(take_namedkv(".. note:"), |v| ParserToken::Note(v)),
            map(take_namedkv(".. caution:"), |v| ParserToken::Caution(v)),
            map(take_namedkv(".. table:"), |v| ParserToken::Table(v)),
            map(take_namedkv(".. qmp-example:"), |v| {
                ParserToken::QmpExample(v)
            }),
            map(take_namedkv(".. admonition:"), |v| {
                ParserToken::Admonition(v)
            }),
            map(
                preceded(
                    tuple((
                        take_namedkey("features"),
                        take_line_end,
                        many0(take_empty_line),
                    )),
                    many1(pair(take_name, take_value)),
                ),
                |v| {
                    let mut features = HashMap::new();
                    for (name, description) in v {
                        features.insert(name, description);
                    }
                    ParserToken::Features(features)
                },
            ),
        )))(input)?;

        // UPSTREAMFIX: This is because `qga/qapi-schema.json` in the `@GuestNetworkRoute` doc block.
        let (input, _) = multispace0(input)?;
        let (input, _) = tag("##")(input)?;

        ////
        // Parsing is now done, we might still return a parsing error after
        // this point, but that will be an issue of a missing field or other
        // required value. The `input` string will no longer be touched though
        ////

        let mut since = None;
        let mut errors = None;
        let mut returns = None;
        let mut features = None;
        let mut notes = vec![];
        let mut cautions = vec![];
        let mut tables = vec![];
        let mut qmp_examples = vec![];
        let mut admonitions = vec![];
        for token in tokens {
            match token {
                ParserToken::Since(v) => since = Some(v),
                ParserToken::Errors(v) => errors = Some(v),
                ParserToken::Returns(v) => returns = Some(v),
                ParserToken::Features(v) => features = Some(v),
                ParserToken::Note(v) => notes.push(v),
                ParserToken::Caution(v) => cautions.push(v),
                ParserToken::Table(v) => tables.push(v),
                ParserToken::QmpExample(v) => qmp_examples.push(v),
                ParserToken::Admonition(v) => admonitions.push(v),
            }
        }
        let features = features.unwrap_or(HashMap::new());

        // sort out fields
        let mut fields = HashMap::new();
        for (name, description) in fields_vec_tuple {
            fields.insert(name, description);
        }

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
            dbg![&result];
            match result {
                Ok((remaining, d)) => {
                    assert_eq!(remaining, "");
                }
                _ => panic!("Failed to parse: ```\n{input}\n```"),
            }
        }
    }
}
