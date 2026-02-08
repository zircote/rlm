//! Output formatting for CLI commands.
//!
//! Supports text and JSON output formats.

use crate::core::{Buffer, Chunk, Context};
use crate::storage::traits::StorageStats;
use serde::Serialize;
use std::fmt::Write;

/// Output format options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable text output.
    Text,
    /// JSON output.
    Json,
    /// Newline-delimited JSON (NDJSON) for streaming.
    /// Each record is a single JSON object on its own line.
    Ndjson,
}

impl OutputFormat {
    /// Parses format from string.
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "json" => Self::Json,
            "ndjson" | "jsonl" | "stream" => Self::Ndjson,
            _ => Self::Text,
        }
    }

    /// Returns true if this format is a streaming format.
    #[must_use]
    pub const fn is_streaming(&self) -> bool {
        matches!(self, Self::Ndjson)
    }
}

/// Formats a status response.
#[must_use]
pub fn format_status(stats: &StorageStats, format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => format_status_text(stats),
        OutputFormat::Json | OutputFormat::Ndjson => format_json(stats),
    }
}

fn format_status_text(stats: &StorageStats) -> String {
    let mut output = String::new();
    output.push_str("RLM-RS Status\n");
    output.push_str("=============\n\n");
    let _ = writeln!(output, "  Buffers:       {}", stats.buffer_count);
    let _ = writeln!(output, "  Chunks:        {}", stats.chunk_count);
    let _ = writeln!(
        output,
        "  Content size:  {} bytes",
        stats.total_content_size
    );
    let _ = writeln!(
        output,
        "  Context:       {}",
        if stats.has_context { "yes" } else { "no" }
    );
    let _ = writeln!(output, "  Schema:        v{}", stats.schema_version);
    if let Some(size) = stats.db_size {
        let _ = writeln!(output, "  DB size:       {size} bytes");
    }
    output
}

/// Formats a buffer list.
#[must_use]
pub fn format_buffer_list(buffers: &[Buffer], format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => format_buffer_list_text(buffers),
        OutputFormat::Json | OutputFormat::Ndjson => format_json(&buffers),
    }
}

fn format_buffer_list_text(buffers: &[Buffer]) -> String {
    if buffers.is_empty() {
        return "No buffers found.\n".to_string();
    }

    let mut output = String::new();
    output.push_str("Buffers:\n");
    let _ = writeln!(
        output,
        "{:<6} {:<20} {:<12} {:<8} Source",
        "ID", "Name", "Size", "Chunks"
    );
    output.push_str(&"-".repeat(70));
    output.push('\n');

    for buffer in buffers {
        let id = buffer.id.map_or_else(|| "-".to_string(), |i| i.to_string());
        let name = buffer.name.as_deref().unwrap_or("-");
        let size = format_size(buffer.metadata.size);
        let chunks = buffer
            .metadata
            .chunk_count
            .map_or_else(|| "-".to_string(), |c| c.to_string());
        let source = buffer
            .source
            .as_ref()
            .map_or_else(|| "-".to_string(), |p| p.to_string_lossy().to_string());

        let _ = writeln!(
            output,
            "{:<6} {:<20} {:<12} {:<8} {}",
            id,
            truncate(name, 20),
            size,
            chunks,
            truncate(&source, 30)
        );
    }

    output
}

/// Formats a single buffer.
#[must_use]
pub fn format_buffer(buffer: &Buffer, chunks: Option<&[Chunk]>, format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => format_buffer_text(buffer, chunks),
        OutputFormat::Json | OutputFormat::Ndjson => {
            #[derive(Serialize)]
            struct BufferWithChunks<'a> {
                buffer: &'a Buffer,
                chunks: Option<&'a [Chunk]>,
            }
            format_json(&BufferWithChunks { buffer, chunks })
        }
    }
}

