//! Driving adapter: the command line. Parses untrusted text into domain types once.

use clap::{Args, Parser, Subcommand};
use snapall_core::{ConfigName, Description, ParseError};

#[derive(Debug, Parser)]
#[command(
    name = "snapall",
    version,
    about = "Snapshot every snapper config in one command",
    long_about = "Snapshot every snapper config in one command.\n\n\
                  With no subcommand, `snapall` behaves like `snapall create`."
)]
pub struct Cli {
    /// Talk to libsnapper directly instead of through snapperd (root only; for containers and CI)
    #[arg(long, global = true)]
    pub no_dbus: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Take one snapshot of every snapper config (the default)
    Create(CreateArgs),
    /// List the snapper configs snapall can see
    List,
}

#[derive(Debug, Args, Default)]
pub struct CreateArgs {
    /// Description stored with each snapshot [default: snapall]
    pub description: Option<String>,

    /// Only snapshot these configs (comma-separated or repeated) [default: all]
    #[arg(long, value_delimiter = ',')]
    pub only: Vec<String>,

    /// Show what would be snapshotted without creating anything
    #[arg(long)]
    pub dry_run: bool,
}

/// A `create` request after parsing: every field is a validated domain value.
#[derive(Debug)]
pub struct CreateRequest {
    pub only: Option<Vec<ConfigName>>,
    pub description: Description,
    pub dry_run: bool,
}

impl CreateArgs {
    pub fn parse(self) -> Result<CreateRequest, ParseError> {
        let only = if self.only.is_empty() {
            None
        } else {
            Some(
                self.only
                    .iter()
                    .map(|name| ConfigName::parse(name))
                    .collect::<Result<Vec<_>, _>>()?,
            )
        };
        let description = match self.description {
            Some(text) => Description::parse(&text)?,
            None => Description::default(),
        };
        Ok(CreateRequest {
            only,
            description,
            dry_run: self.dry_run,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Cli {
        Cli::try_parse_from(std::iter::once("snapall").chain(args.iter().copied())).unwrap()
    }

    #[test]
    fn no_subcommand_is_allowed() {
        assert!(parse(&[]).command.is_none());
    }

    #[test]
    fn only_accepts_commas_and_repetition() {
        let Some(Command::Create(args)) =
            parse(&["create", "--only", "root,home", "--only", "data"]).command
        else {
            panic!("expected create")
        };
        assert_eq!(args.only, ["root", "home", "data"]);
    }

    #[test]
    fn bad_input_is_rejected_at_the_boundary() {
        let bad_config = CreateArgs {
            only: vec!["--evil".into()],
            ..CreateArgs::default()
        };
        assert!(bad_config.parse().is_err());
        let bad_description = CreateArgs {
            description: Some("   ".into()),
            ..CreateArgs::default()
        };
        assert!(bad_description.parse().is_err());
    }

    #[test]
    fn defaults_are_all_configs_and_the_default_description() {
        let request = CreateArgs::default().parse().unwrap();
        assert!(request.only.is_none());
        assert_eq!(request.description.as_str(), "snapall");
        assert!(!request.dry_run);
    }
}
