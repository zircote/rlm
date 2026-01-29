# RLM-RS CLI Reference

Complete command-line interface reference for `rlm-rs`.

> **Deprecated Aliases:** The following top-level commands still work but are deprecated
> and will print a warning directing you to the new grouped syntax:
> `load`, `list`/`ls`, `show`, `delete`/`rm`, `var`, `query`.
> Use `rlm-rs buffer load`, `rlm-rs buffer list`, `rlm-rs buffer show`,
> `rlm-rs buffer delete`, `rlm-rs context var`, and `rlm-rs agent query` instead.

## Global Options

These options apply to all commands:

| Option | Environment | Description |
|--------|-------------|-------------|
| `-d, --db-path <PATH>` | `RLM_DB_PATH` | Path to SQLite database (default: `.rlm/rlm-state.db`) |
| `-v, --verbose` | | Enable verbose output |
| `--format <FORMAT>` | | Output format: `text` (default), `json`, or `ndjson` |
| `-h, --help` | | Print help information |
| `-V, --version` | | Print version |

## Commands

### Database Management

#### `init`

Initialize the RLM database. Creates the database file and schema if they don't exist.

```bash
rlm-rs init [OPTIONS]
```

**Options:**
| Option | Description |
|--------|-------------|
| `-f, --force` | Force re-initialization (destroys existing data) |

**Examples:**
```bash
# Initialize new database
rlm-rs init

# Re-initialize (destroys existing data)
rlm-rs init --force
```

---

#### `status`

Show current RLM state including database info, buffer count, and statistics.

```bash
rlm-rs status
```

**Example Output:**
```
RLM Status
==========
Database: .rlm/rlm-state.db (245 KB)
Buffers: 3
Total chunks: 42
Variables: 2
```

**JSON Output:**
```bash
rlm-rs status --format json
```

---

#### `reset`

Delete all RLM state (buffers, chunks, variables). Use with caution.

```bash
rlm-rs reset [OPTIONS]
```

**Options:**
| Option | Description |
|--------|-------------|
| `-y, --yes` | Skip confirmation prompt |

**Examples:**
```bash
# Interactive reset (prompts for confirmation)
rlm-rs reset

# Non-interactive reset
rlm-rs reset --yes
```

---

### Search Operations

#### `search`

Search chunks using hybrid semantic + BM25 search with Reciprocal Rank Fusion (RRF).

```bash
rlm-rs search [OPTIONS] <QUERY>
```

**Arguments:**
| Argument | Description |
|----------|-------------|
| `<QUERY>` | Search query text |

**Options:**
| Option | Default | Description |
|--------|---------|-------------|
| `-k, --top-k <N>` | `10` | Maximum number of results |
| `-t, --threshold <SCORE>` | `0.3` | Minimum similarity threshold (0.0-1.0) |
| `-m, --mode <MODE>` | `hybrid` | Search mode: `hybrid`, `semantic`, `bm25` |
| `--rrf-k <K>` | `60` | RRF k parameter for rank fusion |
| `-b, --buffer <BUFFER>` | | Filter by buffer ID or name |
| `-p, --preview` | | Include content preview in results |
| `--preview-len <N>` | `150` | Preview length in characters |

**Search Modes:**

| Mode | Description |
|------|-------------|
| `hybrid` | Combines semantic and BM25 scores using RRF (recommended) |
| `semantic` | Vector similarity search using embeddings |
| `bm25` | Traditional full-text search with BM25 scoring |

**Examples:**
```bash
# Basic hybrid search
rlm-rs search "database connection errors"

# Search with more results
rlm-rs search "API endpoints" --top-k 20

# Semantic-only search
rlm-rs search "authentication flow" --mode semantic

# Search specific buffer
rlm-rs search "error handling" --buffer logs

# Search with content preview
rlm-rs search "auth" --preview --preview-len 200

# JSON output for programmatic use
rlm-rs --format json search "your query" --top-k 10
```

**Output (JSON format):**
```json
{
  "count": 2,
  "mode": "hybrid",
  "query": "your query",
  "results": [
    {"chunk_id": 42, "score": 0.0328, "semantic_score": 0.0499, "bm25_score": 1.6e-6},
    {"chunk_id": 17, "score": 0.0323, "semantic_score": 0.0457, "bm25_score": 1.2e-6}
  ]
}
```