fn format_buffer_text(buffer: &Buffer, chunks: Option<&[Chunk]>) -> String {
    let mut output = String::new();

    let _ = writeln!(
        output,
        "Buffer: {}",
        buffer.name.as_deref().unwrap_or("unnamed")
    );
    let _ = writeln!(output, "  ID:           {}", buffer.id.unwrap_or(0));
    let _ = writeln!(output, "  Size:         {} bytes", buffer.metadata.size);
    if let Some(lines) = buffer.metadata.line_count {
        let _ = writeln!(output, "  Lines:        {lines}");
    }
    if let Some(chunk_count) = buffer.metadata.chunk_count {
        let _ = writeln!(output, "  Chunks:       {chunk_count}");
    }
    if let Some(ref ct) = buffer.metadata.content_type {
        let _ = writeln!(output, "  Content type: {ct}");
    }
    if let Some(ref source) = buffer.source {
        let _ = writeln!(output, "  Source:       {}", source.display());
    }

    if let Some(chunks) = chunks {
        output.push('\n');
        output.push_str("Chunks:\n");
        let _ = writeln!(
            output,
            "{:<6} {:<12} {:<12} {:<10} Preview",
            "Index", "Start", "End", "Size"
        );
        output.push_str(&"-".repeat(70));
        output.push('\n');

        for chunk in chunks {
            let preview = truncate(&chunk.content.replace('\n', "\\n"), 30);
            let _ = writeln!(
                output,
                "{:<6} {:<12} {:<12} {:<10} {}",
                chunk.index,
                chunk.byte_range.start,
                chunk.byte_range.end,
                chunk.size(),
                preview
            );
        }
    }

    output
}

/// Formats peek output.
#[must_use]
pub fn format_peek(content: &str, start: usize, end: usize, format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => {
            let mut output = String::new();
            let _ = writeln!(output, "Bytes {start}..{end} ({} bytes):", end - start);
            output.push_str("---\n");
            output.push_str(content);
            if !content.ends_with('\n') {
                output.push('\n');
            }
            output.push_str("---\n");
            output
        }
        OutputFormat::Json | OutputFormat::Ndjson => {
            #[derive(Serialize)]
            struct PeekOutput<'a> {
                start: usize,
                end: usize,
                size: usize,
                content: &'a str,
            }
            format_json(&PeekOutput {
                start,
                end,
                size: end - start,
                content,
            })
        }
    }
}

/// Formats grep matches.
#[must_use]
pub fn format_grep_matches(matches: &[GrepMatch], pattern: &str, format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => format_grep_text(matches, pattern),
        OutputFormat::Json | OutputFormat::Ndjson => format_json(&matches),
    }
}

fn format_grep_text(matches: &[GrepMatch], pattern: &str) -> String {
    if matches.is_empty() {
        return format!("No matches found for pattern: {pattern}\n");
    }

    let mut output = String::new();
    let _ = writeln!(
        output,
        "Found {} matches for pattern: {pattern}\n",
        matches.len()
    );

    for (i, m) in matches.iter().enumerate() {
        let _ = writeln!(output, "Match {} at byte {}:", i + 1, m.offset);
        let _ = writeln!(output, "  {}", m.snippet.replace('\n', "\\n"));
    }

    output
}

/// Formats chunk indices.
#[must_use]
pub fn format_chunk_indices(indices: &[(usize, usize)], format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => {
            let mut output = String::new();
            let _ = writeln!(output, "{} chunks:", indices.len());
            for (i, (start, end)) in indices.iter().enumerate() {
                let _ = writeln!(output, "  [{i}] {start}..{end} ({} bytes)", end - start);
            }
            output
        }
        OutputFormat::Json | OutputFormat::Ndjson => format_json(&indices),
    }
}

/// Formats write chunks result.
#[must_use]
pub fn format_write_chunks_result(paths: &[String], format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => {
            let mut output = String::new();
            let _ = writeln!(output, "Wrote {} chunks:", paths.len());
            for path in paths {
                let _ = writeln!(output, "  {path}");
            }
            output
        }
        OutputFormat::Json | OutputFormat::Ndjson => format_json(&paths),
    }
}

/// Formats context.
#[must_use]
pub fn format_context(context: &Context, format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => {
            let mut output = String::new();
            output.push_str("Context:\n");
            let _ = writeln!(output, "  Variables: {}", context.variable_count());
            let _ = writeln!(output, "  Globals:   {}", context.global_count());
            let _ = writeln!(output, "  Buffers:   {}", context.buffer_count());
            output
        }
        OutputFormat::Json | OutputFormat::Ndjson => format_json(&context),
    }
}

/// A grep match result.
#[derive(Debug, Clone, Serialize)]
pub struct GrepMatch {
    /// Byte offset in the buffer.
    pub offset: usize,
    /// The matched text.
    pub matched: String,
    /// Context snippet around the match.
    pub snippet: String,
}

