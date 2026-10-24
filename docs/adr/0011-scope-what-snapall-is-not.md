# 11. Scope: what snapall deliberately is not

Status: accepted.

## Context
A small tool grows by accretion: each addition is individually reasonable and together they become
the thing nobody can maintain. snapall's purpose is one sentence: *snapshot every snapper config in
one command, and browse, create and delete snapshots in a terminal UI.* Everything else is judged
against it. These are the proposals that were considered and declined, so they are not re-proposed
by someone who has not seen the reasoning.

## Decisions
- **No automatic snapshots around package upgrades.** snap-pac (Arch), the snapper plugins for
  zypper and dnf, and apt-btrfs-snapshot do this, are maintained by the people who maintain the
  package managers, and run in the right place (a hook inside the transaction). A fifth
  implementation inside snapall would be worse at it. snapall is the manual, all-config command you
  run *before the change you are about to make* (a theme, a kernel, a config).
- **Not a backup tool.** Snapshots live on the same disk (the README says so in one line).
- **No log sinks beyond stderr and a file** (journald, syslog, OpenTelemetry). The `Observer` port
  exists exactly so that one can be added in a day by whoever needs it; building one without a user
  is speculation.
- **No Btrfs-specific diagnostics.** snapall drives *snapper*, which supports more than Btrfs. `doctor`
  checks what snapall needs (snapper works, privileges, configs), not the filesystem underneath.
- **No branch protection on `main`.** The repository has one maintainer and release automation
  (release-plz) whose pull requests are created with the workflow token, which GitHub does not run
  CI for; a rule requiring checks would block the automation and gain nothing. The protection that
  matters is after the fact and automatic: CI on every push, and `release-verify.yml` on every
  release.
- **No installing packages on systems CI does not install on.** See `doctor` and AGENTS.md: a
  command runs as root only where a container has run it.

## Consequences
The backlog holds what is wanted and not yet done; this file holds what is *not* wanted and why. If a
declined item becomes wanted, this ADR is superseded by a new one, not edited.
