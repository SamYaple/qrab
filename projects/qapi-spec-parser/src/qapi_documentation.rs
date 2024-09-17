use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::bytes::complete::take_until;
use nom::character::complete::{line_ending, not_line_ending, space0};
use nom::combinator::{map, not, opt, recognize};
use nom::multi::{many0, many1};
use nom::sequence::{delimited, pair, preceded, tuple};
use nom::IResult;
use std::collections::HashMap;

#[derive(Debug)]
pub struct QapiDocumentation {
    name: String,
    description: Option<String>,
    note: Option<String>,
    fields: HashMap<String, String>,
    features: HashMap<String, String>,
    errors: Option<String>,
    since: Option<String>,
    returns: Option<String>,
    admonition: Option<String>,
    example: Option<String>,
    table: Option<String>,
    caution: Option<String>,
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

// TODO make this a proper parser instead of this String type
fn take_description(input: &str) -> IResult<&str, String> {
    let (input, description) = many1(preceded(
        tuple((
            take_empty,
            not(take_name),
            tag("#"),
            not(alt((tag("#"), tag(" ..")))),
            space0,
            not(alt((
                tag("Features:"),
                tag("Returns:"),
                tag("Errors:"),
                alt((tag("Since:"), tag("since:"))),
            ))),
        )),
        not_line_ending,
    ))(input)?;
    let description = {
        let mut cc = String::new();
        for line in description {
            if cc.len() != 0 {
                cc.push_str(" ");
            }
            cc.push_str(line);
        }
        cc
    };
    Ok((input, description))
}

fn take_fields(input: &str) -> IResult<&str, (&str, &str)> {
    delimited(
        take_empty,
        pair(
            take_name,
            recognize(tuple((opt(not_line_ending), opt(take_description)))),
        ),
        take_empty,
    )(input)
}

impl QapiDocumentation {
    pub fn parse(input: &str) -> IResult<&str, Self> {
        let take_table = delimited(
            tuple((take_empty, tag("#"), space0, tag(".. table::"))),
            recognize(tuple((opt(not_line_ending), opt(take_description)))),
            take_empty,
        );
        let take_caution = delimited(
            tuple((take_empty, tag("#"), space0, tag(".. caution::"))),
            recognize(tuple((opt(not_line_ending), opt(take_description)))),
            take_empty,
        );
        let take_note = delimited(
            tuple((take_empty, tag("#"), space0, tag(".. note::"))),
            recognize(tuple((opt(not_line_ending), opt(take_description)))),
            take_empty,
        );
        let take_admonition = delimited(
            tuple((take_empty, tag("#"), space0, tag(".. admonition::"))),
            recognize(tuple((opt(not_line_ending), opt(take_description)))),
            take_empty,
        );
        let take_example = delimited(
            tuple((take_empty, tag("#"), space0, tag(".. qmp-example::"))),
            recognize(tuple((opt(not_line_ending), opt(take_description)))),
            take_empty,
        );
        let take_errors = delimited(
            take_empty,
            preceded(
                tuple((tag("#"), space0, tag("Errors:"), space0)),
                recognize(tuple((opt(not_line_ending), opt(take_description)))),
            ),
            take_empty,
        );
        let take_features = delimited(
            take_empty,
            preceded(
                tuple((tag("#"), space0, tag("Features:"), space0)),
                many0(take_fields),
                //recognize(tuple((opt(not_line_ending), opt(take_description)))),
            ),
            take_empty,
        );
        let take_returns = delimited(
            take_empty,
            preceded(
                tuple((tag("#"), space0, tag("Returns:"), space0)),
                recognize(tuple((opt(not_line_ending), opt(take_description)))),
            ),
            take_empty,
        );
        let take_since = delimited(
            take_empty,
            preceded(
                tuple((
                    tag("#"),
                    space0,
                    alt((tag("Since:"), tag("since:"))),
                    space0,
                )),
                recognize(tuple((opt(not_line_ending), opt(take_description)))),
            ),
            take_empty,
        );

        let (input, _) = tag("##")(input)?;
        let (input, _) = take_empty(input)?;
        let (input, name) = take_name(input)?;
        let (input, _) = take_empty(input)?;
        let (input, description) = opt(take_description)(input)?;
        let (input, _) = take_empty(input)?;
        let (input, fields_vec_tuple) = many0(take_fields)(input)?;
        let (input, _) = take_empty(input)?;
        let (input, features_vec_tuple) = opt(take_features)(input)?;
        let (input, _) = take_empty(input)?;
        let (input, returns) = opt(take_returns)(input)?;
        let (input, _) = take_empty(input)?;
        let (input, errors) = opt(take_errors)(input)?;
        let (input, _) = take_empty(input)?;
        let (input, note) = opt(recognize(many1(take_note)))(input)?;
        let (input, _) = take_empty(input)?;
        let (input, since) = opt(take_since)(input)?;
        let (input, _) = take_empty(input)?;
        let (input, table) = opt(recognize(many1(take_table)))(input)?;
        let (input, _) = take_empty(input)?;
        let (input, caution) = opt(recognize(many1(take_caution)))(input)?;
        let (input, _) = take_empty(input)?;
        let (input, admonition) = opt(recognize(many1(take_admonition)))(input)?;
        let (input, _) = take_empty(input)?;
        let (input, example) = opt(recognize(many1(take_example)))(input)?;
        let (input, _) = take_empty(input)?;
        let (input, _) = tag("##")(input)?;

        let name = name.into();
        let mut fields = HashMap::new();
        for (name, value) in fields_vec_tuple {
            fields.insert(name.into(), value.into());
        }
        let mut features = HashMap::new();
        if let Some(features_vec) = features_vec_tuple {
            for (name, value) in features_vec {
                features.insert(name.into(), value.into());
            }
        };
        let errors = if let Some(v) = errors { Some(v.into()) } else { None };
        let caution = if let Some(v) = caution { Some(v.into()) } else { None };
        let table = if let Some(v) = table { Some(v.into()) } else { None };
        let note = if let Some(v) = note { Some(v.into()) } else { None };
        let returns = if let Some(v) = returns { Some(v.into()) } else { None };
        let since = if let Some(v) = since { Some(v.into()) } else { None };
        let example = if let Some(v) = example { Some(v.into()) } else { None };
        let admonition = if let Some(v) = admonition { Some(v.into()) } else { None };
        Ok((
            input,
            Self {
                features,
                caution,
                table,
                errors,
                note,
                name,
                description,
                returns,
                fields,
                since,
                example,
                admonition,
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_INPUTS: [&str; 11] = [
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
