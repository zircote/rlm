//! Orchestrator for fan-out/collect agentic workflows.
//!
//! Coordinates the full query pipeline: planning → search → fan-out
//! subcall agents → collect findings → synthesize response.

use std::sync::Arc;
use std::time::Instant;

use tokio::sync::Semaphore;

use super::config::AgentConfig;
use super::executor::ToolExecutor;
use super::finding::{Finding, LoadedChunk, QueryResult, Relevance, SubagentResult};
use super::primary::PrimaryAgent;
use super::prompt::{
    ChunkContext, PromptSet, build_primary_prompt, build_subcall_prompt, build_synthesizer_prompt,
};
use super::provider::LlmProvider;
use super::scaling::{DatasetProfile, compute_scaling_profile};
use super::subcall::SubcallAgent;
use super::synthesizer::SynthesizerAgent;
use super::traits::execute_with_tools;
use crate::error::AgentError;
use crate::search::{SearchConfig, SearchResult};
use crate::storage::{SqliteStorage, Storage};

/// Orchestrates the agentic query workflow.
///
/// Coordinates planning, search, concurrent chunk analysis, and synthesis
/// into a single query pipeline.
pub struct Orchestrator {
    provider: Arc<dyn LlmProvider>,
    config: AgentConfig,
    prompts: PromptSet,
}

impl Orchestrator {
    /// Creates a new orchestrator with the given provider and configuration.
    ///
    /// Loads prompt templates from the directory specified in
    /// [`AgentConfig::prompt_dir`], falling back to compiled-in defaults.
    pub fn new(provider: Arc<dyn LlmProvider>, config: AgentConfig) -> Self {
        let prompts = PromptSet::load(config.prompt_dir.as_deref());
        Self {
            provider,
            config,
            prompts,
        }
    }

