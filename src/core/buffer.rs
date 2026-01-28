//! Buffer management for RLM-RS.
//!
//! Buffers represent text content loaded into the RLM system, typically
//! from files or direct input. Each buffer can be chunked for processing.

use crate::io::{current_timestamp, find_char_boundary};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Represents a text buffer in the RLM system.
///
/// Buffers are the primary unit of content storage, containing text
/// that can be chunked and processed by the RLM workflow.
///
/// # Examples
///
/// ```
/// use rlm_rs::core::Buffer;
///
/// let buffer = Buffer::from_content("Hello, world!".to_string());
/// assert_eq!(buffer.size(), 13);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Buffer {
    /// Unique identifier (assigned by storage layer).
    pub id: Option<i64>,

    /// Optional name for the buffer.
    pub name: Option<String>,

    /// Source file path (if loaded from file).
    pub source: Option<PathBuf>,

    /// Buffer content.
    pub content: String,

    /// Buffer metadata.
    pub metadata: BufferMetadata,
}

/// Metadata associated with a buffer.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BufferMetadata {
    /// Content type or file extension (e.g., "txt", "md", "json").
    pub content_type: Option<String>,

    /// Unix timestamp when buffer was created.
    pub created_at: i64,

    /// Unix timestamp when buffer was last modified.
    pub updated_at: i64,

    /// Total size in bytes.
    pub size: usize,

    /// Line count (computed on demand).
    pub line_count: Option<usize>,

    /// Number of chunks (set after chunking).
    pub chunk_count: Option<usize>,

    /// SHA-256 hash of content (for deduplication).
    pub content_hash: Option<String>,
}

impl Buffer {
    /// Creates a new buffer from content string.
    ///
    /// # Arguments
    ///
    /// * `content` - The text content for the buffer.
    ///
    /// # Examples
    ///
    /// ```
    /// use rlm_rs::core::Buffer;
    ///
    /// let buffer = Buffer::from_content("Some text content".to_string());
    /// assert!(buffer.id.is_none());
    /// assert!(buffer.source.is_none());
    /// ```
    #[must_use]
    pub fn from_content(content: String) -> Self {
        let size = content.len();
        let now = current_timestamp();
        Self {
            id: None,
            name: None,
            source: None,
            content,
            metadata: BufferMetadata {
                size,
                created_at: now,
                updated_at: now,
                ..Default::default()
            },
        }
    }

    /// Creates a new buffer from a file path and content.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the source file.
    /// * `content` - The text content read from the file.
    ///
    /// # Examples
    ///
    /// ```
    /// use rlm_rs::core::Buffer;
    /// use std::path::PathBuf;
    ///
    /// let buffer = Buffer::from_file(
    ///     PathBuf::from("example.txt"),
    ///     "File content".to_string(),
    /// );
    /// assert!(buffer.source.is_some());
    /// ```
    #[must_use]
    pub fn from_file(path: PathBuf, content: String) -> Self {
        let size = content.len();
        let content_type = infer_content_type(&path);
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(ToString::to_string);
        let now = current_timestamp();

        Self {
            id: None,
            name,
            source: Some(path),
            content,
            metadata: BufferMetadata {
                content_type,
                size,
                created_at: now,
                updated_at: now,
                ..Default::default()
            },
        }
    }

    /// Creates a new named buffer from content.
    ///
    /// # Arguments
    ///
    /// * `name` - Name for the buffer.
    /// * `content` - The text content for the buffer.
    #[must_use]
    pub fn from_named(name: String, content: String) -> Self {
        let mut buffer = Self::from_content(content);
        buffer.name = Some(name);
        buffer
    }

    /// Returns the size of the buffer in bytes.
    #[must_use]
    pub const fn size(&self) -> usize {
        self.content.len()
    }

    /// Returns the line count of the buffer.
    ///
    /// This is computed on first call and cached.
    pub fn line_count(&mut self) -> usize {
        if let Some(count) = self.metadata.line_count {
            return count;
        }
        let count = self.content.lines().count();
        self.metadata.line_count = Some(count);
        count
    }

    /// Returns a slice of the buffer content.
    ///
    /// # Arguments
    ///
    /// * `start` - Start byte offset.
    /// * `end` - End byte offset.
    ///
    /// # Returns
    ///
    /// The content slice, or `None` if offsets are invalid.
    #[must_use]
    pub fn slice(&self, start: usize, end: usize) -> Option<&str> {
        if start <= end && end <= self.content.len() {
            self.content.get(start..end)
        } else {
            None
        }
    }

    /// Returns a peek of the buffer content from the beginning.
    ///
    /// # Arguments
    ///
    /// * `len` - Maximum number of bytes to return.
    #[must_use]
    pub fn peek(&self, len: usize) -> &str {
        let end = len.min(self.content.len());
        // Find valid UTF-8 boundary
        let end = find_char_boundary(&self.content, end);
        &self.content[..end]
    }

    /// Returns a peek of the buffer content from the end.
    ///
    /// # Arguments
    ///
    /// * `len` - Maximum number of bytes to return.
    #[must_use]
    pub fn peek_end(&self, len: usize) -> &str {
        let start = self.content.len().saturating_sub(len);
        // Find valid UTF-8 boundary
        let start = find_char_boundary(&self.content, start);
        &self.content[start..]
    }

