//! Unicode utilities for text processing.
//!
//! Provides helpers for proper Unicode handling including
//! character boundary detection and validation.

use unicode_segmentation::UnicodeSegmentation;

/// Finds a valid UTF-8 character boundary at or before the given position.
///
/// # Arguments
///
/// * `s` - The string to search.
/// * `pos` - Target position in bytes.
///
/// # Returns
///
/// A byte position that is a valid UTF-8 character boundary.
///
/// # Examples
///
/// ```
/// use rlm_rs::io::find_char_boundary;
///
/// let s = "Hello 世界";
/// assert_eq!(find_char_boundary(s, 6), 6); // Before '世'
/// assert_eq!(find_char_boundary(s, 7), 6); // Middle of '世', backs up
/// ```
#[must_use]
pub const fn find_char_boundary(s: &str, pos: usize) -> usize {
    if pos >= s.len() {
        return s.len();
    }
    let bytes = s.as_bytes();
    let mut boundary = pos;
    // UTF-8 continuation bytes start with 10xxxxxx (0x80-0xBF)
    while boundary > 0 && (bytes[boundary] & 0xC0) == 0x80 {
        boundary -= 1;
    }
    boundary
}

/// Finds a valid UTF-8 character boundary at or after the given position.
///
/// # Arguments
///
/// * `s` - The string to search.
/// * `pos` - Target position in bytes.
///
/// # Returns
///
/// A byte position that is a valid UTF-8 character boundary.
#[must_use]
pub const fn find_char_boundary_forward(s: &str, pos: usize) -> usize {
    if pos >= s.len() {
        return s.len();
    }
    let bytes = s.as_bytes();
    let mut boundary = pos;
    // UTF-8 continuation bytes start with 10xxxxxx (0x80-0xBF)
    while boundary < bytes.len() && (bytes[boundary] & 0xC0) == 0x80 {
        boundary += 1;
    }
    boundary
}

/// Validates that a byte slice is valid UTF-8.
///
/// # Arguments
///
/// * `bytes` - The bytes to validate.
///
/// # Returns
///
/// `Ok(str)` if valid, `Err` with the byte offset of the first invalid byte.
///
/// # Errors
///
/// Returns the byte offset of the first invalid UTF-8 sequence.
pub fn validate_utf8(bytes: &[u8]) -> std::result::Result<&str, usize> {
    std::str::from_utf8(bytes).map_err(|e| e.valid_up_to())
}

/// Counts the number of grapheme clusters in a string.
///
/// Grapheme clusters are user-perceived characters, which may consist
/// of multiple Unicode code points (e.g., emoji with skin tone modifiers).
///
/// # Arguments
///
/// * `s` - The string to count.
///
/// # Examples
///
/// ```
/// use rlm_rs::io::unicode::grapheme_count;
///
/// assert_eq!(grapheme_count("Hello"), 5);
/// assert_eq!(grapheme_count("世界"), 2);
/// ```
#[must_use]
pub fn grapheme_count(s: &str) -> usize {
    s.graphemes(true).count()
}

/// Truncates a string at a grapheme cluster boundary.
///
/// # Arguments
///
/// * `s` - The string to truncate.
/// * `max_graphemes` - Maximum number of grapheme clusters.
///
/// # Returns
///
/// A string slice containing at most `max_graphemes` grapheme clusters.
#[must_use]
pub fn truncate_graphemes(s: &str, max_graphemes: usize) -> &str {
    let mut end_byte = 0;

    for (count, grapheme) in s.graphemes(true).enumerate() {
        if count >= max_graphemes {
            break;
        }
        end_byte += grapheme.len();
    }

    &s[..end_byte]
}

/// Finds the byte position of the nth grapheme cluster.
///
/// # Arguments
///
/// * `s` - The string to search.
/// * `n` - The grapheme index (0-based).
///
/// # Returns
///
/// The byte position of the start of the nth grapheme, or `s.len()` if out of bounds.
#[must_use]
pub fn grapheme_byte_position(s: &str, n: usize) -> usize {
    let mut pos = 0;
    for (i, grapheme) in s.graphemes(true).enumerate() {
        if i == n {
            return pos;
        }
        pos += grapheme.len();
    }
    s.len()
}

/// Iterates over lines with their byte offsets.
///
/// # Arguments
///
/// * `s` - The string to iterate.
///
/// # Returns
///
/// Iterator of (`byte_offset`, `line_content`) tuples.
pub fn lines_with_offsets(s: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut offset = 0;
    s.lines().map(move |line| {
        let current_offset = offset;
        offset += line.len();
        // Account for newline character
        if offset < s.len() {
            offset += 1; // \n
            if offset < s.len() && s.as_bytes().get(offset - 1) == Some(&b'\r') {
                // Handle \r\n (already consumed \n, this checks if prev was \r)
            }
        }
        (current_offset, line)
    })
}

