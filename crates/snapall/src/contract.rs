// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! snapall's invocation surface, declared once with argenv.
//!
//! This file is the single source of truth for flags, positionals and environment
//! variables: parsing, `--help`, the published `snapall contract` document and the
//! validation of invocations all derive from these declarations.

use argenv_resolve::*;

/// What the contract needs to know about a command beyond its name: its words, and which of the shared flags
/// it takes. Declared with the command, once, in the `commands!` macro below; every list of commands below (those that take
/// `--json`, that log, that take `--dry-run`, `--yes` or `--destroy`) is *derived* from it, so a command cannot
/// be taken by a flag in one place and forgotten in another.
#[derive(Clone, Copy)]
pub struct Spec {
    /// How it is typed.
    pub token: &'static str,
    /// One line for the command list in `--help`.
    pub summary: &'static str,
    /// Takes `--json`.
    pub json: bool,
    /// Writes to the log (`--log`, `--log-file`).
    pub logged: bool,
    /// Takes `--dry-run`: it can say what it would do without doing it.
    pub dry_run: bool,
    /// Takes `--yes`: it asks before a change that can be put back, and a script can answer.
    pub yes: bool,
    /// Takes `--destroy`: it asks before a loss for good, and a script can answer. `--yes` cannot.
    pub destroy: bool,
}

impl Spec {
    const fn new(token: &'static str, summary: &'static str) -> Spec {
        Spec {
            token,
            summary,
            json: false,
            logged: false,
            dry_run: false,
            yes: false,
            destroy: false,
        }
    }
    const fn json(mut self) -> Spec {
        self.json = true;
        self
    }
    const fn logged(mut self) -> Spec {
        self.logged = true;
        self
    }
    const fn dry_run(mut self) -> Spec {
        self.dry_run = true;
        self
    }
    const fn yes(mut self) -> Spec {
        self.yes = true;
        self
    }
    const fn destroy(mut self) -> Spec {
        self.destroy = true;
        self
    }
}

/// Declare every subcommand, once. The enum, the list of all of them and the exhaustive `match` that gives
/// each its [`Spec`] come from the same lines, so adding a command is one declaration, and one without its
/// words does not compile.
macro_rules! commands {
    ($($name:ident => $spec:expr,)+) => {
        /// Every subcommand. Inputs scoped to one are checked by the compiler: a typo or a removed
        /// variant fails the build instead of silently doing nothing.
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        pub enum Command {
            $($name,)+
        }

        impl Command {
            pub const ALL: &'static [Command] = &[$(Command::$name,)+];

            pub const fn spec(self) -> Spec {
                match self {
                    $(Command::$name => $spec,)+
                }
            }
        }
    };
}

commands! {
    Create => Spec::new("create", "Take one snapshot of every protected area (the default command)")
            .json()
            .logged()
            .dry_run(),
    List => Spec::new("list", "List the protected areas snapall can see")
            .json()
            .logged(),
    Snapshots => Spec::new("snapshots", "List the snapshots of one protected area, or of all of them")
            .json()
            .logged(),
    Delete => Spec::new("delete", "Delete one snapshot of a protected area")
            .json()
            .logged()
            .destroy(),
    Changes => Spec::new("changes", "Show what restoring a snapshot would change")
            .json()
            .logged(),
    Restore => Spec::new("restore", "Put a protected area, or one path of it, back to how a snapshot had it")
            .json()
            .logged()
            .dry_run()
            .yes(),
    RestoreAll => Spec::new("restore-all", "Put every protected area back to one restore point, in one action")
            .json()
            .logged()
            .dry_run()
            .yes(),
    Points => Spec::new("points", "List the restore points: the snapshots one action took together")
            .json()
            .logged(),
    History => Spec::new("history", "Show how one file stood in each snapshot")
            .json()
            .logged(),
    Health => Spec::new("health", "Show whether protection is working, and how much room it takes")
            .json()
            .logged(),
    Activity => Spec::new("activity", "Show what snapall has done, including earlier sessions, newest first")
            .json(),
    Groups => Spec::new("groups", "Name sets of areas, such as theming, to take snapshots of together")
            .json()
            .yes(),
    Limits => Spec::new("limits", "Show or change how many snapshots an area keeps")
            .json()
            .logged()
            .yes()
            .destroy(),
    Unprotect => Spec::new("unprotect", "Stop protecting an area: deletes all its snapshots for good")
            .json()
            .logged()
            .dry_run()
            .destroy(),
    Undo => Spec::new("undo", "Roll a change back, to the state it recorded, after showing what that is")
            .json()
            .logged()
            .yes()
            .destroy(),
    Cancel => Spec::new("cancel", "Ask a running change to stop after the area it is on")
            .json()
            .yes(),
    Quotas => Spec::new("quotas", "Show how the disk counts the room snapshots take, and switch it on or off")
            .json()
            .logged()
            .yes(),
    Setup => Spec::new("setup", "Protect what has no snapshots yet, explaining each step")
            .json()
            .logged()
            .dry_run()
            .yes(),
    Doctor => Spec::new("doctor", "Check that snapper works, and install it if it is missing")
            .json()
            .logged()
            .yes(),
    Completions => Spec::new("completions", "Print shell completions (bash, zsh, fish, nushell)"),
    Man => Spec::new("man", "Print the manual page"),
    Tui => Spec::new("tui", "Open the interactive terminal UI")
            .logged(),
    Contract => Spec::new("contract", "Print this program's invocation contract as argenv JSON"),
}

