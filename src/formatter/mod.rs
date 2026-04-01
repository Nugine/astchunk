//! Text formatting traits and implementations built on top of AST chunks.

mod canonical;
mod contextual;

pub use canonical::CanonicalFormatter;
pub use contextual::ContextualFormatter;

use crate::error::AstchunkError;
use crate::types::{AstChunk, Document, TextChunk};

/// Converts [`AstChunk`]s into human-readable [`TextChunk`]s.
pub trait Formatter {
    /// Format AST chunks into text chunks.
    ///
    /// # Errors
    ///
    /// Returns an error if formatting fails.
    fn format(
        &self,
        document: &Document,
        chunks: &[AstChunk],
    ) -> Result<Vec<TextChunk>, AstchunkError>;
}
