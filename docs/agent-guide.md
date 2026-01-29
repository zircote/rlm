# Agent Query Engine

> Requires the `agent` feature: `rlm-rs = { version = "2.0", features = ["agent"] }`

The query engine lets you ask natural-language questions about content stored
in RLM buffers. It coordinates multiple LLM calls to plan a search strategy,
extract findings from chunks in parallel, and synthesize a final answer.

## Contents

- [How a Query Works](#how-a-query-works)
  - [Pipeline in Detail](#pipeline-in-detail)
  - [Data Flow Diagram](#data-flow-diagram)
  - [Output](#output)
  - [Error Handling](#error-handling)
- [Running a Query](#running-a-query)
- [Adaptive Scaling](#adaptive-scaling)
- [Parameter Tuning](#parameter-tuning)
- [Configuration](#configuration)
- [Custom Prompts](#custom-prompts)
- [Security Model](#security-model)
- [See Also](#see-also)

## How a Query Works

A single call to `rlm-rs agent query` triggers a six-step pipeline:

1. **Plan** — The orchestrator sends your question and buffer metadata to a
   planning agent. The planner decides the search algorithm, how many chunks
   to examine, and what topics to focus on.

2. **Search** — The chosen algorithm (hybrid, semantic, or BM25) retrieves
   the most relevant chunks from storage.

3. **Scale** — The pipeline inspects the dataset size and picks appropriate
   batch and concurrency settings. A 15-chunk README gets different treatment
   than a 3,000-chunk monorepo. See [Adaptive Scaling](#adaptive-scaling) below.

4. **Fan-out** — Chunks are split into batches and sent to analyst agents
   running concurrently. Each analyst extracts every relevant detail from its
   batch and returns structured findings.

5. **Collect** — Findings are gathered, stamped with temporal metadata
   (buffer and chunk position), filtered by relevance, and sorted.

6. **Synthesize** — A synthesis agent merges all findings into a single
   markdown report, with access to tool calls back into storage if it needs
   to look something up.

The pipeline is designed so each step can fail independently. Failed batches
are reported alongside successful ones, and the synthesizer works with
whatever findings survived.

### Pipeline in Detail

Each step has a specific role, model assignment, and failure mode:

**Step 1 — Plan** (`gpt-5.2-2025-12-11`)

The orchestrator sends your query, chunk count, and buffer byte size to the
planning agent. The planner returns a structured `AnalysisPlan` containing:

- `search_mode` — Which search algorithm to use (`hybrid`, `semantic`, `bm25`)
- `threshold` — Minimum similarity score for search results
- `top_k` — How many search results to retrieve
- `max_chunks` — How many chunks to actually load and analyze
- `batch_size` — How many chunks each analyst agent handles
- `focus_topics` — Topics the analysts should prioritize

Skippable with `--skip-plan`. When skipped, parameters come from CLI flags,
the scaling profile, config defaults, or compiled-in fallbacks.

**Step 2 — Search** (no LLM, runs locally)

Uses the existing `hybrid_search` infrastructure. Depending on the resolved
search mode:

- `hybrid` — Runs both semantic (cosine similarity against BGE-M3 embeddings)
  and BM25 (term frequency), then fuses results with Reciprocal Rank Fusion.
- `semantic` — Cosine similarity only. Best for conceptual questions.
- `bm25` — Keyword matching only. Best for exact strings like function names.

Results are ranked by score and filtered by threshold. If zero results survive,
the pipeline returns an error with actionable hints (lower threshold, switch
mode, or run `rlm-rs chunk embed`).

**Step 3 — Scale** (no LLM, pure computation)

The pipeline builds a `DatasetProfile` (chunk count + total bytes) and maps
it to a `ScalingProfile` via `compute_scaling_profile()`. This fills in any
parameters not set by the CLI or planner — batch size, concurrency ceiling,
search depth, and max chunks to load. See [Adaptive Scaling](#adaptive-scaling).

**Step 4 — Fan-out** (`gpt-5-mini-2025-08-07`, runs N times in parallel)

Chunks are split into batches and dispatched to analyst agents as concurrent
tokio tasks. Each analyst receives:

- The original query
- A batch of chunk content wrapped in `<content>` XML tags
- Chunk metadata (ID, buffer ID, position, search score)

Each analyst returns structured `Finding` objects containing:

- `summary` — What was found
- `evidence` — Supporting text from the chunk
- `relevance` — `Critical`, `High`, `Medium`, `Low`, or `None`
- `chunk_id` — Which chunk the finding came from
- `follow_ups` — Suggested deeper questions

A tokio semaphore caps concurrent API calls (from the scaling profile or
`RLM_MAX_CONCURRENCY`). An optional per-request delay (`request_delay`)
provides rate-limit headroom. Failed batches are recorded but do not abort
the pipeline — remaining findings proceed to synthesis.

**Step 5 — Collect** (no LLM)

Findings from all batches are gathered and post-processed:

1. Stamped with temporal metadata (chunk index and buffer ID) from a lookup
   table built during chunk loading.
2. Filtered by `finding_threshold` (default: `Low`, filtering out `None`).
3. Sorted by relevance (Critical first), then by temporal position
   (buffer ID, chunk index) to preserve document order within each
   relevance tier.

**Step 6 — Synthesize** (`gpt-5.2-2025-12-11`)

The synthesizer agent receives all surviving findings and the original query.
It merges findings into a coherent markdown report. The synthesizer has tool
access — it can call back into storage to retrieve specific chunks by ID or
run additional searches if the findings raise new questions.

The agentic tool loop (`execute_with_tools`) handles multi-turn tool calls
until the synthesizer produces a final text response.

### Data Flow Diagram

```
Query ─→ [Plan] ─→ [Search] ─→ [Scale] ─→ [Fan-out] ─→ [Collect] ─→ [Synthesize] ─→ Report
          │          │            │           │              │             │
          │          │            │           │              │             ├─ tool calls
          │          │            │           │              │             │  back to storage
          │          │            │           │              └─ filter + sort
          │          │            │           └─ N concurrent analysts
          │          │            └─ DatasetProfile → ScalingProfile
          │          └─ hybrid/semantic/BM25 search
          └─ AnalysisPlan (search params)
```

### Output

**Text mode** (default) prints the synthesized report followed by a status line:

```
Scale: medium | Chunks: 47/120 analyzed | Findings: 23 | Batches: 5 ok, 0 failed | Tokens: 18420 | Time: 4.2s
```

With `--verbose`, the analyzed chunk IDs are appended. Batch errors (if any)
are listed individually.

**JSON mode** (`--format json`) returns the full `QueryResult` struct:

```json
{
  "response": "...",
  "scaling_tier": "medium",
  "findings_count": 23,
  "findings_filtered": 2,
  "chunks_analyzed": 47,
  "analyzed_chunk_ids": [1, 5, 12, ...],
  "chunks_available": 120,
  "batches_processed": 5,
  "batches_failed": 0,
  "chunk_load_failures": 0,
  "batch_errors": [],
  "total_tokens": 18420,
  "elapsed": { "secs": 4, "nanos": 200000000 }
}
```

### Error Handling

The pipeline reports errors at each stage without aborting prematurely:

| Stage | Failure | Behavior |
|-------|---------|----------|
| Plan | LLM call fails | Returns `AgentError`, query aborts |
| Search | Zero results | Returns `AgentError::NoChunks` with hints |
| Load | Some chunks fail to load | Continues with loaded chunks; `chunk_load_failures` reported |
| Fan-out | Some batches fail | Continues with successful batches; failures in `batch_errors` |
| Collect | All findings filtered | Synthesizer receives empty input, returns canned message |
| Synthesize | LLM call fails | Returns `AgentError`, but findings were already extracted |

The philosophy: get as much useful output as possible. A query against 200
chunks where 3 batches fail still produces findings from the other batches.

## Running a Query

From the CLI:

```sh
# Basic query against a specific buffer
rlm-rs agent query "How is error handling implemented?" --buffer my-project

# Skip the planning step (saves one LLM round-trip)
rlm-rs agent query "find uses of unwrap()" --buffer src --skip-plan --search-mode bm25

# Control parallelism directly
rlm-rs agent query "summarize the architecture" --buffer docs --num-agents 4
```

From Rust:

```rust
let config = AgentConfig::from_env()?;
let provider = providers::create_provider(&config)?;
let orchestrator = Orchestrator::new(provider, config);

let result = orchestrator.query(&storage, "What error handling patterns are used?",
    Some("my-buffer"), None).await?;

eprintln!("[{}] {} chunks, {} findings, {:.1}s",
    result.scaling_tier, result.chunks_analyzed,
    result.findings_count, result.elapsed.as_secs_f64());
println!("{}", result.response);
```

## Adaptive Scaling

The pipeline automatically adapts to dataset size. Without scaling, a small
README and a 100 MB codebase would use the same batch size and concurrency —
wasting tokens on the small file and under-parallelizing the large one.

Scaling works by classifying the buffer into a tier based on chunk count,
then recommending batch size, concurrency, search depth, and chunk limits:

| Tier | Chunk Count | Batch Size | Concurrency | Search Depth | Max Loaded |
|------|------------|-----------|-------------|-------------|------------|
| Tiny | <20 | 1 (one agent per chunk) | 5 | all | all |
| Small | 20–99 | 5 | 15 | 100 | all |
| Medium | 100–499 | 10 | 30 | 200 | 100 |
| Large | 500–1999 | 20 | 60 | 400 | 200 |
| XLarge | 2000+ | 50 | 100 | 500 | 300 |

**Tiny** datasets get the most thorough treatment: each chunk gets its own
agent, so nothing is missed. **XLarge** datasets get aggressive scoping and
high parallelism — the search layer filters to the 500 best chunks, only 300
are loaded, and they are processed in batches of 50 with up to 100 concurrent
API calls.

Scaling recommendations are advisory. They slot into the parameter resolution
chain and can be overridden at any level:

```
CLI flags  →  LLM Plan  →  Scaling Profile  →  Config  →  Defaults
  (highest)                                                 (lowest)
```

If you pass `--batch-size 5`, that wins. If the planner suggests a batch size,
that wins over scaling. If neither specifies one, scaling fills it in based on
the data. If scaling has no opinion (returns `None`), the config default applies.

The scaling tier is reported in query output — look for `Scale: medium` in the
status line, or `scaling_tier` in JSON output.

## Parameter Tuning

The most common knobs to adjust, and when:

**Search mode** (`--search-mode`) — Use `bm25` for exact keyword searches
("find all `unwrap()` calls"), `semantic` for conceptual questions ("how does
authentication work?"), and `hybrid` (the default) when you're not sure.

**Threshold** (`--threshold`) — Controls how relevant a chunk must be to
survive the search layer. Lower values (0.1–0.2) cast a wide net for
exploratory queries. Higher values (0.4–0.6) focus on exact matches. The
planner adjusts this automatically, but you can override it.

**Num agents** (`--num-agents`) — Sets the target number of parallel analyst
agents. The pipeline divides chunks evenly across agents. Useful when you want
predictable cost: 4 agents means 4 API calls regardless of dataset size.
Mutually exclusive with `--batch-size`.

**Skip plan** (`--skip-plan`) — Saves one LLM round-trip by skipping the
planning agent. The pipeline uses CLI flags, scaling profile, or config
defaults instead. Worth using when you already know the search parameters
you want.

## Configuration

The agent system reads configuration from environment variables:

| Variable | Purpose | Default |
|----------|---------|---------|
| `OPENAI_API_KEY` or `RLM_API_KEY` | API authentication | *(required)* |
| `OPENAI_BASE_URL` or `RLM_BASE_URL` | API endpoint (for proxies or compatible APIs) | OpenAI |
| `RLM_PRIMARY_MODEL` | Model for the planning agent | `gpt-5.2-2025-12-11` |
| `RLM_SUBCALL_MODEL` | Model for analyst agents | `gpt-5-mini-2025-08-07` |
| `RLM_SYNTHESIZER_MODEL` | Model for the synthesis agent | `gpt-5.2-2025-12-11` |
| `RLM_MAX_CONCURRENCY` | Hard ceiling on concurrent API calls | `50` |
| `RLM_BATCH_SIZE` | Default chunks per batch | `10` |
| `RLM_SEARCH_TOP_K` | Default search depth | `200` |
| `RLM_PROMPT_DIR` | Directory for custom prompt files | `~/.config/rlm-rs/prompts/` |

For cost-sensitive workloads, use a smaller model for analysts (they run in
parallel, so cost scales with chunk count) and a stronger model for the
synthesizer (which runs once):

```sh
export RLM_SUBCALL_MODEL=gpt-5-mini-2025-08-07
export RLM_SYNTHESIZER_MODEL=gpt-5.2-2025-12-11
```

## Custom Prompts

Each agent stage uses a system prompt that can be replaced at runtime.
Drop a markdown file in the prompt directory and it takes effect on the
next query:

| File | Controls | Purpose |
|------|----------|---------|
| `primary.md` | Planning agent | How the query gets decomposed into a search plan |
| `subcall.md` | Analyst agents | What the extractors look for and how they structure findings |
| `synthesizer.md` | Synthesis agent | How findings get merged into a final report |

If a file is missing, the compiled-in default is used. See [`docs/prompts/`](prompts/)
for the full prompt specifications and design rationale.

## Security Model

The pipeline processes untrusted content (user documents) through LLM agents.
Several safeguards prevent content from influencing agent behavior:

- **XML isolation** — Chunk content is wrapped in `<content>` tags rather than
  markdown fences, making it harder for embedded text to break out of the
  content boundary.
- **Prompt instructions** — Agent system prompts explicitly instruct the LLM
  to treat chunk content as data, never as instructions.
- **Input validation** — Queries are length-limited (10 KB), regex patterns
  are size-capped (500 bytes) with DFA limits (1 MB) to prevent ReDoS, and
  tool arguments are bounded (100 KB).
- **Output sanitization** — Findings are capped per batch (200), per-finding
  text is truncated (5 KB), and follow-up suggestions are limited (10 per
  finding).

## See Also

- [API Reference](api.md) — Library types and traits
- [CLI Reference](cli-reference.md) — All commands and flags
- [Architecture](architecture.md) — Internal design
- [Prompt Specifications](prompts/) — Agent prompt design and rationale
