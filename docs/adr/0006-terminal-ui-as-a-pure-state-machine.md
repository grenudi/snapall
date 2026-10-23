# 6. The terminal UI is a pure state machine, on ratatui 0.29

Status: accepted

## Context
The canon prefers a functional core and an imperative shell, and tests at the seams. A TUI is usually the opposite: state, drawing and I/O tangled in one event loop that can only be tested by running a terminal.

## Decision
`snapall-tui` separates four things:

- `model`: `update(state, msg) -> Vec<Effect>`, pure, with its own `Key` type so it does not even know crossterm.
- `view`: `render(state, frame)`, drawing only.
- `session`: runs each effect against a `SnapshotBackend` (reusing the core's use cases) and feeds exactly one message back per effect.
- `term`: the terminal, key translation and the event loop; the only untested-by-unit part, covered by a pseudo-terminal end-to-end test against real snapper.

Fetched data is `Load<T>` (not loaded / loaded / failed), never an empty list standing in for three states. The interface uses ratatui **0.29**: 0.30 needs Rust 1.86 to 1.88 and snapall keeps MSRV 1.85 for distro packagers. `Cargo.lock` also pins `instability` (a ratatui dependency) to 0.3.10, because 0.3.11 and later need Rust 1.88 although ratatui only asks for `^0.3`. Dependabot is told to leave ratatui's minor and major versions alone.

## Considered
- An async event loop with a worker thread: snapper calls block the UI for their duration (typically well under a second). Deferred; in the backlog.
- Putting the state machine in `snapall-core`: rejected, the core must not know about UI concepts.

## Consequences
Every key sequence is a plain test. The cost is a message type per effect result; accepted.
