//! Tool executor that dispatches tool calls to internal `rlm-rs` functions.
//!
//! Maps tool names to direct Rust function calls against [`SqliteStorage`],
//! search infrastructure, and regex operations. No subprocess or CLI parsing.

use std::cell::RefCell;

use serde::{Deserialize, Serialize};

use crate::embedding::Embedder;
use crate::error::AgentError;
use crate::storage::{SqliteStorage, Storage};

use super::tool::{ToolCall, ToolResult};

/// Maximum raw byte length of tool argument JSON from the LLM.
const MAX_TOOL_ARGS_LEN: usize = 100_000;
/// Maximum chunk IDs per `get_chunks` or `grep_chunks` call.
const MAX_CHUNK_IDS: usize = 200;
/// Maximum `top_k` for the `search` tool.
const MAX_SEARCH_TOP_K: usize = 500;
/// Maximum context lines for `grep_chunks`.
const MAX_CONTEXT_LINES: usize = 20;
/// Maximum regex pattern length for `grep_chunks`.
const MAX_REGEX_LEN: usize = 500;
/// Maximum compiled regex DFA size (bytes).
const MAX_REGEX_DFA_SIZE: usize = 1_000_000;

/// Executes tool calls by dispatching to internal `rlm-rs` functions.
///
/// Holds a reference to storage and a lazily-created embedder for search,
/// so tools can call Rust functions directly without subprocess overhead.
/// The embedder is initialised on first use and cached for subsequent calls.
pub struct ToolExecutor<'a> {
    storage: &'a SqliteStorage,
    embedder: RefCell<Option<Box<dyn Embedder>>>,
}

impl<'a> ToolExecutor<'a> {
    /// Creates a new executor backed by the given storage.
    #[must_use]
    pub fn new(storage: &'a SqliteStorage) -> Self {
        Self {
            storage,
            embedder: RefCell::new(None),
        }
    }

    /// Returns a reference to the cached embedder, creating it on first call.
    ///
    /// Uses the provided closure to avoid holding the `RefCell` borrow across
    /// the callback, which would prevent recursive tool calls.
    fn with_embedder<F, T>(&self, f: F) -> Result<T, AgentError>
    where
        F: FnOnce(&dyn Embedder) -> Result<T, AgentError>,
    {
        // Ensure embedder is initialised
        {
            let guard = self.embedder.borrow();
            if guard.is_none() {
                drop(guard);
                let emb =
                    crate::embedding::create_embedder().map_err(|e| AgentError::ToolExecution {
                        name: "search".to_string(),
                        message: format!("embedder creation failed: {e}"),
                    })?;
                *self.embedder.borrow_mut() = Some(emb);
            }
        }

        let guard = self.embedder.borrow();
        let embedder = guard.as_ref().ok_or_else(|| AgentError::ToolExecution {
            name: "search".to_string(),
            message: "embedder not initialised".to_string(),
        })?;
        f(embedder.as_ref())
    }

    /// Dispatches a tool call to the appropriate internal function.
    ///
    /// Validates raw argument size before dispatch to prevent oversized payloads.
    #[must_use]
    pub fn execute(&self, call: &ToolCall) -> ToolResult {
        if call.arguments.len() > MAX_TOOL_ARGS_LEN {
            return ToolResult {
                tool_call_id: call.id.clone(),
                content: format!(
                    "tool arguments too large ({} bytes, max {MAX_TOOL_ARGS_LEN})",
                    call.arguments.len()
                ),
                is_error: true,
            };
        }

        let result = match call.name.as_str() {
            "get_chunks" => self.tool_get_chunks(&call.arguments),
            "search" => self.tool_search(&call.arguments),
            "grep_chunks" => self.tool_grep_chunks(&call.arguments),
            "get_buffer" => self.tool_get_buffer(&call.arguments),
            "list_buffers" => self.tool_list_buffers(&call.arguments),
            "storage_stats" => self.tool_storage_stats(&call.arguments),
            other => Err(AgentError::ToolExecution {
                name: other.to_string(),
                message: "unknown tool".to_string(),
            }),
        };

        match result {
            Ok(content) => ToolResult {
                tool_call_id: call.id.clone(),
                content,
                is_error: false,
            },
            Err(e) => ToolResult {
                tool_call_id: call.id.clone(),
                content: e.to_string(),
                is_error: true,
            },
        }
    }

    // -----------------------------------------------------------------------
    // Tool implementations
    // -----------------------------------------------------------------------

