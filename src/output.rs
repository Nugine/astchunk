//! Output record types for serializing formatted chunks into downstream formats.

use bytestring::ByteString;

use crate::error::AstchunkError;
use crate::types::{AstChunk, Document, TextChunk};

/// JSON export record.
#[derive(Debug, Clone, serde::Serialize)]
pub struct JsonRecord {
    /// The chunk text content.
    pub content: ByteString,
    /// Associated metadata.
    pub metadata: JsonMetadata,
}

/// Metadata for JSON export.
#[derive(Debug, Clone, serde::Serialize)]
pub struct JsonMetadata {
    /// Repository-relative file path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<ByteString>,
    /// Repository name or identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo: Option<ByteString>,
    /// 1-based start line number in the original source.
    pub source_start_line_number: u32,
    /// 1-based end line number in the original source.
    pub source_end_line_number: u32,
    /// Number of lines in the final content.
    pub content_line_count: u32,
    /// Non-whitespace character count of the final content.
    pub content_nws_size: u32,
    /// Number of AST nodes in this chunk.
    pub node_count: u32,
}

impl JsonRecord {
    /// Build JSON records from a document's AST chunks and text chunks.
    ///
    /// # Panics
    ///
    /// Panics if `chunks` and `text_chunks` have different lengths.
    pub fn build(document: &Document, chunks: &[AstChunk], text_chunks: &[TextChunk]) -> Vec<Self> {
        assert_eq!(
            chunks.len(),
            text_chunks.len(),
            "chunks and text_chunks must have the same length"
        );

        chunks
            .iter()
            .zip(text_chunks)
            .map(|(ast_chunk, text_chunk)| {
                let line_numbers = text_chunk.source_line_index_range.to_line_number_range();
                Self {
                    content: text_chunk.content.clone(),
                    metadata: JsonMetadata {
                        path: document.origin.path.clone(),
                        repo: document.origin.repo.clone(),
                        source_start_line_number: line_numbers.start,
                        source_end_line_number: line_numbers.end,
                        content_line_count: text_chunk.metrics.content_line_count,
                        content_nws_size: text_chunk.metrics.content_nws_size,
                        node_count: ast_chunk.metrics.node_count,
                    },
                }
            })
            .collect()
    }
}

/// `RepoEval` export record.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RepoEvalRecord {
    /// The chunk text content.
    pub content: ByteString,
    /// Associated metadata.
    pub metadata: RepoEvalMetadata,
}

/// Metadata for `RepoEval` export.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RepoEvalMetadata {
    /// Forward-slash–split path components.
    pub fpath_tuple: Vec<ByteString>,
    /// Repository name or identifier.
    pub repo: ByteString,
    /// 1-based start line number in the original source.
    pub source_start_line_number: u32,
    /// 1-based end line number in the original source.
    pub source_end_line_number: u32,
    /// Number of lines in the final content.
    pub content_line_count: u32,
    /// Non-whitespace character count of the final content.
    pub content_nws_size: u32,
    /// Number of AST nodes in this chunk.
    pub node_count: u32,
}

impl RepoEvalRecord {
    /// Build `RepoEval` records from a document's AST chunks and text chunks.
    ///
    /// # Errors
    ///
    /// Returns `ExportRequirementMissing` if `origin.path` or `origin.repo` is missing.
    ///
    /// # Panics
    ///
    /// Panics if `chunks` and `text_chunks` have different lengths.
    pub fn build(
        document: &Document,
        chunks: &[AstChunk],
        text_chunks: &[TextChunk],
    ) -> Result<Vec<Self>, AstchunkError> {
        assert_eq!(
            chunks.len(),
            text_chunks.len(),
            "chunks and text_chunks must have the same length"
        );

        let path =
            document
                .origin
                .path
                .as_ref()
                .ok_or(AstchunkError::ExportRequirementMissing {
                    exporter: "RepoEvalRecord",
                    field: "origin.path",
                })?;
        let repo =
            document
                .origin
                .repo
                .as_ref()
                .ok_or(AstchunkError::ExportRequirementMissing {
                    exporter: "RepoEvalRecord",
                    field: "origin.repo",
                })?;

        let fpath_tuple: Vec<ByteString> = path.split('/').map(ByteString::from).collect();

        let records = chunks
            .iter()
            .zip(text_chunks)
            .map(|(ast_chunk, text_chunk)| {
                let line_numbers = text_chunk.source_line_index_range.to_line_number_range();
                Self {
                    content: text_chunk.content.clone(),
                    metadata: RepoEvalMetadata {
                        fpath_tuple: fpath_tuple.clone(),
                        repo: repo.clone(),
                        source_start_line_number: line_numbers.start,
                        source_end_line_number: line_numbers.end,
                        content_line_count: text_chunk.metrics.content_line_count,
                        content_nws_size: text_chunk.metrics.content_nws_size,
                        node_count: ast_chunk.metrics.node_count,
                    },
                }
            })
            .collect();

        Ok(records)
    }
}