    /// Executes the full query pipeline.
    ///
    /// # Steps
    ///
    /// 1. Plan analysis strategy via [`PrimaryAgent`]
    /// 2. Search for relevant chunks
    /// 3. Load chunk content from storage
    /// 4. Fan out subcall agents concurrently
    /// 5. Collect findings
    /// 6. Synthesize final response
    ///
    /// # Arguments
    ///
    /// * `storage` - Storage backend for chunk retrieval
    /// * `query` - User's query text
    /// * `buffer_name` - Optional buffer to scope the search
    /// * `cli_overrides` - Optional CLI overrides for search parameters
    ///
    /// # Errors
    ///
    /// Returns [`AgentError`] on planning, search, API, or synthesis failures.
    #[allow(clippy::future_not_send, clippy::too_many_lines)]
    pub async fn query(
        &self,
        storage: &SqliteStorage,
        query: &str,
        buffer_name: Option<&str>,
        cli_overrides: Option<CliOverrides>,
    ) -> Result<QueryResult, AgentError> {
        const MAX_QUERY_LEN: usize = 10_000;

        if query.trim().is_empty() {
            return Err(AgentError::Orchestration {
                message: "Query cannot be empty".to_string(),
            });
        }

        if query.len() > MAX_QUERY_LEN {
            return Err(AgentError::Orchestration {
                message: format!(
                    "Query exceeds maximum length ({} bytes, max {MAX_QUERY_LEN})",
                    query.len()
                ),
            });
        }

        let start = Instant::now();
        let overrides = cli_overrides.unwrap_or_default();

        // Step 1: Plan analysis strategy (skippable)
        let plan = if overrides.skip_plan {
            super::finding::AnalysisPlan::default()
        } else {
            let (plan, _plan_response) = self.plan_analysis(storage, query, buffer_name).await?;
            plan
        };

        // Compute dataset profile for adaptive scaling
        let dataset_profile = Self::build_dataset_profile(storage, buffer_name);
        let scaling = compute_scaling_profile(&dataset_profile);

        // Resolve parameters: CLI → Plan → Scaling → Config → Default
        let search_mode = overrides
            .search_mode
            .as_deref()
            .unwrap_or(&plan.search_mode);
        let threshold = overrides.threshold.or(plan.threshold).unwrap_or(0.3);
        let max_chunks = overrides
            .max_chunks
            .or(plan.max_chunks)
            .or(scaling.max_chunks)
            .unwrap_or(0);
        let top_k = overrides
            .top_k
            .or(plan.top_k)
            .or(scaling.top_k)
            .unwrap_or(self.config.search_top_k);

        // Step 2: Search for relevant chunks (with fallback across modes)
        let cli_locked_mode = overrides.search_mode.is_some();
        let search_results = Self::search_with_fallback(
            storage,
            query,
            buffer_name,
            search_mode,
            threshold,
            top_k,
            cli_locked_mode,
        )?;

        let chunks_available = search_results.len();

        // Step 3: Load chunk content (must happen on the sync thread)
        let (chunks, chunk_load_failures) = Self::load_chunks(storage, &search_results, max_chunks);

        if chunks.is_empty() {
            return Err(AgentError::NoChunks {
                hint: format!(
                    "Search found {chunks_available} results but all chunks \
                     failed to load from storage ({chunk_load_failures} failures). \
                     The database may be corrupted."
                ),
            });
        }

        // Resolve batch_size: num_agents takes priority over batch_size
        // Resolution: CLI → Plan → Scaling → Config → Default
        let batch_size = if let Some(agents) = overrides.num_agents {
            // ceil(chunks / agents)
            let agents = agents.max(1);
            chunks.len().div_ceil(agents)
        } else {
            overrides
                .batch_size
                .or(plan.batch_size)
                .or(scaling.batch_size)
                .unwrap_or(self.config.batch_size)
        };

        // Resolve concurrency from scaling profile
        let max_concurrency = scaling
            .max_concurrency
            .unwrap_or(self.config.max_concurrency);

        // Wrap chunks in Arc to share across fan-out tasks without cloning
        let shared_chunks: Arc<[LoadedChunk]> = Arc::from(chunks.into_boxed_slice());

        // Step 4: Fan out across batches (with scaled concurrency)
        let subcall_results = self
            .fan_out(
                query,
                Arc::clone(&shared_chunks),
                batch_size,
                max_concurrency,
            )
            .await;

        // Build chunk metadata lookup for stamping findings
        let chunk_meta: std::collections::HashMap<i64, (usize, i64)> = shared_chunks
            .iter()
            .map(|c| (c.chunk_id, (c.index, c.buffer_id)))
            .collect();

        // Step 5: Collect findings
        let mut all_findings: Vec<Finding> = Vec::new();
        let mut total_tokens: u32 = 0;
        let mut batches_processed: usize = 0;
        let mut batches_failed: usize = 0;
        let mut batch_errors: Vec<String> = Vec::new();

        let batch_size_used = batch_size.max(1);
        for (idx, result) in subcall_results.iter().enumerate() {
            match result {
                Ok(sr) => {
                    batches_processed += 1;
                    total_tokens = total_tokens.saturating_add(sr.usage.total_tokens);
                    all_findings.extend(sr.findings.iter().cloned());
                }
                Err(e) => {
                    batches_failed += 1;
                    // Include chunk IDs from the failed batch for diagnostics
                    let range_start = idx * batch_size_used;
                    let range_end = (range_start + batch_size_used).min(shared_chunks.len());
                    let ids: Vec<i64> = shared_chunks[range_start..range_end]
                        .iter()
                        .map(|c| c.chunk_id)
                        .collect();
                    batch_errors.push(format!("batch {idx} (chunks {ids:?}): {e}"));
                }
            }
        }

        // Stamp temporal metadata onto findings from chunk lookup
        for finding in &mut all_findings {
            if let Some(&(index, buffer_id)) = chunk_meta.get(&finding.chunk_id) {
                finding.chunk_index = Some(index);
                finding.chunk_buffer_id = Some(buffer_id);
            }
        }

        // Filter to relevant findings
        let finding_threshold = overrides.finding_threshold.unwrap_or(Relevance::Low);
        let pre_filter_count = all_findings.len();
        all_findings.retain(|f| f.relevance.meets_threshold(finding_threshold));
        let findings_filtered = pre_filter_count - all_findings.len();

        // Sort by relevance (high first), then by temporal position
        all_findings.sort_by(|a, b| {
            a.relevance.cmp(&b.relevance).then_with(|| {
                a.chunk_buffer_id
                    .cmp(&b.chunk_buffer_id)
                    .then_with(|| a.chunk_index.cmp(&b.chunk_index))
            })
        });

        let findings_count = all_findings.len();

        // Step 6: Synthesize (with tool-calling support)
        let executor = ToolExecutor::new(storage);
        let response = if all_findings.is_empty() {
            "No relevant findings were identified for the query.".to_string()
        } else {
            let (synthesis, synth_response) =
                self.synthesize(query, &all_findings, &executor).await?;
            total_tokens = total_tokens.saturating_add(synth_response.usage.total_tokens);
            synthesis
        };

        Ok(QueryResult {
            response,
            scaling_tier: scaling.tier.to_string(),
            findings_count,
            findings_filtered,
            chunks_analyzed: shared_chunks.len(),
            analyzed_chunk_ids: shared_chunks.iter().map(|c| c.chunk_id).collect(),
            chunks_available,
            batches_processed,
            batches_failed,
            chunk_load_failures,
            batch_errors,
            total_tokens,
            elapsed: start.elapsed(),
        })
    }

