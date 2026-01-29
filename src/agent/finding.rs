//! Data types for agent findings and query results.
//!
//! These types represent the structured output from subcall agents
//! and the aggregated results from the orchestrator.

use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::message::TokenUsage;

// Re-export from core so existing `agent::finding::Relevance` paths still work.
pub use crate::core::Relevance;

/// A chunk loaded from storage with its search metadata preserved.
///
/// Created by the orchestrator's `load_chunks` step, this struct keeps
/// all [`SearchResult`](crate::search::SearchResult) metadata alongside
/// the full chunk content so downstream stages can reason about temporal
/// ordering, relevance scores, and provenance.
#[derive(Debug, Clone)]
pub struct LoadedChunk {
    /// Database chunk ID.
    pub chunk_id: i64,
    /// Buffer this chunk belongs to.
    pub buffer_id: i64,
    /// Sequential index within the buffer (0-based, temporal position).
    pub index: usize,
    /// Combined search relevance score.
    pub score: f64,
    /// Semantic similarity score (if available).
    pub semantic_score: Option<f32>,
    /// BM25 score (if available).
    pub bm25_score: Option<f64>,
    /// Full chunk content.
    pub content: String,
}

/// A single finding from a subcall agent analyzing a chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    /// ID of the analyzed chunk.
    pub chunk_id: i64,
    /// Relevance assessment.
    pub relevance: Relevance,
    /// Specific findings (observations, code references, etc.).
    #[serde(default)]
    pub findings: Vec<String>,
    /// Brief summary of the chunk's relevance.
    #[serde(default)]
    pub summary: Option<String>,
    /// Suggested follow-up queries or areas to investigate.
    #[serde(default)]
    pub follow_up: Vec<String>,
    /// Sequential index of the chunk within its buffer (temporal position).
    /// Populated by the orchestrator after subcall agents return findings.
    #[serde(default, skip_deserializing)]
    pub chunk_index: Option<usize>,
    /// Buffer ID this chunk belongs to.
    /// Populated by the orchestrator after subcall agents return findings.
    #[serde(default, skip_deserializing)]
    pub chunk_buffer_id: Option<i64>,
}

/// Result from a single subcall agent batch.
#[derive(Debug, Clone)]
pub struct SubagentResult {
    /// Batch index (0-based).
    pub batch_index: usize,
    /// Findings from this batch.
    pub findings: Vec<Finding>,
    /// Token usage for this batch.
    pub usage: TokenUsage,
    /// Elapsed time for this batch.
    pub elapsed: Duration,
}

/// Final result from the orchestrator query pipeline.
#[derive(Debug, Clone, Serialize)]
pub struct QueryResult {
    /// Synthesized markdown response.
    pub response: String,
    /// Adaptive scaling tier used for this query.
    pub scaling_tier: String,
    /// Total findings collected across all batches (after filtering).
    pub findings_count: usize,
    /// Findings filtered out below the relevance threshold.
    pub findings_filtered: usize,
    /// Number of chunks loaded and sent to subagents.
    pub chunks_analyzed: usize,
    /// IDs of chunks that were loaded and sent to subagents.
    pub analyzed_chunk_ids: Vec<i64>,
    /// Number of search results returned before truncation.
    pub chunks_available: usize,
    /// Number of batches processed successfully.
    pub batches_processed: usize,
    /// Number of batches that failed.
    pub batches_failed: usize,
    /// Number of chunks that failed to load from storage.
    pub chunk_load_failures: usize,
    /// Error messages from failed batches.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub batch_errors: Vec<String>,
    /// Total tokens consumed.
    pub total_tokens: u32,
    /// Total elapsed time.
    #[serde(serialize_with = "serialize_duration")]
    pub elapsed: Duration,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn serialize_duration<S>(d: &Duration, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    s.serialize_f64(d.as_secs_f64())
}

/// Analysis plan from the primary agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisPlan {
    /// Search mode to use (hybrid, semantic, bm25).
    #[serde(default = "default_search_mode")]
    pub search_mode: String,
    /// Batch size override.
    #[serde(default)]
    pub batch_size: Option<usize>,
    /// Relevance threshold.
    #[serde(default)]
    pub threshold: Option<f32>,
    /// Focus areas for the analysis.
    #[serde(default)]
    pub focus_areas: Vec<String>,
    /// Maximum chunks to analyze.
    #[serde(default)]
    pub max_chunks: Option<usize>,
    /// Search depth (top-k results from the search layer).
    #[serde(default)]
    pub top_k: Option<usize>,
}

fn default_search_mode() -> String {
    "hybrid".to_string()
}

impl Default for AnalysisPlan {
    fn default() -> Self {
        Self {
            search_mode: default_search_mode(),
            batch_size: None,
            threshold: None,
            focus_areas: Vec::new(),
            max_chunks: None,
            top_k: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_finding_deserialization() {
        let json = r#"{
            "chunk_id": 42,
            "relevance": "high",
            "findings": ["Found error handling"],
            "summary": "Contains error types"
        }"#;
        let finding: Finding = serde_json::from_str(json).unwrap_or_else(|_| unreachable!());
        assert_eq!(finding.chunk_id, 42);
        assert_eq!(finding.relevance, Relevance::High);
        assert_eq!(finding.findings.len(), 1);
    }

    #[test]
    fn test_finding_defaults() {
        let json = r#"{"chunk_id": 1, "relevance": "none"}"#;
        let finding: Finding = serde_json::from_str(json).unwrap_or_else(|_| unreachable!());
        assert!(finding.findings.is_empty());
        assert!(finding.summary.is_none());
        assert!(finding.follow_up.is_empty());
    }

    #[test]
    fn test_analysis_plan_defaults() {
        let plan = AnalysisPlan::default();
        assert_eq!(plan.search_mode, "hybrid");
        assert!(plan.batch_size.is_none());
        assert!(plan.focus_areas.is_empty());
    }
}