    /// Retrieves 1..N chunks by ID. Returns JSON array with null for missing IDs.
    fn tool_get_chunks(&self, args: &str) -> Result<String, AgentError> {
        #[derive(Deserialize)]
        struct Args {
            chunk_ids: Vec<i64>,
        }
        let args: Args = serde_json::from_str(args).map_err(|e| AgentError::ToolExecution {
            name: "get_chunks".to_string(),
            message: format!("invalid arguments: {e}"),
        })?;

        if args.chunk_ids.len() > MAX_CHUNK_IDS {
            return Err(AgentError::ToolExecution {
                name: "get_chunks".to_string(),
                message: format!(
                    "too many chunk IDs ({}, max {MAX_CHUNK_IDS})",
                    args.chunk_ids.len()
                ),
            });
        }

        let results: Vec<Option<ChunkView>> = args
            .chunk_ids
            .iter()
            .map(|&id| {
                self.storage
                    .get_chunk(id)
                    .ok()
                    .flatten()
                    .map(ChunkView::from)
            })
            .collect();

        serde_json::to_string_pretty(&results).map_err(|e| AgentError::ToolExecution {
            name: "get_chunks".to_string(),
            message: format!("serialization error: {e}"),
        })
    }

    /// Searches for chunks matching a query.
    fn tool_search(&self, args: &str) -> Result<String, AgentError> {
        #[derive(Deserialize)]
        struct Args {
            query: String,
            top_k: Option<usize>,
            mode: Option<String>,
        }
        let args: Args = serde_json::from_str(args).map_err(|e| AgentError::ToolExecution {
            name: "search".to_string(),
            message: format!("invalid arguments: {e}"),
        })?;

        let top_k = args.top_k.unwrap_or(10).min(MAX_SEARCH_TOP_K);
        let mode = args.mode.as_deref().unwrap_or("hybrid");

        let results = if mode == "bm25" {
            crate::search::search_bm25(self.storage, &args.query, top_k).map_err(|e| {
                AgentError::ToolExecution {
                    name: "search".to_string(),
                    message: format!("search failed: {e}"),
                }
            })?
        } else {
            let query = args.query.clone();
            let storage = self.storage;
            self.with_embedder(|embedder| {
                let results = if mode == "semantic" {
                    crate::search::search_semantic(storage, embedder, &query, top_k, 0.3)
                } else {
                    let config = crate::search::SearchConfig {
                        top_k,
                        similarity_threshold: 0.3,
                        use_semantic: true,
                        use_bm25: true,
                        ..crate::search::SearchConfig::default()
                    };
                    crate::search::hybrid_search(storage, embedder, &query, &config)
                }
                .map_err(|e| AgentError::ToolExecution {
                    name: "search".to_string(),
                    message: format!("search failed: {e}"),
                })?;
                Ok(results)
            })?
        };

        let views: Vec<SearchResultView> = results.iter().map(SearchResultView::from).collect();
        serde_json::to_string_pretty(&views).map_err(|e| AgentError::ToolExecution {
            name: "search".to_string(),
            message: format!("serialization error: {e}"),
        })
    }

