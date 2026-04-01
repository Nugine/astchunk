use bytestring::ByteString;

use crate::error::AstchunkError;
use crate::internal::materialize::rebuild_from_segments;
use crate::internal::nws::nws_count_direct;
use crate::types::{AstChunk, Document, ScopeFrame, TextChunk, TextMetrics, TextMode};

use super::Formatter;

/// Contextual formatter — produces text with scope/path context header prepended.
#[derive(Debug, Clone, Default)]
pub struct ContextualFormatter {
    _priv: (),
}

impl Formatter for ContextualFormatter {
    fn format(
        &self,
        document: &Document,
        chunks: &[AstChunk],
    ) -> Result<Vec<TextChunk>, AstchunkError> {
        let source = document.source.as_bytes();
        let text_chunks = chunks
            .iter()
            .map(|chunk| format_one(document, chunk, source))
            .collect();
        Ok(text_chunks)
    }
}

fn format_one(document: &Document, chunk: &AstChunk, source: &[u8]) -> TextChunk {
    let canonical_text = rebuild_from_segments(&chunk.segments, source);
    let content = build_contextual_text(
        &canonical_text,
        document.origin.path.as_deref(),
        &chunk.scopes,
    );

    let content_nws_size = nws_count_direct(&content);
    let content_line_count = if content.is_empty() {
        0
    } else {
        u32::try_from(content.lines().count()).expect("line count overflow")
    };

    TextChunk {
        ast_chunk_id: chunk.chunk_id,
        content: ByteString::from(content),
        text_mode: TextMode::Contextual,
        source_line_index_range: chunk.line_index_range,
        metrics: TextMetrics {
            content_line_count,
            content_nws_size,
        },
    }
}

/// Build contextual text with scope/path header.
///
/// Format:
/// ```text
/// '''
/// <path if present>
/// <scope_0 if present>
///     <scope_1 if present>
/// '''
/// <canonical text>
/// ```
///
/// Uses 4 spaces per indentation level.
fn build_contextual_text(
    canonical_text: &str,
    path: Option<&str>,
    scopes: &[ScopeFrame],
) -> String {
    let mut header = String::from("'''\n");

    if let Some(p) = path
        && !p.is_empty()
    {
        header.push_str(p);
        header.push('\n');
    }

    for (i, scope) in scopes.iter().enumerate() {
        for _ in 0..i {
            header.push_str("    ");
        }
        header.push_str(&scope.display);
        header.push('\n');
    }

    header.push_str("'''");

    format!("{header}\n{canonical_text}")
}

#[cfg(test)]
mod tests {
    use bytestring::ByteString;

    use super::*;
    use crate::chunker::{CastChunker, CastChunkerOptions, Chunker};
    use crate::lang::Language;
    use crate::types::{Document, DocumentId, Origin, ScopeKind};

    fn make_document_with_path(code: &str, language: Language, path: &str) -> Document {
        Document {
            document_id: DocumentId(0),
            language,
            source: ByteString::from(code),
            origin: Origin {
                path: Some(ByteString::from(path)),
                ..Origin::default()
            },
        }
    }

    #[test]
    fn test_contextual_header_format() {
        let code = "class MyClass:\n    def method(self):\n        pass\n";
        let doc = make_document_with_path(code, Language::Python, "src/example.py");
        let chunker = CastChunker::new(CastChunkerOptions::default());
        let chunks = chunker.chunk(&doc).unwrap();
        let formatter = ContextualFormatter::default();
        let text_chunks = formatter.format(&doc, &chunks).unwrap();

        for tc in &text_chunks {
            assert_eq!(tc.text_mode, TextMode::Contextual);
            assert!(tc.content.starts_with("'''\n"));
            assert!(tc.content.contains("src/example.py"));
        }
    }

    #[test]
    fn test_contextual_header_with_scopes() {
        let header = build_contextual_text(
            "pass",
            Some("src/main.py"),
            &[
                ScopeFrame {
                    kind: ScopeKind::Class,
                    display: ByteString::from("class Foo:"),
                },
                ScopeFrame {
                    kind: ScopeKind::Method,
                    display: ByteString::from("def bar(self):"),
                },
            ],
        );

        let expected = "'''\nsrc/main.py\nclass Foo:\n    def bar(self):\n'''\npass";
        assert_eq!(header, expected);
    }

    #[test]
    fn test_contextual_header_four_space_indent() {
        let header = build_contextual_text(
            "body",
            Some("a.py"),
            &[
                ScopeFrame {
                    kind: ScopeKind::Class,
                    display: ByteString::from("class A:"),
                },
                ScopeFrame {
                    kind: ScopeKind::Class,
                    display: ByteString::from("class B:"),
                },
                ScopeFrame {
                    kind: ScopeKind::Function,
                    display: ByteString::from("def c():"),
                },
            ],
        );

        let lines: Vec<&str> = header.lines().collect();
        assert_eq!(lines[0], "'''");
        assert_eq!(lines[1], "a.py");
        assert_eq!(lines[2], "class A:");
        assert_eq!(lines[3], "    class B:");
        assert_eq!(lines[4], "        def c():");
        assert_eq!(lines[5], "'''");
        assert_eq!(lines[6], "body");
    }

    #[test]
    fn test_contextual_header_no_path() {
        let header = build_contextual_text(
            "code",
            None,
            &[ScopeFrame {
                kind: ScopeKind::Function,
                display: ByteString::from("def foo():"),
            }],
        );

        let expected = "\
'''\n\
def foo():\n\
'''\n\
code";
        assert_eq!(header, expected);
    }
}