    /// Plans the analysis strategy using the primary agent.
    #[allow(clippy::future_not_send)]
    async fn plan_analysis(
        &self,
        storage: &SqliteStorage,
        query: &str,
        buffer_name: Option<&str>,
    ) -> Result<(super::finding::AnalysisPlan, super::traits::AgentResponse), AgentError> {
        let (chunk_count, buffer_size) = if let Some(name) = buffer_name {
            let buffer = storage
                .get_buffer_by_name(name)
                .map_err(|e| AgentError::Orchestration {
                    message: format!("Failed to get buffer: {e}"),
                })?
                .ok_or_else(|| AgentError::Orchestration {
                    message: format!("Buffer not found: {name}"),
                })?;
            let buffer_id = buffer.id.ok_or_else(|| AgentError::Orchestration {
                message: "Buffer has no ID".to_string(),
            })?;
            let chunks = storage
                .get_chunks(buffer_id)
                .map_err(|e| AgentError::Orchestration {
                    message: format!("Failed to get chunks: {e}"),
                })?;
            (chunks.len(), buffer.content.len())
        } else {
            (0, 0)
        };

        let user_msg = build_primary_prompt(query, chunk_count, None, buffer_size);
        let primary = PrimaryAgent::new(&self.config, self.prompts.primary.clone());
        primary.plan(&*self.provider, &user_msg, true).await
    }

    /// Searches with automatic fallback across modes when the initial
    /// mode returns zero results. If the CLI explicitly locked the mode,
    /// no fallback is attempted.
    fn search_with_fallback(
        storage: &SqliteStorage,
        query: &str,
        buffer_name: Option<&str>,
        initial_mode: &str,
        threshold: f32,
        top_k: usize,
        cli_locked: bool,
    ) -> Result<Vec<SearchResult>, AgentError> {
        let results =
            Self::search_chunks(storage, query, buffer_name, initial_mode, threshold, top_k)?;
        if !results.is_empty() || cli_locked {
            if results.is_empty() {
                let buf_hint =
                    buffer_name.map_or_else(String::new, |b| format!(" in buffer '{b}'"));
                return Err(AgentError::NoChunks {
                    hint: format!(
                        "Search returned 0 results{buf_hint} \
                         (mode={initial_mode}, threshold={threshold}, top_k={top_k}). \
                         Try: lowering --threshold, switching --search-mode, \
                         or running `rlm-rs embed` if chunks lack embeddings."
                    ),
                });
            }
            return Ok(results);
        }

        // Fallback order: hybrid → bm25 → semantic (skipping the already-tried mode)
        let fallbacks: &[&str] = &["hybrid", "bm25", "semantic"];
        for &mode in fallbacks {
            if mode == initial_mode {
                continue;
            }
            if let Ok(fallback) =
                Self::search_chunks(storage, query, buffer_name, mode, threshold, top_k)
                && !fallback.is_empty()
            {
                return Ok(fallback);
            }
        }

        let buf_hint = buffer_name.map_or_else(String::new, |b| format!(" in buffer '{b}'"));
        Err(AgentError::NoChunks {
            hint: format!(
                "Search returned 0 results{buf_hint} after trying all modes \
                 (threshold={threshold}, top_k={top_k}). \
                 Try: lowering --threshold or running `rlm-rs embed` if chunks lack embeddings."
            ),
        })
    }

    /// Searches for relevant chunks using the existing search infrastructure.
    fn search_chunks(
        storage: &SqliteStorage,
        query: &str,
        buffer_name: Option<&str>,
        search_mode: &str,
        threshold: f32,
        top_k: usize,
    ) -> Result<Vec<SearchResult>, AgentError> {
        let buffer_id = if let Some(name) = buffer_name {
            let buffer = storage
                .get_buffer_by_name(name)
                .map_err(|e| AgentError::Orchestration {
                    message: format!("Buffer lookup failed: {e}"),
                })?
                .ok_or_else(|| AgentError::Orchestration {
                    message: format!("Buffer not found: {name}"),
                })?;
            buffer.id
        } else {
            None
        };

        let embedder =
            crate::embedding::create_embedder().map_err(|e| AgentError::Orchestration {
                message: format!("Embedder creation failed: {e}"),
            })?;

        let config = SearchConfig::new()
            .with_top_k(top_k)
            .with_threshold(threshold)
            .with_mode(search_mode)
            .with_buffer_id(buffer_id);

        crate::search::hybrid_search(storage, &*embedder, query, &config).map_err(|e| {
            AgentError::Orchestration {
                message: format!("Search failed: {e}"),
            }
        })
    }

