//! Chunking traits and concrete implementations for producing AST chunks.

mod cast;

pub use cast::{CastChunker, CastChunkerOptions};

use crate::error::AstchunkError;
use crate::types::{AstChunk, Document};

/// Splits a [`Document`] into structural [`AstChunk`]s.
pub trait Chunker {
    /// Chunk a document into AST-level structural chunks.
    ///
    /// # Errors
    ///
    /// Returns an error if parsing or chunking fails.
    fn chunk(&self, document: &Document) -> Result<Vec<AstChunk>, AstchunkError>;
}
