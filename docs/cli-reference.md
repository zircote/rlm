# RLM-RS CLI Reference

Complete command-line interface reference for `rlm-rs`.

## Global Options

These options apply to all commands:

| Option | Environment | Description |
|--------|-------------|-------------|
| `-d, --db-path <PATH>` | `RLM_DB_PATH` | Path to SQLite database (default: `.rlm/rlm-state.db`) |
| `-v, --verbose` | | Enable verbose output |
| `--format <FORMAT>` | | Output format: `text` (default) or `json` |
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

### Buffer Operations

#### `load`

Load a file into a buffer with automatic chunking and embedding generation.

Embeddings are automatically generated during load for semantic search support.

```bash
rlm-rs load [OPTIONS] <FILE>
```

**Arguments:**
| Argument | Description |
|----------|-------------|
| `<FILE>` | Path to the file to load |

**Options:**
| Option | Default | Description |
|--------|---------|-------------|
| `-n, --name <NAME>` | filename | Custom name for the buffer |
| `-c, --chunker <STRATEGY>` | `semantic` | Chunking strategy: `fixed`, `semantic`, `parallel` |
| `--chunk-size <SIZE>` | `3000` | Chunk size in characters (~750 tokens) |
| `--overlap <SIZE>` | `500` | Overlap between chunks in characters |

**Chunking Strategies:**

| Strategy | Best For | Description |
|----------|----------|-------------|
| `semantic` | Markdown, code, prose | Splits at sentence/paragraph boundaries |
| `fixed` | Logs, binary, raw text | Splits at exact character boundaries |
| `parallel` | Large files (>10MB) | Multi-threaded fixed chunking |

**Examples:**
```bash
# Load with default settings (semantic chunking)
rlm-rs load document.md

# Load with custom name
rlm-rs load document.md --name my-docs

# Load with fixed chunking and custom size
rlm-rs load logs.txt --chunker fixed --chunk-size 50000

# Load large file with parallel chunking
rlm-rs load huge-file.txt --chunker parallel --chunk-size 100000 --overlap 1000
```

---

#### `list` (alias: `ls`)

List all buffers in the database.

```bash
rlm-rs list
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
rlm-rs list --format json
```

---

#### `show`

Show detailed information about a specific buffer.

```bash
rlm-rs show [OPTIONS] <BUFFER>
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
rlm-rs show document.md

# Show buffer by ID
rlm-rs show 1

# Show buffer with chunk details
rlm-rs show document.md --chunks
```

---

#### `delete` (alias: `rm`)

Delete a buffer and its associated chunks.

```bash
rlm-rs delete [OPTIONS] <BUFFER>
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
rlm-rs delete document.md

# Delete without confirmation
rlm-rs delete 1 --yes
```

---

#### `add-buffer`

Create a new buffer from text content. Useful for storing intermediate results.

```bash
rlm-rs add-buffer <NAME> [CONTENT]
```

**Arguments:**
| Argument | Description |
|----------|-------------|
| `<NAME>` | Name for the new buffer |
| `[CONTENT]` | Text content (reads from stdin if omitted) |

**Examples:**
```bash
# Add buffer with inline content
rlm-rs add-buffer summary "This is the summary of chunk 1..."

# Add buffer from stdin
echo "Content from pipe" | rlm-rs add-buffer piped-content

# Add buffer from file via stdin
cat results.txt | rlm-rs add-buffer results
```

---

#### `export-buffers`

Export all buffers to a file (JSON format).

```bash
rlm-rs export-buffers [OPTIONS]
```

**Options:**
| Option | Description |
|--------|-------------|
| `-o, --output <FILE>` | Output file path (stdout if omitted) |
| `-p, --pretty` | Pretty-print JSON output |

**Examples:**
```bash
# Export to stdout
rlm-rs export-buffers --format json

# Export to file
rlm-rs export-buffers --output backup.json --pretty
```

---

### Content Operations

#### `peek`

View a slice of buffer content without loading the entire buffer.

```bash
rlm-rs peek [OPTIONS] <BUFFER>
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
rlm-rs peek document.md

# View specific range
rlm-rs peek document.md --start 1000 --end 5000

# View from offset to default length
rlm-rs peek document.md --start 10000
```

---

#### `grep`

Search buffer content using regular expressions.

```bash
rlm-rs grep [OPTIONS] <BUFFER> <PATTERN>
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
rlm-rs grep document.md "error"

# Case-insensitive search
rlm-rs grep document.md "TODO" --ignore-case

# Regex pattern with context
rlm-rs grep logs.txt "ERROR.*timeout" --window 200 --max-matches 50

# Search by buffer ID
rlm-rs grep 1 "function.*async"
```

---

### Chunking Operations

#### `chunk-indices`

Calculate and display chunk boundaries for a buffer without writing files.

```bash
rlm-rs chunk-indices [OPTIONS] <BUFFER>
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
rlm-rs chunk-indices document.md

# Custom chunk size
rlm-rs chunk-indices document.md --chunk-size 20000 --overlap 1000
```

---

#### `write-chunks`

Split a buffer into chunk files for processing.

```bash
rlm-rs write-chunks [OPTIONS] <BUFFER>
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
rlm-rs write-chunks document.md

# Custom output directory and prefix
rlm-rs write-chunks document.md --out-dir ./output --prefix doc

# Custom chunk size for smaller chunks
rlm-rs write-chunks large.txt --chunk-size 20000 --overlap 500
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

### Chunk Operations

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

Generate embeddings for buffer chunks. Note: Embeddings are automatically generated during `load`, so this is typically only needed with `--force` to re-embed.

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

### Variable Operations

#### `var`

Manage context-scoped variables (persisted per session/context).

```bash
rlm-rs var [OPTIONS] <NAME> [VALUE]
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
rlm-rs var current_chunk 3

# Get a variable
rlm-rs var current_chunk

# Delete a variable
rlm-rs var current_chunk --delete
```

---

#### `global`

Manage global variables (persisted across all contexts).

```bash
rlm-rs global [OPTIONS] <NAME> [VALUE]
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
rlm-rs global project_name "my-project"

# Get a global variable
rlm-rs global project_name

# Delete a global variable
rlm-rs global project_name --delete
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

## JSON Output Format

All commands support `--format json` for machine-readable output:

```bash
# Status as JSON
rlm-rs status --format json

# List buffers as JSON
rlm-rs list --format json

# Search results as JSON
rlm-rs grep document.md "pattern" --format json
```

---

## See Also

- [README.md](../README.md) - Project overview and quick start
- [Architecture](architecture.md) - Internal architecture documentation
- [RLM Paper](https://arxiv.org/abs/2512.24601) - Recursive Language Model pattern