**Extract chunk IDs:** `jq -r '.results[].chunk_id'`

---

### Buffer Operations (`buffer`)

All buffer operations are accessed via the `rlm-rs buffer` subcommand group.

#### `buffer load`

Load a file into a buffer with automatic chunking and embedding generation.

Embeddings are automatically generated during load for semantic search support.

```bash
rlm-rs buffer load [OPTIONS] <FILE>
```

**Arguments:**
| Argument | Description |
|----------|-------------|
| `<FILE>` | Path to the file to load |

**Options:**
| Option | Default | Description |
|--------|---------|-------------|
| `-n, --name <NAME>` | filename | Custom name for the buffer |
| `-c, --chunker <STRATEGY>` | `semantic` | Chunking strategy: `fixed`, `semantic`, `code`, `parallel` |
| `--chunk-size <SIZE>` | `3000` | Chunk size in characters (~750 tokens) |
| `--overlap <SIZE>` | `500` | Overlap between chunks in characters |

**Chunking Strategies:**

| Strategy | Best For | Description |
|----------|----------|-------------|
| `semantic` | Markdown, prose | Splits at sentence/paragraph boundaries |
| `code` | Source code | Language-aware chunking at function/class boundaries |
| `fixed` | Logs, binary, raw text | Splits at exact character boundaries |
| `parallel` | Large files (>10MB) | Multi-threaded fixed chunking |

**Code Chunker Supported Languages:**
Rust, Python, JavaScript, TypeScript, Go, Java, C/C++, Ruby, PHP

**Examples:**
```bash
# Load with default settings (semantic chunking)
rlm-rs buffer load document.md

# Load with custom name
rlm-rs buffer load document.md --name my-docs

# Load with fixed chunking and custom size
rlm-rs buffer load logs.txt --chunker fixed --chunk-size 50000

# Load large file with parallel chunking
rlm-rs buffer load huge-file.txt --chunker parallel --chunk-size 100000 --overlap 1000
```

---

#### `buffer list` (alias: `buffer ls`)

List all buffers in the database.

```bash
rlm-rs buffer list
```

**Example Output:**
```
ID  Name           Size      Chunks  Created
1   document.md    125,432   4       2024-01-15 10:30:00
2   config.json    2,048     1       2024-01-15 10:35:00
3   logs.txt       1,048,576 26      2024-01-15 10:40:00
```

**JSON Output:**
```bash
rlm-rs buffer list --format json
```

---

#### `buffer show`

Show detailed information about a specific buffer.

```bash
rlm-rs buffer show [OPTIONS] <BUFFER>
```

**Arguments:**
| Argument | Description |
|----------|-------------|
| `<BUFFER>` | Buffer ID (number) or name |

**Options:**
| Option | Description |
|--------|-------------|
| `-c, --chunks` | Include chunk details |

**Examples:**
```bash
# Show buffer by name
rlm-rs buffer show document.md

# Show buffer by ID
rlm-rs buffer show 1

# Show buffer with chunk details
rlm-rs buffer show document.md --chunks
```

---

#### `buffer delete` (alias: `buffer rm`)

Delete a buffer and its associated chunks.

```bash
rlm-rs buffer delete [OPTIONS] <BUFFER>
```

**Arguments:**
| Argument | Description |
|----------|-------------|
| `<BUFFER>` | Buffer ID or name to delete |

**Options:**
| Option | Description |
|--------|-------------|
| `-y, --yes` | Skip confirmation prompt |

**Examples:**
```bash
# Delete with confirmation
rlm-rs buffer delete document.md

# Delete without confirmation
rlm-rs buffer delete 1 --yes
```

---

#### `buffer add`

Create a new buffer from text content. Useful for storing intermediate results.

```bash
rlm-rs buffer add <NAME> [CONTENT]
```

**Arguments:**
| Argument | Description |
|----------|-------------|
| `<NAME>` | Name for the new buffer |
| `[CONTENT]` | Text content (reads from stdin if omitted) |

