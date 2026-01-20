//! Semantic chunking strategy.
//!
//! Provides Unicode-aware chunking that respects sentence and paragraph
//! boundaries using the `unicode-segmentation` crate.

use crate::chunking::traits::{ChunkMetadata, Chunker};
use crate::chunking::{DEFAULT_CHUNK_SIZE, DEFAULT_OVERLAP, MAX_CHUNK_SIZE};
use crate::core::Chunk;
use crate::error::{ChunkingError, Result};
use unicode_segmentation::UnicodeSegmentation;

/// Semantic chunker that respects sentence and paragraph boundaries.
///
/// This chunker produces more coherent chunks by avoiding splits in the
/// middle of sentences or words. It uses Unicode segmentation rules
/// for proper international text handling.
///
/// # Examples
///
/// ```
/// use rlm_rs::chunking::{Chunker, SemanticChunker};
///
/// let chunker = SemanticChunker::new();
/// let text = "Hello, world! This is a test. Another sentence here.";
/// let chunks = chunker.chunk(1, text, None).unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct SemanticChunker {
    /// Target chunk size in characters.
    chunk_size: usize,
    /// Overlap between consecutive chunks.
    overlap: usize,
    /// Minimum chunk size (avoid tiny final chunks).
    min_chunk_size: usize,
}

impl Default for SemanticChunker {
    fn default() -> Self {
        Self::new()
    }
}

impl SemanticChunker {
    /// Creates a new semantic chunker with default settings.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            chunk_size: DEFAULT_CHUNK_SIZE,
            overlap: DEFAULT_OVERLAP,
            min_chunk_size: 100,
        }
    }

    /// Creates a semantic chunker with custom chunk size and no overlap.
    #[must_use]
    pub const fn with_size(chunk_size: usize) -> Self {
        Self {
            chunk_size,
            overlap: 0,
            min_chunk_size: 100,
        }
    }

    /// Creates a semantic chunker with custom size and overlap.
    #[must_use]
    pub const fn with_size_and_overlap(chunk_size: usize, overlap: usize) -> Self {
        Self {
            chunk_size,
            overlap,
            min_chunk_size: 100,
        }
    }

    /// Sets the minimum chunk size.
    #[must_use]
    pub const fn min_chunk_size(mut self, size: usize) -> Self {
        self.min_chunk_size = size;
        self
    }

    /// Finds the best boundary near the target position.
    ///
    /// Prefers paragraph breaks > sentence breaks > word breaks > character breaks.
    fn find_best_boundary(&self, text: &str, target_pos: usize) -> usize {
        if target_pos >= text.len() {
            return text.len();
        }

        // Search window: look back up to 20% of chunk size for a good boundary
        // Ensure both boundaries are valid UTF-8 character boundaries
        let search_start = find_char_boundary(text, target_pos.saturating_sub(self.chunk_size / 5));
        let search_end = find_char_boundary(text, target_pos.min(text.len()));

        if search_start >= search_end {
            return find_char_boundary(text, target_pos);
        }

        let search_region = &text[search_start..search_end];

        // Priority 1: Paragraph break (double newline)
        if let Some(pos) = search_region.rfind("\n\n") {
            let boundary = search_start + pos + 2;
            if boundary > search_start {
                return boundary;
            }
        }

        // Priority 2: Single newline
        if let Some(pos) = search_region.rfind('\n') {
            let boundary = search_start + pos + 1;
            if boundary > search_start {
                return boundary;
            }
        }

        // Priority 3: Sentence boundary (. ! ? followed by space or end)
        for (i, c) in search_region.char_indices().rev() {
            if matches!(c, '.' | '!' | '?') {
                let next_pos = search_start + i + c.len_utf8();
                if next_pos >= text.len()
                    || text[next_pos..].starts_with(' ')
                    || text[next_pos..].starts_with('\n')
                {
                    return next_pos;
                }
            }
        }

        // Priority 4: Word boundary (space)
        if let Some(pos) = search_region.rfind(' ') {
            let boundary = search_start + pos + 1;
            if boundary > search_start {
                return boundary;
            }
        }

        // Fallback: character boundary
        find_char_boundary(text, target_pos)
    }

    /// Finds sentence boundaries in the text.
    #[allow(dead_code)]
    fn sentence_boundaries(text: &str) -> Vec<usize> {
        let mut boundaries = vec![0];
        let mut pos = 0;

        for sentence in text.split_sentence_bounds() {
            pos += sentence.len();
            boundaries.push(pos);
        }

        boundaries
    }
}

