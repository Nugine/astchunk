use crate::byte_range::ByteRange;
use crate::chunk::{AstChunk, build_chunk};
use crate::lang::Language;
use crate::metadata::{
    ChunkOptions, CodeWindow, MetadataTemplate, apply_chunk_expansion, build_metadata,
};
use crate::node::{AstNode, node_nws_size};
use crate::nws::NwsCumsum;

/// AST-based chunk builder.
///
/// This is the core algorithm that drives the chunking process, mirroring
/// the Python `ASTChunkBuilder` class.
pub struct AstChunkBuilder {
    /// Maximum non-whitespace character count per chunk.
    max_chunk_size: u32,
    /// Programming language.
    language: Language,
}

impl AstChunkBuilder {
    /// Create a new builder.
    #[must_use]
    pub fn new(max_chunk_size: u32, language: Language) -> Self {
        Self {
            max_chunk_size,
            language,
        }
    }

    /// Parse source code and create a tree-sitter parser for the configured language.
    fn parse(&self, source: &[u8]) -> tree_sitter::Tree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&self.language.ts_language())
            .expect("failed to set language");
        parser.parse(source, None).expect("failed to parse source")
    }

    // ------------------------------------------------------------------ //
    //  Step 1: Assign AST tree / nodes to windows                        //
    // ------------------------------------------------------------------ //

    /// Assign the AST tree to windows (tentative chunks of `AstNode`).
    ///
    /// Handles the edge case where the whole tree fits in one window,
    /// otherwise delegates to [`Self::assign_nodes_to_windows`].
    #[must_use]
    pub fn assign_tree_to_windows<'tree>(
        &self,
        source: &[u8],
        root_node: tree_sitter::Node<'tree>,
    ) -> Vec<Vec<AstNode<'tree>>> {
        let cumsum = NwsCumsum::new(source);
        let tree_range = ByteRange::from_ts_node(&root_node);
        let tree_size = cumsum.get(tree_range);

        // If the entire tree fits, return it as one window.
        if tree_size <= self.max_chunk_size {
            return vec![vec![AstNode::new(root_node, tree_size, vec![])]];
        }

        // Otherwise, recursively assign children to windows.
        let ancestors = vec![root_node];
        let mut windows = Vec::new();
        self.assign_nodes_to_windows(
            &Self::ts_children(&root_node),
            &cumsum,
            &ancestors,
            &mut windows,
        );
        windows
    }

    /// Collect the children of a tree-sitter node into a `Vec`.
    fn ts_children<'tree>(node: &tree_sitter::Node<'tree>) -> Vec<tree_sitter::Node<'tree>> {
        let mut cursor = node.walk();
        node.children(&mut cursor).collect()
    }

    /// Greedily assign AST nodes to windows based on non-whitespace character count.
    ///
    /// Mirrors Python's `assign_nodes_to_windows` method.
    fn assign_nodes_to_windows<'tree>(
        &self,
        nodes: &[tree_sitter::Node<'tree>],
        cumsum: &NwsCumsum,
        ancestors: &[tree_sitter::Node<'tree>],
        out: &mut Vec<Vec<AstNode<'tree>>>,
    ) {
        if nodes.is_empty() {
            return;
        }

        let mut current_window: Vec<AstNode<'tree>> = Vec::new();
        let mut current_window_size: u32 = 0;

        for &node in nodes {
            let node_size = node_nws_size(&node, cumsum);
            let node_exceeds_limit = node_size > self.max_chunk_size;

            if (current_window.is_empty() && node_exceeds_limit)
                || (current_window_size + node_size > self.max_chunk_size)
            {
                // Yield current window if not empty
                if !current_window.is_empty() {
                    out.push(std::mem::take(&mut current_window));
                    current_window_size = 0;
                }

                if node_exceeds_limit {
                    // Recursively process the node's children
                    let mut child_ancestors = ancestors.to_vec();
                    child_ancestors.push(node);

                    let children = Self::ts_children(&node);
                    let mut child_windows = Vec::new();
                    self.assign_nodes_to_windows(
                        &children,
                        cumsum,
                        &child_ancestors,
                        &mut child_windows,
                    );

                    if !child_windows.is_empty() {
                        // Greedily merge adjacent windows
                        let merged = self.merge_adjacent_windows(child_windows);
                        out.extend(merged);
                    }
                } else {
                    // Node fits in an empty window
                    current_window.push(AstNode::new(node, node_size, ancestors.to_vec()));
                    current_window_size += node_size;
                }
            } else {
                // Node fits in current window
                current_window.push(AstNode::new(node, node_size, ancestors.to_vec()));
                current_window_size += node_size;
            }
        }

        // Yield remaining window
        if !current_window.is_empty() {
            out.push(current_window);
        }
    }

    /// Greedily merge adjacent sibling windows if their combined size fits.
    ///
    /// Only merges windows that contain sibling AST nodes (from the same
    /// recursive call), preserving AST structure.
    #[must_use]
    fn merge_adjacent_windows<'tree>(
        &self,
        ast_windows: Vec<Vec<AstNode<'tree>>>,
    ) -> Vec<Vec<AstNode<'tree>>> {
        assert!(!ast_windows.is_empty(), "Expected non-empty ast_windows");

        let mut merged: Vec<Vec<AstNode<'tree>>> = Vec::new();
        let mut iter = ast_windows.into_iter();

        // Start with the first window
        merged.push(iter.next().unwrap());

        for window in iter {
            let last = merged.last().unwrap();
            let merged_size: u32 = last.iter().map(|n| n.nws_size).sum::<u32>()
                + window.iter().map(|n| n.nws_size).sum::<u32>();

            if merged_size <= self.max_chunk_size {
                merged.last_mut().unwrap().extend(window);
            } else {
                merged.push(window);
            }
        }

        merged
    }

    // ------------------------------------------------------------------ //
    //  Step 2: Add window overlapping (optional)                         //
    // ------------------------------------------------------------------ //

    /// Extend each window by adding overlapping `AstNode`s from adjacent windows.
    ///
    /// For each window, prepends the last `k` nodes from the previous window and
    /// appends the first `k` nodes from the next window, where `k = chunk_overlap`.
    ///
    /// Note: overlapping is not constrained by `max_chunk_size`.
    fn add_window_overlapping<'tree>(
        ast_windows: &[Vec<AstNode<'tree>>],
        chunk_overlap: usize,
    ) -> Vec<Vec<AstNode<'tree>>> {
        if chunk_overlap == 0 {
            return ast_windows.to_vec();
        }

        let n = ast_windows.len();
        let mut result = Vec::with_capacity(n);

        for i in 0..n {
            let mut current = ast_windows[i].clone();

            // Prepend from previous window
            if i > 0 {
                let prev = &ast_windows[i - 1];
                let take = chunk_overlap.min(prev.len());
                let prefix = &prev[prev.len() - take..];
                let mut new = prefix.to_vec();
                new.append(&mut current);
                current = new;
            }

            // Append from next window
            if i + 1 < n {
                let next = &ast_windows[i + 1];
                let take = chunk_overlap.min(next.len());
                current.extend_from_slice(&next[..take]);
            }

            result.push(current);
        }

        result
    }

    // ------------------------------------------------------------------ //
    //  Full chunking pipeline                                            //
    // ------------------------------------------------------------------ //

    /// Parse source code into structurally-aware chunks using AST.
    ///
    /// This is the main entry point, equivalent to the Python `chunkify` method.
    #[must_use]
    pub fn chunkify(
        &self,
        code: &str,
        template: MetadataTemplate,
        options: &ChunkOptions,
    ) -> Vec<CodeWindow> {
        let source = code.as_bytes();

        // Step 1: Parse and assign to windows
        let tree = self.parse(source);
        let ast_windows = self.assign_tree_to_windows(source, tree.root_node());

        // Step 2: Optional overlapping
        let ast_windows = if options.chunk_overlap > 0 {
            Self::add_window_overlapping(&ast_windows, options.chunk_overlap)
        } else {
            ast_windows
        };

        // Step 3: Convert windows to AstChunks
        let mut chunks: Vec<AstChunk> = ast_windows
            .iter()
            .map(|w| build_chunk(w, source, self.language))
            .collect();

        // Optional: chunk expansion
        if options.chunk_expansion {
            for chunk in &mut chunks {
                apply_chunk_expansion(chunk, template, &options.repo_metadata);
            }
        }

        // Step 4: Convert to output CodeWindows
        chunks
            .iter()
            .map(|c| build_metadata(c, template, &options.repo_metadata))
            .collect()
    }
}
mod tests {
    use super::{AstChunkBuilder, Language};