**Examples:**
```bash
# Add buffer with inline content
rlm-rs buffer add summary "This is the summary of chunk 1..."

# Add buffer from stdin
echo "Content from pipe" | rlm-rs buffer add piped-content

# Add buffer from file via stdin
cat results.txt | rlm-rs buffer add results
```

---

#### `buffer update`

Update an existing buffer with new content, re-chunking and optionally re-embedding.

```bash
rlm-rs buffer update [OPTIONS] <BUFFER> [CONTENT]
```

**Arguments:**
| Argument | Description |
|----------|-------------|
| `<BUFFER>` | Buffer ID or name |
| `[CONTENT]` | New content (reads from stdin if omitted) |

**Options:**
| Option | Default | Description |
|--------|---------|-------------|
| `-e, --embed` | | Automatically embed new chunks after update |
| `--strategy <STRATEGY>` | `semantic` | Chunking strategy |
| `--chunk-size <SIZE>` | `3000` | Chunk size in characters |
| `--overlap <SIZE>` | `500` | Overlap between chunks |

**Examples:**
```bash
# Update from stdin
cat updated.txt | rlm-rs buffer update main-source

# Update with inline content
rlm-rs buffer update my-buffer "new content here"

# Update and re-embed
rlm-rs buffer update my-buffer --embed

# Update with custom chunking
cat new_code.rs | rlm-rs buffer update code-buffer --strategy code
```

---

#### `buffer export`

Export all buffers to a file (JSON format).

```bash
rlm-rs buffer export [OPTIONS]
```

**Options:**
| Option | Description |
|--------|-------------|
| `-o, --output <FILE>` | Output file path (stdout if omitted) |
| `-p, --pretty` | Pretty-print JSON output |

**Examples:**
```bash
# Export to stdout
rlm-rs buffer export --format json

# Export to file
rlm-rs buffer export --output backup.json --pretty
```

---

#### `buffer peek`

View a slice of buffer content without loading the entire buffer.

```bash
rlm-rs buffer peek [OPTIONS] <BUFFER>
```

**Arguments:**
| Argument | Description |
|----------|-------------|
| `<BUFFER>` | Buffer ID or name |

**Options:**
| Option | Default | Description |
|--------|---------|-------------|
| `--start <OFFSET>` | `0` | Start offset in bytes |
| `--end <OFFSET>` | `start + 3000` | End offset in bytes |

**Examples:**
```bash
# View first 3000 bytes (default)
rlm-rs buffer peek document.md

# View specific range
rlm-rs buffer peek document.md --start 1000 --end 5000

# View from offset to default length
rlm-rs buffer peek document.md --start 10000
```

---

#### `buffer grep`

Search buffer content using regular expressions.

```bash
rlm-rs buffer grep [OPTIONS] <BUFFER> <PATTERN>
```

**Arguments:**
| Argument | Description |
|----------|-------------|
| `<BUFFER>` | Buffer ID or name |
| `<PATTERN>` | Regular expression pattern |

**Options:**
| Option | Default | Description |
|--------|---------|-------------|
| `-n, --max-matches <N>` | `20` | Maximum matches to return |
| `-w, --window <SIZE>` | `120` | Context characters around each match |
| `-i, --ignore-case` | | Case-insensitive search |

**Examples:**
```bash
# Basic search
rlm-rs buffer grep document.md "error"

# Case-insensitive search
rlm-rs buffer grep document.md "TODO" --ignore-case

# Regex pattern with context
rlm-rs buffer grep logs.txt "ERROR.*timeout" --window 200 --max-matches 50

# Search by buffer ID
rlm-rs buffer grep 1 "function.*async"
```

---

### Chunk Operations (`chunk`)

All chunk operations are accessed via the `rlm-rs chunk` subcommand group.

#### `chunk get`

Get a chunk by ID (primary pass-by-reference mechanism for subagents).

```bash
rlm-rs chunk get [OPTIONS] <ID>
```

**Arguments:**
| Argument | Description |
|----------|-------------|
| `<ID>` | Chunk ID (globally unique across all buffers) |

**Options:**
| Option | Description |
|--------|-------------|
| `-m, --metadata` | Include metadata in output |