impl Command {
    pub const fn token(self) -> &'static str {
        self.spec().token
    }

    pub fn from_token(token: &str) -> Option<Command> {
        Command::ALL.iter().copied().find(|c| c.token() == token)
    }

    /// One line for the command list in `--help`.
    pub const fn summary(self) -> &'static str {
        self.spec().summary
    }
}

/// It is spelled as it is typed (`restore-all`), which is how the contract names it.
impl serde::Serialize for Command {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.token())
    }
}

/// Which shared flag a list of commands is about.
#[derive(Clone, Copy)]
enum Scope {
    Json,
    Logged,
    DryRun,
    Yes,
    Destroy,
}

const fn takes(command: Command, scope: Scope) -> bool {
    let spec = command.spec();
    match scope {
        Scope::Json => spec.json,
        Scope::Logged => spec.logged,
        Scope::DryRun => spec.dry_run,
        Scope::Yes => spec.yes,
        Scope::Destroy => spec.destroy,
    }
}

const fn how_many(scope: Scope) -> usize {
    let (mut found, mut at) = (0, 0);
    while at < Command::ALL.len() {
        if takes(Command::ALL[at], scope) {
            found += 1;
        }
        at += 1;
    }
    found
}

const fn commands_that<const N: usize>(scope: Scope) -> [Command; N] {
    let mut out = [Command::Create; N];
    let (mut filled, mut at) = (0, 0);
    while at < Command::ALL.len() {
        if takes(Command::ALL[at], scope) {
            out[filled] = Command::ALL[at];
            filled += 1;
        }
        at += 1;
    }
    out
}

const JSON_COMMANDS: [Command; how_many(Scope::Json)] = commands_that(Scope::Json);
const LOGGED_COMMANDS: [Command; how_many(Scope::Logged)] = commands_that(Scope::Logged);
const DRY_RUN_COMMANDS: [Command; how_many(Scope::DryRun)] = commands_that(Scope::DryRun);
const YES_COMMANDS: [Command; how_many(Scope::Yes)] = commands_that(Scope::Yes);
const DESTROY_COMMANDS: [Command; how_many(Scope::Destroy)] = commands_that(Scope::Destroy);

/// How events are written. `Off` is the default: a command-line tool is quiet unless asked.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    Off,
    Text,
    Json,
}

impl LogFormat {
    pub const TOKENS: &'static [&'static str] = &["off", "text", "json"];
}

impl std::fmt::Display for LogFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            LogFormat::Off => "off",
            LogFormat::Text => "text",
            LogFormat::Json => "json",
        })
    }
}

impl FromRaw for LogFormat {
    fn from_raw(raw: &str) -> Option<LogFormat> {
        Some(match raw.trim() {
            "off" | "none" => LogFormat::Off,
            "text" => LogFormat::Text,
            "json" => LogFormat::Json,
            _ => return None,
        })
    }
}

