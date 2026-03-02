# AGENTS.md

This file describes the project structure and AI agent workflow for the `astchunk` crate.

## Project Overview

`astchunk` is a Rust library that implements AST-based code chunking, reproducing the algorithm from the [cAST paper](https://arxiv.org/abs/2506.15655). The source lives in `src/lib.rs`.

## Repository Layout

```
Cargo.toml      # Crate manifest (metadata, lints, dependencies)
justfile        # Task runner recipes
src/
  lib.rs        # Library entry point and all implementation
```

## Development Commands

All day-to-day tasks are driven by [`just`](https://github.com/casey/just):

| Command       | Description                                      |
|---------------|--------------------------------------------------|
| `just dev`    | Run the full local development cycle (fmt → lint → test) |
| `just fmt`    | Format code with `cargo fmt`                     |
| `just lint`   | Lint with `cargo clippy --all-features --all-targets` |
| `just test`   | Run tests with `cargo test --all-features`       |
| `just doc`    | Build and open documentation                     |
| `just ci`     | Strict CI check (fmt --check, clippy -D warnings, test) |

**Always run `just dev` before committing** to ensure the code is formatted, lint-clean, and all tests pass.

## Lint Policy

The crate enforces strict lints via `Cargo.toml`:

- `unsafe_code = "forbid"` — no unsafe Rust allowed.
- `clippy::all`, `clippy::pedantic`, `clippy::cargo` — all denied at warning level during normal development and as hard errors in CI.

Any new code must pass these checks without `#[allow(...)]` suppressions unless there is a well-justified reason.
