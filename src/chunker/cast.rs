use bytestring::ByteString;

use crate::error::AstchunkError;
use crate::internal::byte_range::ByteRange;
use crate::internal::materialize::rebuild_code;
use crate::internal::node::AstNode;
use crate::internal::nws::nws_count_direct;
use crate::internal::partition;
use crate::lang::Language;
use crate::types::{
    AstChunk, ChunkId, ChunkMetrics, Document, DocumentId, LineIndexRange, ScopeFrame, ScopeKind,
};

use super::Chunker;

/// Options for the cAST chunking algorithm.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct CastChunkerOptions {
    /// Maximum non-whitespace character count per chunk.
    pub max_nws_size: u32,
    /// Number of AST nodes to overlap between adjacent windows.
    pub overlap_nodes: usize,
}

impl Default for CastChunkerOptions {
    fn default() -> Self {
        Self {
            max_nws_size: 1500,
            overlap_nodes: 0,
        }
    }
}

/// Structural chunker implementing the cAST paper algorithm.
///
/// Splits a [`Document`] into [`AstChunk`]s by partitioning
/// AST nodes into windows based on non-whitespace character budgets.
#[derive(Debug, Clone)]
pub struct CastChunker {
    /// Chunking options.
    pub options: CastChunkerOptions,
}

impl CastChunker {
    /// Create a new chunker with the given options.
    #[must_use]
    pub fn new(options: CastChunkerOptions) -> Self {
        Self { options }
    }
}

impl Chunker for CastChunker {
    fn chunk(&self, document: &Document) -> Result<Vec<AstChunk>, AstchunkError> {
        let source = document.source.as_bytes();
        let language = document.language;

        // Step 1: Parse and assign to windows
        let tree = partition::parse(language, source);
        let ast_windows =
            partition::assign_tree_to_windows(self.options.max_nws_size, source, tree.root_node());

        // Step 2: Optional overlapping
        let ast_windows = if self.options.overlap_nodes > 0 {
            partition::add_window_overlapping(&ast_windows, self.options.overlap_nodes)
        } else {
            ast_windows
        };

        // Step 3: Convert windows to AstChunks
        let chunks = ast_windows
            .iter()
            .enumerate()
            .map(|(i, window)| {
                build_ast_chunk(
                    window,
                    source,
                    language,
                    document.document_id,
                    ChunkId(u32::try_from(i).expect("chunk index overflow")),
                )
            })
            .collect();

        Ok(chunks)
    }
}

