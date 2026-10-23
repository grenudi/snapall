# Glossary

The words snapall uses, and what each one means here. The code, the CLI, the docs and the tests use these words and no others: the canon's *ubiquitous language* ([domain-driven design](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/architectures.md#domain-driven-design)). If a word is missing or means something else in code, fix one of them.

## Snapshots (the core)

| Term | Meaning |
|---|---|
| **config** | A named snapper configuration (`root`, `home`): one subvolume snapper snapshots. A `ConfigName` is a validated name. snapall never invents configs; it uses the ones snapper reports. |
| **snapshot** | One saved state of a config, with a number, a kind, a date and a description. |
| **snapshot id** | The number snapper gives a snapshot. Never 0: 0 is the *current system*, which is not a snapshot and cannot be deleted. |
| **current system** | Snapper's pseudo-snapshot 0: the live state. Never listed, never deletable. |
| **kind** | `single`, `pre` or `post`. A `post` always knows its `pre`. |
| **description** | The text stored with a snapshot: non-empty, one line, at most 200 characters. |
| **cleanup algorithm** | Snapper's rule for pruning old snapshots. snapall's snapshots use `number`, so snapper's own timer prunes them. |
| **plan** | The set of configs a `create` will snapshot: non-empty, sorted, unique, all of them real. Built only by `plan()`. |
| **target** | One config in a plan. |
| **report** | One outcome per target. One failing target never stops the others. |

## Ports and adapters

| Term | Meaning |
|---|---|
| **port** | An interface the core owns: `SnapshotBackend` (what snapall needs from a snapshot system) and `Observer` (where reports go). |
| **backend** | Something that implements the snapshot port: the snapper CLI, or the in-memory one used in tests. |
| **driving adapter** | Something that *calls* the core: the CLI and the TUI. |
| **driven adapter** | Something the core *calls*: the snapper adapter and the observability adapter. |
| **contract** (backend) | The behaviour every backend must have (`backend_contract`), run against the in-memory backend and against real snapper. |
| **snapperd / D-Bus** | How snapper normally answers. `--no-dbus` makes snapall talk to libsnapper directly (root only), for containers and CI. |

## Observability

| Term | Meaning |
|---|---|
| **event** | A fact about something snapall did, with its result and duration. |
| **observer** | Whatever receives events. It must never make the observed operation fail. |
| **audit trail** | The append-only, private (0600) JSON Lines file written with `--log-file`. |

## The machine (doctor)

| Term | Meaning |
|---|---|
| **probe** | The facts about this machine that `doctor` gathered: OS, snapper's version, whether this is root, the configs. Read by the shell, judged by pure functions. |
| **family** | How a distribution installs software (Debian, Fedora, Arch, ...), decided from `ID` and `ID_LIKE`. |
| **install plan** | Steps snapall may run, or a manual explanation when running a command would be wrong. |
| **approved** | Steps the user agreed to. Only approved steps can run. |

## The invocation (CLI)

| Term | Meaning |
|---|---|
| **contract** (invocation) | The declared surface of the command line: every flag, argument and variable, and which command each belongs to. One declaration, `contract.rs`, published as `snapall.argenv.json`. |
| **command** | `create`, `list`, `snapshots`, `delete`, `doctor`, `tui` or `contract`. |
| **scope** | The commands an input belongs to. A flag given to a command outside its scope is an error, never ignored. |
| **shorthand** | `snapall "text"`, meaning `snapall create "text"`. |
| **door** | A way a value can arrive: the flag or the `SNAPALL_*` variable. One setting, two doors; the flag wins. |
