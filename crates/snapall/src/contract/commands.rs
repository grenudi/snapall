// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The commands: what can be asked for, each with the acts it does and the flags their weight gives it.

use snapall_core::{Act, Weight};

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
    /// What it does, in the app's own words ([`Act`]). Whether it asks, and what a script must say to answer,
    /// is not declared here a second time: it is the weight of these.
    pub acts: [Option<Act>; 2],
    /// Takes `--yes`: some act of it is a change that can be put back. Derived from [`Spec::acts`].
    pub yes: bool,
    /// Takes `--destroy`: some act of it loses something for good. Derived from [`Spec::acts`]; `--yes` cannot answer.
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
            acts: [None, None],
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
    /// One thing it does. Its weight decides which of `--yes` and `--destroy` it takes.
    const fn does(mut self, act: Act) -> Spec {
        match act.weight() {
            Weight::Light => {}
            Weight::Changes => self.yes = true,
            Weight::Destroys => self.destroy = true,
        }
        if self.acts[0].is_none() {
            self.acts[0] = Some(act);
        } else {
            self.acts[1] = Some(act);
        }
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
        .dry_run()
        .does(Act::TakeSnapshot),
    List => Spec::new("list", "List the protected areas snapall can see")
        .json()
        .logged(),
    Snapshots => Spec::new("snapshots", "List the snapshots of one protected area, or of all of them")
        .json()
        .logged(),
    Delete => Spec::new("delete", "Delete one snapshot of a protected area")
        .json()
        .logged()
        .does(Act::DeleteSnapshots),
    Changes => Spec::new("changes", "Show what restoring a snapshot would change")
        .json()
        .logged(),
    Restore => Spec::new("restore", "Put a protected area, or one path of it, back to how a snapshot had it")
        .json()
        .logged()
        .dry_run()
        .does(Act::Restore),
    RestoreAll => Spec::new("restore-all", "Put every protected area back to one restore point, in one action")
        .json()
        .logged()
        .dry_run()
        .does(Act::RestorePoint),
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
        .does(Act::ChangeGroups),
    Limits => Spec::new("limits", "Show or change how many snapshots an area keeps")
        .json()
        .logged()
        .does(Act::ChangeLimits { removes: false })
        .does(Act::ChangeLimits { removes: true }),
    Unprotect => Spec::new("unprotect", "Stop protecting an area: deletes all its snapshots for good")
        .json()
        .logged()
        .dry_run()
        .does(Act::StopProtecting),
    Undo => Spec::new("undo", "Roll a change back, to the state it recorded, after showing what that is")
        .json()
        .logged()
        .does(Act::Undo { doing: Weight::Changes })
        .does(Act::Undo { doing: Weight::Destroys }),
    Cancel => Spec::new("cancel", "Ask a running change to stop after the area it is on")
        .json()
        .does(Act::StopChange),
    Quotas => Spec::new("quotas", "Show how the disk counts the room snapshots take, and switch it on or off")
        .json()
        .logged()
        .does(Act::RoomCounting),
    Setup => Spec::new("setup", "Protect what has no snapshots yet, explaining each step")
        .json()
        .logged()
        .dry_run()
        .does(Act::Protect),
    Doctor => Spec::new("doctor", "Check that snapper works, and install it if it is missing")
        .json()
        .logged()
        .does(Act::InstallSnapper),
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

pub(super) const JSON_COMMANDS: [Command; how_many(Scope::Json)] = commands_that(Scope::Json);
pub(super) const LOGGED_COMMANDS: [Command; how_many(Scope::Logged)] = commands_that(Scope::Logged);
pub(super) const DRY_RUN_COMMANDS: [Command; how_many(Scope::DryRun)] =
    commands_that(Scope::DryRun);
pub(super) const YES_COMMANDS: [Command; how_many(Scope::Yes)] = commands_that(Scope::Yes);
pub(super) const DESTROY_COMMANDS: [Command; how_many(Scope::Destroy)] =
    commands_that(Scope::Destroy);