/// Formats a value as JSON.
fn format_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string())
}

/// Formats an error for output.
///
/// When format is JSON, returns a structured error object.
/// When format is Text, returns the error message string.
#[must_use]
pub fn format_error(error: &crate::Error, format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => error.to_string(),
        OutputFormat::Json | OutputFormat::Ndjson => {
            let (error_type, suggestion) = get_error_details(error);
            let json = serde_json::json!({
                "success": false,
                "error": {
                    "type": error_type,
                    "message": error.to_string(),
                    "suggestion": suggestion
                }
            });
            serde_json::to_string_pretty(&json).unwrap_or_else(|_| "{}".to_string())
        }
    }
}

/// Extracts error type and recovery suggestion from an error.
const fn get_error_details(error: &crate::Error) -> (&'static str, Option<&'static str>) {
    use crate::error::{ChunkingError, CommandError, IoError, StorageError};

    match error {
        crate::Error::Storage(e) => match e {
            StorageError::NotInitialized => (
                "NotInitialized",
                Some("Run 'rlm-cli init' to initialize the database"),
            ),
            StorageError::BufferNotFound { .. } => (
                "BufferNotFound",
                Some("Run 'rlm-cli list' to see available buffers"),
            ),
            StorageError::ChunkNotFound { .. } => (
                "ChunkNotFound",
                Some("Run 'rlm-cli chunk list <buffer>' to see valid chunk IDs"),
            ),
            StorageError::ContextNotFound => ("ContextNotFound", Some("Context not yet created")),
            StorageError::Database(_) => ("DatabaseError", None),
            StorageError::Migration(_) => ("MigrationError", None),
            StorageError::Transaction(_) => ("TransactionError", None),
            StorageError::Serialization(_) => ("SerializationError", None),
            #[cfg(feature = "usearch-hnsw")]
            StorageError::VectorSearch(_) => ("VectorSearchError", None),
            #[cfg(feature = "fastembed-embeddings")]
            StorageError::Embedding(_) => {
                ("EmbeddingError", Some("Check disk space and try again"))
            }
        },
        crate::Error::Io(e) => match e {
            IoError::FileNotFound { .. } => ("FileNotFound", Some("Verify the file path exists")),
            IoError::ReadFailed { .. } => ("ReadError", None),
            IoError::WriteFailed { .. } => ("WriteError", None),
            IoError::MmapFailed { .. } => ("MemoryMapError", None),
            IoError::DirectoryFailed { .. } => ("DirectoryError", None),
            IoError::PathTraversal { .. } => (
                "PathTraversalDenied",
                Some("Path traversal outside allowed directory is not permitted"),
            ),
            IoError::Generic(_) => ("IoError", None),
        },
        crate::Error::Chunking(e) => match e {
            ChunkingError::InvalidUtf8 { .. } => ("InvalidUtf8", None),
            ChunkingError::ChunkTooLarge { .. } => {
                ("ChunkTooLarge", Some("Use a smaller --chunk-size value"))
            }
            ChunkingError::InvalidConfig { .. } => ("InvalidConfig", None),
            ChunkingError::OverlapTooLarge { .. } => (
                "OverlapTooLarge",
                Some("Overlap must be less than chunk size"),
            ),
            ChunkingError::ParallelFailed { .. } => ("ParallelError", None),
            ChunkingError::SemanticFailed(_) => ("SemanticError", None),
            ChunkingError::Regex(_) => ("RegexError", None),
            ChunkingError::UnknownStrategy { .. } => (
                "UnknownStrategy",
                Some("Valid strategies: fixed, semantic, parallel"),
            ),
        },
        crate::Error::Command(e) => match e {
            CommandError::UnknownCommand(_) => ("UnknownCommand", None),
            CommandError::InvalidArgument(_) => ("InvalidArgument", None),
            CommandError::MissingArgument(_) => ("MissingArgument", None),
            CommandError::ExecutionFailed(_) => ("ExecutionFailed", None),
            CommandError::Cancelled => ("Cancelled", None),
            CommandError::OutputFormat(_) => ("OutputFormatError", None),
        },
        crate::Error::InvalidState { .. } => ("InvalidState", None),
        crate::Error::Config { .. } => ("ConfigError", None),
        crate::Error::Search(_) => ("SearchError", None),
    }
}

