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

## Update: work runs off the UI thread

The first version ran each snapper call inline, so a slow one (a delete on a large subvolume, a busy disk) froze the screen until it returned. It now does not, and the state machine is still pure:

- `Executor` (in `session.rs`) performs one effect against the backend and answers with one message. It is the only place that touches the backend, and it is what a worker thread runs.
- The model counts effects in flight (`pending`: every effect `update` returns adds one, every result message settles one) and takes two new messages: `Start` (load the configs) and `Tick` (animate the indicator). While `pending > 0` the keys that would start more work (`c`, `a`, `d`, `r`) are refused with a reason; moving, reading and help stay available.
- `term.rs` runs one worker thread inside `std::thread::scope`, sends it effects over a channel, polls results every 100 ms, and redraws. Quitting while work is in flight waits for it (the header says "finishing") instead of abandoning a half-finished delete. Signals do not wait.
- `Session` still runs the same executor *synchronously*, so every key sequence in the tests is deterministic, and `pending` is back to zero after each message.
- The backend must be `Sync` to be shared with the worker, so the observer in the binary is `Send + Sync`; nothing else changed.

Proved in the pseudo-terminal test with a shim that makes every snapper create and delete take two seconds: the header shows "working", a key pressed during the wait is answered within a fraction of a second, a second `c` is refused with "busy", and `q` during the wait exits cleanly after the operation completes.

## Considered
- An async event loop with a worker thread: snapper calls block the UI for their duration (typically well under a second). Deferred; in the backlog.
- Putting the state machine in `snapall-core`: rejected, the core must not know about UI concepts.

## Consequences
Every key sequence is a plain test. The cost is a message type per effect result; accepted.