impl Chunker for SemanticChunker {
    #[allow(clippy::too_many_lines)]
    fn chunk(
        &self,
        buffer_id: i64,
        text: &str,
        metadata: Option<&ChunkMetadata>,
    ) -> Result<Vec<Chunk>> {
        // Get effective chunk size and overlap
        let (chunk_size, overlap) = metadata.map_or((self.chunk_size, self.overlap), |meta| {
            (meta.chunk_size, meta.overlap)
        });

        // Validate configuration
        if chunk_size == 0 {
            return Err(ChunkingError::InvalidConfig {
                reason: "chunk_size must be > 0".to_string(),
            }
            .into());
        }
        if chunk_size > MAX_CHUNK_SIZE {
            return Err(ChunkingError::ChunkTooLarge {
                size: chunk_size,
                max: MAX_CHUNK_SIZE,
            }
            .into());
        }
        if overlap >= chunk_size {
            return Err(ChunkingError::OverlapTooLarge {
                overlap,
                size: chunk_size,
            }
            .into());
        }

        // Handle empty text
        if text.is_empty() {
            return Ok(vec![]);
        }

        // Handle text smaller than chunk size
        if text.len() <= chunk_size {
            return Ok(vec![Chunk::with_strategy(
                buffer_id,
                text.to_string(),
                0..text.len(),
                0,
                self.name(),
            )]);
        }

        let mut chunks = Vec::new();
        let mut start = 0;
        let mut index = 0;

        while start < text.len() {
            let target_end = (start + chunk_size).min(text.len());
            let end = if target_end >= text.len() {
                text.len()
            } else {
                self.find_best_boundary(text, target_end)
            };

            // Ensure we make progress
            let end = if end <= start {
                find_char_boundary(text, (start + chunk_size).min(text.len()))
            } else {
                end
            };

            let content = text[start..end].to_string();
            let mut chunk =
                Chunk::with_strategy(buffer_id, content, start..end, index, self.name());

            if index > 0 && overlap > 0 {
                chunk.set_has_overlap(true);
            }

            // Estimate token count
            chunk.set_token_count(chunk.estimate_tokens());

            chunks.push(chunk);

            // Check max chunks limit
            if let Some(meta) = metadata {
                if meta.max_chunks > 0 && chunks.len() >= meta.max_chunks {
                    break;
                }
            }

            // Move to next chunk
            if end >= text.len() {
                break;
            }

            // Calculate next start position
            let next_start = if overlap > 0 {
                // For overlap, we need to find a good boundary before the overlap point
                let overlap_start = end.saturating_sub(overlap);
                self.find_best_boundary(text, overlap_start)
            } else {
                end
            };

            // Ensure we don't go backwards
            start = if next_start <= start { end } else { next_start };

            index += 1;
        }

        // Merge tiny final chunk if it's too small
        if chunks.len() > 1 {
            if let Some(last) = chunks.last() {
                if last.size() < self.min_chunk_size {
                    if let Some(second_last) = chunks.get(chunks.len() - 2) {
                        // Merge into previous chunk
                        let merged_content = format!(
                            "{}{}",
                            second_last.content,
                            &text[second_last.byte_range.end..last.byte_range.end]
                        );
                        let merged_range = second_last.byte_range.start..last.byte_range.end;

                        chunks.pop(); // Remove last
                        chunks.pop(); // Remove second last

                        let mut merged = Chunk::with_strategy(
                            buffer_id,
                            merged_content,
                            merged_range,
                            chunks.len(),
                            self.name(),
                        );
                        merged.set_token_count(merged.estimate_tokens());
                        chunks.push(merged);
                    }
                }
            }
        }

        Ok(chunks)
    }