    /// Grep chunk content with a regex pattern and optional scoping.
    fn tool_grep_chunks(&self, args: &str) -> Result<String, AgentError> {
        #[derive(Deserialize)]
        struct Args {
            pattern: String,
            chunk_ids: Option<Vec<i64>>,
            buffer_id: Option<i64>,
            context_lines: Option<usize>,
        }
        let args: Args = serde_json::from_str(args).map_err(|e| AgentError::ToolExecution {
            name: "grep_chunks".to_string(),
            message: format!("invalid arguments: {e}"),
        })?;

        if args.pattern.len() > MAX_REGEX_LEN {
            return Err(AgentError::ToolExecution {
                name: "grep_chunks".to_string(),
                message: format!(
                    "regex pattern too long ({} bytes, max {MAX_REGEX_LEN})",
                    args.pattern.len()
                ),
            });
        }

        if let Some(ref ids) = args.chunk_ids
            && ids.len() > MAX_CHUNK_IDS
        {
            return Err(AgentError::ToolExecution {
                name: "grep_chunks".to_string(),
                message: format!("too many chunk IDs ({}, max {MAX_CHUNK_IDS})", ids.len()),
            });
        }

        let re = regex::RegexBuilder::new(&args.pattern)
            .size_limit(MAX_REGEX_DFA_SIZE)
            .build()
            .map_err(|e| AgentError::ToolExecution {
                name: "grep_chunks".to_string(),
                message: format!("invalid regex: {e}"),
            })?;

        let context_lines = args.context_lines.unwrap_or(0).min(MAX_CONTEXT_LINES);

        // Resolve which chunks to grep
        let chunks = if let Some(ref ids) = args.chunk_ids {
            // Scope: specific chunk IDs
            ids.iter()
                .filter_map(|&id| self.storage.get_chunk(id).ok().flatten())
                .collect::<Vec<_>>()
        } else if let Some(bid) = args.buffer_id {
            // Scope: all chunks in a buffer
            self.storage
                .get_chunks(bid)
                .map_err(|e| AgentError::ToolExecution {
                    name: "grep_chunks".to_string(),
                    message: format!("failed to get buffer chunks: {e}"),
                })?
        } else {
            // Scope: all chunks across all buffers (bounded to prevent OOM)
            const MAX_GREP_CHUNKS: usize = 5000;
            let buffers = self
                .storage
                .list_buffers()
                .map_err(|e| AgentError::ToolExecution {
                    name: "grep_chunks".to_string(),
                    message: format!("failed to list buffers: {e}"),
                })?;
            let mut all = Vec::new();
            for buf in &buffers {
                if all.len() >= MAX_GREP_CHUNKS {
                    break;
                }
                if let Some(id) = buf.id
                    && let Ok(chunks) = self.storage.get_chunks(id)
                {
                    let remaining = MAX_GREP_CHUNKS - all.len();
                    all.extend(chunks.into_iter().take(remaining));
                }
            }
            all
        };

        // Apply regex with context lines
        let mut matches = Vec::new();
        for chunk in &chunks {
            let chunk_id = chunk.id.unwrap_or(-1);
            let lines: Vec<&str> = chunk.content.lines().collect();

            for (line_num, line) in lines.iter().enumerate() {
                if re.is_match(line) {
                    let start = line_num.saturating_sub(context_lines);
                    let end = (line_num + context_lines + 1).min(lines.len());
                    let context: Vec<String> = lines[start..end]
                        .iter()
                        .enumerate()
                        .map(|(i, l)| format!("{}: {l}", start + i + 1))
                        .collect();

                    matches.push(GrepMatch {
                        chunk_id,
                        line_number: line_num + 1,
                        matched_line: (*line).to_string(),
                        context,
                    });
                }
            }
        }

        serde_json::to_string_pretty(&matches).map_err(|e| AgentError::ToolExecution {
            name: "grep_chunks".to_string(),
            message: format!("serialization error: {e}"),
        })
    }

    /// Retrieves a buffer by name or ID.
    fn tool_get_buffer(&self, args: &str) -> Result<String, AgentError> {
        #[derive(Deserialize)]
        struct Args {
            name: Option<String>,
            id: Option<i64>,
        }
        let args: Args = serde_json::from_str(args).map_err(|e| AgentError::ToolExecution {
            name: "get_buffer".to_string(),
            message: format!("invalid arguments: {e}"),
        })?;

        let buffer = if let Some(ref name) = args.name {
            self.storage
                .get_buffer_by_name(name)
                .map_err(|e| AgentError::ToolExecution {
                    name: "get_buffer".to_string(),
                    message: format!("lookup failed: {e}"),
                })?
        } else if let Some(id) = args.id {
            self.storage
                .get_buffer(id)
                .map_err(|e| AgentError::ToolExecution {
                    name: "get_buffer".to_string(),
                    message: format!("lookup failed: {e}"),
                })?
        } else {
            return Err(AgentError::ToolExecution {
                name: "get_buffer".to_string(),
                message: "either 'name' or 'id' must be provided".to_string(),
            });
        };

        let view = buffer.map(BufferView::from);
        serde_json::to_string_pretty(&view).map_err(|e| AgentError::ToolExecution {
            name: "get_buffer".to_string(),
            message: format!("serialization error: {e}"),
        })
    }

    /// Lists all buffers (metadata only, no content).
    fn tool_list_buffers(&self, _args: &str) -> Result<String, AgentError> {
        let buffers = self
            .storage
            .list_buffers()
            .map_err(|e| AgentError::ToolExecution {
                name: "list_buffers".to_string(),
                message: format!("failed: {e}"),
            })?;

        let views: Vec<BufferSummary> = buffers.iter().map(BufferSummary::from).collect();
        serde_json::to_string_pretty(&views).map_err(|e| AgentError::ToolExecution {
            name: "list_buffers".to_string(),
            message: format!("serialization error: {e}"),
        })
    }