    /// Loads chunk content from storage, preserving search metadata.
    ///
    /// Returns the loaded chunks sorted in temporal order
    /// `(buffer_id, index)` and the number of chunks that failed to load.
    /// Must run on the sync thread because `rusqlite::Connection` is
    /// `!Send`.
    fn load_chunks(
        storage: &SqliteStorage,
        search_results: &[SearchResult],
        max_chunks: usize,
    ) -> (Vec<LoadedChunk>, usize) {
        let limit = if max_chunks > 0 {
            max_chunks
        } else {
            search_results.len()
        };

        let mut chunks = Vec::with_capacity(limit);
        let mut failures: usize = 0;

        for result in search_results.iter().take(limit) {
            match storage.get_chunk(result.chunk_id) {
                Ok(Some(chunk)) => {
                    chunks.push(LoadedChunk {
                        chunk_id: chunk.id.unwrap_or(result.chunk_id),
                        buffer_id: result.buffer_id,
                        index: result.index,
                        score: result.score,
                        semantic_score: result.semantic_score,
                        bm25_score: result.bm25_score,
                        content: chunk.content,
                    });
                }
                Ok(None) | Err(_) => {
                    failures += 1;
                }
            }
        }

        // Sort by temporal position: (buffer_id, index within buffer)
        chunks.sort_by(|a, b| {
            a.buffer_id
                .cmp(&b.buffer_id)
                .then_with(|| a.index.cmp(&b.index))
        });

        (chunks, failures)
    }

    /// Builds a [`DatasetProfile`] from storage metadata.
    ///
    /// When a buffer is specified, counts chunks and bytes for that buffer.
    /// Otherwise returns zero (the scaling profile will use Tiny defaults,
    /// which are conservative and safe).
    fn build_dataset_profile(storage: &SqliteStorage, buffer_name: Option<&str>) -> DatasetProfile {
        buffer_name.map_or(
            DatasetProfile {
                chunk_count: 0,
                total_bytes: 0,
            },
            |name| {
                let (chunk_count, total_bytes) = storage
                    .get_buffer_by_name(name)
                    .ok()
                    .flatten()
                    .and_then(|buf| {
                        let id = buf.id?;
                        let chunks = storage.get_chunks(id).ok()?;
                        let bytes: usize = chunks.iter().map(|c| c.content.len()).sum();
                        Some((chunks.len(), bytes))
                    })
                    .unwrap_or((0, 0));
                DatasetProfile {
                    chunk_count,
                    total_bytes,
                }
            },
        )
    }

    /// Fans out subcall agents concurrently across batches.
    ///
    /// Chunk data is shared via `Arc` to avoid cloning per task.
    /// Takes an `Arc` directly to avoid re-cloning when the caller
    /// already owns the data. The `max_concurrency` parameter comes
    /// from the adaptive scaling profile.
    async fn fan_out(
        &self,
        query: &str,
        shared_chunks: Arc<[LoadedChunk]>,
        batch_size: usize,
        max_concurrency: usize,
    ) -> Vec<Result<SubagentResult, AgentError>> {
        let semaphore = Arc::new(Semaphore::new(max_concurrency));
        let provider = Arc::clone(&self.provider);
        let config = self.config.clone();
        let query = query.to_string();
        let subcall_prompt = self.prompts.subcall.clone();

        let batch_ranges: Vec<(usize, usize, usize)> = shared_chunks
            .chunks(batch_size.max(1))
            .enumerate()
            .map(|(idx, slice)| {
                let start = idx * batch_size.max(1);
                (idx, start, start + slice.len())
            })
            .collect();

        let mut handles = Vec::with_capacity(batch_ranges.len());

        for (batch_idx, range_start, range_end) in batch_ranges {
            let sem = Arc::clone(&semaphore);
            let prov = Arc::clone(&provider);
            let cfg = config.clone();
            let q = query.clone();
            let prompt = subcall_prompt.clone();
            let chunks_ref = Arc::clone(&shared_chunks);

            let request_delay = self.config.request_delay;
            let handle = tokio::spawn(async move {
                let _permit = sem.acquire().await.map_err(|e| AgentError::Orchestration {
                    message: format!("Semaphore acquire failed: {e}"),
                })?;

                if !request_delay.is_zero() {
                    tokio::time::sleep(request_delay).await;
                }

                let start = Instant::now();
                let agent = SubcallAgent::new(&cfg, prompt);

                // Build prompt from shared slice with full context
                let batch = &chunks_ref[range_start..range_end];
                let chunk_refs: Vec<ChunkContext<'_>> = batch
                    .iter()
                    .map(|c| ChunkContext {
                        chunk_id: c.chunk_id,
                        buffer_id: c.buffer_id,
                        index: c.index,
                        score: c.score,
                        content: &c.content,
                    })
                    .collect();
                let user_msg = build_subcall_prompt(&q, &chunk_refs);

                let (findings, response) = agent.execute_and_parse(&*prov, &user_msg).await?;

                Ok(SubagentResult {
                    batch_index: batch_idx,
                    findings,
                    usage: response.usage,
                    elapsed: start.elapsed(),
                })
            });

            handles.push(handle);
        }

        // Collect results
        let expected = handles.len();
        let mut results = Vec::with_capacity(expected);
        for handle in handles {
            match handle.await {
                Ok(result) => results.push(result),
                Err(e) => results.push(Err(AgentError::Orchestration {
                    message: format!("Task join failed: {e}"),
                })),
            }
        }

        debug_assert_eq!(
            results.len(),
            expected,
            "Batch result count mismatch: expected {expected}, got {}",
            results.len()
        );

        results
    }