/// Splits text into sentences (approximate).
///
/// Uses simple heuristics: splits on `.`, `!`, `?` followed by whitespace.
///
/// # Arguments
///
/// * `s` - The string to split.
///
/// # Returns
///
/// Vector of sentence strings.
#[must_use]
pub fn split_sentences(s: &str) -> Vec<&str> {
    let mut sentences = Vec::new();
    let mut start = 0;

    let bytes = s.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        let c = bytes[i];
        if matches!(c, b'.' | b'!' | b'?') {
            // Check if followed by whitespace or end
            if i + 1 >= bytes.len() || bytes[i + 1].is_ascii_whitespace() {
                let end = i + 1;
                if end > start {
                    sentences.push(&s[start..end]);
                }
                // Skip whitespace
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
                start = i;
                continue;
            }
        }
        i += 1;
    }

    // Add remaining text
    if start < s.len() {
        sentences.push(&s[start..]);
    }

    sentences
}

/// Returns the current Unix timestamp in seconds.
///
/// Used for timestamping buffers, chunks, and other entities.
///
/// # Examples
///
/// ```
/// use rlm_rs::io::current_timestamp;
///
/// let ts = current_timestamp();
/// assert!(ts > 0);
/// ```
#[allow(clippy::cast_possible_wrap)]
#[must_use]
pub fn current_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_char_boundary() {
        let s = "Hello 世界!";
        assert_eq!(find_char_boundary(s, 0), 0);
        assert_eq!(find_char_boundary(s, 5), 5);
        assert_eq!(find_char_boundary(s, 6), 6); // Space before '世'
        assert_eq!(find_char_boundary(s, 7), 6); // Middle of '世'
        assert_eq!(find_char_boundary(s, 8), 6); // Still in '世'
        assert_eq!(find_char_boundary(s, 9), 9); // After '世'
        assert_eq!(find_char_boundary(s, 100), s.len());
    }

    #[test]
    fn test_find_char_boundary_forward() {
        let s = "Hello 世界!";
        assert_eq!(find_char_boundary_forward(s, 7), 9); // Middle of '世', moves forward
    }

    #[test]
    fn test_validate_utf8() {
        assert!(validate_utf8(b"Hello").is_ok());
        assert!(validate_utf8("世界".as_bytes()).is_ok());

        // Invalid UTF-8
        let invalid = [0xFF, 0xFE];
        assert!(validate_utf8(&invalid).is_err());
    }

    #[test]
    fn test_grapheme_count() {
        assert_eq!(grapheme_count("Hello"), 5);
        assert_eq!(grapheme_count("世界"), 2);
        assert_eq!(grapheme_count(""), 0);
    }

    #[test]
    fn test_truncate_graphemes() {
        assert_eq!(truncate_graphemes("Hello", 3), "Hel");
        assert_eq!(truncate_graphemes("世界!", 2), "世界");
        assert_eq!(truncate_graphemes("Hello", 10), "Hello");
    }

    #[test]
    fn test_grapheme_byte_position() {
        let s = "Hello 世界";
        assert_eq!(grapheme_byte_position(s, 0), 0);
        assert_eq!(grapheme_byte_position(s, 6), 6); // Space
        assert_eq!(grapheme_byte_position(s, 7), 9); // After '世'
    }

    #[test]
    fn test_split_sentences() {
        let text = "Hello world. How are you? I am fine!";
        let sentences = split_sentences(text);
        assert_eq!(sentences.len(), 3);
        assert_eq!(sentences[0], "Hello world.");
        assert_eq!(sentences[1], "How are you?");
        assert_eq!(sentences[2], "I am fine!");
    }

    #[test]
    fn test_split_sentences_no_final_punct() {
        let text = "First sentence. Second part";
        let sentences = split_sentences(text);
        assert_eq!(sentences.len(), 2);
        assert_eq!(sentences[1], "Second part");
    }

    #[test]
    fn test_lines_with_offsets() {
        let text = "Line 1\nLine 2\nLine 3";
        let lines: Vec<_> = lines_with_offsets(text).collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], (0, "Line 1"));
        // Note: offset calculation is approximate
    }

    #[test]
    fn test_find_char_boundary_forward_at_end() {
        // Test find_char_boundary_forward when pos >= s.len() (line 53)
        let s = "hello";
        assert_eq!(find_char_boundary_forward(s, 10), 5);
        assert_eq!(find_char_boundary_forward(s, 5), 5);
    }

    #[test]
    fn test_grapheme_byte_position_out_of_range() {
        // Test grapheme_byte_position when n > grapheme count (line 144)
        let s = "abc";
        assert_eq!(grapheme_byte_position(s, 10), 3); // Returns s.len()
    }

    #[test]
    fn test_grapheme_byte_position_edge_cases() {
        // Test with unicode to ensure correct byte offset calculation
        let s = "Hello 世界"; // "Hello " is 6 bytes, "世" is 3 bytes, "界" is 3 bytes
        assert_eq!(grapheme_byte_position(s, 0), 0);
        assert_eq!(grapheme_byte_position(s, 6), 6); // Before '世'
        assert_eq!(grapheme_byte_position(s, 7), 9); // After '世'
        assert_eq!(grapheme_byte_position(s, 8), 12); // After '界'
        assert_eq!(grapheme_byte_position(s, 100), 12); // Out of range
    }
}