**Examples:**
```bash
# Get chunk content
rlm-rs chunk get 42

# Get chunk with metadata (JSON)
rlm-rs --format json chunk get 42 --metadata
```

---

#### `chunk list`

List all chunks for a buffer.

```bash
rlm-rs chunk list <BUFFER>
```

**Arguments:**
| Argument | Description |
|----------|-------------|
| `<BUFFER>` | Buffer ID or name |

**Examples:**
```bash
# List chunks for buffer
rlm-rs chunk list docs

# JSON output
rlm-rs --format json chunk list docs
```

---

#### `chunk embed`

Generate embeddings for buffer chunks. Note: Embeddings are automatically generated during `buffer load`, so this is typically only needed with `--force` to re-embed.

```bash
rlm-rs chunk embed [OPTIONS] <BUFFER>
```

**Arguments:**
| Argument | Description |
|----------|-------------|
| `<BUFFER>` | Buffer ID or name |

**Options:**
| Option | Description |
|--------|-------------|
| `-f, --force` | Force re-embedding even if embeddings exist |

**Examples:**
```bash
# Check if embeddings exist (will report "already embedded")
rlm-rs chunk embed docs

# Force re-embedding
rlm-rs chunk embed docs --force
```

---

#### `chunk status`

Show embedding status for all buffers.

```bash
rlm-rs chunk status
```

**Example Output:**
```
Embedding Status
================

Total: 42/42 chunks embedded

Buffer           ID    Chunks  Embedded
docs             1     15      15
logs             2     27      27
```

---

#### `chunk indices`

Calculate and display chunk boundaries for a buffer without writing files.

```bash
rlm-rs chunk indices [OPTIONS] <BUFFER>
```

**Arguments:**
| Argument | Description |
|----------|-------------|
| `<BUFFER>` | Buffer ID or name |

**Options:**
| Option | Default | Description |
|--------|---------|-------------|
| `--chunk-size <SIZE>` | `3000` | Chunk size in characters |
| `--overlap <SIZE>` | `500` | Overlap between chunks |

**Examples:**
```bash
# Show chunk boundaries with defaults
rlm-rs chunk indices document.md

# Custom chunk size
rlm-rs chunk indices document.md --chunk-size 20000 --overlap 1000
```

---

#### `chunk write`

Split a buffer into chunk files for processing.

```bash
rlm-rs chunk write [OPTIONS] <BUFFER>
```

**Arguments:**
| Argument | Description |
|----------|-------------|
| `<BUFFER>` | Buffer ID or name |

**Options:**
| Option | Default | Description |
|--------|---------|-------------|
| `-o, --out-dir <DIR>` | `.rlm/chunks` | Output directory |
| `--chunk-size <SIZE>` | `3000` | Chunk size in characters |
| `--overlap <SIZE>` | `500` | Overlap between chunks |
| `--prefix <PREFIX>` | `chunk` | Filename prefix |

**Output Files:**
Files are named `{prefix}_{index}.txt` (e.g., `chunk_0.txt`, `chunk_1.txt`).

**Examples:**
```bash
# Write chunks with defaults
rlm-rs chunk write document.md

# Custom output directory and prefix
rlm-rs chunk write document.md --out-dir ./output --prefix doc

# Custom chunk size for smaller chunks
rlm-rs chunk write large.txt --chunk-size 20000 --overlap 500
```

---

### Context Operations (`context`)

All context variable operations are accessed via the `rlm-rs context` subcommand group.

#### `context var`

Manage context-scoped variables (persisted per session/context).

```bash
rlm-rs context var [OPTIONS] <NAME> [VALUE]
```

**Arguments:**
| Argument | Description |
|----------|-------------|
| `<NAME>` | Variable name |
| `[VALUE]` | Value to set (omit to get current value) |

**Options:**
| Option | Description |
|--------|-------------|
| `-d, --delete` | Delete the variable |

**Examples:**
```bash
# Set a variable
rlm-rs context var current_chunk 3

# Get a variable
rlm-rs context var current_chunk

# Delete a variable
rlm-rs context var current_chunk --delete
```

---

#### `context global`

Manage global variables (persisted across all contexts).

