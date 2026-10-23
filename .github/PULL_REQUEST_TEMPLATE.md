## What changed

## Checklist
- [ ] `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass locally
- [ ] The dependency rule still holds (`crates/snapall/tests/architecture.rs` passes); no I/O or parser dependency leaked into `snapall-core`
- [ ] New behaviour is covered at the seam it lives on (core with the in-memory backend, adapter with scripted commands, real snapper only for the contract)
- [ ] Anything decided or deferred is recorded in `docs/adr/` or `docs/backlog.md`
- [ ] The PR title is a Conventional Commit (it becomes the commit on `main`; release-plz reads it)