    /// Checks if the buffer is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Returns the display name for this buffer.
    #[must_use]
    pub fn display_name(&self) -> String {
        if let Some(ref name) = self.name {
            return name.clone();
        }
        if let Some(ref path) = self.source
            && let Some(name) = path.file_name()
            && let Some(s) = name.to_str()
        {
            return s.to_string();
        }
        if let Some(id) = self.id {
            return format!("buffer-{id}");
        }
        "unnamed".to_string()
    }

    /// Sets the chunk count after chunking.
    pub fn set_chunk_count(&mut self, count: usize) {
        self.metadata.chunk_count = Some(count);
        self.metadata.updated_at = current_timestamp();
    }

    /// Computes and sets the content hash.
    pub fn compute_hash(&mut self) {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        self.content.hash(&mut hasher);
        self.metadata.content_hash = Some(format!("{:016x}", hasher.finish()));
    }
}

/// Infers content type from file extension.
fn infer_content_type(path: &std::path::Path) -> Option<String> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_lowercase)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_from_content() {
        let buffer = Buffer::from_content("Hello, world!".to_string());
        assert!(buffer.id.is_none());
        assert!(buffer.source.is_none());
        assert_eq!(buffer.size(), 13);
        assert!(!buffer.is_empty());
    }

    #[test]
    fn test_buffer_from_file() {
        let buffer = Buffer::from_file(PathBuf::from("test.txt"), "content".to_string());
        assert_eq!(buffer.source, Some(PathBuf::from("test.txt")));
        assert_eq!(buffer.metadata.content_type, Some("txt".to_string()));
        assert_eq!(buffer.name, Some("test.txt".to_string()));
    }

    #[test]
    fn test_buffer_from_named() {
        let buffer = Buffer::from_named("my-buffer".to_string(), "content".to_string());
        assert_eq!(buffer.name, Some("my-buffer".to_string()));
    }

    #[test]
    fn test_buffer_slice() {
        let buffer = Buffer::from_content("Hello, world!".to_string());
        assert_eq!(buffer.slice(0, 5), Some("Hello"));
        assert_eq!(buffer.slice(7, 12), Some("world"));
        assert_eq!(buffer.slice(0, 100), None); // Out of bounds
        assert_eq!(buffer.slice(10, 5), None); // Invalid range
    }

    #[test]
    fn test_buffer_peek() {
        let buffer = Buffer::from_content("Hello, world!".to_string());
        assert_eq!(buffer.peek(5), "Hello");
        assert_eq!(buffer.peek(100), "Hello, world!"); // Clamped
    }

    #[test]
    fn test_buffer_peek_end() {
        let buffer = Buffer::from_content("Hello, world!".to_string());
        assert_eq!(buffer.peek_end(6), "world!");
        assert_eq!(buffer.peek_end(100), "Hello, world!"); // Clamped
    }

    #[test]
    fn test_buffer_line_count() {
        let mut buffer = Buffer::from_content("line1\nline2\nline3".to_string());
        assert_eq!(buffer.line_count(), 3);
        // Second call uses cached value
        assert_eq!(buffer.line_count(), 3);
        assert_eq!(buffer.metadata.line_count, Some(3));
    }

    #[test]
    fn test_buffer_display_name() {
        let buffer1 = Buffer::from_named("named".to_string(), String::new());
        assert_eq!(buffer1.display_name(), "named");

        let buffer2 = Buffer::from_file(PathBuf::from("/path/to/file.txt"), String::new());
        assert_eq!(buffer2.display_name(), "file.txt");

        let mut buffer3 = Buffer::from_content(String::new());
        buffer3.id = Some(42);
        assert_eq!(buffer3.display_name(), "buffer-42");

        let buffer4 = Buffer::from_content(String::new());
        assert_eq!(buffer4.display_name(), "unnamed");
    }

    #[test]
    fn test_buffer_display_name_source_without_name() {
        // Test display_name when buffer has source path but no name (lines 232-234)
        let mut buffer = Buffer::from_content(String::new());
        buffer.source = Some(PathBuf::from("/some/path/to/document.md"));
        // name is None, source is Some - should extract filename from path
        assert_eq!(buffer.display_name(), "document.md");
    }

    #[test]
    fn test_buffer_hash() {
        let mut buffer = Buffer::from_content("Hello".to_string());
        buffer.compute_hash();
        assert!(buffer.metadata.content_hash.is_some());

        let mut buffer2 = Buffer::from_content("Hello".to_string());
        buffer2.compute_hash();
        assert_eq!(buffer.metadata.content_hash, buffer2.metadata.content_hash);
    }

    #[test]
    fn test_buffer_empty() {
        let buffer = Buffer::from_content(String::new());
        assert!(buffer.is_empty());
        assert_eq!(buffer.size(), 0);
    }

    #[test]
    fn test_buffer_serialization() {
        let buffer = Buffer::from_named("test".to_string(), "content".to_string());
        let json = serde_json::to_string(&buffer);
        assert!(json.is_ok());

        let deserialized: Result<Buffer, _> = serde_json::from_str(&json.unwrap());
        assert!(deserialized.is_ok());
        assert_eq!(deserialized.unwrap().content, "content");
    }
}
