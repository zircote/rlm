//! Tool type definitions for internal function-calling.
//!
//! Provides provider-agnostic types for tool definitions, calls, and results.
//! Tools expose internal `rlm-rs` operations (storage, search, grep) as
//! function-calling targets for LLM agents.

use serde::{Deserialize, Serialize};
use serde_json::json;

/// A tool definition that can be sent to an LLM for function-calling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name (must match dispatch table in executor).
    pub name: String,
    /// Human-readable description of what the tool does.
    pub description: String,
    /// JSON Schema object describing the tool's parameters.
    pub parameters: serde_json::Value,
}

/// A tool call requested by the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique identifier for this call (assigned by the provider).
    pub id: String,
    /// Name of the tool to invoke.
    pub name: String,
    /// JSON-encoded arguments for the tool.
    pub arguments: String,
}

/// The result of executing a tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// ID of the tool call this result corresponds to.
    pub tool_call_id: String,
    /// Result content (JSON string on success, error message on failure).
    pub content: String,
    /// Whether this result represents an error.
    pub is_error: bool,
}

/// A set of tool definitions scoped to an agent role.
///
/// Different agents get different tool subsets:
/// - Synthesizer: all six tools (`get_chunks`, `search`, `grep_chunks`,
///   `get_buffer`, `list_buffers`, `storage_stats`)
/// - Subcall agents / Primary agent: no tools (receive context directly)
#[derive(Debug, Clone, Default)]
pub struct ToolSet {
    definitions: Vec<ToolDefinition>,
}

impl ToolSet {
    /// Returns the tool definitions in this set.
    #[must_use]
    pub fn definitions(&self) -> &[ToolDefinition] {
        &self.definitions
    }

    /// Returns `true` if this set contains no tools.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }

    /// Returns the number of tools in this set.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.definitions.len()
    }

    /// Tool set for the synthesizer agent.
    ///
    /// Includes all six tools: `get_chunks`, `search`, `grep_chunks`,
    /// `get_buffer`, `list_buffers`, `storage_stats`.
    #[must_use]
    pub fn synthesizer_tools() -> Self {
        Self {
            definitions: vec![
                def_get_chunks(),
                def_search(),
                def_grep_chunks(),
                def_get_buffer(),
                def_list_buffers(),
                def_storage_stats(),
            ],
        }
    }

    /// Empty tool set (no tools available).
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }
}

// ---------------------------------------------------------------------------
// Tool schema definitions
// ---------------------------------------------------------------------------

/// Defines the `get_chunks` tool.
fn def_get_chunks() -> ToolDefinition {
    ToolDefinition {
        name: "get_chunks".to_string(),
        description: "Retrieve one or more chunks by ID. Returns an array of chunk objects \
                       (content + metadata) in the same order. Missing IDs return null."
            .to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "chunk_ids": {
                    "type": "array",
                    "items": { "type": "integer" },
                    "minItems": 1,
                    "description": "Array of chunk IDs to retrieve."
                }
            },
            "required": ["chunk_ids"],
            "additionalProperties": false
        }),
    }
}

/// Defines the `search` tool.
fn def_search() -> ToolDefinition {
    ToolDefinition {
        name: "search".to_string(),
        description: "Search for chunks matching a query using hybrid (semantic + BM25), \
                       semantic-only, or BM25-only search."
            .to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query text."
                },
                "top_k": {
                    "type": "integer",
                    "description": "Maximum number of results to return. Defaults to 10.",
                    "default": 10
                },
                "mode": {
                    "type": "string",
                    "enum": ["hybrid", "semantic", "bm25"],
                    "description": "Search mode. Defaults to 'hybrid'.",
                    "default": "hybrid"
                }
            },
            "required": ["query"],
            "additionalProperties": false
        }),
    }
}

/// Defines the `grep_chunks` tool.
fn def_grep_chunks() -> ToolDefinition {
    ToolDefinition {
        name: "grep_chunks".to_string(),
        description: "Search chunk content with a regex pattern. Scope by chunk_ids (highest \
                       priority), buffer_id, or search all chunks. Returns matching lines \
                       with optional context."
            .to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regex pattern to search for in chunk content."
                },
                "chunk_ids": {
                    "type": "array",
                    "items": { "type": "integer" },
                    "description": "Grep only within these specific chunks (highest priority)."
                },
                "buffer_id": {
                    "type": "integer",
                    "description": "Grep all chunks belonging to this buffer. Ignored if chunk_ids is set."
                },
                "context_lines": {
                    "type": "integer",
                    "description": "Number of context lines before and after each match. Defaults to 0.",
                    "default": 0
                }
            },
            "required": ["pattern"],
            "additionalProperties": false
        }),
    }
}

/// Defines the `get_buffer` tool.
fn def_get_buffer() -> ToolDefinition {
    ToolDefinition {
        name: "get_buffer".to_string(),
        description: "Retrieve a buffer by name or ID. Returns buffer metadata and content."
            .to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Buffer name to look up."
                },
                "id": {
                    "type": "integer",
                    "description": "Buffer ID to look up. Ignored if name is provided."
                }
            },
            "additionalProperties": false,
            "minProperties": 1
        }),
    }
}

/// Defines the `list_buffers` tool.
fn def_list_buffers() -> ToolDefinition {
    ToolDefinition {
        name: "list_buffers".to_string(),
        description: "List all buffers in storage with their metadata (no content).".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }),
    }
}

/// Defines the `storage_stats` tool.
fn def_storage_stats() -> ToolDefinition {
    ToolDefinition {
        name: "storage_stats".to_string(),
        description: "Get storage statistics: buffer count, chunk count, total content size, \
                       schema version."
            .to_string(),
        parameters: json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toolset_synthesizer() {
        let ts = ToolSet::synthesizer_tools();
        assert_eq!(ts.len(), 6);
        let names: Vec<&str> = ts.definitions().iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"get_buffer"));
        assert!(names.contains(&"list_buffers"));
        assert!(names.contains(&"storage_stats"));
    }

    #[test]
    fn test_toolset_none() {
        let ts = ToolSet::none();
        assert!(ts.is_empty());
        assert_eq!(ts.len(), 0);
    }

    #[test]
    fn test_tool_definition_serialization() {
        let def = def_get_chunks();
        let json = serde_json::to_string(&def).unwrap_or_default();
        assert!(json.contains("get_chunks"));
        assert!(json.contains("chunk_ids"));
    }

    #[test]
    fn test_tool_call_serialization() {
        let call = ToolCall {
            id: "call_123".to_string(),
            name: "get_chunks".to_string(),
            arguments: r#"{"chunk_ids":[1,2,3]}"#.to_string(),
        };
        let json = serde_json::to_string(&call).unwrap_or_default();
        assert!(json.contains("call_123"));
        assert!(json.contains("get_chunks"));
    }

    #[test]
    fn test_tool_result_serialization() {
        let result = ToolResult {
            tool_call_id: "call_123".to_string(),
            content: r#"[{"id":1,"content":"hello"}]"#.to_string(),
            is_error: false,
        };
        let json = serde_json::to_string(&result).unwrap_or_default();
        assert!(json.contains("call_123"));
        assert!(!result.is_error);
    }

    #[test]
    fn test_all_definitions_have_valid_schemas() {
        let all = vec![
            def_get_chunks(),
            def_search(),
            def_grep_chunks(),
            def_get_buffer(),
            def_list_buffers(),
            def_storage_stats(),
        ];
        for def in &all {
            assert!(!def.name.is_empty());
            assert!(!def.description.is_empty());
            assert!(def.parameters.is_object());
            assert_eq!(def.parameters["type"], "object");
        }
    }
}
