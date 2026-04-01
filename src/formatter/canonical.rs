use bytestring::ByteString;

use crate::error::AstchunkError;
use crate::internal::materialize::rebuild_from_segments;
use crate::internal::nws::nws_count_direct;
use crate::types::{AstChunk, Document, TextChunk, TextMetrics, TextMode};

use super::Formatter;

/// Canonical formatter — produces plain rebuilt text without context header.
#[derive(Debug, Clone, Default)]
pub struct CanonicalFormatter {
    _priv: (),
}

impl Formatter for CanonicalFormatter {
    fn format(
        &self,
        document: &Document,
        chunks: &[AstChunk],
    ) -> Result<Vec<TextChunk>, AstchunkError> {
        let source = document.source.as_bytes();
        let text_chunks = chunks
            .iter()
            .map(|chunk| format_one(chunk, source))
            .collect();
        Ok(text_chunks)
    }
}

fn format_one(chunk: &AstChunk, source: &[u8]) -> TextChunk {
    let content = rebuild_from_segments(&chunk.segments, source);
    let content_nws_size = nws_count_direct(&content);
    let content_line_count = if content.is_empty() {
        0
    } else {
        u32::try_from(content.lines().count()).expect("line count overflow")
    };

    TextChunk {
        ast_chunk_id: chunk.chunk_id,
        content: ByteString::from(content),
        text_mode: TextMode::Canonical,
        source_line_index_range: chunk.line_index_range,
        metrics: TextMetrics {
            content_line_count,
            content_nws_size,
        },
    }
}

#[cfg(test)]
mod tests {
    use bytestring::ByteString;

    use super::*;
    use crate::chunker::{CastChunker, CastChunkerOptions, Chunker};
    use crate::lang::Language;
    use crate::types::{Document, DocumentId, Origin};

    fn make_document(code: &str, language: Language) -> Document {
        Document {
            document_id: DocumentId(0),
            language,
            source: ByteString::from(code),
            origin: Origin::default(),
        }
    }

    #[test]
    fn test_canonical_nws_sizes_match_v1() {
        let expected_sizes: &[u32] = &[
            1613, 1671, 1370, 980, 598, 1785, 1783, 1776, 1789, 1719, 1782, 473, 1431, 1655, 322,
            1754, 1745, 764,
        ];

        let code = include_str!("../../tests/source_code.txt");
        let doc = make_document(code, Language::Python);
        let chunker = CastChunker::new(CastChunkerOptions {
            max_nws_size: 1800,
            overlap_nodes: 0,
        });
        let chunks = chunker.chunk(&doc).unwrap();
        let formatter = CanonicalFormatter::default();
        let text_chunks = formatter.format(&doc, &chunks).unwrap();

        assert_eq!(text_chunks.len(), expected_sizes.len());
        for (i, (tc, &expected)) in text_chunks.iter().zip(expected_sizes).enumerate() {
            assert_eq!(
                tc.metrics.content_nws_size, expected,
                "Chunk {i}: expected NWS {expected}, got {}",
                tc.metrics.content_nws_size
            );
        }
    }

    #[test]
    fn test_canonical_text_mode() {
        let code = "x = 1\ny = 2\n";
        let doc = make_document(code, Language::Python);
        let chunker = CastChunker::new(CastChunkerOptions::default());
        let chunks = chunker.chunk(&doc).unwrap();
        let formatter = CanonicalFormatter::default();
        let text_chunks = formatter.format(&doc, &chunks).unwrap();

        for tc in &text_chunks {
            assert_eq!(tc.text_mode, TextMode::Canonical);
            assert!(!tc.content.is_empty());
            assert!(!tc.content.starts_with("'''"));
        }
    }

    #[test]
    fn test_text_metrics_correct() {
        let code = "x = 1\ny = 2\nz = 3\n";
        let doc = make_document(code, Language::Python);
        let chunker = CastChunker::new(CastChunkerOptions::default());
        let chunks = chunker.chunk(&doc).unwrap();
        let formatter = CanonicalFormatter::default();
        let text_chunks = formatter.format(&doc, &chunks).unwrap();

        for tc in &text_chunks {
            let actual_lines = if tc.content.is_empty() {
                0
            } else {
                u32::try_from(tc.content.lines().count()).unwrap()
            };
            assert_eq!(tc.metrics.content_line_count, actual_lines);

            let actual_nws = crate::internal::nws::nws_count_direct(&tc.content);
            assert_eq!(tc.metrics.content_nws_size, actual_nws);
        }
    }

    #[test]
    fn test_output_order_preserved() {
        let code = include_str!("../../tests/source_code.txt");
        let doc = make_document(code, Language::Python);
        let chunker = CastChunker::new(CastChunkerOptions {
            max_nws_size: 1800,
            overlap_nodes: 0,
        });
        let chunks = chunker.chunk(&doc).unwrap();
        let formatter = CanonicalFormatter::default();
        let text_chunks = formatter.format(&doc, &chunks).unwrap();

        assert_eq!(text_chunks.len(), chunks.len());
        for (tc, ac) in text_chunks.iter().zip(chunks.iter()) {
            assert_eq!(tc.ast_chunk_id, ac.chunk_id);
        }
    }

    #[test]
    fn test_empty_chunks() {
        let code = "x = 1\n";
        let doc = make_document(code, Language::Python);
        let formatter = CanonicalFormatter::default();
        let text_chunks = formatter.format(&doc, &[]).unwrap();
        assert!(text_chunks.is_empty());
    }
}
