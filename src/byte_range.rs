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

    /// Returns `true` if `self` fully contains `other`.
    #[must_use]
    pub fn contains_range(self, other: Self) -> bool {
        self.start <= other.start && self.end >= other.end
    }

    /// Returns `true` if the two ranges have a non-zero intersection.
    #[must_use]
    pub fn overlaps(self, other: Self) -> bool {
        self.start.max(other.start) < self.end.min(other.end)
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
    fn test_byte_range_contains() {
        let outer = ByteRange::new(0, 10);
        let inner = ByteRange::new(2, 8);
        let partial = ByteRange::new(5, 15);
        let same = ByteRange::new(0, 10);

        assert!(outer.contains_range(inner));
        assert!(!outer.contains_range(partial));
        assert!(outer.contains_range(same));
        assert!(!inner.contains_range(outer));
    }

    #[test]
    fn test_byte_range_overlaps() {
        let a = ByteRange::new(0, 10);
        let b = ByteRange::new(5, 15);
        let c = ByteRange::new(10, 20);
        let d = ByteRange::new(0, 0);

        assert!(a.overlaps(b));
        assert!(b.overlaps(a));
        assert!(!a.overlaps(c)); // [0,10) and [10,20) don't overlap
        assert!(!a.overlaps(d)); // empty range
    }
}
