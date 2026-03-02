//! # astchunk
//!
//! AST-based code chunking, implementing the algorithm from
//! the [cAST paper](https://arxiv.org/abs/2506.15655).
//!
//! The main entry point is [`AstChunkBuilder`], which splits source code
//! into semantically meaningful chunks based on tree-sitter AST analysis.
//!
//! ## Quick start
//!
//! ```rust
//! use astchunk::{AstChunkBuilder, Language};
//!
//! let code = "def hello():\n    print('hello')\n\ndef world():\n    print('world')\n";
//! let chunks = AstChunkBuilder::new(Language::Python)
//!     .max_chunk_size(50)
//!     .chunkify(code);
//! assert!(!chunks.is_empty());
//! ```
//!
//! ## Feature flags
//!
//! | Feature | Description |
//! |---------|-------------|
//! | `cli` | Build the command-line interface |

mod builder;
mod byte_range;
mod chunk;
mod lang;
mod metadata;
mod node;
mod nws;

pub use builder::AstChunkBuilder;
pub use lang::Language;
pub use metadata::{CodeWindow, MetadataTemplate, RepoMetadata};
