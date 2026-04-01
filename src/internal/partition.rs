use crate::internal::byte_range::ByteRange;
use crate::internal::node::{AstNode, node_nws_size};
use crate::internal::nws::NwsCumsum;
use crate::lang::Language;

/// Parse source code into a tree-sitter tree.
#[must_use]
pub fn parse(language: Language, source: &[u8]) -> tree_sitter::Tree {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&language.ts_language())
        .expect("failed to set language");
    parser.parse(source, None).expect("failed to parse source")
}

/// Collect the children of a tree-sitter node.
fn ts_children<'tree>(node: &tree_sitter::Node<'tree>) -> Vec<tree_sitter::Node<'tree>> {
    let mut cursor = node.walk();
    node.children(&mut cursor).collect()
}

/// Assign the AST tree to windows (tentative chunks of `AstNode`).
///
/// Handles the edge case where the whole tree fits in one window,
/// otherwise delegates to [`assign_nodes_to_windows`].
#[must_use]
pub fn assign_tree_to_windows<'tree>(
    max_chunk_size: u32,
    source: &[u8],
    root_node: tree_sitter::Node<'tree>,
) -> Vec<Vec<AstNode<'tree>>> {
    let cumsum = NwsCumsum::new(source);
    let tree_range = ByteRange::from_ts_node(&root_node);
    let tree_size = cumsum.get(tree_range);

    // If the entire tree fits, return it as one window.
    if tree_size <= max_chunk_size {
        return vec![vec![AstNode::new(root_node, tree_size, vec![])]];
    }

    // Otherwise, recursively assign children to windows.
    let ancestors = vec![root_node];
    let mut windows = Vec::new();
    assign_nodes_to_windows(
        max_chunk_size,
        &ts_children(&root_node),
        &cumsum,
        &ancestors,
        &mut windows,
    );
    windows
}

/// Greedily assign AST nodes to windows based on non-whitespace character count.
///
/// Mirrors Python's `assign_nodes_to_windows` method.
fn assign_nodes_to_windows<'tree>(
    max_chunk_size: u32,
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
        let node_exceeds_limit = node_size > max_chunk_size;

        if (current_window.is_empty() && node_exceeds_limit)
            || (current_window_size + node_size > max_chunk_size)
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

                let children = ts_children(&node);
                let mut child_windows = Vec::new();
                assign_nodes_to_windows(
                    max_chunk_size,
                    &children,
                    cumsum,
                    &child_ancestors,
                    &mut child_windows,
                );

                if !child_windows.is_empty() {
                    // Greedily merge adjacent windows
                    let merged = merge_adjacent_windows(max_chunk_size, child_windows);
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
    max_chunk_size: u32,
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

        if merged_size <= max_chunk_size {
            merged.last_mut().unwrap().extend(window);
        } else {
            merged.push(window);
        }
    }

    merged
}

/// Extend each window by adding overlapping `AstNode`s from adjacent windows.
///
/// For each window, prepends the last `k` nodes from the previous window and
/// appends the first `k` nodes from the next window, where `k = chunk_overlap`.
///
/// Note: overlapping is not constrained by `max_chunk_size`.
#[must_use]
pub fn add_window_overlapping<'tree>(
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
