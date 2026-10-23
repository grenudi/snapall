//! Composition root and imperative shell: wires the CLI adapter, the snapper adapter and
//! the core together, performs the I/O, and maps the outcome to an exit code.

mod cli;
mod render;

use std::error::Error;
use std::process::ExitCode;

use clap::Parser;
use snapall_core::{plan_snapshots, run_plan, SnapshotBackend};
use snapall_snapper::SnapperCli;

use crate::cli::{Cli, Command, CreateArgs};

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("snapall: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode, Box<dyn Error>> {
    let backend = SnapperCli::system().with_no_dbus(cli.no_dbus);
    match cli
        .command
        .unwrap_or_else(|| Command::Create(CreateArgs::default()))
    {
        Command::List => {
            print!("{}", render::configs(&backend.list_configs()?));
            Ok(ExitCode::SUCCESS)
        }
        Command::Create(args) => {
            let request = args.parse()?;
            let plan = plan_snapshots(&backend, request.only.as_deref())?;
            if request.dry_run {
                print!("{}", render::dry_run(&plan, &request.description));
                return Ok(ExitCode::SUCCESS);
            }
            let report = run_plan(&backend, &plan, &request.description);
            print!("{}", render::report(&report));
            Ok(if report.is_success() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
    }
}
