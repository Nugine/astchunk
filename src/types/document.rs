use bytestring::ByteString;

use crate::lang::Language;

/// Unique identifier for a document within a processing batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DocumentId(
    /// Numeric identifier.
    pub u32,
);

/// Source origin metadata.
#[derive(Debug, Clone, Default)]
pub struct Origin {
    /// Repository-relative logical path (forward-slash normalized).
    pub path: Option<ByteString>,
    /// Repository name or identifier.
    pub repo: Option<ByteString>,
    /// Revision (commit hash, tag, branch).
    pub revision: Option<ByteString>,
}

/// A source document to be chunked.
#[derive(Debug, Clone)]
pub struct Document {
    /// Unique identifier for this document.
    pub document_id: DocumentId,
    /// Programming language of the source.
    pub language: Language,
    /// Full source code.
    pub source: ByteString,
    /// Origin metadata.
    pub origin: Origin,
}