/// SWE-bench Lite export record.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SwebenchLiteRecord {
    /// Unique identifier.
    #[serde(rename = "_id")]
    pub id: ByteString,
    /// Title (typically the file name).
    pub title: ByteString,
    /// Chunk text content.
    pub text: ByteString,
}

impl SwebenchLiteRecord {
    /// Build SWE-bench Lite records from a document's text chunks.
    ///
    /// # Errors
    ///
    /// Returns `ExportRequirementMissing` if `origin.path` is missing.
    pub fn build(
        document: &Document,
        text_chunks: &[TextChunk],
        instance_id: &str,
    ) -> Result<Vec<Self>, AstchunkError> {
        let path =
            document
                .origin
                .path
                .as_ref()
                .ok_or(AstchunkError::ExportRequirementMissing {
                    exporter: "SwebenchLiteRecord",
                    field: "origin.path",
                })?;

        let title: &str = path.rsplit('/').next().unwrap_or(path.as_ref());

        let records = text_chunks
            .iter()
            .map(|text_chunk| {
                let line_numbers = text_chunk.source_line_index_range.to_line_number_range();
                let id = format!(
                    "{}_{}-{}",
                    instance_id, line_numbers.start, line_numbers.end
                );
                Self {
                    id: ByteString::from(id),
                    title: ByteString::from(title),
                    text: text_chunk.content.clone(),
                }
            })
            .collect();

        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use bytestring::ByteString;

    use super::*;
    use crate::lang::Language;
    use crate::types::{
        AstChunk, ByteRange, ChunkId, ChunkMetrics, DocumentId, LineIndexRange, Origin,
        TextMetrics, TextMode,
    };

    fn test_document(path: Option<&str>, repo: Option<&str>) -> Document {
        Document {
            document_id: DocumentId(0),
            language: Language::Python,
            source: ByteString::from("# placeholder"),
            origin: Origin {
                path: path.map(ByteString::from),
                repo: repo.map(ByteString::from),
                revision: None,
            },
        }
    }

    fn test_ast_chunk(id: u32, line_start: u32, line_end: u32, node_count: u32) -> AstChunk {
        AstChunk {
            chunk_id: ChunkId(id),
            document_id: DocumentId(0),
            segments: vec![ByteRange { start: 0, end: 10 }],
            envelope: ByteRange { start: 0, end: 10 },
            line_index_range: LineIndexRange {
                start: line_start,
                end: line_end,
            },
            scopes: vec![],
            metrics: ChunkMetrics {
                nws_size: 5,
                node_count,
                segment_count: 1,
            },
        }
    }

    fn test_text_chunk(
        ast_chunk_id: u32,
        content: &str,
        line_start: u32,
        line_end: u32,
        line_count: u32,
        nws: u32,
    ) -> TextChunk {
        TextChunk {
            ast_chunk_id: ChunkId(ast_chunk_id),
            content: ByteString::from(content),
            text_mode: TextMode::Canonical,
            source_line_index_range: LineIndexRange {
                start: line_start,
                end: line_end,
            },
            metrics: TextMetrics {
                content_line_count: line_count,
                content_nws_size: nws,
            },
        }
    }

    // --- JsonRecord tests ---

    #[test]
    fn json_record_basic() {
        let doc = test_document(Some("src/main.py"), Some("my-repo"));
        let chunks = [test_ast_chunk(0, 0, 5, 3)];
        let text_chunks = [test_text_chunk(0, "print('hi')", 0, 5, 1, 10)];

        let records = JsonRecord::build(&doc, &chunks, &text_chunks);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].content, "print('hi')");
        assert_eq!(records[0].metadata.path.as_deref(), Some("src/main.py"));
        assert_eq!(records[0].metadata.repo.as_deref(), Some("my-repo"));
        assert_eq!(records[0].metadata.content_line_count, 1);
        assert_eq!(records[0].metadata.content_nws_size, 10);
        assert_eq!(records[0].metadata.node_count, 3);
    }

    #[test]
    fn json_record_line_numbers() {
        let doc = test_document(Some("f.py"), None);
        let chunks = [test_ast_chunk(0, 0, 10, 1)];
        let text_chunks = [test_text_chunk(0, "code", 0, 10, 10, 4)];

        let records = JsonRecord::build(&doc, &chunks, &text_chunks);
        assert_eq!(records[0].metadata.source_start_line_number, 1);
        assert_eq!(records[0].metadata.source_end_line_number, 10);
    }

    #[test]
    fn json_record_optional_fields() {
        let doc = test_document(None, None);
        let chunks = [test_ast_chunk(0, 0, 1, 1)];
        let text_chunks = [test_text_chunk(0, "x", 0, 1, 1, 1)];

        let records = JsonRecord::build(&doc, &chunks, &text_chunks);
        assert!(records[0].metadata.path.is_none());
        assert!(records[0].metadata.repo.is_none());
    }

    // --- RepoEvalRecord tests ---

    #[test]
    fn repo_eval_record_basic() {
        let doc = test_document(Some("src/lib/utils.py"), Some("org/repo"));
        let chunks = [test_ast_chunk(0, 0, 5, 7)];
        let text_chunks = [test_text_chunk(0, "code", 0, 5, 5, 4)];

        let records = RepoEvalRecord::build(&doc, &chunks, &text_chunks).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(
            records[0].metadata.fpath_tuple,
            vec![
                ByteString::from("src"),
                ByteString::from("lib"),
                ByteString::from("utils.py"),
            ]
        );
        assert_eq!(records[0].metadata.repo, "org/repo");
        assert_eq!(records[0].metadata.node_count, 7);
    }

    #[test]
    fn repo_eval_record_missing_path() {
        let doc = test_document(None, Some("repo"));
        let chunks = [test_ast_chunk(0, 0, 1, 1)];
        let text_chunks = [test_text_chunk(0, "x", 0, 1, 1, 1)];

        let err = RepoEvalRecord::build(&doc, &chunks, &text_chunks).unwrap_err();
        match err {
            AstchunkError::ExportRequirementMissing { exporter, field } => {
                assert_eq!(exporter, "RepoEvalRecord");
                assert_eq!(field, "origin.path");
            }
            _ => panic!("unexpected error variant"),
        }
    }

    #[test]
    fn repo_eval_record_missing_repo() {
        let doc = test_document(Some("f.py"), None);
        let chunks = [test_ast_chunk(0, 0, 1, 1)];
        let text_chunks = [test_text_chunk(0, "x", 0, 1, 1, 1)];

        let err = RepoEvalRecord::build(&doc, &chunks, &text_chunks).unwrap_err();
        match err {
            AstchunkError::ExportRequirementMissing { exporter, field } => {
                assert_eq!(exporter, "RepoEvalRecord");
                assert_eq!(field, "origin.repo");
            }
            _ => panic!("unexpected error variant"),
        }
    }

    // --- SwebenchLiteRecord tests ---

    #[test]
    fn swebench_lite_record_basic() {
        let doc = test_document(Some("src/utils/helper.py"), None);
        let chunks = [test_ast_chunk(0, 5, 15, 2)];
        let text_chunks = [test_text_chunk(0, "text", 5, 15, 10, 4)];

        let records = SwebenchLiteRecord::build(&doc, &text_chunks, "inst-42").unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].id, "inst-42_6-15");
        assert_eq!(records[0].title, "helper.py");
        assert_eq!(records[0].text, "text");
        // Prevent unused variable warning
        let _ = &chunks;
    }

    #[test]
    fn swebench_lite_record_missing_path() {
        let doc = test_document(None, None);
        let text_chunks = [test_text_chunk(0, "x", 0, 1, 1, 1)];

        let err = SwebenchLiteRecord::build(&doc, &text_chunks, "id").unwrap_err();
        match err {
            AstchunkError::ExportRequirementMissing { exporter, field } => {
                assert_eq!(exporter, "SwebenchLiteRecord");
                assert_eq!(field, "origin.path");
            }
            _ => panic!("unexpected error variant"),
        }
    }

    // --- Order preserved ---

    #[test]
    fn output_order_preserved() {
        let doc = test_document(Some("f.py"), Some("r"));
        let chunks = [
            test_ast_chunk(0, 0, 3, 1),
            test_ast_chunk(1, 3, 6, 2),
            test_ast_chunk(2, 6, 9, 3),
        ];
        let text_chunks = [
            test_text_chunk(0, "a", 0, 3, 3, 1),
            test_text_chunk(1, "b", 3, 6, 3, 1),
            test_text_chunk(2, "c", 6, 9, 3, 1),
        ];

        let json = JsonRecord::build(&doc, &chunks, &text_chunks);
        assert_eq!(json[0].content, "a");
        assert_eq!(json[1].content, "b");
        assert_eq!(json[2].content, "c");

        let repo_eval = RepoEvalRecord::build(&doc, &chunks, &text_chunks).unwrap();
        assert_eq!(repo_eval[0].content, "a");
        assert_eq!(repo_eval[1].content, "b");
        assert_eq!(repo_eval[2].content, "c");

        let swebench = SwebenchLiteRecord::build(&doc, &text_chunks, "id").unwrap();
        assert_eq!(swebench[0].text, "a");
        assert_eq!(swebench[1].text, "b");
        assert_eq!(swebench[2].text, "c");
    }
}
