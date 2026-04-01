//! Core data types shared across the chunking, formatting, and output pipeline.

mod chunk;
mod document;

pub use bytestring::ByteString;
pub use chunk::{
    AstChunk, ByteRange, ChunkId, ChunkMetrics, LineIndexRange, LineNumberRange, ScopeFrame,
    ScopeKind, TextChunk, TextMetrics, TextMode,
};
pub use document::{Document, DocumentId, Origin};
