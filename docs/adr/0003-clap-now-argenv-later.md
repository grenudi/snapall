# 3. Argument parsing: clap now, argenv later

Status: accepted, revisit (see backlog)

## Context
The intent is to use `argenv` (a typed, machine-readable contract for a program's argv and env) as the invocation layer. Its API was not yet studied for this project.

## Decision
Use `clap` in the binary crate, behind one module (`cli.rs`) that converts raw text into domain types. Nothing else in the workspace knows the parser, and an architecture test keeps it that way, so replacing it touches one file.

## Consequences
Adopting `argenv` is a contained change, tracked in `docs/backlog.md`.