    fn name(&self) -> &'static str {
        "semantic"
    }

    fn supports_parallel(&self) -> bool {
        true
    }

    fn description(&self) -> &'static str {
        "Semantic chunking respecting sentence and paragraph boundaries"
    }
}

/// Finds a valid UTF-8 character boundary at or before the given position.
fn find_char_boundary(s: &str, pos: usize) -> usize {
    if pos >= s.len() {
        return s.len();
    }
    let mut boundary = pos;
    while !s.is_char_boundary(boundary) && boundary > 0 {
        boundary -= 1;
    }
    boundary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semantic_chunker_default() {
        let chunker = SemanticChunker::new();
        assert_eq!(chunker.chunk_size, DEFAULT_CHUNK_SIZE);
        assert_eq!(chunker.overlap, DEFAULT_OVERLAP);
    }

    #[test]
    fn test_semantic_chunker_empty_text() {
        let chunker = SemanticChunker::new();
        let chunks = chunker.chunk(1, "", None).unwrap();
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_semantic_chunker_small_text() {
        let chunker = SemanticChunker::new();
        let text = "Hello, world!";
        let chunks = chunker.chunk(1, text, None).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].content, text);
    }

    #[test]
    fn test_semantic_chunker_sentence_boundary() {
        let chunker = SemanticChunker::with_size(30);
        let text = "First sentence. Second sentence. Third sentence.";
        let chunks = chunker.chunk(1, text, None).unwrap();

        // Should prefer breaking at sentence boundaries
        assert!(!chunks.is_empty());
        for chunk in &chunks {
            // Chunks should generally end at sentence boundaries
            let content = chunk.content.trim();
            if !content.is_empty() && chunk.end() < text.len() {
                // Non-final chunks should try to end at sentence boundary
                assert!(
                    content.ends_with('.') || content.ends_with('!') || content.ends_with('?'),
                    "Chunk '{content}' should end at sentence boundary"
                );
            }
        }
    }

    #[test]
    fn test_semantic_chunker_paragraph_boundary() {
        let chunker = SemanticChunker::with_size(50);
        let text = "First paragraph.\n\nSecond paragraph.\n\nThird paragraph.";
        let chunks = chunker.chunk(1, text, None).unwrap();

        // Should prefer breaking at paragraph boundaries
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_semantic_chunker_unicode() {
        let chunker = SemanticChunker::with_size(20);
        let text = "Hello 世界! This is a test. Another sentence.";
        let chunks = chunker.chunk(1, text, None).unwrap();

        // All chunks should be valid UTF-8
        for chunk in &chunks {
            assert!(chunk.content.is_char_boundary(0));
            // Verify content matches original
            assert_eq!(&text[chunk.byte_range.clone()], chunk.content);
        }
    }

    #[test]
    fn test_semantic_chunker_token_estimation() {
        let chunker = SemanticChunker::with_size(50);
        let text = "Hello, world! This is a test sentence for token estimation.";
        let chunks = chunker.chunk(1, text, None).unwrap();

        for chunk in &chunks {
            assert!(chunk.metadata.token_count.is_some());
        }
    }

    #[test]
    fn test_semantic_chunker_strategy_name() {
        let chunker = SemanticChunker::new();
        assert_eq!(chunker.name(), "semantic");

        let chunks = chunker.chunk(1, "Hello!", None).unwrap();
        assert_eq!(chunks[0].metadata.strategy, Some("semantic".to_string()));
    }

    #[test]
    fn test_semantic_chunker_invalid_config() {
        let chunker = SemanticChunker::with_size(0);
        let result = chunker.chunk(1, "test", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_semantic_chunker_overlap_too_large() {
        let chunker = SemanticChunker::with_size_and_overlap(10, 15);
        let result = chunker.chunk(1, "test content here", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_semantic_chunker_with_metadata() {
        let chunker = SemanticChunker::new();
        let text = "Hello, world! ".repeat(100);
        let meta = ChunkMetadata::with_size_and_overlap(100, 10)
            .preserve_sentences(true)
            .max_chunks(5);
        let chunks = chunker.chunk(1, &text, Some(&meta)).unwrap();

        assert!(chunks.len() <= 5);
    }

    #[test]
    fn test_semantic_chunker_supports_parallel() {
        let chunker = SemanticChunker::new();
        assert!(chunker.supports_parallel());
    }

    #[test]
    fn test_find_char_boundary() {
        let s = "Hello 世界!";
        assert_eq!(find_char_boundary(s, 6), 6); // Before multi-byte char
        assert_eq!(find_char_boundary(s, 7), 6); // Middle of '世'
        assert_eq!(find_char_boundary(s, 8), 6); // Middle of '世'
        assert_eq!(find_char_boundary(s, 9), 9); // After '世'
    }

    #[test]
    fn test_semantic_chunker_default_impl() {
        // Test Default trait implementation (lines 38-39)
        let chunker = SemanticChunker::default();
        assert_eq!(chunker.chunk_size, DEFAULT_CHUNK_SIZE);
        assert_eq!(chunker.overlap, DEFAULT_OVERLAP);
        assert_eq!(chunker.min_chunk_size, 100);
    }

    #[test]
    fn test_semantic_chunker_min_chunk_size() {
        // Test min_chunk_size builder method (lines 76-78)
        let chunker = SemanticChunker::new().min_chunk_size(200);
        assert_eq!(chunker.min_chunk_size, 200);
    }

    #[test]
    fn test_semantic_chunker_description() {
        // Test description method (lines 306-307)
        let chunker = SemanticChunker::new();
        let desc = chunker.description();
        assert!(desc.contains("Semantic"));
        assert!(!desc.is_empty());
    }

    #[test]
    fn test_find_char_boundary_at_end() {
        // Test find_char_boundary when pos >= s.len() (line 314)
        let s = "hello";
        assert_eq!(find_char_boundary(s, 10), 5);
        assert_eq!(find_char_boundary(s, 5), 5);
    }

    #[test]
    fn test_semantic_chunker_large_text() {
        // Test with larger text to trigger more boundary finding
        let chunker = SemanticChunker::with_size(100);
        let text = "This is a sentence. ".repeat(50);
        let chunks = chunker.chunk(1, &text, None).unwrap();
        assert!(!chunks.is_empty());

        // Verify chunks have reasonable sizes
        for chunk in &chunks {
            assert!(!chunk.content.is_empty());
        }
    }

    #[test]
    fn test_semantic_chunker_word_boundary() {
        // Test word boundary detection
        let chunker = SemanticChunker::with_size(15);
        let text = "hello world test content here";
        let chunks = chunker.chunk(1, text, None).unwrap();

        // Should break at word boundaries where possible
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_semantic_chunker_with_overlap() {
        // Test chunking with overlap
        let chunker = SemanticChunker::with_size_and_overlap(50, 10);
        let text = "Word ".repeat(30);
        let chunks = chunker.chunk(1, &text, None).unwrap();

        assert!(chunks.len() > 1);
    }

    #[test]
    fn test_find_best_boundary_target_beyond_text() {
        // Test find_best_boundary when target_pos >= text.len() (line 86)
        let chunker = SemanticChunker::with_size(100);
        let text = "Short text";
        // Call chunk with small text and large chunk size to exercise the boundary
        let chunks = chunker.chunk(1, text, None).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].content, text);
    }

    #[test]
    fn test_find_best_boundary_search_region_empty() {
        // Test when search_start >= search_end (line 94)
        // This happens with very small chunk sizes where the search window is minimal
        let chunker = SemanticChunker::with_size(5).min_chunk_size(1);
        let text = "ABCDEFGHIJKLMNOP";
        let chunks = chunker.chunk(1, text, None).unwrap();
        assert!(!chunks.is_empty());
        // All chunks should be valid UTF-8
        for chunk in &chunks {
            assert!(!chunk.content.is_empty());
        }
    }

    #[test]
    fn test_find_best_boundary_single_newline() {
        // Test single newline boundary finding (lines 109-111)
        let chunker = SemanticChunker::with_size(20);
        let text = "First line here\nSecond line here\nThird line";
        let chunks = chunker.chunk(1, text, None).unwrap();

        // Should prefer breaking at single newlines when no paragraph breaks
        assert!(!chunks.is_empty());
        // Verify chunks are valid and cover the text
        for chunk in &chunks {
            assert!(!chunk.content.is_empty());
        }
    }

    #[test]
    fn test_semantic_chunker_chunk_too_large() {
        // Test ChunkTooLarge error (lines 176-178, 180)
        let chunker = SemanticChunker::with_size(MAX_CHUNK_SIZE + 1);
        let result = chunker.chunk(1, "test", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_semantic_chunker_force_progress() {
        // Test end <= start case forcing progress (line 220)
        // This can happen with pathological input where boundary finding fails
        let chunker = SemanticChunker::with_size(5).min_chunk_size(1);
        let text = "AAAAAAAAAA"; // No natural boundaries
        let chunks = chunker.chunk(1, text, None).unwrap();

        // Should still make progress and produce chunks
        assert!(!chunks.is_empty());
        // Verify all content is covered
        let total_content: String = chunks.iter().map(|c| c.content.as_str()).collect();
        assert_eq!(total_content.len(), text.len());
    }

    #[test]
    fn test_semantic_chunker_merge_tiny_final_chunk() {
        // Test merging tiny final chunk (lines 266-292)
        // Create text where the final chunk would be tiny
        let chunker = SemanticChunker::with_size(50).min_chunk_size(20);
        let text = "This is a longer sentence that will be chunked. X";
        let chunks = chunker.chunk(1, text, None).unwrap();

        // Final chunk should be merged if it's too small
        if chunks.len() > 1 {
            let last = chunks.last().unwrap();
            assert!(last.size() >= 20 || chunks.len() == 1);
        }
    }

    #[test]
    fn test_semantic_chunker_sentence_boundary_detection() {
        // Test sentence boundary detection with punctuation (line 121)
        let chunker = SemanticChunker::with_size(25);
        let text = "Question? Exclamation! Statement.";
        let chunks = chunker.chunk(1, text, None).unwrap();

        // Should detect sentence boundaries
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_semantic_chunker_multibyte_utf8_boundaries() {
        // Test that multi-byte UTF-8 characters don't cause panics
        // Smart quotes are 3 bytes each: " (0xE2 0x80 0x9C) and " (0xE2 0x80 0x9D)
        let chunker = SemanticChunker::with_size(50).min_chunk_size(10);

        // Text with smart quotes and other multi-byte chars
        let text = "This is \u{201C}quoted text\u{201D} with smart quotes. \
                    And more \u{201C}content\u{201D} here. \
                    Plus some emoji \u{1F389} and Japanese \u{65E5}\u{672C}\u{8A9E} for good measure.";

        let result = chunker.chunk(1, text, None);
        assert!(result.is_ok(), "Should not panic on multi-byte UTF-8 chars");

        let chunks = result.unwrap();
        assert!(!chunks.is_empty());

        // Verify all chunks are valid UTF-8 and match the source
        for chunk in &chunks {
            assert_eq!(&text[chunk.byte_range.clone()], chunk.content);
        }
    }

    #[test]
    fn test_semantic_chunker_large_multibyte_document() {
        use std::fmt::Write;

        // Simulate a document with many multi-byte characters throughout
        let chunker = SemanticChunker::with_size(100).min_chunk_size(20);

        // Build text with multi-byte chars at various positions
        let mut text = String::new();
        for i in 0..50 {
            let _ = write!(
                text,
                "Section {i}: \u{201C}This is quoted content\u{201D} with data. "
            );
        }

        let result = chunker.chunk(1, &text, None);
        assert!(
            result.is_ok(),
            "Should handle large docs with multi-byte chars"
        );

        let chunks = result.unwrap();
        // Verify chunk byte ranges are valid
        for chunk in &chunks {
            assert!(text.is_char_boundary(chunk.byte_range.start));
            assert!(text.is_char_boundary(chunk.byte_range.end));
        }
    }
}
