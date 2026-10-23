# 9. Observability as a port, and graceful shutdown

Status: accepted. Replaces the "deferred" note on observability in ARCHITECTURE.md.

## Context
The canon lists *observability as a port* and *twelve-factor discipline* as practices. snapall was first built without either: it printed results and nothing else, and a termination signal killed the TUI with the terminal left in raw mode. That was fine for a tool that only read state. It is not fine for a tool that creates and deletes snapshots as root, often unattended from cron, and installs packages. When something is deleted, someone will ask who, when and whether it worked.

## Decision
**Observability is a port.** The core defines `Event` (pure data: what, the result, how long) and the `Observer` trait. It reads no clock and writes nothing. The `snapall-observe` crate is a decorator: `Observed<B, O>` wraps any `SnapshotBackend`, times each call and reports it, so no use case changes and the wrapped backend still passes `backend_contract` (a test checks it).

- **Streams, not files.** `--log text|json` writes to stderr, an event stream the environment routes (a pipe, journald, a container runtime). Off by default: a command-line tool is quiet unless asked.
- **An audit trail on request.** `--log-file PATH` appends JSON Lines, created with mode 0600 because it records privileged actions, and implies `--log json`.
- **The TUI logs only to a file**, because stderr would draw over the screen. The CLI refuses `tui --log` without `--log-file`.
- **A broken sink never breaks the operation.** A line that cannot be written is dropped. Losing a log line is better than failing a snapshot.
- **A hostile string cannot forge a log line.** Newlines in descriptions and error text are escaped (JSON escaping; `{:?}` quoting in text), and a test feeds them in.
- **`doctor --install` is observed too**: each package-manager step is an event.

**Typestate for the install gate.** `Approved` is constructible only by `approve`, which shows the steps and obtains consent (`--yes` or an answer). `execute` takes an `Approved`. Running unapproved commands as root is not a state the program can reach, and no boolean can be forgotten.

**Graceful shutdown.** SIGINT, SIGTERM and SIGHUP are registered before the terminal is touched; the event loop polls with a 100 ms tick, restores the terminal and returns `Exit::Signal(n)`; the process exits `128 + n`. A pseudo-terminal test sends each signal to the real TUI and checks the status and the restored terminal.

**Total functions are enforced.** Clippy denies `unwrap_used`, `expect_used` and `panic` in every crate's production code (test code opts out). The first run found exactly one `expect` in the CLI, which was removed by making the normalisation step return the command it chose.

## Considered
- `tracing` or `log`: a global subscriber and macros everywhere, which is the opposite of a port. An adapter that implements `Observer` on top of `tracing` is possible later.
- Verbosity levels: nothing here needs them yet (YAGNI); events are either wanted or not.
- A built-in log rotation: that is the environment's job (logrotate, journald), not the program's.
- Reading the clock in the core: it would make the core impure. The adapter stamps time; tests inject a fixed clock.
- The `ctrlc` crate or a signal thread: `signal-hook`'s flag registration is enough, and it is already in the tree through crossterm, so no new copy of it ships.

## Consequences
A new `Event` variant must have a log-format test and a name in `docs/glossary.md`. A new operation goes through the observed backend or reports an event itself. Observers are synchronous and single-threaded; an asynchronous one would be a new adapter.