```bash
rlm-rs context global [OPTIONS] <NAME> [VALUE]
```

**Arguments:**
| Argument | Description |
|----------|-------------|
| `<NAME>` | Variable name |
| `[VALUE]` | Value to set (omit to get current value) |

**Options:**
| Option | Description |
|--------|-------------|
| `-d, --delete` | Delete the variable |

**Examples:**
```bash
# Set a global variable
rlm-rs context global project_name "my-project"

# Get a global variable
rlm-rs context global project_name

# Delete a global variable
rlm-rs context global project_name --delete
```

---

### Agent Operations (`agent`)

All agent operations are accessed via the `rlm-rs agent` subcommand group.

> **Requires:** `agent` feature flag and an OpenAI-compatible API key.

#### `agent query`

Analyze a buffer using the agentic LLM pipeline. Fans out analysis across chunks using concurrent subcall agents, then synthesizes findings into a unified report.

```bash
rlm-rs agent query <QUERY> [OPTIONS]
```

**Options:**
| Option | Description |
|--------|-------------|
| `-b, --buffer <NAME\|ID>` | Buffer to scope the analysis |
| `--concurrency <N>` | Maximum concurrent API calls (default: 50) |
| `--batch-size <N>` | Chunks per subcall batch (overrides plan) |
| `--subcall-model <MODEL>` | Model for subcall agents (default: `gpt-5-mini-2025-08-07`) |
| `--synthesizer-model <MODEL>` | Model for the synthesizer agent |
| `--search-mode <MODE>` | Search mode: `hybrid`, `semantic`, `bm25` |
| `--similarity-threshold <F>` | Minimum similarity threshold (default: 0.3) |
| `--max-chunks <N>` | Maximum chunks to analyze (0 = unlimited) |
| `--top-k <N>` | Search depth: max results from search layer |
| `--num-agents <N>` | Target concurrent subagents (auto-computes batch size) |
| `--finding-threshold <LEVEL>` | Minimum relevance: `none`, `low`, `medium`, `high` |
| `--prompt-dir <DIR>` | Directory containing prompt template files |
| `--skip-plan` | Skip the planning step (use CLI flags directly) |
| `-v, --verbose` | Show diagnostics: chunk IDs, batch errors, timing |

**Environment Variables:**
| Variable | Description |
|----------|-------------|
| `OPENAI_API_KEY` or `RLM_API_KEY` | API key for the LLM provider |
| `RLM_SUBCALL_MODEL` | Default subcall model |
| `RLM_SYNTHESIZER_MODEL` | Default synthesizer model |
| `RLM_PRIMARY_MODEL` | Default primary/planning model |
| `RLM_BATCH_SIZE` | Default batch size |
| `RLM_PROMPT_DIR` | Default prompt template directory |

**Examples:**
```bash
# Basic query
OPENAI_API_KEY=sk-... rlm-rs agent query "explain the auth flow" --buffer api

# Scoped with verbose diagnostics
rlm-rs agent query "find error handling patterns" --buffer src --verbose

# Custom model and batch size
rlm-rs agent query "summarize architecture" --batch-size 5

# Skip planning for faster execution
rlm-rs agent query "list all endpoints" --buffer api --skip-plan --search-mode hybrid
```

---

#### `agent init-prompts`

Initialize prompt template files in the specified directory for customization.

```bash
rlm-rs agent init-prompts [OPTIONS]
```

**Options:**
| Option | Description |
|--------|-------------|
| `--prompt-dir <DIR>` | Directory to write prompt templates (default: `.rlm/prompts`) |

**Examples:**
```bash
# Initialize prompts in default location
rlm-rs agent init-prompts

# Initialize in custom directory
rlm-rs agent init-prompts --prompt-dir ./my-prompts
```

---

#### `agent dispatch`

Split chunks into batches for parallel subagent processing. Returns batch assignments with chunk IDs for orchestrator use.

```bash
rlm-rs agent dispatch [OPTIONS] <BUFFER>
```

**Arguments:**
| Argument | Description |
|----------|-------------|
| `<BUFFER>` | Buffer ID or name |