/// Convert a window of `AstNode`s into an `AstChunk`.
fn build_ast_chunk(
    window: &[AstNode<'_>],
    source: &[u8],
    language: Language,
    document_id: DocumentId,
    chunk_id: ChunkId,
) -> AstChunk {
    assert!(!window.is_empty(), "Cannot build chunk from empty window");

    // Compute segments: each AstNode becomes a ByteRange segment
    let segments: Vec<ByteRange> = window
        .iter()
        .map(|n| ByteRange::from_ts_node(&n.node))
        .collect();

    // Compute envelope (min start, max end across all segments)
    let envelope_start = segments.iter().map(|s| s.start).min().unwrap();
    let envelope_end = segments.iter().map(|s| s.end).max().unwrap();
    let envelope = ByteRange::new(envelope_start, envelope_end);

    // Line range (0-based)
    let start_line = window.first().unwrap().start_line();
    let end_line = window.last().unwrap().end_line();
    // LineIndexRange is half-open [start, end)
    let line_index_range = LineIndexRange {
        start: start_line,
        end: end_line + 1,
    };

    // Build scopes from ancestors
    let scopes = build_scopes(&window[0].ancestors, source, language);

    // Compute NWS size from rebuilt text so it matches v1 behavior
    let text = rebuild_code(window, source);
    let nws_size = nws_count_direct(&text);

    let metrics = ChunkMetrics {
        nws_size,
        node_count: u32::try_from(window.len()).expect("node count overflow"),
        segment_count: u32::try_from(segments.len()).expect("segment count overflow"),
    };

    AstChunk {
        chunk_id,
        document_id,
        segments,
        envelope,
        line_index_range,
        scopes,
        metrics,
    }
}

/// Build scope frames from ancestor tree-sitter nodes.
fn build_scopes(
    ancestors: &[tree_sitter::Node<'_>],
    source: &[u8],
    language: Language,
) -> Vec<ScopeFrame> {
    let types = language.ancestor_node_types();
    let mut result = Vec::new();

    for ancestor in ancestors {
        let kind_str = ancestor.kind();
        if types.contains(&kind_str) {
            let start = ancestor.start_byte();
            let end = ancestor.end_byte();
            let text = std::str::from_utf8(&source[start..end]).unwrap_or("");
            let first_line = text.lines().next().unwrap_or("");

            let kind = classify_scope_kind(kind_str, language);
            result.push(ScopeFrame {
                kind,
                display: ByteString::from(first_line),
            });
        }
    }

    result
}

/// Classify a tree-sitter node type string into a `ScopeKind`.
fn classify_scope_kind(node_type: &str, language: Language) -> ScopeKind {
    match node_type {
        "function_definition" if language == Language::Python => ScopeKind::Function,
        "class_definition" | "class_specifier" | "struct_specifier" | "struct_item"
        | "enum_item" | "class_declaration" => ScopeKind::Class,
        "namespace_definition" => ScopeKind::Namespace,
        "impl_item" => ScopeKind::Impl,
        "trait_item" => ScopeKind::Trait,
        "mod_item" => ScopeKind::Module,
        "method_declaration" | "method_definition" => ScopeKind::Method,
        "constructor_declaration" => ScopeKind::Constructor,
        "interface_declaration" => ScopeKind::Interface,
        _ => ScopeKind::Function,
    }
}

#[cfg(test)]
mod tests {
    use bytestring::ByteString;

    use super::*;
    use crate::types::{DocumentId, Origin};

    fn make_document(code: &str, language: Language) -> Document {
        Document {
            document_id: DocumentId(0),
            language,
            source: ByteString::from(code),
            origin: Origin::default(),
        }
    }

    #[test]
    fn test_basic_chunk_count() {
        let code = include_str!("../../tests/source_code.txt");
        let doc = make_document(code, Language::Python);
        let chunker = CastChunker::new(CastChunkerOptions {
            max_nws_size: 1800,
            overlap_nodes: 0,
        });
        let chunks = chunker.chunk(&doc).unwrap();
        assert_eq!(chunks.len(), 18, "Expected 18 chunks, got {}", chunks.len());
    }

    #[test]
    fn test_chunk_ids_sequential() {
        let code = include_str!("../../tests/source_code.txt");
        let doc = make_document(code, Language::Python);
        let chunker = CastChunker::new(CastChunkerOptions {
            max_nws_size: 1800,
            overlap_nodes: 0,
        });
        let chunks = chunker.chunk(&doc).unwrap();
        for (i, chunk) in chunks.iter().enumerate() {
            assert_eq!(
                chunk.chunk_id.0,
                u32::try_from(i).unwrap(),
                "Chunk ID mismatch at index {i}"
            );
        }
    }

    #[test]
    fn test_nws_sizes_match_v1() {
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
        assert_eq!(chunks.len(), expected_sizes.len());

        for (i, (chunk, &expected)) in chunks.iter().zip(expected_sizes).enumerate() {
            assert_eq!(
                chunk.metrics.nws_size, expected,
                "Chunk {i}: expected NWS size {expected}, got {}",
                chunk.metrics.nws_size
            );
        }
    }

    #[test]
    fn test_overlap_preserves_count() {
        let code = include_str!("../../tests/source_code.txt");
        let doc = make_document(code, Language::Python);
        let chunker = CastChunker::new(CastChunkerOptions {
            max_nws_size: 1800,
            overlap_nodes: 2,
        });
        let chunks = chunker.chunk(&doc).unwrap();
        assert_eq!(chunks.len(), 18, "Overlap should preserve chunk count");

        for chunk in &chunks {
            assert!(chunk.metrics.node_count > 0);
        }
    }

    #[test]
    fn test_document_id_propagated() {
        let code = "x = 1\ny = 2\n";
        let mut doc = make_document(code, Language::Python);
        doc.document_id = DocumentId(42);
        let chunker = CastChunker::new(CastChunkerOptions::default());
        let chunks = chunker.chunk(&doc).unwrap();
        for chunk in &chunks {
            assert_eq!(chunk.document_id, DocumentId(42));
        }
    }

    #[test]
    fn test_java_smoke() {
        let code = r"
public class Calculator {
    private int value;

    public Calculator(int initial) {
        this.value = initial;
    }

    public int add(int x) {
        this.value += x;
        return this.value;
    }

    public int subtract(int x) {
        this.value -= x;
        return this.value;
    }
}
";
        let doc = make_document(code, Language::Java);
        let chunker = CastChunker::new(CastChunkerOptions {
            max_nws_size: 50,
            overlap_nodes: 0,
        });
        let chunks = chunker.chunk(&doc).unwrap();
        assert!(!chunks.is_empty(), "Java chunking should produce chunks");
    }

    #[test]
    fn test_rust_smoke() {
        let code = r#"
pub struct Calculator {
    value: i32,
}

impl Calculator {
    pub fn new(value: i32) -> Self {
        Self { value }
    }

    pub fn add(&mut self, x: i32) -> i32 {
        self.value += x;
        self.value
    }
}

fn main() {
    let mut calc = Calculator::new(0);
    println!("{}", calc.add(42));
}
"#;
        let doc = make_document(code, Language::Rust);
        let chunker = CastChunker::new(CastChunkerOptions {
            max_nws_size: 50,
            overlap_nodes: 0,
        });
        let chunks = chunker.chunk(&doc).unwrap();
        assert!(!chunks.is_empty(), "Rust chunking should produce chunks");
    }

    #[test]
    fn test_scopes_for_nested_python() {
        let code = "\
class Foo:
    def bar(self):
        x = 1
        y = 2
        z = 3
        w = 4
        a = 5
        b = 6
        c = 7
        d = 8
";
        let doc = make_document(code, Language::Python);
        let chunker = CastChunker::new(CastChunkerOptions {
            max_nws_size: 10,
            overlap_nodes: 0,
        });
        let chunks = chunker.chunk(&doc).unwrap();
        assert!(!chunks.is_empty());

        let has_scopes = chunks.iter().any(|c| !c.scopes.is_empty());
        assert!(has_scopes, "Expected at least one chunk with scopes");
    }

    #[test]
    fn test_line_index_range() {
        let code = include_str!("../../tests/source_code.txt");
        let doc = make_document(code, Language::Python);
        let chunker = CastChunker::new(CastChunkerOptions {
            max_nws_size: 1800,
            overlap_nodes: 0,
        });
        let chunks = chunker.chunk(&doc).unwrap();

        for (i, chunk) in chunks.iter().enumerate() {
            assert!(
                chunk.line_index_range.start < chunk.line_index_range.end,
                "Chunk {i} has invalid line range: {:?}",
                chunk.line_index_range
            );
        }

        assert_eq!(chunks[0].line_index_range.start, 0);
    }
}
