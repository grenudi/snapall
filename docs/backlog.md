# Backlog

Every known gap lives here, not in a prose TODO.

## Product
| # | Item | Status |
|---|---|---|
| 1 | Publish to the AUR (`snapall`, built from the release PKGBUILD). Needs the maintainer's AUR account and an SSH key; the release already attaches a PKGBUILD with the real version and checksum | open |
| 2 | Signed apt and rpm repositories (or the openSUSE Build Service / COPR / a PPA) so that `apt install snapall` works. Needs a long-lived signing key and hosting, so only on demand | on demand |
| 3 | Submit the package to nixpkgs (the flake and its `flake.lock` are in the repository; update the lock with `nix flake update`) | open |
| 4 | TUI: a termination signal returns at once, but leaving the worker's thread scope joins it, so a snapper call that hangs forever would delay the exit (systemd would SIGKILL after its timeout). Fix: a detached worker (needs an owned, `'static` backend) or exiting the process after the terminal is restored | open |
| 5 | TUI: browse the changed files of a snapshot (restore of a whole config and `snapall changes` exist; picking single paths in the TUI does not) | open |
| 6 | `doctor`: show the boot partition's size against limine-snapper-sync's 4 GiB recommendation (read-only; the commonest practical blocker) | open |
| 7 | Confirm snapper's allowed config-name characters; `AreaName` is currently stricter than necessary | open |

What is deliberately *not* here (package-manager hooks, more log sinks, Btrfs checks, branch protection) is in [ADR 11](adr/0011-scope-what-snapall-is-not.md).

## Structure
Where the code still does not say what snapall is for. Each iteration is a branch and a pull request, and none depends on anyone remembering the last; how one goes, and the tools, are in `AGENTS.md`. Progress is measured by `dev/structure-metrics.py` and by the cold-read test (`crates/snapall/tests/findability.rs`), and recorded in `docs/development-log.md`. The debts themselves are data in `crates/snapall/tests/ratchets.json`, which can only be lowered.

| # | Item | Status |
|---|---|---|
| 1 | Nine bare booleans remain in the commands, of a different kind from the ones that were named: states a function reports (`stopped`, `changed`, `ended`), a direction (`turn_on`, on or off) and facts about how the program was invoked (`hints`, `json`, `no_dbus` in `Context`). Name `turn_on` if a fourth place needs it | open |
| 2 | Three tests write a script and run it at once (`binary.rs` and the snapper adapter's tests): the same `text file busy` race the background runner's test had. Give them the same retry when one fails | open |
| 3 | The openSUSE container job's package install (`zypper`, in the step called `A user's tools`, which is ours and not the runner's) flaked three times. It now retries three times; judge by the next releases, and if it still flakes try the image from `registry.opensuse.org` instead of Docker Hub | open |
| 4 | Record each change's weight in the journal (asked at: changed, or lost for good), so the Activity tab can say it | open |

Decided, so not open: `setup` is a conversation that explains and then asks about each part, so it is the one change that begins without a single leave (`begin_asking_along`, kept to one place by a test). Asking once up front would make the wizard worse, and it already takes its facts and its way of talking as inputs, so every branch is tested as a conversation with a scripted person. Revisit only if each of its changes should be journaled separately.

Decided as well: the files between 250 and 299 lines were read one by one instead of split to satisfy a number. Four held two languages and were split (the journal's events and its history; the operating system and the install plan; the mount table, what is covered and what a place is worth; the explanations, the cleanup words and what a person is told about an area). The ten that remain are one language each near its natural size (the two backends, the Health rows, the window's messages, the Snapshots tab's drawing, the setup conversation, the job catalogue, the machine port, the boot question, the contract's commands) and stay.

Decided too: handlers speak as they go, and do not return one value at the end. Several say something and then read an answer (a question, a list of what a rollback will run), so speaking and listening interleave, and a story that returned its words would have to give that up. What is kept instead is that there is one place that prints (`output.rs`, and the asker for what it asks): a test allows no print anywhere else, and the debt of fifteen was paid.

The guards' own debt list holds one entry: `contract/inputs.rs` (514 lines), the table of every flag and argument of the command line. It is one language and reads as a table, so it stays unless a seam by purpose shows (the shared flags against each command's own).

## Upstream: argenv
| # | Item | Status |
|---|---|---|
| C | `Type::List` needs a closed `allowed` set, so a runtime-determined list cannot be declared (argenv#25). Worked around: `--only` is a string, pinned by a test | open upstream |
| F | Migrate argenv to schemars 1.x (argenv#28); no effect on snapall | open upstream |

## Dependencies
| # | Item | Status |
|---|---|---|
| X | Move to ratatui 0.30 when the MSRV moves to 1.88, and drop the `instability` pin | open |
