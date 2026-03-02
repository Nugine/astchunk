use crate::byte_range::ByteRange;

/// The set of ASCII whitespace bytes, matching Python's `string.whitespace`:
/// space (0x20), tab (0x09), newline (0x0A), carriage return (0x0D),
/// vertical tab (0x0B), form feed (0x0C).
const fn is_ascii_whitespace(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)
}

/// Cumulative sum of non-whitespace character counts over a byte string.
///
/// Enables O(1) range queries after O(n) preprocessing.
pub struct NwsCumsum {
    /// `cumsum[i]` = number of non-whitespace bytes in `source[0..i]`.
    /// Length = `source.len() + 1`, with `cumsum[0] = 0`.
    cumsum: Vec<u32>,
}

impl NwsCumsum {
    /// Build the cumulative sum from a source byte string.
    #[must_use]
    pub fn new(source: &[u8]) -> Self {
        let mut cumsum = Vec::with_capacity(source.len() + 1);
        cumsum.push(0);
        let mut acc: u32 = 0;
        for &b in source {
            if !is_ascii_whitespace(b) {
                acc += 1;
            }
            cumsum.push(acc);
        }
        Self { cumsum }
    }

    /// Query the non-whitespace character count in `[range.start, range.end)`.
    ///
    /// Runs in O(1).
    #[must_use]
    pub fn get(&self, range: ByteRange) -> u32 {
        self.cumsum[range.end as usize] - self.cumsum[range.start as usize]
    }
}

/// Directly count non-whitespace characters in a string.
///
/// O(n) computation, useful as a verifier.
#[must_use]
pub fn nws_count_direct(code: &str) -> u32 {
    let count = code.bytes().filter(|b| !is_ascii_whitespace(*b)).count();
    u32::try_from(count).expect("non-whitespace count exceeds u32")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn len32(s: &[u8]) -> u32 {
        u32::try_from(s.len()).unwrap()
    }

    #[test]
    fn test_nws_count_basic() {
        let code = "def foo():\n    print('hello world')\n    return 42";
        let source = code.as_bytes();
        let cumsum = NwsCumsum::new(source);

        let full_range = ByteRange::new(0, len32(source));
        let nws_cumsum = cumsum.get(full_range);
        let nws_direct = nws_count_direct(code);

        assert_eq!(nws_cumsum, nws_direct);
    }

    #[test]
    fn test_nws_count_partial() {
        let code = "def foo():\n    print('hello world')\n    return 42";
        let source = code.as_bytes();
        let cumsum = NwsCumsum::new(source);

        // Partial range: first 11 bytes = "def foo():\n"
        // Note: Python test uses ByteRange(0, 11) which includes bytes 0..10
        let partial_range = ByteRange::new(0, 11);
        let partial_nws = cumsum.get(partial_range);

        let partial_code = std::str::from_utf8(&source[..11]).unwrap();
        let partial_direct = nws_count_direct(partial_code);

        assert_eq!(partial_nws, partial_direct);
    }

    #[test]
    fn test_nws_count_empty() {
        let cumsum = NwsCumsum::new(b"");
        assert_eq!(cumsum.get(ByteRange::new(0, 0)), 0);
    }

    #[test]
    fn test_nws_count_all_whitespace() {
        let code = "   \t\n\r";
        let cumsum = NwsCumsum::new(code.as_bytes());
        let full = ByteRange::new(0, len32(code.as_bytes()));
        assert_eq!(cumsum.get(full), 0);
        assert_eq!(nws_count_direct(code), 0);
    }

    #[test]
    fn test_nws_count_no_whitespace() {
        let code = "abc123";
        let cumsum = NwsCumsum::new(code.as_bytes());
        let full = ByteRange::new(0, len32(code.as_bytes()));
        assert_eq!(cumsum.get(full), 6);
        assert_eq!(nws_count_direct(code), 6);
    }
}
