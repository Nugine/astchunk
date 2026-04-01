use bytestring::ByteString;

use crate::internal::byte_range;

pub use byte_range::ByteRange;

/// Unique identifier for a chunk within a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkId(
    /// Sequential numeric identifier.
    pub u32,
);

/// 0-based line range, half-open: `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineIndexRange {
    /// Inclusive start line (0-based).
    pub start: u32,
    /// Exclusive end line (0-based).
    pub end: u32,
}

/// 1-based line range, closed: `[start, end]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineNumberRange {
    /// Inclusive start line (1-based).
    pub start: u32,
    /// Inclusive end line (1-based).
    pub end: u32,
}

impl LineIndexRange {
    /// Convert to 1-based closed line number range.
    #[must_use]
    pub fn to_line_number_range(self) -> LineNumberRange {
        LineNumberRange {
            start: self.start + 1,
            end: self.end,
        }
    }
}

/// Structural scope information for a chunk.
#[derive(Debug, Clone)]
pub struct ScopeFrame {
    /// The kind of scope (class, function, etc.).
    pub kind: ScopeKind,
    /// Display text for the scope (e.g. first line of definition).
    pub display: ByteString,
}

/// Kind of scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    /// Module-level scope.
    Module,
    /// Namespace scope (C++).
    Namespace,
    /// Class definition.
    Class,
    /// Interface definition.
    Interface,
    /// Trait definition (Rust).
    Trait,
    /// Impl block (Rust).
    Impl,
    /// Free function.
    Function,
    /// Method inside a class or impl.
    Method,
    /// Constructor method.
    Constructor,
}

/// Structural metrics for an AST chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkMetrics {
    /// Non-whitespace character count.
    pub nws_size: u32,
    /// Number of AST nodes.
    pub node_count: u32,
    /// Number of byte-range segments.
    pub segment_count: u32,
}

/// A structural AST chunk — the output of the chunking stage.
#[derive(Debug, Clone)]
pub struct AstChunk {
    /// Chunk identifier (sequential within a document).
    pub chunk_id: ChunkId,
    /// Parent document identifier.
    pub document_id: DocumentId,
    /// Byte ranges of source segments comprising this chunk.
    pub segments: Vec<ByteRange>,
    /// Minimum bounding byte range covering all segments.
    pub envelope: ByteRange,
    /// 0-based line range in the original source.
    pub line_index_range: LineIndexRange,
    /// Scope ancestry chain (outermost first).
    pub scopes: Vec<ScopeFrame>,
    /// Structural metrics.
    pub metrics: ChunkMetrics,
}

/// Text formatting mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextMode {
    /// Plain rebuilt text without context header.
    Canonical,
    /// Text with scope/path context header prepended.
    Contextual,
}

/// Text metrics for a formatted text chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextMetrics {
    /// Number of lines in the final content.
    pub content_line_count: u32,
    /// Non-whitespace character count of the final content.
    pub content_nws_size: u32,
}

/// A formatted text chunk — the output of the formatting stage.
#[derive(Debug, Clone)]
pub struct TextChunk {
    /// Identifier of the upstream AST chunk.
    pub ast_chunk_id: ChunkId,
    /// Final text content (may include context header).
    pub content: ByteString,
    /// Text formatting mode used.
    pub text_mode: TextMode,
    /// 0-based line range in the original source (excludes context header lines).
    pub source_line_index_range: LineIndexRange,
    /// Text metrics of the final content.
    pub metrics: TextMetrics,
}

use super::document::DocumentId;