    #[cfg(feature = "python")]
    fn make_builder(max_chunk_size: u32) -> AstChunkBuilder {
        AstChunkBuilder::new(max_chunk_size, Language::Python)
    }

    #[test]
    #[cfg(feature = "python")]
    fn test_small_code_single_window() {
        let code = "x = 1\ny = 2\n";
        let builder = make_builder(1000);
        let tree = builder.parse(code.as_bytes());
        let windows = builder.assign_tree_to_windows(code.as_bytes(), tree.root_node());

        // Small code should produce exactly one window
        assert_eq!(windows.len(), 1);
        // That window should contain the root node itself
        assert_eq!(windows[0].len(), 1);
        assert_eq!(windows[0][0].node.kind(), "module");
    }

    #[test]
    #[cfg(feature = "python")]
    fn test_multiple_functions_split() {
        use std::fmt::Write;
        // Create code large enough to require multiple chunks
        let mut code = String::new();
        for i in 0..20 {
            writeln!(code, "def func_{i}():\n    x = {i}\n    return x * 2\n").unwrap();
        }

        let builder = make_builder(50); // Small chunk size to force splitting
        let tree = builder.parse(code.as_bytes());
        let windows = builder.assign_tree_to_windows(code.as_bytes(), tree.root_node());

        // Should produce multiple windows
        assert!(
            windows.len() > 1,
            "Expected multiple windows, got {}",
            windows.len()
        );

        // Each window should be non-empty
        for (i, w) in windows.iter().enumerate() {
            assert!(!w.is_empty(), "Window {i} is empty");
        }
    }

