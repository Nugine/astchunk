# AGENTS.md

This file provides project context for AI coding agents working on the `astchunk` crate.

## Project Overview

`astchunk` is a Rust library and CLI tool that implements AST-based code chunking, reproducing the algorithm from the [cAST paper](https://arxiv.org/abs/2506.15655). It splits source code into semantically meaningful chunks by leveraging tree-sitter AST analysis, suitable for code RAG (Retrieval-Augmented Generation) pipelines.

Supported languages: Python, Java, C++, C#, TypeScript, Rust.

## Repository Layout

```
Cargo.toml          # Crate manifest (metadata, lints, dependencies, features)
justfile            # Task runner recipes (dev, fmt, lint, test, ci, doc, install)
AGENTS.md           # This file — AI agent context
README.md           # User-facing documentation with algorithm description
src/
  lib.rs            # Library entry point, re-exports public API
  main.rs           # CLI binary (requires `cli` feature)
  builder.rs        # AstChunkBuilder — core chunking algorithm and pipeline
  chunk.rs          # AstChunk struct, code rebuilding, ancestor extraction
  metadata.rs       # MetadataTemplate, CodeWindow, RepoMetadata, chunk metadata formatting
  lang.rs           # Language enum, tree-sitter bindings, file extension detection
  node.rs           # AstNode wrapper with size and ancestor info
  nws.rs            # Non-whitespace character counting (cumulative sum + direct)
  byte_range.rs     # ByteRange helper type
tests/
  source_code.txt   # Python reference file for cross-validation tests
```

## Public API

The library exposes a builder-pattern API:

```rust
use astchunk::{AstChunkBuilder, Language, MetadataTemplate, RepoMetadata};

let chunks = AstChunkBuilder::new(Language::Python)
    .max_chunk_size(1500)
    .chunk_overlap(2)
    .chunk_expansion(true)
    .template(MetadataTemplate::Default)
    .repo_metadata(RepoMetadata { filepath: "main.py".into(), ..Default::default() })
    .chunkify(code);
```

Exported types: `AstChunkBuilder`, `Language`, `MetadataTemplate`, `CodeWindow`, `RepoMetadata`.

## Feature Flags

| Feature | Description |
|---------|-------------|
| `cli`   | Enables the CLI binary with clap, comfy-table, rayon, mimalloc, etc. |

## Development Commands

All tasks are driven by [`just`](https://github.com/casey/just):

| Command        | Description                                                |
|----------------|------------------------------------------------------------|
| `just dev`     | Full local cycle: fmt → lint → test                        |
| `just fmt`     | Format code with `cargo fmt`                               |
| `just lint`    | Lint with `cargo clippy --all-features --all-targets`      |
| `just test`    | Run tests with `cargo test --all-features`                 |
| `just doc`     | Build and open documentation                               |
| `just ci`      | Strict CI: fmt --check, clippy -D warnings, test           |
| `just install` | Install the CLI binary locally                             |

**Always run `just dev` before committing.**

## Lint Policy

Enforced via `Cargo.toml`:

- `unsafe_code = "forbid"` — no unsafe Rust.
- `clippy::all`, `clippy::pedantic`, `clippy::cargo` — all denied.
- No `#[allow(dead_code)]` — remove unused code instead of suppressing warnings.

Any new code must pass `just ci` with zero errors and zero warnings.

## Testing

- 25 unit tests across modules (cross-validation against Python reference output).
- 2 doctests (lib.rs module doc + `AstChunkBuilder` struct doc).
- Reference data: `tests/source_code.txt` with expected chunk sizes/line counts from the Python implementation.

## Architecture Notes

1. **Parsing**: tree-sitter parses source → AST.
2. **Window assignment**: Greedy recursive algorithm assigns AST nodes to windows respecting `max_chunk_size` (NWS count).
3. **Merging**: Adjacent sibling windows are merged when combined size fits.
4. **Overlapping** (optional): Nodes from adjacent windows are prepended/appended.
5. **Code rebuilding**: Source text is reconstructed from node byte ranges, preserving indentation.
6. **Chunk expansion** (optional): Ancestry context header is prepended.
7. **Metadata**: Each chunk is wrapped in a `CodeWindow` with configurable metadata templates.

The CLI binary (`main.rs`) provides table/JSON/brief output modes with parallel file processing via rayon.
