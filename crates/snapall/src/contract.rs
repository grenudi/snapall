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
    Tui,
    Contract,
}

impl Command {
    pub const ALL: &'static [Command] = &[
        Command::Create,
        Command::List,
        Command::Snapshots,
        Command::Delete,
        Command::Tui,
        Command::Contract,
    ];

    pub const fn token(self) -> &'static str {
        match self {
            Command::Create => "create",
            Command::List => "list",
            Command::Snapshots => "snapshots",
            Command::Delete => "delete",
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
            Command::Tui => "Open the interactive terminal UI",
            Command::Contract => "Print this program's invocation contract as argenv JSON",
        }
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
        subcommands: &[Command::Create],
        arg:         Some(Arg::long("dry-run")),
        group:       "create",
        summary:     "Show what would be snapshotted without creating anything.",
        ..Input::EMPTY
    };

    // ---- output ----------------------------------------------------------------------

    /// Machine-readable output.
    JSON: bool, Command = Input {
        key:         "json",
        ty:          Type::Bool,
        default:     Some(false),
        subcommands: &[Command::Create, Command::List, Command::Snapshots],
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

    /// The config to delete from. Required by `delete`, but not declared `required`: argenv's
    /// lint ignores subcommand scope, so a `required` here would make `argenv lint` reject
    /// every *other* command. snapall enforces it itself (see the characterization tests).
    DELETE_CONFIG: String, Command = Input {
        key:         "delete_config",
        ty:          Type::String,
        subcommands: &[Command::Delete],
        arg:         Some(Arg { value_name: "CONFIG", ..Arg::positional(0) }),
        group:       "delete",
        summary:     "The config the snapshot belongs to (required).",
        ..Input::EMPTY
    };

    /// The snapshot to delete.
    SNAPSHOT_ID: u64, Command = Input {
        key:         "snapshot_id",
        ty:          Type::Uint,
        subcommands: &[Command::Delete],
        arg:         Some(Arg { value_name: "ID", ..Arg::positional(1) }),
        group:       "delete",
        example:     "snapall delete home 12",
        summary:     "The snapshot number to delete (required). 0 is the current system and is refused.",
        ..Input::EMPTY
    };
}