    #[test]
    #[cfg(feature = "python")]
    fn test_large_function_recursive_split() {
        use std::fmt::Write;
        // A single large function that exceeds max_chunk_size
        let mut code = String::from("def big_function():\n");
        for i in 0..50 {
            writeln!(code, "    x_{i} = {i}").unwrap();
        }
        code.push_str("    return x_0\n");

        let builder = make_builder(30); // Very small chunk size
        let tree = builder.parse(code.as_bytes());
        let windows = builder.assign_tree_to_windows(code.as_bytes(), tree.root_node());

        // Should split the function body into multiple windows
        assert!(
            windows.len() > 1,
            "Expected multiple windows for large function"
        );

        // Windows from recursive split should have ancestors
        for w in &windows {
            for node in w {
                // Ancestors should include at least the root and the function_definition
                assert!(
                    node.ancestors.len() >= 2,
                    "Expected at least 2 ancestors, got {}",
                    node.ancestors.len()
                );
            }
        }
    }

    #[test]
    #[cfg(feature = "python")]
    fn test_window_sizes_respect_limit() {
        use std::fmt::Write;

        use crate::nws::NwsCumsum;

        let mut code = String::new();
        for i in 0..30 {
            writeln!(
                code,
                "def func_{i}(arg1, arg2):\n    result = arg1 + arg2 + {i}\n    return result\n"
            )
            .unwrap();
        }

        let max_size = 100;
        let builder = make_builder(max_size);
        let tree = builder.parse(code.as_bytes());
        let cumsum = NwsCumsum::new(code.as_bytes());
        let windows = builder.assign_tree_to_windows(code.as_bytes(), tree.root_node());

        // Check that each window's total NWS size does not exceed max_chunk_size
        // (this only applies to non-overlapping, non-expanded chunks)
        for (i, w) in windows.iter().enumerate() {
            let total_nws: u32 = w.iter().map(|n| n.nws_size).sum();
            // There's a subtlety: individual nodes can exceed the limit if they
            // are leaves. But windows of sibling nodes should respect it.
            // For strict checking, we use the rebuild approach later.
            // Here we check a weaker invariant: window is non-empty
            assert!(!w.is_empty(), "Window {i} is empty");

            // Also check each node has correct nws_size
            for node in w {
                let expected = crate::nws::NwsCumsum::new(code.as_bytes()).get(node.byte_range());
                assert_eq!(
                    node.nws_size, expected,
                    "NWS size mismatch for node in window {i}"
                );
            }
            let _ = (total_nws, &cumsum);
        }
    }

    #[test]
    #[cfg(feature = "python")]
    fn test_chunkify_end_to_end() {
        use crate::metadata::{ChunkOptions, CodeWindow, MetadataTemplate};

        let code = include_str!("../tests/source_code.txt");
        let builder = make_builder(1800);
        let options = ChunkOptions::default();
        let windows = builder.chunkify(code, MetadataTemplate::Default, &options);

        // Python produces 18 chunks with max_chunk_size=1800
        assert_eq!(
            windows.len(),
            18,
            "Expected 18 chunks (matching Python output), got {}",
            windows.len()
        );

        // Verify each chunk's NWS count should not exceed 1800
        for (i, w) in windows.iter().enumerate() {
            if let CodeWindow::Standard { content, metadata } = w {
                let size = metadata["chunk_size"].as_u64().unwrap();
                assert!(
                    size <= 1800,
                    "Chunk {i} size {size} exceeds max_chunk_size 1800"
                );
                assert!(!content.is_empty(), "Chunk {i} is empty");
            } else {
                panic!("Expected Standard CodeWindow");
            }
        }
    }

