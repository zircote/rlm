//! Command-line argument parsing.
//!
//! Defines the CLI structure using clap derive macros.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::chunking::{DEFAULT_CHUNK_SIZE, DEFAULT_OVERLAP};

/// RLM-RS: Recursive Language Model REPL for Claude Code.
///
/// A CLI tool for handling large context files via chunking and
/// recursive sub-LLM calls.
#[derive(Parser, Debug)]
#[command(name = "rlm-rs")]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Path to the RLM database file.
    ///
    /// Defaults to `.rlm/rlm-state.db` in the current directory.
    #[arg(short, long, env = "RLM_DB_PATH")]
    pub db_path: Option<PathBuf>,

    /// Enable verbose output.
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Output format (text, json).
    #[arg(long, default_value = "text", global = true)]
    pub format: String,

    /// The subcommand to execute.
    #[command(subcommand)]
    pub command: Commands,
}

/// Available CLI commands.
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Initialize the RLM database.
    ///
    /// Creates the database file and schema if they don't exist.
    #[command(after_help = r#"Examples:
  rlm-rs init                    # Initialize in current directory
  rlm-rs init --force            # Re-initialize (destroys existing data)
  rlm-rs --db-path ./my.db init  # Initialize with custom path
"#)]
    Init {
        /// Force re-initialization (destroys existing data).
        #[arg(short, long)]
        force: bool,
    },

    /// Show current RLM state status.
    Status,

    /// Reset RLM state (delete all data).
    Reset {
        /// Skip confirmation prompt.
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Search chunks using hybrid semantic + BM25 search.
    ///
    /// Returns chunk IDs and scores. Use `chunk get <id>` to retrieve content.
    #[command(after_help = r#"Examples:
  rlm-rs search "error handling"                  # Hybrid search (default)
  rlm-rs search "authentication" -k 5             # Top 5 results
  rlm-rs search "config" --mode bm25              # BM25 keyword search only
  rlm-rs search "API" --mode semantic             # Semantic search only
  rlm-rs search "bug fix" --buffer main-source    # Filter by buffer
  rlm-rs search "auth" --preview                  # Include content preview
  rlm-rs --format json search "test" | jq '.results[].chunk_id'
"#)]
    Search {
        /// Search query text.
        query: String,

        /// Maximum number of results.
        #[arg(short = 'k', long, default_value = "10")]
        top_k: usize,

        /// Minimum similarity threshold (0.0-1.0).
        #[arg(short, long, default_value = "0.3")]
        threshold: f32,

        /// Search mode: hybrid, semantic, bm25.
        #[arg(short, long, default_value = "hybrid")]
        mode: String,

        /// RRF k parameter for rank fusion.
        #[arg(long, default_value = "60")]
        rrf_k: u32,

        /// Filter by buffer ID or name.
        #[arg(short, long)]
        buffer: Option<String>,

        /// Include content preview in results.
        #[arg(short, long)]
        preview: bool,

        /// Preview length in characters.
        #[arg(long, default_value = "150")]
        preview_len: usize,
    },

    /// Buffer operations (load, list, show, delete, add, update, export, peek, grep).
    #[command(subcommand)]
    Buffer(BufferCommands),

    /// Chunk operations (get, list, embed, status, indices, write).
    #[command(subcommand)]
    Chunk(ChunkCommands),

    /// Context variable operations (var, global).
    #[command(subcommand)]
    Context(ContextCommands),

    /// Agentic LLM operations (query, init-prompts, dispatch, aggregate).
    #[cfg(feature = "agent")]
    #[command(subcommand)]
    Agent(AgentCommands),

    /// Start MCP (Model Context Protocol) server.
    #[cfg(feature = "mcp")]
    #[command(subcommand)]
    Mcp(McpCommands),

    // ── Hidden deprecated aliases ────────────────────────────────
    // These keep old scripts working but are hidden from --help.
    // They will be removed in a future major version.
    /// Load a context file into a buffer.
    #[command(hide = true)]
    Load {
        /// Path to the context file.
        file: PathBuf,

        /// Optional name for the buffer.
        #[arg(short, long)]
        name: Option<String>,

        /// Chunking strategy (fixed, semantic, code, parallel).
        #[arg(short, long, default_value = "semantic")]
        chunker: String,

        /// Chunk size in characters.
        #[arg(long, default_value_t = DEFAULT_CHUNK_SIZE)]
        chunk_size: usize,

        /// Overlap between chunks in characters.
        #[arg(long, default_value_t = DEFAULT_OVERLAP)]
        overlap: usize,
    },

    /// List all buffers.
    #[command(name = "list", alias = "ls", hide = true)]
    ListBuffers,

    /// Show buffer details.
    #[command(name = "show", hide = true)]
    ShowBuffer {
        /// Buffer ID or name.
        buffer: String,

        /// Show chunks as well.
        #[arg(short, long)]
        chunks: bool,
    },

    /// Delete a buffer.
    #[command(name = "delete", alias = "rm", hide = true)]
    DeleteBuffer {
        /// Buffer ID or name.
        buffer: String,

        /// Skip confirmation prompt.
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Set or get context variables.
    #[command(name = "var", hide = true)]
    Variable {
        /// Variable name.
        name: String,

        /// Value to set (omit to get current value).
        value: Option<String>,

        /// Delete the variable.
        #[arg(short, long)]
        delete: bool,
    },

    /// Query a buffer using agentic LLM analysis.
    #[cfg(feature = "agent")]
    #[command(name = "query", hide = true)]
    LegacyQuery {
        /// The query to analyze.
        query: String,

        /// Buffer to scope the analysis (ID or name).
        #[arg(short, long)]
        buffer: Option<String>,

        /// Maximum concurrent API calls.
        #[arg(long, default_value = "50")]
        concurrency: usize,

        /// Chunks per subcall batch.
        #[arg(long)]
        batch_size: Option<usize>,

        /// Model for subcall agents.
        #[arg(long)]
        subcall_model: Option<String>,

        /// Model for the synthesizer agent.
        #[arg(long)]
        synthesizer_model: Option<String>,

        /// Search mode (hybrid, semantic, bm25).
        #[arg(long)]
        search_mode: Option<String>,

        /// Minimum similarity threshold for search results.
        #[arg(long = "similarity-threshold")]
        similarity_threshold: Option<f32>,

        /// Maximum chunks to analyze (0 = unlimited).
        #[arg(long, default_value = "0")]
        max_chunks: usize,

        /// Search depth: maximum results retrieved from the search layer.
        #[arg(long)]
        top_k: Option<usize>,

        /// Target number of concurrent subagents.
        #[arg(long, conflicts_with = "batch_size")]
        num_agents: Option<usize>,

        /// Minimum relevance level for findings passed to the synthesizer.
        #[arg(long)]
        finding_threshold: Option<String>,

        /// Directory containing prompt template files.
        #[arg(long)]
        prompt_dir: Option<PathBuf>,

        /// Skip the planning step.
        #[arg(long)]
        skip_plan: bool,

        /// Show detailed diagnostics.
        #[arg(long, short)]
        verbose: bool,
    },
}

/// Buffer subcommands for managing loaded content.
#[derive(Subcommand, Debug)]
pub enum BufferCommands {
    /// Load a context file into a buffer.
    #[command(after_help = r#"Examples:
  rlm-rs buffer load large_file.txt                      # Load with semantic chunking
  rlm-rs buffer load src/main.rs --name main-source      # Load with custom name
  rlm-rs buffer load src/lib.rs --chunker code           # Code-aware chunking
  rlm-rs buffer load doc.md --chunker fixed --chunk-size 2000
  rlm-rs buffer load big.log --chunker parallel          # Parallel for large files
  rlm-rs --format json buffer load file.txt | jq '.buffer_id'
"#)]
    Load {
        /// Path to the context file.
        file: PathBuf,

        /// Optional name for the buffer.
        #[arg(short, long)]
        name: Option<String>,

        /// Chunking strategy (fixed, semantic, code, parallel).
        #[arg(short, long, default_value = "semantic")]
        chunker: String,

        /// Chunk size in characters.
        #[arg(long, default_value_t = DEFAULT_CHUNK_SIZE)]
        chunk_size: usize,

        /// Overlap between chunks in characters.
        #[arg(long, default_value_t = DEFAULT_OVERLAP)]
        overlap: usize,
    },

    /// List all buffers.
    #[command(alias = "ls")]
    #[command(after_help = r#"Examples:
  rlm-rs buffer list                            # List all buffers
  rlm-rs buffer ls                              # Alias for list
  rlm-rs --format json buffer list | jq '.[].name'
"#)]
    List,

    /// Show buffer details.
    #[command(after_help = r#"Examples:
  rlm-rs buffer show main-source                # Show buffer by name
  rlm-rs buffer show 1                          # Show buffer by ID
  rlm-rs buffer show 1 --chunks                 # Include chunk list
  rlm-rs --format json buffer show 1            # JSON output
"#)]
    Show {
        /// Buffer ID or name.
        buffer: String,

        /// Show chunks as well.
        #[arg(short, long)]
        chunks: bool,
    },

    /// Delete a buffer.
    #[command(alias = "rm")]
    Delete {
        /// Buffer ID or name.
        buffer: String,

        /// Skip confirmation prompt.
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Add text to a buffer (intermediate results).
    Add {
        /// Buffer name.
        name: String,

        /// Content to add (reads from stdin if not provided).
        content: Option<String>,
    },

    /// Update an existing buffer with new content.
    ///
    /// Re-chunks the buffer and incrementally updates embeddings.
    #[command(after_help = r#"Examples:
  cat updated.txt | rlm-rs buffer update main-source   # Update from stdin
  rlm-rs buffer update my-buffer "new content"         # Update with inline content
  rlm-rs buffer update my-buffer --embed               # Update and generate embeddings
  rlm-rs buffer update my-buffer --chunk-size 500      # Custom chunk size"#)]
    Update {
        /// Buffer ID or name.
        buffer: String,

        /// New content (reads from stdin if not provided).
        content: Option<String>,

        /// Automatically embed new chunks after update.
        #[arg(short, long)]
        embed: bool,

        /// Chunking strategy (semantic, fixed, parallel).
        #[arg(long, default_value = "semantic")]
        strategy: String,

        /// Chunk size in characters.
        #[arg(long, default_value_t = DEFAULT_CHUNK_SIZE)]
        chunk_size: usize,

        /// Chunk overlap in characters.
        #[arg(long, default_value_t = DEFAULT_OVERLAP)]
        overlap: usize,
    },

    /// Export all buffers to a file.
    Export {
        /// Output file path (stdout if not specified).
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Pretty-print if JSON format.
        #[arg(short, long)]
        pretty: bool,
    },

    /// Peek at buffer content.
    Peek {
        /// Buffer ID or name.
        buffer: String,

        /// Start offset in bytes.
        #[arg(long, default_value = "0")]
        start: usize,

        /// End offset in bytes (default: start + 3000).
        #[arg(long)]
        end: Option<usize>,
    },

    /// Search buffer content with regex.
    Grep {
        /// Buffer ID or name.
        buffer: String,

        /// Search pattern (regex).
        pattern: String,

        /// Maximum number of matches.
        #[arg(short = 'n', long, default_value = "20")]
        max_matches: usize,

        /// Context window size around matches.
        #[arg(short, long, default_value = "120")]
        window: usize,

        /// Case-insensitive search.
        #[arg(short, long)]
        ignore_case: bool,
    },
}

/// Chunk subcommands for pass-by-reference retrieval.
#[derive(Subcommand, Debug)]
pub enum ChunkCommands {
    /// Get a chunk by ID.
    ///
    /// Returns the chunk content and metadata. This is the primary
    /// pass-by-reference retrieval mechanism for subagents.
    #[command(after_help = r#"Examples:
  rlm-rs chunk get 42                    # Get chunk content
  rlm-rs chunk get 42 --metadata         # Include byte range, token count
  rlm-rs --format json chunk get 42      # JSON output for programmatic use
"#)]
    Get {
        /// Chunk ID.
        id: i64,

        /// Include metadata in output.
        #[arg(short, long)]
        metadata: bool,
    },

    /// List chunks for a buffer.
    #[command(after_help = r#"Examples:
  rlm-rs chunk list main-source          # List chunk IDs
  rlm-rs chunk list 1 --preview          # Show content preview
  rlm-rs --format json chunk list 1 | jq '.[].id'
"#)]
    List {
        /// Buffer ID or name.
        buffer: String,

        /// Show content preview.
        #[arg(short, long)]
        preview: bool,

        /// Preview length in characters.
        #[arg(long, default_value = "100")]
        preview_len: usize,
    },

    /// Generate embeddings for buffer chunks.
    #[command(after_help = r#"Examples:
  rlm-rs chunk embed main-source         # Generate embeddings
  rlm-rs chunk embed 1 --force           # Re-embed existing chunks
"#)]
    Embed {
        /// Buffer ID or name.
        buffer: String,

        /// Re-embed even if already embedded.
        #[arg(short, long)]
        force: bool,
    },

    /// Show embedding status for buffers.
    Status,

    /// Get chunk indices for a buffer.
    Indices {
        /// Buffer ID or name.
        buffer: String,

        /// Chunk size in characters.
        #[arg(long, default_value_t = DEFAULT_CHUNK_SIZE)]
        chunk_size: usize,

        /// Overlap between chunks in characters.
        #[arg(long, default_value_t = DEFAULT_OVERLAP)]
        overlap: usize,
    },

    /// Write chunks to files.
    Write {
        /// Buffer ID or name.
        buffer: String,

        /// Output directory.
        #[arg(short, long, default_value = ".rlm/chunks")]
        out_dir: PathBuf,

        /// Chunk size in characters.
        #[arg(long, default_value_t = DEFAULT_CHUNK_SIZE)]
        chunk_size: usize,

        /// Overlap between chunks in characters.
        #[arg(long, default_value_t = DEFAULT_OVERLAP)]
        overlap: usize,

        /// Filename prefix.
        #[arg(long, default_value = "chunk")]
        prefix: String,
    },
}

/// Context variable subcommands.
#[derive(Subcommand, Debug)]
pub enum ContextCommands {
    /// Set or get context variables.
    Var {
        /// Variable name.
        name: String,

        /// Value to set (omit to get current value).
        value: Option<String>,

        /// Delete the variable.
        #[arg(short, long)]
        delete: bool,
    },

    /// Set or get global variables.
    Global {
        /// Variable name.
        name: String,

        /// Value to set (omit to get current value).
        value: Option<String>,

        /// Delete the variable.
        #[arg(short, long)]
        delete: bool,
    },
}

/// MCP server subcommands.
#[cfg(feature = "mcp")]
#[derive(Subcommand, Debug)]
pub enum McpCommands {
    /// Start MCP server with stdio transport.
    ///
    /// Reads JSON-RPC messages from stdin, writes responses to stdout.
    /// This is the standard transport for Claude Code integration.
    #[command(after_help = r#"Examples:
  rlm-rs mcp stdio                         # Start stdio MCP server
  OPENAI_API_KEY=sk-... rlm-rs mcp stdio   # With API key
"#)]
    Stdio,

    /// Start MCP server with SSE/HTTP transport.
    ///
    /// Listens for incoming HTTP connections using streamable HTTP transport.
    #[command(after_help = r#"Examples:
  rlm-rs mcp sse                            # Listen on 127.0.0.1:3000
  rlm-rs mcp sse --host 0.0.0.0 --port 8080
"#)]
    Sse {
        /// Host to bind to.
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// Port to bind to.
        #[arg(long, default_value = "3000")]
        port: u16,
    },
}

/// Agent subcommands for agentic LLM workflows.
#[cfg(feature = "agent")]
#[derive(Subcommand, Debug)]
pub enum AgentCommands {
    /// Query a buffer using agentic LLM analysis.
    ///
    /// Fans out analysis across chunks using concurrent LLM subcalls,
    /// then synthesizes a coherent response. The synthesizer can call
    /// internal tools (`get_chunks`, `search`, `grep_chunks`) to verify and
    /// enrich findings. Requires an OpenAI-compatible API key.
    #[command(after_help = r#"Examples:
  rlm-rs agent query "How does error handling work?" --buffer main-source
  rlm-rs agent query "Find security vulnerabilities" --buffer app-code
  rlm-rs agent query "Summarize the architecture" --concurrency 20
  rlm-rs agent query "What patterns are used?" --search-mode semantic
  rlm-rs agent query "Compare X across all sections" --skip-plan --verbose
  OPENAI_API_KEY=sk-... rlm-rs agent query "explain auth flow" --buffer api
"#)]
    Query {
        /// The query to analyze.
        query: String,

        /// Buffer to scope the analysis (ID or name).
        #[arg(short, long)]
        buffer: Option<String>,

        /// Maximum concurrent API calls.
        #[arg(long, default_value = "50")]
        concurrency: usize,

        /// Chunks per subcall batch. When omitted, the primary agent's plan
        /// or config default is used.
        #[arg(long)]
        batch_size: Option<usize>,

        /// Model for subcall agents.
        #[arg(long)]
        subcall_model: Option<String>,

        /// Model for the synthesizer agent.
        #[arg(long)]
        synthesizer_model: Option<String>,

        /// Search mode (hybrid, semantic, bm25). When omitted, the primary
        /// agent's plan or config default is used.
        #[arg(long)]
        search_mode: Option<String>,

        /// Minimum similarity threshold for search results. When omitted,
        /// the primary agent's plan or config default (0.3) is used.
        #[arg(long = "similarity-threshold")]
        similarity_threshold: Option<f32>,

        /// Maximum chunks to analyze (0 = unlimited).
        #[arg(long, default_value = "0")]
        max_chunks: usize,

        /// Search depth: maximum results retrieved from the search layer
        /// before batching.
        #[arg(long)]
        top_k: Option<usize>,

        /// Target number of concurrent subagents. When set, batch size is
        /// computed automatically as `ceil(chunks / num_agents)`.
        #[arg(long, conflicts_with = "batch_size")]
        num_agents: Option<usize>,

        /// Minimum relevance level for findings passed to the synthesizer.
        /// Values: none, low, medium, high.
        #[arg(long)]
        finding_threshold: Option<String>,

        /// Directory containing prompt template files.
        #[arg(long)]
        prompt_dir: Option<PathBuf>,

        /// Skip the planning step (saves tokens and latency when all
        /// search parameters are specified via CLI flags).
        #[arg(long)]
        skip_plan: bool,

        /// Show detailed diagnostics: analyzed chunk IDs, batch errors,
        /// per-batch timing, and search parameters.
        #[arg(long, short)]
        verbose: bool,
    },

    /// Write default prompt templates to disk for customization.
    ///
    /// Creates markdown template files in the prompt directory so users
    /// can customize agent system prompts without recompiling.
    #[command(name = "init-prompts")]
    #[command(after_help = r#"Examples:
  rlm-rs agent init-prompts                              # Write to ~/.config/rlm-rs/prompts/
  rlm-rs agent init-prompts --dir ./my-prompts           # Write to custom directory
"#)]
    InitPrompts {
        /// Target directory for prompt templates.
        ///
        /// Defaults to `~/.config/rlm-rs/prompts/`.
        #[arg(long)]
        dir: Option<PathBuf>,
    },

    /// Dispatch chunks for parallel subagent processing.
    ///
    /// Splits chunks into batches suitable for parallel subagent analysis.
    /// Returns batch assignments with chunk IDs and metadata.
    #[command(after_help = r#"Examples:
  rlm-rs agent dispatch my-buffer                     # Dispatch all chunks
  rlm-rs agent dispatch my-buffer --batch-size 5      # 5 chunks per batch
  rlm-rs agent dispatch my-buffer --workers 4         # Split into 4 batches
  rlm-rs agent dispatch my-buffer --query "error"     # Only relevant chunks
  rlm-rs --format json agent dispatch my-buffer       # JSON for orchestrator"#)]
    Dispatch {
        /// Buffer ID or name.
        buffer: String,

        /// Number of chunks per batch (overrides --workers).
        #[arg(long, default_value = "10")]
        batch_size: usize,

        /// Number of worker batches to create (alternative to --batch-size).
        #[arg(long)]
        workers: Option<usize>,

        /// Filter to chunks matching this search query.
        #[arg(short, long)]
        query: Option<String>,

        /// Search mode for query filtering (hybrid, semantic, bm25).
        #[arg(long, default_value = "hybrid")]
        mode: String,

        /// Minimum similarity threshold for query filtering.
        #[arg(long, default_value = "0.3")]
        threshold: f32,
    },

    /// Aggregate findings from analyst subagents.
    ///
    /// Reads JSON findings from stdin or a buffer, groups by relevance,
    /// deduplicates, and outputs a synthesizer-ready report.
    #[command(after_help = r#"Examples:
  cat findings.json | rlm-rs agent aggregate           # Aggregate from stdin
  rlm-rs agent aggregate --buffer findings             # Read from buffer
  rlm-rs agent aggregate --min-relevance medium        # Filter low relevance
  rlm-rs --format json agent aggregate | jq '.findings'

Input format (JSON array of analyst findings):
[
  {"chunk_id": 12, "relevance": "high", "findings": ["..."], "summary": "..."},
  {"chunk_id": 27, "relevance": "medium", "findings": ["..."], "summary": "..."}
]"#)]
    Aggregate {
        /// Read findings from a buffer instead of stdin.
        #[arg(short, long)]
        buffer: Option<String>,

        /// Minimum relevance to include (none, low, medium, high).
        #[arg(long, default_value = "low")]
        min_relevance: String,

        /// Group findings by this field (`chunk_id`, `relevance`, `none`).
        #[arg(long, default_value = "relevance")]
        group_by: String,

        /// Sort findings by this field (`relevance`, `chunk_id`, `findings_count`).
        #[arg(long, default_value = "relevance")]
        sort_by: String,

        /// Store aggregated results in a new buffer with this name.
        #[arg(short, long)]
        output_buffer: Option<String>,
    },
}

impl Cli {
    /// Returns the database path, using the default if not specified.
    #[must_use]
    pub fn get_db_path(&self) -> PathBuf {
        self.db_path
            .clone()
            .unwrap_or_else(|| PathBuf::from(crate::storage::DEFAULT_DB_PATH))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn test_cli_parse() {
        // Test that CLI can be created
        Cli::command().debug_assert();
    }

    #[test]
    fn test_default_db_path() {
        let cli = Cli {
            db_path: None,
            verbose: false,
            format: "text".to_string(),
            command: Commands::Status,
        };
        assert_eq!(
            cli.get_db_path(),
            PathBuf::from(crate::storage::DEFAULT_DB_PATH)
        );
    }

    #[test]
    fn test_custom_db_path() {
        let cli = Cli {
            db_path: Some(PathBuf::from("/custom/path.db")),
            verbose: false,
            format: "text".to_string(),
            command: Commands::Status,
        };
        assert_eq!(cli.get_db_path(), PathBuf::from("/custom/path.db"));
    }
}