    /// Returns storage statistics.
    fn tool_storage_stats(&self, _args: &str) -> Result<String, AgentError> {
        let stats = self
            .storage
            .stats()
            .map_err(|e| AgentError::ToolExecution {
                name: "storage_stats".to_string(),
                message: format!("failed: {e}"),
            })?;

        serde_json::to_string_pretty(&stats).map_err(|e| AgentError::ToolExecution {
            name: "storage_stats".to_string(),
            message: format!("serialization error: {e}"),
        })
    }
}

// ---------------------------------------------------------------------------
// View types for serialization (subset of full structs)
// ---------------------------------------------------------------------------

/// Serializable view of a chunk returned by tools.
#[derive(Debug, Clone, Serialize)]
struct ChunkView {
    id: Option<i64>,
    buffer_id: i64,
    content: String,
    index: usize,
    byte_start: usize,
    byte_end: usize,
}

impl From<crate::core::Chunk> for ChunkView {
    fn from(c: crate::core::Chunk) -> Self {
        Self {
            id: c.id,
            buffer_id: c.buffer_id,
            content: c.content,
            index: c.index,
            byte_start: c.byte_range.start,
            byte_end: c.byte_range.end,
        }
    }
}

/// Serializable view of a search result.
#[derive(Debug, Clone, Serialize)]
struct SearchResultView {
    chunk_id: i64,
    buffer_id: i64,
    score: f64,
    semantic_score: Option<f32>,
    bm25_score: Option<f64>,
}

impl From<&crate::search::SearchResult> for SearchResultView {
    fn from(r: &crate::search::SearchResult) -> Self {
        Self {
            chunk_id: r.chunk_id,
            buffer_id: r.buffer_id,
            score: r.score,
            semantic_score: r.semantic_score,
            bm25_score: r.bm25_score,
        }
    }
}

/// Serializable view of a buffer (includes content).
#[derive(Debug, Clone, Serialize)]
struct BufferView {
    id: Option<i64>,
    name: Option<String>,
    content_size: usize,
    content_type: Option<String>,
    chunk_count: Option<usize>,
    content: String,
}

impl From<crate::core::Buffer> for BufferView {
    fn from(b: crate::core::Buffer) -> Self {
        Self {
            id: b.id,
            name: b.name,
            content_size: b.metadata.size,
            content_type: b.metadata.content_type,
            chunk_count: b.metadata.chunk_count,
            content: b.content,
        }
    }
}

/// Serializable buffer summary (no content).
#[derive(Debug, Clone, Serialize)]
struct BufferSummary {
    id: Option<i64>,
    name: Option<String>,
    content_size: usize,
    content_type: Option<String>,
    chunk_count: Option<usize>,
}

impl From<&crate::core::Buffer> for BufferSummary {
    fn from(b: &crate::core::Buffer) -> Self {
        Self {
            id: b.id,
            name: b.name.clone(),
            content_size: b.metadata.size,
            content_type: b.metadata.content_type.clone(),
            chunk_count: b.metadata.chunk_count,
        }
    }
}

/// A grep match within a chunk.
#[derive(Debug, Clone, Serialize)]
struct GrepMatch {
    chunk_id: i64,
    line_number: usize,
    matched_line: String,
    context: Vec<String>,
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;
    use crate::core::{Buffer, Chunk};
    use crate::storage::Storage;

    fn setup_storage() -> SqliteStorage {
        let mut storage =
            SqliteStorage::in_memory().unwrap_or_else(|e| panic!("in_memory failed: {e}"));
        storage
            .init()
            .unwrap_or_else(|e| panic!("init failed: {e}"));
        storage
    }

    fn add_test_buffer(storage: &mut SqliteStorage) -> i64 {
        let buffer = Buffer::from_named(
            "test-buffer".to_string(),
            "hello world\nfoo bar\nbaz qux".to_string(),
        );
        let buf_id = storage
            .add_buffer(&buffer)
            .unwrap_or_else(|e| panic!("add_buffer failed: {e}"));

        let chunks = vec![
            Chunk::new(buf_id, "hello world".to_string(), 0..11, 0),
            Chunk::new(buf_id, "foo bar".to_string(), 12..19, 1),
            Chunk::new(buf_id, "baz qux".to_string(), 20..27, 2),
        ];
        storage
            .add_chunks(buf_id, &chunks)
            .unwrap_or_else(|e| panic!("add_chunks failed: {e}"));
        buf_id
    }

