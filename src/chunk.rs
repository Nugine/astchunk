use crate::byte_range::ByteRange;
use crate::lang::Language;
use crate::node::AstNode;
use crate::nws::nws_count_direct;

/// A code chunk produced by AST-based chunking.
///
/// Contains the rebuilt source text, location metadata, and
/// ancestor path information. This is the owned output of the
/// chunking process (no lifetime ties to the tree-sitter tree).
#[derive(Debug, Clone)]
pub struct AstChunk {
    /// Rebuilt source code text for this chunk.
    pub text: String,
    /// Byte range in the original source `[start, end)`.
    pub byte_range: ByteRange,
    /// 0-indexed start line.
    pub start_line: u32,
    /// 0-indexed end line.
    pub end_line: u32,
    /// Non-whitespace character count of the rebuilt text.
    pub size: u32,
    /// Number of AST nodes in this chunk.
    pub node_count: usize,
    /// Ancestor path strings (first line of each class/function definition).
    pub ancestors: Vec<String>,
}

impl AstChunk {
    /// Number of lines covered by this chunk.
    #[must_use]
    pub fn line_count(&self) -> u32 {
        self.end_line - self.start_line + 1
    }
}

/// Rebuild source code from a window of `AstNode`s.
///
/// Restores newlines and indentation between nodes based on their
/// line/column coordinates, matching the Python `rebuild_code` method exactly.
pub fn rebuild_code(window: &[AstNode<'_>], source: &[u8]) -> String {
    if window.is_empty() {
        return String::new();
    }

    let first = &window[0];
    let mut current_line = first.start_line();
    let mut current_col = first.start_col();

    // Pre-allocate with a rough estimate
    let mut code = String::with_capacity(source.len() / 2);

    // Leading indentation for the first node
    for _ in 0..current_col {
        code.push(' ');
    }

    for node in window {
        let node_start_line = node.start_line();
        let node_start_col = node.start_col();

        // Add newlines if we need to jump to a new line
        if node_start_line > current_line {
            let line_diff = node_start_line - current_line;
            for _ in 0..line_diff {
                code.push('\n');
            }
            current_col = 0;
        }

        // Add spaces for indentation
        if node_start_col > current_col {
            let col_diff = node_start_col - current_col;
            for _ in 0..col_diff {
                code.push(' ');
            }
        }

        // Append node text
        code.push_str(node.text(source));

        // Update cursor
        current_line = node.end_line();
        current_col = node.end_col();
    }

    code
}

/// Build ancestor path strings from the ancestor nodes.
///
/// Filters ancestors to class/function definitions (based on language)
/// and extracts the first line of each.
pub fn build_chunk_ancestors(
    ancestors: &[tree_sitter::Node<'_>],
    source: &[u8],
    language: Language,
) -> Vec<String> {
    let types = language.ancestor_node_types();
    let mut result = Vec::new();

    for ancestor in ancestors {
        if types.contains(&ancestor.kind()) {
            let start = ancestor.start_byte();
            let end = ancestor.end_byte();
            let text = std::str::from_utf8(&source[start..end]).unwrap_or("");
            // Extract first line
            let first_line = text.lines().next().unwrap_or("");
            result.push(first_line.to_string());
        }
    }

    result
}

/// Convert a window of `AstNode`s into an `AstChunk`.
pub fn build_chunk(window: &[AstNode<'_>], source: &[u8], language: Language) -> AstChunk {
    assert!(!window.is_empty(), "Cannot build chunk from empty window");

    let text = rebuild_code(window, source);
    let size = nws_count_direct(&text);

    let byte_range = ByteRange::new(
        window.first().unwrap().byte_range().start,
        window.last().unwrap().byte_range().end,
    );
    let start_line = window.first().unwrap().start_line();
    let end_line = window.last().unwrap().end_line();
    let node_count = window.len();
    let ancestors = build_chunk_ancestors(&window[0].ancestors, source, language);

    AstChunk {
        text,
        byte_range,
        start_line,
        end_line,
        size,
        node_count,
        ancestors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::{AstNode, node_nws_size};
    use crate::nws::NwsCumsum;

    #[test]
    #[cfg(feature = "python")]
    fn test_rebuild_code_simple() {
        let code = "x = 1\ny = 2\n";
        let source = code.as_bytes();
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();
        let cumsum = NwsCumsum::new(source);

        // Get the children of the module (two expression_statements)
        let mut cursor = root.walk();
        let children: Vec<_> = root.children(&mut cursor).collect();

        let ast_nodes: Vec<AstNode<'_>> = children
            .iter()
            .map(|c| AstNode::new(*c, node_nws_size(c, &cumsum), vec![]))
            .collect();

        let rebuilt = rebuild_code(&ast_nodes, source);
        // The rebuilt code should match the original (sans trailing newline from the module)
        assert_eq!(rebuilt, "x = 1\ny = 2");
    }

    #[test]
    #[cfg(feature = "python")]
    fn test_rebuild_code_indented() {
        let code = "def foo():\n    x = 1\n    y = 2\n";
        let source = code.as_bytes();
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();
        let cumsum = NwsCumsum::new(source);

        // Get function_definition > block > children (the statements inside the function)
        let func_def = root.child(0).unwrap();
        let block = func_def.child_by_field_name("body").unwrap();
        let mut cursor = block.walk();
        let stmts: Vec<_> = block.children(&mut cursor).collect();

        // Filter only named children (statements, not the ":" delimiter etc.)
        let named_stmts: Vec<_> = stmts.iter().filter(|n| n.is_named()).copied().collect();

        let ancestors = vec![root, func_def];
        let ast_nodes: Vec<AstNode<'_>> = named_stmts
            .iter()
            .map(|c| AstNode::new(*c, node_nws_size(c, &cumsum), ancestors.clone()))
            .collect();

        let rebuilt = rebuild_code(&ast_nodes, source);
        // Should preserve indentation
        assert_eq!(rebuilt, "    x = 1\n    y = 2");
    }

    #[test]
    #[cfg(feature = "python")]
    fn test_build_chunk_ancestors() {
        let code = "class MyClass:\n    def method(self):\n        pass\n";
        let source = code.as_bytes();
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();

        let class_def = root.child(0).unwrap();
        assert_eq!(class_def.kind(), "class_definition");

        let class_body = class_def.child_by_field_name("body").unwrap();
        let method_def = class_body.named_child(0).unwrap();
        assert_eq!(method_def.kind(), "function_definition");

        let ancestors_nodes = vec![root, class_def, method_def];
        let ancestors = build_chunk_ancestors(&ancestors_nodes, source, Language::Python);

        assert_eq!(ancestors.len(), 2); // class and function, but not module
        assert_eq!(ancestors[0], "class MyClass:");
        assert_eq!(ancestors[1], "def method(self):");
    }

    #[test]
    #[cfg(feature = "python")]
    fn test_build_chunk() {
        let code = "x = 1\ny = 2\nz = 3\n";
        let source = code.as_bytes();
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();
        let cumsum = NwsCumsum::new(source);

        let mut cursor = root.walk();
        let children: Vec<_> = root.children(&mut cursor).collect();

        let ast_nodes: Vec<AstNode<'_>> = children
            .iter()
            .map(|c| AstNode::new(*c, node_nws_size(c, &cumsum), vec![root]))
            .collect();

        let chunk = build_chunk(&ast_nodes, source, Language::Python);

        assert_eq!(chunk.text, "x = 1\ny = 2\nz = 3");
        assert_eq!(chunk.start_line, 0);
        assert_eq!(chunk.end_line, 2);
        assert_eq!(chunk.node_count, 3);
        assert!(chunk.size > 0);
        assert_eq!(chunk.line_count(), 3);
    }
}