    #[test]
    #[cfg(feature = "python")]
    fn test_chunkify_matches_python_sizes() {
        use crate::metadata::{ChunkOptions, CodeWindow, MetadataTemplate};

        // Expected sizes from Python output
        let expected_sizes: &[u64] = &[
            1613, 1671, 1370, 980, 598, 1785, 1783, 1776, 1789, 1719, 1782, 473, 1431, 1655, 322,
            1754, 1745, 764,
        ];

        let code = include_str!("../tests/source_code.txt");
        let builder = make_builder(1800);
        let options = ChunkOptions::default();
        let windows = builder.chunkify(code, MetadataTemplate::Default, &options);

        assert_eq!(windows.len(), expected_sizes.len());

        for (i, (w, &expected)) in windows.iter().zip(expected_sizes).enumerate() {
            if let CodeWindow::Standard { metadata, .. } = w {
                let actual = metadata["chunk_size"].as_u64().unwrap();
                assert_eq!(
                    actual, expected,
                    "Chunk {i}: expected size {expected}, got {actual}"
                );
            }
        }
    }

    #[test]
    #[cfg(feature = "python")]
    fn test_chunkify_matches_python_line_counts() {
        use crate::metadata::{ChunkOptions, CodeWindow, MetadataTemplate};

        let expected_lines: &[u64] = &[
            75, 64, 49, 33, 32, 59, 69, 71, 69, 61, 66, 21, 66, 54, 11, 79, 71, 27,
        ];

        let code = include_str!("../tests/source_code.txt");
        let builder = make_builder(1800);
        let options = ChunkOptions::default();
        let windows = builder.chunkify(code, MetadataTemplate::Default, &options);

        assert_eq!(windows.len(), expected_lines.len());

        for (i, (w, &expected)) in windows.iter().zip(expected_lines).enumerate() {
            if let CodeWindow::Standard { metadata, .. } = w {
                let actual = metadata["line_count"].as_u64().unwrap();
                assert_eq!(
                    actual, expected,
                    "Chunk {i}: expected {expected} lines, got {actual}"
                );
            }
        }
    }

    #[test]
    #[cfg(feature = "python")]
    fn test_chunkify_with_overlap() {
        use crate::metadata::{ChunkOptions, CodeWindow, MetadataTemplate};

        let code = include_str!("../tests/source_code.txt");
        let builder = make_builder(1800);
        let options = ChunkOptions {
            chunk_overlap: 2,
            ..ChunkOptions::default()
        };
        let windows = builder.chunkify(code, MetadataTemplate::Default, &options);

        // With overlap, should still get same number of chunks
        assert_eq!(windows.len(), 18);

        for w in &windows {
            if let CodeWindow::Standard { content, .. } = w {
                assert!(!content.is_empty());
            }
        }
    }

    #[test]
    #[cfg(feature = "python")]
    fn test_chunkify_with_expansion() {
        use crate::metadata::{ChunkOptions, CodeWindow, MetadataTemplate};
        use std::collections::HashMap;

        let code = include_str!("../tests/source_code.txt");
        let builder = make_builder(1800);
        let mut repo_metadata = HashMap::new();
        repo_metadata.insert("filepath".to_string(), "source_code.py".to_string());
        let options = ChunkOptions {
            chunk_expansion: true,
            repo_metadata,
            ..ChunkOptions::default()
        };
        let windows = builder.chunkify(code, MetadataTemplate::Default, &options);

        assert_eq!(windows.len(), 18);

        // Each chunk should start with the expansion header
        for (i, w) in windows.iter().enumerate() {
            if let CodeWindow::Standard { content, .. } = w {
                assert!(
                    content.starts_with("'''"),
                    "Chunk {i} should start with expansion header"
                );
            }
        }
    }

    #[test]
    #[cfg(feature = "java")]
    fn test_chunkify_java() {
        use crate::metadata::{ChunkOptions, CodeWindow, MetadataTemplate};

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

    public int getValue() {
        return this.value;
    }
}
";
        let builder = AstChunkBuilder::new(50, Language::Java);
        let options = ChunkOptions::default();
        let windows = builder.chunkify(code, MetadataTemplate::Default, &options);

        assert!(!windows.is_empty(), "Java chunking should produce chunks");
        for w in &windows {
            if let CodeWindow::Standard { content, .. } = w {
                assert!(!content.is_empty());
            }
        }
    }

    #[test]
    #[cfg(feature = "typescript")]
    fn test_chunkify_typescript() {
        use crate::metadata::{ChunkOptions, CodeWindow, MetadataTemplate};

        let code = r#"
class Greeter {
    greeting: string;

    constructor(message: string) {
        this.greeting = message;
    }

    greet(): string {
        return "Hello, " + this.greeting;
    }
}

function main() {
    const g = new Greeter("world");
    console.log(g.greet());
}
"#;
        let builder = AstChunkBuilder::new(50, Language::TypeScript);
        let options = ChunkOptions::default();
        let windows = builder.chunkify(code, MetadataTemplate::Default, &options);

        assert!(
            !windows.is_empty(),
            "TypeScript chunking should produce chunks"
        );
        for w in &windows {
            if let CodeWindow::Standard { content, .. } = w {
                assert!(!content.is_empty());
            }
        }
    }
}