    /// Synthesizes findings into a final response.
    ///
    /// When the synthesizer agent has tools configured, runs the agentic
    /// loop so it can call back into storage/search. Otherwise falls
    /// through to a single-shot execution.
    #[allow(clippy::future_not_send)]
    async fn synthesize(
        &self,
        query: &str,
        findings: &[Finding],
        executor: &ToolExecutor<'_>,
    ) -> Result<(String, super::traits::AgentResponse), AgentError> {
        let user_msg = build_synthesizer_prompt(query, findings);
        let agent = SynthesizerAgent::new(&self.config, self.prompts.synthesizer.clone());
        let response = execute_with_tools(&agent, &*self.provider, &user_msg, executor).await?;
        let content = response.content.clone();
        Ok((content, response))
    }
}

/// CLI overrides for query parameters.
///
/// # Parameter Resolution
///
/// Each parameter is resolved in priority order: **CLI flag → Plan → Config → Default**.
/// The primary agent's plan is skipped when `skip_plan` is set.
///
/// # Key Interactions
///
/// - **`num_agents` vs `batch_size`**: Mutually exclusive. When `num_agents` is set,
///   batch size is computed as `ceil(chunks / num_agents)`, ignoring `batch_size`.
/// - **`top_k` vs `max_chunks`**: `top_k` controls how many results the search layer
///   returns; `max_chunks` limits how many of those are actually loaded and sent to
///   agents. Set `top_k >= max_chunks` to avoid fetching results that are immediately
///   discarded.
/// - **`threshold` vs `finding_threshold`**: `threshold` filters at the search layer
///   (similarity score), while `finding_threshold` filters after subcall agents return
///   (relevance assessment). Both reduce work for the synthesizer but at different
///   pipeline stages.
/// - **`skip_plan`**: When all search parameters are specified via CLI flags, skipping
///   the plan saves one LLM round-trip. If parameters are omitted, the planner fills
///   them in — so skipping the plan uses config defaults instead.
#[derive(Debug, Clone, Default)]
pub struct CliOverrides {
    /// Override search mode (`hybrid`, `semantic`, or `bm25`).
    pub search_mode: Option<String>,
    /// Override batch size (chunks per agent). Ignored when `num_agents` is set.
    pub batch_size: Option<usize>,
    /// Override similarity threshold for the search layer.
    pub threshold: Option<f32>,
    /// Override max chunks to load from search results (0 = unlimited).
    pub max_chunks: Option<usize>,
    /// Override search depth (top-k results from the search layer).
    pub top_k: Option<usize>,
    /// Target number of concurrent subagents. When set, batch size is
    /// computed as `ceil(chunks / num_agents)`, overriding `batch_size`.
    pub num_agents: Option<usize>,
    /// Minimum relevance level for findings passed to the synthesizer.
    /// Defaults to `Low` (filters out `None`).
    pub finding_threshold: Option<Relevance>,
    /// Skip the primary agent planning step.
    pub skip_plan: bool,
}

impl std::fmt::Debug for Orchestrator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Orchestrator")
            .field("provider", &self.provider.name())
            .field("config", &self.config)
            .field("prompts", &self.prompts)
            .finish()
    }
}
