//! # astchunk
//!
//! AST-based code chunking, implementing the algorithm from
//! the [cAST paper](https://arxiv.org/abs/2506.15655).
//!
//! ## Quick start
//!
//! A typical pipeline is:
//! [`Document`](types::Document) -> [`AstChunk`](types::AstChunk) ->
//! [`TextChunk`](types::TextChunk) -> [`JsonRecord`](output::JsonRecord).
//!
//! ```
//! use astchunk::chunker::{Chunker, CastChunker, CastChunkerOptions};
//! use astchunk::formatter::{CanonicalFormatter, Formatter};
//! use astchunk::output::JsonRecord;
//! use astchunk::types::{Document, DocumentId, Origin};
//! use astchunk::lang::Language;
//!
//! let source = "def hello():\n    print('hi')\n";
//! let document = Document {
//!     document_id: DocumentId(0),
//!     language: Language::Python,
//!     source: source.into(),
//!     origin: Origin::default(),
//! };
//! let chunker = CastChunker::new(CastChunkerOptions::default());
//! let ast_chunks = chunker.chunk(&document).unwrap();
//!
//! let formatter = CanonicalFormatter::default();
//! let text_chunks = formatter.format(&document, &ast_chunks).unwrap();
//!
//! let records = JsonRecord::build(&document, &ast_chunks, &text_chunks);
//!
//! assert!(!ast_chunks.is_empty());
//! assert_eq!(text_chunks.len(), ast_chunks.len());
//! assert_eq!(records.len(), text_chunks.len());
//! ```
//!
//! ## Modules
//!
//! - [`types`] — Data types: [`Document`](types::Document), [`AstChunk`](types::AstChunk), [`TextChunk`](types::TextChunk), etc.
//! - [`chunker`] — [`Chunker`](chunker::Chunker) trait and [`CastChunker`](chunker::CastChunker) implementation.
//! - [`formatter`] — [`Formatter`](formatter::Formatter) trait with [`CanonicalFormatter`](formatter::CanonicalFormatter) and [`ContextualFormatter`](formatter::ContextualFormatter).
//! - [`output`] — Output record types: [`JsonRecord`](output::JsonRecord), [`RepoEvalRecord`](output::RepoEvalRecord), [`SwebenchLiteRecord`](output::SwebenchLiteRecord).
//! - [`lang`] — [`Language`](lang::Language) enum and tree-sitter bindings.
//! - [`error`] — [`AstchunkError`](error::AstchunkError) type.
//!
//! ## Feature flags
//!
//! | Feature | Description |
//! |---------|-------------|
//! | `cli` | Build the command-line interface |

pub mod chunker;
pub mod error;
pub mod formatter;
pub(crate) mod internal;
pub mod lang;
pub mod output;
pub mod types;