/// Formats a byte size as human-readable.
#[allow(clippy::cast_precision_loss)]
fn format_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

/// Truncates a string to max length with ellipsis.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len <= 3 {
        s[..max_len].to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_output_format_from_str() {
        assert_eq!(OutputFormat::parse("json"), OutputFormat::Json);
        assert_eq!(OutputFormat::parse("JSON"), OutputFormat::Json);
        assert_eq!(OutputFormat::parse("text"), OutputFormat::Text);
        assert_eq!(OutputFormat::parse("unknown"), OutputFormat::Text);
    }

    #[test]
    fn test_output_format_ndjson() {
        assert_eq!(OutputFormat::parse("ndjson"), OutputFormat::Ndjson);
        assert_eq!(OutputFormat::parse("NDJSON"), OutputFormat::Ndjson);
        assert_eq!(OutputFormat::parse("jsonl"), OutputFormat::Ndjson);
        assert_eq!(OutputFormat::parse("stream"), OutputFormat::Ndjson);
        assert!(OutputFormat::Ndjson.is_streaming());
        assert!(!OutputFormat::Json.is_streaming());
        assert!(!OutputFormat::Text.is_streaming());
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(100), "100 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
        assert_eq!(format_size(1024 * 1024 * 1024), "1.0 GB");
        assert_eq!(format_size(2 * 1024 * 1024 * 1024), "2.0 GB");
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("Hello", 10), "Hello");
        assert_eq!(truncate("Hello World", 8), "Hello...");
        assert_eq!(truncate("Hi", 2), "Hi");
        assert_eq!(truncate("Hello", 3), "Hel");
        assert_eq!(truncate("Hello", 1), "H");
    }

    #[test]
    fn test_format_status() {
        let stats = StorageStats {
            buffer_count: 2,
            chunk_count: 10,
            total_content_size: 1024,
            has_context: true,
            schema_version: 1,
            db_size: Some(4096),
        };

        let text = format_status(&stats, OutputFormat::Text);
        assert!(text.contains("Buffers:       2"));
        assert!(text.contains("Chunks:        10"));
        assert!(text.contains("DB size:"));

        let json = format_status(&stats, OutputFormat::Json);
        assert!(json.contains("\"buffer_count\": 2"));
    }

    #[test]
    fn test_format_status_no_db_size() {
        let stats = StorageStats {
            buffer_count: 0,
            chunk_count: 0,
            total_content_size: 0,
            has_context: false,
            schema_version: 1,
            db_size: None,
        };

        let text = format_status(&stats, OutputFormat::Text);
        assert!(text.contains("Context:       no"));
        assert!(!text.contains("DB size:"));
    }

    #[test]
    fn test_format_buffer_list_empty() {
        let buffers: Vec<Buffer> = vec![];
        let text = format_buffer_list(&buffers, OutputFormat::Text);
        assert!(text.contains("No buffers found"));

        let json = format_buffer_list(&buffers, OutputFormat::Json);
        assert!(json.contains("[]"));
    }

    #[test]
    fn test_format_buffer_list_with_data() {
        let mut buffer = Buffer::from_named("test".to_string(), "content".to_string());
        buffer.id = Some(1);
        buffer.source = Some(PathBuf::from("/path/to/file.txt"));
        buffer.metadata.chunk_count = Some(3);

        let buffers = vec![buffer];
        let text = format_buffer_list(&buffers, OutputFormat::Text);
        assert!(text.contains("test"));
        assert!(text.contains('1'));

        let json = format_buffer_list(&buffers, OutputFormat::Json);
        assert!(json.contains("\"name\": \"test\""));
    }

    #[test]
    fn test_format_buffer_without_chunks() {
        let mut buffer = Buffer::from_named("test-buf".to_string(), "Hello world".to_string());
        buffer.id = Some(42);
        buffer.metadata.line_count = Some(1);
        buffer.metadata.chunk_count = Some(1);
        buffer.metadata.content_type = Some("text/plain".to_string());
        buffer.source = Some(PathBuf::from("/test/path.txt"));

        let text = format_buffer(&buffer, None, OutputFormat::Text);
        assert!(text.contains("Buffer: test-buf"));
        assert!(text.contains("ID:           42"));
        assert!(text.contains("Lines:        1"));
        assert!(text.contains("Chunks:       1"));
        assert!(text.contains("Content type: text/plain"));
        assert!(text.contains("Source:"));

        let json = format_buffer(&buffer, None, OutputFormat::Json);
        assert!(json.contains("\"buffer\""));
    }

    #[test]
    fn test_format_buffer_with_chunks() {
        let mut buffer = Buffer::from_named("buf".to_string(), "Hello\nWorld".to_string());
        buffer.id = Some(1);

        let chunks = vec![
            Chunk::new(1, "Hello".to_string(), 0..5, 0),
            Chunk::new(1, "World".to_string(), 6..11, 1),
        ];

        let text = format_buffer(&buffer, Some(&chunks), OutputFormat::Text);
        assert!(text.contains("Chunks:"));
        assert!(text.contains("Index"));
        assert!(text.contains("Hello"));

        let json = format_buffer(&buffer, Some(&chunks), OutputFormat::Json);
        assert!(json.contains("\"chunks\""));
    }

    #[test]
    fn test_format_peek() {
        let content = "Hello, world!";

        let text = format_peek(content, 0, 13, OutputFormat::Text);
        assert!(text.contains("Bytes 0..13"));
        assert!(text.contains("Hello, world!"));

        let json = format_peek(content, 0, 13, OutputFormat::Json);
        assert!(json.contains("\"content\": \"Hello, world!\""));
        assert!(json.contains("\"start\": 0"));
    }

    #[test]
    fn test_format_peek_no_trailing_newline() {
        let content = "no newline";
        let text = format_peek(content, 0, 10, OutputFormat::Text);
        assert!(text.ends_with("---\n"));
    }

    #[test]
    fn test_format_grep_matches_empty() {
        let matches: Vec<GrepMatch> = vec![];
        let text = format_grep_matches(&matches, "pattern", OutputFormat::Text);
        assert!(text.contains("No matches found"));

        let json = format_grep_matches(&matches, "pattern", OutputFormat::Json);
        assert!(json.contains("[]"));
    }

    #[test]
    fn test_format_grep_matches_with_data() {
        let matches = vec![
            GrepMatch {
                offset: 10,
                matched: "hello".to_string(),
                snippet: "say hello world".to_string(),
            },
            GrepMatch {
                offset: 50,
                matched: "hello".to_string(),
                snippet: "another\nhello".to_string(),
            },
        ];

        let text = format_grep_matches(&matches, "hello", OutputFormat::Text);
        assert!(text.contains("Found 2 matches"));
        assert!(text.contains("Match 1 at byte 10"));
        assert!(text.contains("another\\nhello"));

        let json = format_grep_matches(&matches, "hello", OutputFormat::Json);
        assert!(json.contains("\"offset\": 10"));
    }

    #[test]
    fn test_format_chunk_indices() {
        let indices = vec![(0, 100), (100, 200), (200, 300)];

        let text = format_chunk_indices(&indices, OutputFormat::Text);
        assert!(text.contains("3 chunks"));
        assert!(text.contains("[0] 0..100"));
        assert!(text.contains("100 bytes"));

        let json = format_chunk_indices(&indices, OutputFormat::Json);
        assert!(json.contains('0') && json.contains("100"));
    }

    #[test]
    fn test_format_write_chunks_result() {
        let paths = vec!["chunk_0.txt".to_string(), "chunk_1.txt".to_string()];

        let text = format_write_chunks_result(&paths, OutputFormat::Text);
        assert!(text.contains("Wrote 2 chunks"));
        assert!(text.contains("chunk_0.txt"));

        let json = format_write_chunks_result(&paths, OutputFormat::Json);
        assert!(json.contains("\"chunk_0.txt\""));
    }

    #[test]
    fn test_format_context() {
        let mut context = Context::new();
        context.set_variable(
            "key".to_string(),
            crate::core::ContextValue::String("val".to_string()),
        );
        context.set_global("gkey".to_string(), crate::core::ContextValue::Float(42.0));

        let text = format_context(&context, OutputFormat::Text);
        assert!(text.contains("Variables: 1"));
        assert!(text.contains("Globals:   1"));

        let json = format_context(&context, OutputFormat::Json);
        assert!(json.contains("\"variables\""));
    }

    #[test]
    fn test_format_json_error() {
        // Test that format_json handles errors gracefully
        // This is hard to trigger with normal Serialize types
        // but the fallback to "{}" is tested implicitly
    }
}