**Options:**
| Option | Default | Description |
|--------|---------|-------------|
| `--batch-size <N>` | `10` | Number of chunks per batch |
| `--workers <N>` | | Number of worker batches (alternative to batch-size) |
| `-q, --query <QUERY>` | | Filter to chunks matching this search query |
| `--mode <MODE>` | `hybrid` | Search mode for query filtering |
| `--threshold <SCORE>` | `0.3` | Minimum similarity threshold for filtering |

**Examples:**
```bash
# Dispatch all chunks in batches of 10
rlm-rs agent dispatch my-buffer

# Create 4 batches for 4 parallel workers
rlm-rs agent dispatch my-buffer --workers 4

# Only dispatch chunks relevant to a query
rlm-rs agent dispatch my-buffer --query "error handling"

# JSON output for orchestrator
rlm-rs --format json agent dispatch my-buffer
```

**Output (JSON format):**
```json
{
  "buffer_id": 1,
  "total_chunks": 42,
  "batch_count": 5,
  "batches": [
    {"batch_id": 0, "chunk_ids": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]},
    {"batch_id": 1, "chunk_ids": [11, 12, 13, 14, 15, 16, 17, 18, 19, 20]}
  ]
}
```

---

#### `agent aggregate`

Combine findings from analyst subagents. Reads JSON findings, filters by relevance, groups, and outputs a synthesizer-ready report.

```bash
rlm-rs agent aggregate [OPTIONS]
```

**Options:**
| Option | Default | Description |
|--------|---------|-------------|
| `-b, --buffer <BUFFER>` | | Read findings from a buffer (stdin if omitted) |
| `--min-relevance <LEVEL>` | `low` | Minimum relevance: `none`, `low`, `medium`, `high` |
| `--group-by <FIELD>` | `relevance` | Group by: `chunk_id`, `relevance`, `none` |
| `--sort-by <FIELD>` | `relevance` | Sort by: `relevance`, `chunk_id`, `findings_count` |
| `-o, --output-buffer <NAME>` | | Store results in a new buffer |

**Input Format (JSON array of analyst findings):**
```json
[
  {"chunk_id": 12, "relevance": "high", "findings": ["Bug found"], "summary": "Critical issue"},
  {"chunk_id": 27, "relevance": "medium", "findings": ["Minor issue"], "summary": "Needs review"}
]
```

**Examples:**
```bash
# Aggregate from stdin
cat findings.json | rlm-rs agent aggregate

# Read from buffer
rlm-rs agent aggregate --buffer analyst-findings

# Filter to high relevance only
rlm-rs agent aggregate --min-relevance high

# Store aggregated results
rlm-rs agent aggregate --output-buffer synthesis-input

# JSON output
rlm-rs --format json agent aggregate
```

---

## Configuration

### Default Chunk Sizes

| Parameter | Default | Description |
|-----------|---------|-------------|
| `chunk_size` | 3,000 chars | ~750 tokens (optimized for semantic search) |
| `overlap` | 500 chars | Context continuity between chunks |
| `max_chunk_size` | 50,000 chars | Maximum allowed chunk size |

### Environment Variables

| Variable | Description |
|----------|-------------|
| `RLM_DB_PATH` | Default database path |

---

## Exit Codes

| Code | Description |
|------|-------------|
| `0` | Success |
| `1` | General error |
| `2` | Invalid arguments |

---

## Output Formats

All commands support multiple output formats via `--format`:

| Format | Description |
|--------|-------------|
| `text` | Human-readable text (default) |
| `json` | JSON for programmatic use |
| `ndjson` | Newline-delimited JSON for streaming |

```bash
# Status as JSON
rlm-rs status --format json

# List buffers as JSON
rlm-rs buffer list --format json

# Search results as JSON
rlm-rs buffer grep document.md "pattern" --format json

# NDJSON for streaming pipelines
rlm-rs --format ndjson chunk list my-buffer
```

---

## See Also

- [Agent Guide](agent-guide.md) - Query engine pipeline, adaptive scaling, and configuration
- [README.md](../README.md) - Project overview and quick start
- [Architecture](architecture.md) - Internal architecture documentation
- [RLM Paper](https://arxiv.org/abs/2512.24601) - Recursive Language Model pattern
