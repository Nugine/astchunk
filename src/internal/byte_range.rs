/// Byte range `[start, end)` — a half-open interval over byte offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ByteRange {
    /// Inclusive start offset.
    pub start: u32,
    /// Exclusive end offset.
    pub end: u32,
}

impl ByteRange {
    /// Creates a new byte range.
    ///
    /// # Panics
    ///
    /// Panics if `start > end`.
    #[must_use]
    pub fn new(start: u32, end: u32) -> Self {
        assert!(
            start <= end,
            "Invalid ByteRange: start ({start}) > end ({end})"
        );
        Self { start, end }
    }

    /// Creates a byte range from a tree-sitter node's byte offsets.
    #[must_use]
    pub fn from_ts_node(node: &tree_sitter::Node<'_>) -> Self {
        Self::new(to_u32(node.start_byte()), to_u32(node.end_byte()))
    }
}

/// Convert `usize` to `u32`, panicking on overflow.
///
/// Source files are assumed to be smaller than 4 GiB.
#[must_use]
pub fn to_u32(v: usize) -> u32 {
    u32::try_from(v).expect("value exceeds u32")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_byte_range_new() {
        let range = ByteRange::new(5, 10);
        assert_eq!(range.start, 5);
        assert_eq!(range.end, 10);
    }

    #[test]
    #[should_panic(expected = "Invalid ByteRange")]
    fn test_byte_range_invalid() {
        let _ = ByteRange::new(10, 5);
    }
}