contract! {
    // ---- general ---------------------------------------------------------------------

    /// `-h`, `--help`.
    HELP: bool = Input {
        key:     "help",
        ty:      Type::Bool,
        default: Some(false),
        arg:     Some(Arg::pair("help", 'h')),
        group:   "general",
        summary: "Show this help and exit.",
        ..Input::EMPTY
    };

    /// `-V`, `--version`.
    VERSION: bool = Input {
        key:     "version",
        ty:      Type::Bool,
        default: Some(false),
        arg:     Some(Arg::pair("version", 'V')),
        group:   "general",
        summary: "Print the version and exit.",
        ..Input::EMPTY
    };

    /// Talk to libsnapper directly.
    NO_DBUS: bool = Input {
        key:     "no_dbus",
        ty:      Type::Bool,
        default: Some(false),
        env:     Some(Env::new("SNAPALL_NO_DBUS")),
        arg:     Some(Arg::long("no-dbus")),
        group:   "general",
        summary: "Talk to libsnapper directly instead of through snapperd (root only; for containers and CI).",
        ..Input::EMPTY
    };

    /// Tips.
    HINTS: bool, Command = Input {
        key:         "hints",
        ty:          Type::Bool,
        default:     Some(true),
        subcommands: &[Command::Create, Command::Tui],
        env:         Some(Env::new("SNAPALL_HINTS")),
        arg:         Some(Arg { negatable: true, ..Arg::long("hints") }),
        group:       "output",
        summary:     "Show a one-line tip when snapshots cannot be booted from the boot menu. --no-hints (or SNAPALL_HINTS=off) silences it.",
        ..Input::EMPTY
    };

    // ---- logging ---------------------------------------------------------------------

    /// What to log.
    LOG: LogFormat, Command = Input {
        key:         "log",
        ty:          Type::Enum,
        default:     Some(LogFormat::Off),
        allowed:     LogFormat::TOKENS,
        subcommands: &LOGGED_COMMANDS,
        env:         Some(Env::new("SNAPALL_LOG")),
        arg:         Some(Arg { value_name: "FORMAT", ..Arg::long("log") }),
        group:       "logging",
        example:     "snapall --log json create",
        summary:     "Write one line per operation (create, delete, list, install step) to stderr, as `text` or `json`. Default: off. Logs are an event stream: route them with the environment (journald, a file, a pipe).",
        ..Input::EMPTY
    };

    /// Where to log.
    LOG_FILE: std::path::PathBuf, Command = Input {
        key:         "log_file",
        ty:          Type::Path,
        subcommands: &LOGGED_COMMANDS,
        env:         Some(Env::new("SNAPALL_LOG_FILE")),
        arg:         Some(Arg { value_name: "PATH", ..Arg::long("log-file") }),
        group:       "logging",
        example:     "sudo snapall --log-file /var/log/snapall.jsonl create",
        summary:     "Append the log to this file (created with mode 0600) instead of stderr: an audit trail of everything snapall did. Implies `--log json` unless `--log` says otherwise. Required to log from `tui`, which owns the terminal.",
        ..Input::EMPTY
    };

    // ---- create ----------------------------------------------------------------------

    /// The text stored with each snapshot.
    DESCRIPTION: String, Command = Input {
        key:         "description",
        ty:          Type::String,
        subcommands: &[Command::Create],
        arg:         Some(Arg { value_name: "DESCRIPTION", ..Arg::positional(0) }),
        group:       "create",
        example:     "snapall create \"before theming\"",
        summary:     "Text stored with each snapshot (default: snapall). `snapall <DESCRIPTION>` is shorthand for `snapall create <DESCRIPTION>`.",
        ..Input::EMPTY
    };

    /// Which configs to snapshot.
    ///
    /// A `String`, not a list: argenv lists need a closed set of allowed tokens, and config
    /// names are only known at runtime. snapall splits and validates it itself.
    ONLY: String, Command = Input {
        key:         "only",
        ty:          Type::String,
        subcommands: &[Command::Create],
        env:         Some(Env::new("SNAPALL_ONLY")),
        arg:         Some(Arg { value_name: "CONFIGS", ..Arg::long("only") }),
        group:       "create",
        example:     "snapall create --only root,home",
        summary:     "Comma-separated names of the areas to snapshot (default: all of them; `snapall list` shows the names). A name that does not exist is an error.",
        ..Input::EMPTY
    };

    /// Show the plan without acting.
    DESTROY: bool, Command = Input {
        key:         "destroy",
        ty:          Type::Bool,
        default:     Some(false),
        subcommands: &DESTROY_COMMANDS,
        arg:         Some(Arg::long("destroy")),
        group:       "destroy",
        summary:     "Go ahead without asking, where it deletes snapshots for good. `--yes` is for changes that can be put back and is not enough for this.",
        ..Input::EMPTY
    };

    DRY_RUN: bool, Command = Input {
        key:         "dry_run",
        ty:          Type::Bool,
        default:     Some(false),
        subcommands: &DRY_RUN_COMMANDS,
        arg:         Some(Arg::long("dry-run")),
        group:       "create",
        summary:     "Show what would happen without changing anything.",
        ..Input::EMPTY
    };

    // ---- output ----------------------------------------------------------------------

    /// Machine-readable output.
    JSON: bool, Command = Input {
        key:         "json",
        ty:          Type::Bool,
        default:     Some(false),
        subcommands: &JSON_COMMANDS,
        env:         Some(Env::new("SNAPALL_JSON")),
        arg:         Some(Arg::long("json")),
        group:       "output",
        summary:     "Print JSON instead of text.",
        ..Input::EMPTY
    };

    // ---- snapshots -------------------------------------------------------------------

    /// Which config's snapshots to list.
    SNAPSHOTS_CONFIG: String, Command = Input {
        key:         "snapshots_config",
        ty:          Type::String,
        subcommands: &[Command::Snapshots],
        arg:         Some(Arg { value_name: "AREA", ..Arg::positional(0) }),
        group:       "snapshots",
        summary:     "List only this area's snapshots (default: all areas).",
        ..Input::EMPTY
    };

    // ---- delete ----------------------------------------------------------------------

    /// The config a snapshot belongs to.
    TARGET_CONFIG: String, Command = Input {
        key:         "config",
        ty:          Type::String,
        required:    true,
        subcommands: &[Command::Delete, Command::Changes, Command::Restore],
        arg:         Some(Arg { value_name: "AREA", ..Arg::positional(0) }),
        group:       "snapshot",
        summary:     "The name of the protected area, as `snapall list` shows it (required).",
        ..Input::EMPTY
    };

    /// The snapshot to act on.
    SNAPSHOT_ID: u64, Command = Input {
        key:         "snapshot_id",
        ty:          Type::Uint,
        required:    true,
        subcommands: &[Command::Delete, Command::Changes, Command::Restore],
        arg:         Some(Arg { value_name: "ID", ..Arg::positional(1) }),
        group:       "snapshot",
        example:     "snapall restore home 12",
        summary:     "The snapshot number (required). 0 is the current system, not a snapshot, and is refused.",
        ..Input::EMPTY
    };

    /// Limit changes and restore to one path.
    PATH: std::path::PathBuf, Command = Input {
        key:         "path",
        ty:          Type::Path,
        subcommands: &[Command::Changes, Command::Restore],
        arg:         Some(Arg { value_name: "PATH", ..Arg::positional(2) }),
        group:       "snapshot",
        example:     "snapall restore home 12 /home/me/notes.txt",
        summary:     "Only this file or directory (an absolute path inside the area). Default: everything that differs.",
        ..Input::EMPTY
    };

    /// The file whose history to show.
    HISTORY_FILE: std::path::PathBuf, Command = Input {
        key:         "history_file",
        ty:          Type::Path,
        required:    true,
        subcommands: &[Command::History],
        arg:         Some(Arg { value_name: "FILE", ..Arg::positional(0) }),
        group:       "history",
        example:     "snapall history ~/notes.txt",
        summary:     "The file (required). The protected area it is in is found from where it is.",
        ..Input::EMPTY
    };

    /// Open the TUI on one file.
    TUI_FILE: std::path::PathBuf, Command = Input {
        key:         "tui_file",
        ty:          Type::Path,
        subcommands: &[Command::Tui],
        arg:         Some(Arg::long("file")),
        group:       "tui",
        example:     "snapall tui --file ~/notes.txt",
        summary:     "Open on the history of this file, with its versions to restore, instead of on all areas.",
        ..Input::EMPTY
    };

    /// Which restore point to put every area back to.
    RESTORE_POINT: String, Command = Input {
        key:         "point",
        ty:          Type::String,
        subcommands: &[Command::RestoreAll],
        arg:         Some(Arg { value_name: "POINT", ..Arg::positional(0) }),
        group:       "restore",
        example:     "snapall restore-all 20261025T181500Z",
        summary:     "The restore point to go back to, as `snapall points` shows it (required).",
        ..Input::EMPTY
    };

    /// Protect this part of the disk.
    SETUP_AREA: std::path::PathBuf, Command = Input {
        key:         "area",
        ty:          Type::Path,
        subcommands: &[Command::Setup],
        arg:         Some(Arg { value_name: "AREA", ..Arg::positional(0) }),
        group:       "setup",
        example:     "snapall setup /srv",
        summary:     "A part of the disk to protect, such as /srv, instead of the ones snapall recommends. It is explained and asked about like the others.",
        ..Input::EMPTY
    };

    // ---- undo ---------------------------------------------------------------------------

    /// Which change to roll back.
    UNDO_CHANGE: String, Command = Input {
        key:         "undo_change",
        ty:          Type::String,
        subcommands: &[Command::Undo],
        arg:         Some(Arg { value_name: "CHANGE", ..Arg::positional(0) }),
        group:       "undo",
        example:     "snapall undo 4242-1793029083222-c",
        summary:     "The name of the change, as `snapall activity` shows it (required). Where nothing can be rolled back it says why instead.",
        ..Input::EMPTY
    };

    // ---- unprotect ----------------------------------------------------------------------

    /// Which area to stop protecting.
    UNPROTECT_AREA: String, Command = Input {
        key:         "unprotect_area",
        ty:          Type::String,
        subcommands: &[Command::Unprotect],
        arg:         Some(Arg { value_name: "AREA", ..Arg::positional(0) }),
        group:       "unprotect",
        example:     "snapall unprotect games --dry-run",
        summary:     "The area, as `snapall list` names it (required). The system area is never stopped.",
        ..Input::EMPTY
    };

    // ---- limits -------------------------------------------------------------------------

    /// Which area.
    LIMITS_AREA: String, Command = Input {
        key:         "limits_area",
        ty:          Type::String,
        subcommands: &[Command::Limits],
        arg:         Some(Arg { value_name: "AREA", ..Arg::positional(0) }),
        group:       "limits",
        example:     "snapall limits home --keep 30",
        summary:     "The area, as `snapall list` names it (required).",
        ..Input::EMPTY
    };

    /// How many to keep.
    LIMITS_KEEP: String, Command = Input {
        key:         "keep",
        ty:          Type::String,
        subcommands: &[Command::Limits],
        arg:         Some(Arg::long("keep")),
        group:       "limits",
        example:     "snapall limits home --keep 30",
        summary:     "How many of the snapshots that cleanup looks after to keep: the newest this many stay, the older ones go at the next cleanup.",
        ..Input::EMPTY
    };

    /// How young is too young to remove.
    LIMITS_MIN_AGE: String, Command = Input {
        key:         "min_age",
        ty:          Type::String,
        subcommands: &[Command::Limits],
        arg:         Some(Arg::long("min-age")),
        group:       "limits",
        example:     "snapall limits home --min-age 60",
        summary:     "Minutes: cleanup never removes a snapshot younger than this, however many there are.",
        ..Input::EMPTY
    };

    /// Whether cleanup runs for this area.
    LIMITS_CLEANUP: String, Command = Input {
        key:         "limits_cleanup",
        ty:          Type::String,
        subcommands: &[Command::Limits],
        arg:         Some(Arg::long("cleanup")),
        group:       "limits",
        example:     "snapall limits home --cleanup off",
        summary:     "`on` or `off`: whether cleanup removes anything of this area at all.",
        ..Input::EMPTY
    };

    // ---- groups -------------------------------------------------------------------------

    /// What to do with groups.
    GROUPS_ACTION: String, Command = Input {
        key:         "action",
        ty:          Type::String,
        subcommands: &[Command::Groups],
        arg:         Some(Arg { value_name: "ACTION", ..Arg::positional(0) }),
        group:       "groups",
        example:     "snapall groups add theming home,data",
        summary:     "`add` or `remove`. Without it, the groups are listed.",
        ..Input::EMPTY
    };

    /// Which group.
    GROUPS_NAME: String, Command = Input {
        key:         "group_name",
        ty:          Type::String,
        subcommands: &[Command::Groups],
        arg:         Some(Arg { value_name: "NAME", ..Arg::positional(1) }),
        group:       "groups",
        example:     "snapall groups remove theming",
        summary:     "The group's name: lower-case letters, digits, - and _, starting with a letter. `all` is always there and cannot be changed.",
        ..Input::EMPTY
    };

    /// The areas of the group.
    GROUPS_AREAS: String, Command = Input {
        key:         "group_areas",
        ty:          Type::String,
        subcommands: &[Command::Groups],
        arg:         Some(Arg { value_name: "AREAS", ..Arg::positional(2) }),
        group:       "groups",
        example:     "snapall groups add theming home,data",
        summary:     "With `add`: the areas, separated by commas, as `snapall list` names them.",
        ..Input::EMPTY
    };

    /// Which group to take a snapshot of.
    GROUP: String, Command = Input {
        key:         "group",
        ty:          Type::String,
        subcommands: &[Command::Create],
        arg:         Some(Arg::long("group")),
        group:       "create",
        example:     "snapall create --group theming \"before the new theme\"",
        summary:     "Take a snapshot of the areas of this group (see `snapall groups`) instead of all of them. Not together with --only.",
        ..Input::EMPTY
    };

    // ---- running changes ----------------------------------------------------------------

    /// The name this change has in the journal.
    JOB: String, Command = Input {
        key:         "job",
        ty:          Type::String,
        subcommands: &[Command::Create, Command::RestoreAll],
        arg:         Some(Arg::long("job")),
        group:       "create",
        example:     "snapall create --job nightly-1",
        summary:     "Name this change in the journal, so that `snapall cancel NAME` can ask it to stop. snapall gives its own name when the window starts a change in the background.",
        ..Input::EMPTY
    };

    /// Which change to ask to stop.
    CANCEL_JOB: String, Command = Input {
        key:         "change",
        ty:          Type::String,
        subcommands: &[Command::Cancel],
        arg:         Some(Arg { value_name: "CHANGE", ..Arg::positional(0) }),
        group:       "cancel",
        example:     "snapall cancel 4242-1793029083222-c",
        summary:     "The name of the change, as `snapall activity --json` shows it (required).",
        ..Input::EMPTY
    };

    // ---- quotas ----------------------------------------------------------------------

    /// Whether the disk should count the room snapshots take.
    QUOTAS_STATE: String, Command = Input {
        key:         "state",
        ty:          Type::String,
        subcommands: &[Command::Quotas],
        arg:         Some(Arg { value_name: "STATE", ..Arg::positional(0) }),
        group:       "quotas",
        example:     "sudo snapall quotas on",
        summary:     "`on` or `off`. Without it, only shows how the disk counts room now and what switching it would mean.",
        ..Input::EMPTY
    };

    // ---- doctor ----------------------------------------------------------------------

    /// Install what is missing.
    INSTALL: bool, Command = Input {
        key:         "install",
        ty:          Type::Bool,
        default:     Some(false),
        subcommands: &[Command::Doctor],
        arg:         Some(Arg::long("install")),
        group:       "doctor",
        example:     "sudo snapall doctor --install",
        summary:     "Install snapper with this system's package manager if it is missing. Shows the commands and asks first; needs root.",
        ..Input::EMPTY
    };

    /// Do not ask.
    YES: bool, Command = Input {
        key:         "yes",
        ty:          Type::Bool,
        default:     Some(false),
        subcommands: &YES_COMMANDS,
        arg:         Some(Arg::pair("yes", 'y')),
        group:       "doctor",
        summary:     "Answer yes where snapall asks before a change that can be put back (for scripts and CI). It does not answer for deleting snapshots: that needs --destroy. With doctor, only with --install.",
        ..Input::EMPTY
    };

    // ---- completions and man ---------------------------------------------------------

    /// Which shell.
    SHELL: String, Command = Input {
        key:         "shell",
        ty:          Type::Enum,
        required:    true,
        allowed:     &["bash", "zsh", "fish", "nushell"],
        subcommands: &[Command::Completions],
        arg:         Some(Arg { value_name: "SHELL", ..Arg::positional(0) }),
        group:       "completions",
        example:     "snapall completions bash",
        summary:     "The shell to print completions for. Derived from this very contract, so they cannot disagree with what snapall accepts.",
        ..Input::EMPTY
    };

    /// A reproducible man page date.
    SOURCE_DATE_EPOCH: u64, Command = Input {
        key:         "source_date_epoch",
        ty:          Type::Uint,
        subcommands: &[Command::Man],
        env:         Some(Env::new("SOURCE_DATE_EPOCH")),
        group:       "man",
        summary:     "Seconds since the Unix epoch to use as the man page date, so that builds are reproducible (the SOURCE_DATE_EPOCH convention). Default: now.",
        ..Input::EMPTY
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use snapall_core::{Act, Weight};

    /// The command that carries out each act. Exhaustive, so a new act does not compile until it is placed.
    fn carried_out_by(act: Act) -> Option<&'static str> {
        match act {
            Act::Read | Act::TakeSnapshot => None,
            Act::DeleteSnapshots => Some("delete"),
            Act::Restore => Some("restore"),
            Act::RestorePoint => Some("restore-all"),
            Act::Protect => Some("setup"),
            Act::StopProtecting => Some("unprotect"),
            Act::ChangeLimits { .. } => Some("limits"),
            Act::RoomCounting => Some("quotas"),
            Act::ChangeGroups => Some("groups"),
            // stopping a running change is `cancel`; the window's job controls have no command at all
            Act::StopChange => Some("cancel"),
            Act::ControlJob => None,
            Act::InstallSnapper => Some("doctor"),
            Act::Undo { .. } => Some("undo"),
        }
    }

    const EVERY_ACT: [Act; 16] = [
        Act::Read,
        Act::TakeSnapshot,
        Act::DeleteSnapshots,
        Act::Restore,
        Act::RestorePoint,
        Act::Protect,
        Act::StopProtecting,
        Act::ChangeLimits { removes: true },
        Act::ChangeLimits { removes: false },
        Act::RoomCounting,
        Act::ChangeGroups,
        Act::ControlJob,
        Act::StopChange,
        Act::InstallSnapper,
        // it weighs what the change it rolls back weighs: so it takes both flags
        Act::Undo {
            doing: Weight::Changes,
        },
        Act::Undo {
            doing: Weight::Destroys,
        },
    ];

    #[test]
    fn a_command_takes_the_flag_that_answers_what_it_does_and_no_flag_that_does_not() {
        for act in EVERY_ACT {
            let Some(token) = carried_out_by(act) else {
                continue;
            };
            let spec = Command::from_token(token)
                .expect("a command that exists")
                .spec();
            match act.weight() {
                Weight::Destroys => assert!(
                    spec.destroy,
                    "`{token}` does {act:?}, which loses something for good: it needs --destroy"
                ),
                Weight::Changes => {
                    assert!(spec.yes, "`{token}` does {act:?}, a change: it needs --yes")
                }
                Weight::Light => {}
            }
        }
        // and the other way: --destroy is only on a command that can destroy, and a command that can only
        // destroy has no --yes, which could not answer for it
        for command in Command::ALL {
            let spec = command.spec();
            let weights: Vec<Weight> = EVERY_ACT
                .iter()
                .filter(|act| carried_out_by(**act) == Some(spec.token))
                .map(|act| act.weight())
                .collect();
            assert!(
                !spec.destroy || weights.contains(&Weight::Destroys),
                "`{}` takes --destroy but destroys nothing",
                spec.token
            );
            assert!(
                !spec.yes || weights.contains(&Weight::Changes),
                "`{}` takes --yes but changes nothing it asks about",
                spec.token
            );
            let only_destroys =
                !weights.is_empty() && weights.iter().all(|w| *w == Weight::Destroys);
            assert!(
                !(only_destroys && spec.yes),
                "`{}` only destroys, so --yes must not be accepted",
                spec.token
            );
        }
    }

    #[test]
    fn every_command_has_its_own_words_and_the_derived_lists_say_what_the_declarations_say() {
        let mut seen = std::collections::BTreeSet::new();
        for command in Command::ALL {
            let spec = command.spec();
            assert!(
                seen.insert(spec.token),
                "`{}` is declared twice",
                spec.token
            );
            assert!(
                !spec.summary.is_empty()
                    && spec.summary.chars().next().is_some_and(char::is_uppercase),
                "{}: a summary is a sentence",
                spec.token
            );
            assert_eq!(Command::from_token(spec.token), Some(*command));
            assert_eq!(JSON_COMMANDS.contains(command), spec.json, "{}", spec.token);
            assert_eq!(
                LOGGED_COMMANDS.contains(command),
                spec.logged,
                "{}",
                spec.token
            );
            assert_eq!(YES_COMMANDS.contains(command), spec.yes, "{}", spec.token);
            assert_eq!(
                DESTROY_COMMANDS.contains(command),
                spec.destroy,
                "{}",
                spec.token
            );
            assert_eq!(
                DRY_RUN_COMMANDS.contains(command),
                spec.dry_run,
                "{}",
                spec.token
            );
        }
    }
}
