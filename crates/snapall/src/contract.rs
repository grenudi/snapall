// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! snapall's invocation surface, declared once with argenv.
//!
//! This file is the single source of truth for flags, positionals and environment
//! variables: parsing, `--help`, the published `snapall contract` document and the
//! validation of invocations all derive from these declarations.

use argenv_resolve::*;

/// Every subcommand. Inputs scoped to one are checked by the compiler: a typo or a removed
/// variant fails the build instead of silently doing nothing.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Command {
    Create,
    List,
    Snapshots,
    Delete,
    Changes,
    Restore,
    Doctor,
    Completions,
    Man,
    Tui,
    Contract,
}

impl Command {
    pub const ALL: &'static [Command] = &[
        Command::Create,
        Command::List,
        Command::Snapshots,
        Command::Delete,
        Command::Changes,
        Command::Restore,
        Command::Doctor,
        Command::Completions,
        Command::Man,
        Command::Tui,
        Command::Contract,
    ];

    pub const fn token(self) -> &'static str {
        match self {
            Command::Create => "create",
            Command::List => "list",
            Command::Snapshots => "snapshots",
            Command::Delete => "delete",
            Command::Changes => "changes",
            Command::Restore => "restore",
            Command::Doctor => "doctor",
            Command::Completions => "completions",
            Command::Man => "man",
            Command::Tui => "tui",
            Command::Contract => "contract",
        }
    }

    pub fn from_token(token: &str) -> Option<Command> {
        Command::ALL.iter().copied().find(|c| c.token() == token)
    }

    /// One line for the command list in `--help`.
    pub const fn summary(self) -> &'static str {
        match self {
            Command::Create => "Take one snapshot of every snapper config (the default command)",
            Command::List => "List the snapper configs snapall can see",
            Command::Snapshots => "List the snapshots of one config, or of every config",
            Command::Delete => "Delete one snapshot of a config",
            Command::Changes => "Show what restoring a snapshot would change",
            Command::Restore => "Put a config, or one path of it, back to how a snapshot had it",
            Command::Doctor => "Check that snapper works, and install it if it is missing",
            Command::Completions => "Print shell completions (bash, zsh, fish, nushell)",
            Command::Man => "Print the manual page",
            Command::Tui => "Open the interactive terminal UI",
            Command::Contract => "Print this program's invocation contract as argenv JSON",
        }
    }
}

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

/// The commands that do something observable (everything but `contract`).
const OBSERVED: &[Command] = &[
    Command::Create,
    Command::List,
    Command::Snapshots,
    Command::Delete,
    Command::Doctor,
    Command::Tui,
];

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
        subcommands: OBSERVED,
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
        subcommands: OBSERVED,
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
        summary:     "Comma-separated configs to snapshot (default: every config). A name snapper does not know is an error.",
        ..Input::EMPTY
    };

    /// Show the plan without acting.
    DRY_RUN: bool, Command = Input {
        key:         "dry_run",
        ty:          Type::Bool,
        default:     Some(false),
        subcommands: &[Command::Create, Command::Restore],
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
        subcommands: &[
            Command::Create,
            Command::List,
            Command::Snapshots,
            Command::Delete,
            Command::Changes,
            Command::Restore,
            Command::Doctor,
        ],
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
        arg:         Some(Arg { value_name: "CONFIG", ..Arg::positional(0) }),
        group:       "snapshots",
        summary:     "List only this config's snapshots (default: every config).",
        ..Input::EMPTY
    };

    // ---- delete ----------------------------------------------------------------------

    /// The config a snapshot belongs to.
    TARGET_CONFIG: String, Command = Input {
        key:         "config",
        ty:          Type::String,
        required:    true,
        subcommands: &[Command::Delete, Command::Changes, Command::Restore],
        arg:         Some(Arg { value_name: "CONFIG", ..Arg::positional(0) }),
        group:       "snapshot",
        summary:     "The config the snapshot belongs to (required).",
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
        summary:     "Only this file or directory (an absolute path inside the config). Default: everything that differs.",
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
        subcommands: &[Command::Doctor, Command::Restore],
        arg:         Some(Arg::pair("yes", 'y')),
        group:       "doctor",
        summary:     "Do not ask for confirmation (for scripts and CI). With doctor, only with --install.",
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
