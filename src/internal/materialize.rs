use crate::internal::byte_range::ByteRange;
use crate::internal::node::AstNode;

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

/// Compute 0-based (line, column) position at a byte offset in source.
fn byte_offset_position(source: &[u8], offset: usize) -> (u32, u32) {
    let mut line: u32 = 0;
    let mut col: u32 = 0;
    for &b in &source[..offset] {
        if b == b'\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// Rebuild source code from byte-range segments.
///
/// Restores newlines and indentation between segments based on their
/// positions in the original source.
pub fn rebuild_from_segments(segments: &[ByteRange], source: &[u8]) -> String {
    if segments.is_empty() {
        return String::new();
    }

    let mut code = String::with_capacity(source.len() / 2);

    let mut current_line: u32 = 0;
    let mut current_col: u32 = 0;
    let mut first = true;

    for seg in segments {
        let start = seg.start as usize;
        let end = seg.end as usize;
        let seg_text = std::str::from_utf8(&source[start..end]).unwrap_or("");

        let (seg_start_line, seg_start_col) = byte_offset_position(source, start);

        if first {
            for _ in 0..seg_start_col {
                code.push(' ');
            }
            current_line = seg_start_line;
            current_col = seg_start_col;
            first = false;
        }

        if seg_start_line > current_line {
            let line_diff = seg_start_line - current_line;
            for _ in 0..line_diff {
                code.push('\n');
            }
            current_col = 0;
        }

        if seg_start_col > current_col {
            let col_diff = seg_start_col - current_col;
            for _ in 0..col_diff {
                code.push(' ');
            }
        }

        code.push_str(seg_text);

        let (end_line, end_col) = byte_offset_position(source, end);
        current_line = end_line;
        current_col = end_col;
    }

    code
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::internal::byte_range::ByteRange;
    use crate::internal::node::{AstNode, node_nws_size};
    use crate::internal::nws::NwsCumsum;

    #[test]
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
    fn test_rebuild_from_segments_simple() {
        let code = "x = 1\ny = 2\n";
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
            .map(|c| AstNode::new(*c, node_nws_size(c, &cumsum), vec![]))
            .collect();

        let segments: Vec<ByteRange> = ast_nodes
            .iter()
            .map(|n| ByteRange::from_ts_node(&n.node))
            .collect();

        let from_nodes = rebuild_code(&ast_nodes, source);
        let from_segments = rebuild_from_segments(&segments, source);
        assert_eq!(from_nodes, from_segments);
    }

    #[test]
    fn test_rebuild_from_segments_indented() {
        let code = "def foo():\n    x = 1\n    y = 2\n";
        let source = code.as_bytes();
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();
        let cumsum = NwsCumsum::new(source);

        let func_def = root.child(0).unwrap();
        let block = func_def.child_by_field_name("body").unwrap();
        let mut cursor = block.walk();
        let stmts: Vec<_> = block.children(&mut cursor).collect();
        let named_stmts: Vec<_> = stmts.iter().filter(|n| n.is_named()).copied().collect();

        let ancestors = vec![root, func_def];
        let ast_nodes: Vec<AstNode<'_>> = named_stmts
            .iter()
            .map(|c| AstNode::new(*c, node_nws_size(c, &cumsum), ancestors.clone()))
            .collect();

        let segments: Vec<ByteRange> = ast_nodes
            .iter()
            .map(|n| ByteRange::from_ts_node(&n.node))
            .collect();

        let from_nodes = rebuild_code(&ast_nodes, source);
        let from_segments = rebuild_from_segments(&segments, source);
        assert_eq!(from_nodes, from_segments);
    }
}
