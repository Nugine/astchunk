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
//! use astchunk::{AstChunkBuilder, ChunkOptions, Language, MetadataTemplate};
//!
//! let code = "def hello():\n    print('hello')\n\ndef world():\n    print('world')\n";
//! let builder = AstChunkBuilder::new(50, Language::Python);
//! let chunks = builder.chunkify(code, MetadataTemplate::Default, &ChunkOptions::default());
//! assert!(!chunks.is_empty());
//! ```
//!
//! ## Feature flags
//!
//! | Feature | Description |
//! |---------|-------------|
//! | `python` (default) | Python language support |
//! | `java` | Java language support |
//! | `csharp` | C# language support |
//! | `typescript` | TypeScript / TSX support |
//! | `all-languages` | Enable all language features |
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
pub use metadata::{ChunkOptions, CodeWindow, MetadataTemplate};