    #[test]
    fn test_get_chunks_existing() {
        let mut storage = setup_storage();
        let buf_id = add_test_buffer(&mut storage);
        let executor = ToolExecutor::new(&storage);

        let chunks = storage
            .get_chunks(buf_id)
            .unwrap_or_else(|e| panic!("get_chunks failed: {e}"));
        let first_id = chunks[0].id.unwrap_or(0);

        let call = ToolCall {
            id: "call_1".to_string(),
            name: "get_chunks".to_string(),
            arguments: format!(r#"{{"chunk_ids":[{first_id}]}}"#),
        };

        let result = executor.execute(&call);
        assert!(
            !result.is_error,
            "Expected success, got: {}",
            result.content
        );
        assert!(result.content.contains("hello world"));
    }

    #[test]
    fn test_get_chunks_missing() {
        let mut storage = setup_storage();
        let _buf_id = add_test_buffer(&mut storage);
        let executor = ToolExecutor::new(&storage);

        let call = ToolCall {
            id: "call_1".to_string(),
            name: "get_chunks".to_string(),
            arguments: r#"{"chunk_ids":[99999]}"#.to_string(),
        };

        let result = executor.execute(&call);
        assert!(!result.is_error);
        assert!(result.content.contains("null"));
    }

    #[test]
    fn test_grep_chunks_pattern() {
        let mut storage = setup_storage();
        let buf_id = add_test_buffer(&mut storage);
        let executor = ToolExecutor::new(&storage);

        let call = ToolCall {
            id: "call_1".to_string(),
            name: "grep_chunks".to_string(),
            arguments: format!(r#"{{"pattern":"foo","buffer_id":{buf_id}}}"#),
        };

        let result = executor.execute(&call);
        assert!(
            !result.is_error,
            "Expected success, got: {}",
            result.content
        );
        assert!(result.content.contains("foo bar"));
    }

    #[test]
    fn test_grep_chunks_invalid_regex() {
        let storage = setup_storage();
        let executor = ToolExecutor::new(&storage);

        let call = ToolCall {
            id: "call_1".to_string(),
            name: "grep_chunks".to_string(),
            arguments: r#"{"pattern":"[invalid"}"#.to_string(),
        };

        let result = executor.execute(&call);
        assert!(result.is_error);
        assert!(result.content.contains("invalid regex"));
    }

    #[test]
    fn test_get_buffer_by_name() {
        let mut storage = setup_storage();
        let _buf_id = add_test_buffer(&mut storage);
        let executor = ToolExecutor::new(&storage);

        let call = ToolCall {
            id: "call_1".to_string(),
            name: "get_buffer".to_string(),
            arguments: r#"{"name":"test-buffer"}"#.to_string(),
        };

        let result = executor.execute(&call);
        assert!(
            !result.is_error,
            "Expected success, got: {}",
            result.content
        );
        assert!(result.content.contains("test-buffer"));
    }

    #[test]
    fn test_list_buffers() {
        let mut storage = setup_storage();
        let _buf_id = add_test_buffer(&mut storage);
        let executor = ToolExecutor::new(&storage);

        let call = ToolCall {
            id: "call_1".to_string(),
            name: "list_buffers".to_string(),
            arguments: "{}".to_string(),
        };

        let result = executor.execute(&call);
        assert!(
            !result.is_error,
            "Expected success, got: {}",
            result.content
        );
        assert!(result.content.contains("test-buffer"));
    }

    #[test]
    fn test_storage_stats() {
        let mut storage = setup_storage();
        let _buf_id = add_test_buffer(&mut storage);
        let executor = ToolExecutor::new(&storage);

        let call = ToolCall {
            id: "call_1".to_string(),
            name: "storage_stats".to_string(),
            arguments: "{}".to_string(),
        };

        let result = executor.execute(&call);
        assert!(
            !result.is_error,
            "Expected success, got: {}",
            result.content
        );
        assert!(result.content.contains("buffer_count"));
    }

    #[test]
    fn test_unknown_tool() {
        let storage = setup_storage();
        let executor = ToolExecutor::new(&storage);

        let call = ToolCall {
            id: "call_1".to_string(),
            name: "nonexistent_tool".to_string(),
            arguments: "{}".to_string(),
        };

        let result = executor.execute(&call);
        assert!(result.is_error);
        assert!(result.content.contains("unknown tool"));
    }
}
