# AGENTS.md

This file provides project context for AI coding agents working on the `astchunk` crate.

## Project Overview

`astchunk` is a Rust library and CLI tool that implements AST-based code chunking, reproducing the algorithm from the [cAST paper](https://arxiv.org/abs/2506.15655). It splits source code into semantically meaningful chunks by leveraging tree-sitter AST analysis, suitable for code RAG (Retrieval-Augmented Generation) pipelines.

Supported languages: Python, Java, C++, C#, TypeScript, Rust.

## Repository Layout

```
Cargo.toml            # Crate manifest and lint policy
justfile              # Task runner recipes
AGENTS.md             # AI agent context
README.md             # User-facing documentation
.github/agents/       # Repo-local architect / developer / tester agent definitions
src/
  lib.rs              # Library entry point, module exports, crate-level docs
  main.rs             # CLI binary (requires `cli` feature)
  error.rs            # AstchunkError enum
  lang.rs             # Language enum and tree-sitter bindings
  output.rs           # Output record builders and metadata types
  chunker/
    mod.rs            # Chunker trait and CastChunker re-exports
    cast.rs           # CastChunker and CastChunkerOptions
  formatter/
    mod.rs            # Formatter trait and formatter re-exports
    canonical.rs      # CanonicalFormatter
    contextual.rs     # ContextualFormatter
  internal/
    mod.rs            # Internal module re-exports
    byte_range.rs     # ByteRange helper type
    materialize.rs    # Code rebuild functions
    node.rs           # AstNode wrapper with size and ancestor info
    nws.rs            # Non-whitespace character counting
    partition.rs      # Parsing, window assignment, merge, overlap logic
  types/
    mod.rs            # Public type re-exports
    chunk.rs          # AstChunk, TextChunk, metrics, scopes, line ranges
    document.rs       # Document, DocumentId, Origin
tests/
  source_code.txt     # Python reference file for cross-validation tests
```

## Public API

The library exposes module-based APIs rather than crate-root re-exports:

```rust
use astchunk::chunker::{CastChunker, CastChunkerOptions, Chunker};
use astchunk::formatter::{CanonicalFormatter, Formatter};
use astchunk::lang::Language;
use astchunk::output::JsonRecord;
use astchunk::types::{Document, DocumentId, Origin};

let document = Document {
    document_id: DocumentId(0),
    language: Language::Python,
    source: code.into(),
    origin: Origin::default(),
};
let chunker = CastChunker::new(CastChunkerOptions::default());
let ast_chunks = chunker.chunk(&document).unwrap();
let formatter = CanonicalFormatter::default();
let text_chunks = formatter.format(&document, &ast_chunks).unwrap();
let records = JsonRecord::build(&document, &ast_chunks, &text_chunks);
```

Public modules: `chunker`, `formatter`, `lang`, `output`, `types`, `error`.

Key public items: `CastChunker`, `CastChunkerOptions`, `Chunker`, `CanonicalFormatter`, `ContextualFormatter`, `Formatter`, `Language`, `JsonRecord`, `RepoEvalRecord`, `SwebenchLiteRecord`, `Document`, `DocumentId`, `Origin`, `AstChunk`, `ChunkId`, `ChunkMetrics`, `TextChunk`, `TextMetrics`, `TextMode`, `ScopeFrame`, `ScopeKind`, `LineIndexRange`, `LineNumberRange`, `AstchunkError`.

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

Repository convention:

- Prefer removing unused code over adding `#[allow(dead_code)]`.

Any new code must pass `just ci` with zero errors and zero warnings.

## Testing

- 39 unit tests across modules + 1 doctest.
- Reference data: `tests/source_code.txt` with expected chunk sizes and line counts from the Python implementation.

## Architecture Notes

1. **Parsing**: `tree-sitter` parses source into an AST.
2. **Chunking** (`CastChunker`): window assignment → sibling-window merge → optional overlap → `Vec<AstChunk>`.
3. **Formatting** (`CanonicalFormatter` / `ContextualFormatter`): `Vec<AstChunk>` → `Vec<TextChunk>`.
4. **Output** (`JsonRecord::build`, `RepoEvalRecord::build`, `SwebenchLiteRecord::build`): document + chunks → serializable downstream records.

The CLI binary (`main.rs`) provides table, JSON, and brief output modes with parallel file processing via rayon. `--template repo-eval` is supported via `--repo`; stdin exports that require a logical source path also use `--stdin-path`.
