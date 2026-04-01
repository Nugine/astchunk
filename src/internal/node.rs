use super::byte_range::{ByteRange, to_u32};
use super::nws::NwsCumsum;

/// An AST node wrapper that carries size and ancestor information.
///
/// Mirrors the Python `ASTNode` class. Wraps a `tree_sitter::Node` with:
/// - `size`: non-whitespace character count of the node
/// - `ancestors`: chain of ancestor tree-sitter nodes (for path construction)
#[derive(Clone)]
pub struct AstNode<'tree> {
    /// The underlying tree-sitter node.
    pub node: tree_sitter::Node<'tree>,
    /// Non-whitespace character count of this node.
    pub nws_size: u32,
    /// Ancestor nodes (from root toward this node's parent).
    pub ancestors: Vec<tree_sitter::Node<'tree>>,
}

impl<'tree> AstNode<'tree> {
    /// Create a new `AstNode`.
    #[must_use]
    pub fn new(
        node: tree_sitter::Node<'tree>,
        nws_size: u32,
        ancestors: Vec<tree_sitter::Node<'tree>>,
    ) -> Self {
        Self {
            node,
            nws_size,
            ancestors,
        }
    }

    /// 0-indexed start line.
    #[must_use]
    pub fn start_line(&self) -> u32 {
        to_u32(self.node.start_position().row)
    }

    /// 0-indexed end line.
    #[must_use]
    pub fn end_line(&self) -> u32 {
        to_u32(self.node.end_position().row)
    }

    /// Start column (byte offset within the line).
    #[must_use]
    pub fn start_col(&self) -> u32 {
        to_u32(self.node.start_position().column)
    }

    /// End column.
    #[must_use]
    pub fn end_col(&self) -> u32 {
        to_u32(self.node.end_position().column)
    }

    /// The text of this node, extracted from the source bytes.
    ///
    /// # Panics
    ///
    /// Panics if the byte range is not valid UTF-8.
    #[must_use]
    pub fn text<'a>(&self, source: &'a [u8]) -> &'a str {
        let start = self.node.start_byte();
        let end = self.node.end_byte();
        std::str::from_utf8(&source[start..end]).expect("node text is not valid UTF-8")
    }
}

/// Compute the non-whitespace size of a tree-sitter node using the cumulative sum.
#[must_use]
pub fn node_nws_size(node: &tree_sitter::Node<'_>, cumsum: &NwsCumsum) -> u32 {
    let range = ByteRange::from_ts_node(node);
    cumsum.get(range)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ast_node_properties() {
        let code = "def foo():\n    return 42\n";
        let source = code.as_bytes();
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();

        let cumsum = NwsCumsum::new(source);

        // The root node should be "module"
        assert_eq!(root.kind(), "module");

        // First child should be function_definition
        let func_def = root.child(0).unwrap();
        assert_eq!(func_def.kind(), "function_definition");

        let nws = node_nws_size(&func_def, &cumsum);
        let ast_node = AstNode::new(func_def, nws, vec![root]);

        assert_eq!(ast_node.start_line(), 0);
        assert_eq!(ast_node.end_line(), 1);
        assert_eq!(ast_node.start_col(), 0);
        assert!(ast_node.nws_size > 0);
        assert_eq!(ast_node.text(source), "def foo():\n    return 42");
        assert_eq!(ast_node.ancestors.len(), 1);
    }
}
