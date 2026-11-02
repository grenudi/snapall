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
| 1 | The openSUSE container job's package install (`zypper`, in the step called `A user's tools`, which is ours and not the runner's) flaked three times. It now retries three times; judge by the next releases, and if it still flakes try the image from `registry.opensuse.org` instead of Docker Hub. First release with the retry (0.28.0): 15 jobs ran the step and none needed a retry, so the hypothesis is still untested | open |
| 2 | Record each change's weight in the journal (asked at: changed, or lost for good), so the Activity tab can say it | open |
| 3 | Nine plurals are still spelled by hand in five files (the debt list `hand_spelled_plurals`): the shapes `counted` does not fit, such as a plural with no number ("the disks holding"), verb agreement, and the unit helper that is itself the general function. Say them through a second small helper, or leave each with its reason | open |
| 4 | The group editor's `Vec<(AreaName, bool)>` is a pair whose second half is unnamed (whether the area is in the group): a small noun | open |
| 5 | The core hard-codes a Linux path to a snapper plugin (`/usr/lib/snapper/plugins/10-limine-snapper-sync`) and asks the generic `file_exists`; the machine port should be asked whether the boot menu plugin is there, and the path belongs to the adapter (`boot_menu.rs` already knows it) | open |

Decided, so not open: `setup` is a conversation that explains and then asks about each part, so it is the one change that begins without a single leave (`begin_asking_along`, kept to one place by a test). Asking once up front would make the wizard worse, and it already takes its facts and its way of talking as inputs, so every branch is tested as a conversation with a scripted person. Revisit only if each of its changes should be journaled separately.

Decided as well: the files between 250 and 299 lines were read one by one instead of split to satisfy a number. Four held two languages and were split (the journal's events and its history; the operating system and the install plan; the mount table, what is covered and what a place is worth; the explanations, the cleanup words and what a person is told about an area). The ten that remain are one language each near its natural size (the two backends, the Health rows, the window's messages, the Snapshots tab's drawing, the setup conversation, the job catalogue, the machine port, the boot question, the contract's commands) and stay.

Decided too: handlers speak as they go, and do not return one value at the end. Several say something and then read an answer (a question, a list of what a rollback will run), so speaking and listening interleave, and a story that returned its words would have to give that up. What is kept instead is that there is one place that prints (`output.rs`, and the asker for what it asks): a test allows no print anywhere else, and the debt of fifteen was paid.

The guards' own debt list holds one entry: `contract/inputs.rs` (514 lines), the table of every flag and argument of the command line. It is one language and reads as a table, so it stays unless a seam by purpose shows (the shared flags against each command's own).

The tests that write a script and run it at once (the binary's) were thought not to hit the `text file busy` race the background runner's test once had, on the strength of 740 clean local runs. That was wrong: it appeared once on main's CI (a slower two-core runner) as `could not run snapper: Text file busy`. Absence in many runs on a fast machine is weak evidence against a rare, timing-dependent race. The binary's tests now start every process and write every script under one lock, so a fork can never overlap a script being written. A stress run with sixteen threads and four CPU burners showed the original fails 120 of 120 (not from that error but from the same cause family: the restore tests take the machine's real one-change-at-a-time lock and refuse each other) and the fixed version 0 of 120. The adapter's script tests are not exposed: they use an in-memory runner and never execute a script.

## Upstream: argenv
| # | Item | Status |
|---|---|---|
| C | `Type::List` needs a closed `allowed` set, so a runtime-determined list cannot be declared (argenv#25). Worked around: `--only` is a string, pinned by a test | open upstream |
| F | Migrate argenv to schemars 1.x (argenv#28); no effect on snapall | open upstream |

## Dependencies
| # | Item | Status |
|---|---|---|
| X | Move to ratatui 0.30 when the MSRV moves to 1.88, and drop the `instability` pin | open |
