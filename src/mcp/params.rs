//! MCP tool parameter types.
//!
//! Defines the input schemas for MCP tools using `schemars` for automatic
//! JSON Schema generation required by the MCP protocol.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Parameters for the `query` MCP tool.
///
/// Runs the full agentic pipeline: plan → search → fan-out subcalls → synthesis.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct QueryParams {
    /// The analysis question or task.
    pub query: String,

    /// Buffer name to scope the analysis.
    pub buffer_name: String,

    /// Search mode override: `"hybrid"`, `"semantic"`, or `"bm25"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_mode: Option<String>,

    /// Chunks per subcall agent batch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch_size: Option<usize>,

    /// Search depth: maximum results from the search layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_k: Option<usize>,

    /// Minimum similarity threshold (0.0–1.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f32>,

    /// Maximum chunks to analyze (0 or absent = unlimited).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_chunks: Option<usize>,

    /// Target number of concurrent subagents. When set, batch size is
    /// computed as `ceil(chunks / num_agents)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_agents: Option<usize>,

    /// Skip the primary agent planning step.
    #[serde(default)]
    pub skip_plan: bool,

    /// Minimum relevance level for findings: `"none"`, `"low"`, `"medium"`, `"high"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finding_threshold: Option<String>,
}
