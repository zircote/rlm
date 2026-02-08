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
#[command(name = "rlm-cli")]
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
  rlm-cli init                    # Initialize in current directory
  rlm-cli init --force            # Re-initialize (destroys existing data)
  rlm-cli --db-path ./my.db init  # Initialize with custom path
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

    /// Load a context file into a buffer.
    #[command(after_help = r#"Examples:
  rlm-cli load large_file.txt                      # Load with semantic chunking
  rlm-cli load src/main.rs --name main-source      # Load with custom name
  rlm-cli load src/lib.rs --chunker code           # Code-aware chunking
  rlm-cli load doc.md --chunker fixed --chunk-size 2000
  rlm-cli load big.log --chunker parallel          # Parallel for large files
  rlm-cli --format json load file.txt | jq '.buffer_id'
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
    #[command(name = "list", alias = "ls")]
    #[command(after_help = r#"Examples:
  rlm-cli list                            # List all buffers
  rlm-cli ls                              # Alias for list
  rlm-cli --format json list | jq '.[].name'
"#)]
    ListBuffers,

    /// Show buffer details.
    #[command(name = "show")]
    #[command(after_help = r#"Examples:
  rlm-cli show main-source                # Show buffer by name
  rlm-cli show 1                          # Show buffer by ID
  rlm-cli show 1 --chunks                 # Include chunk list
  rlm-cli --format json show 1            # JSON output
"#)]
    ShowBuffer {
        /// Buffer ID or name.
        buffer: String,

        /// Show chunks as well.
        #[arg(short, long)]
        chunks: bool,
    },

    /// Delete a buffer.
    #[command(name = "delete", alias = "rm")]
    DeleteBuffer {
        /// Buffer ID or name.
        buffer: String,

        /// Skip confirmation prompt.
        #[arg(short = 'y', long)]
        yes: bool,
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

    /// Get chunk indices for a buffer.
    ChunkIndices {
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
    WriteChunks {
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

    /// Add text to a buffer (intermediate results).
    AddBuffer {
        /// Buffer name.
        name: String,

        /// Content to add (reads from stdin if not provided).
        content: Option<String>,
    },

    /// Update an existing buffer with new content.
    ///
    /// Re-chunks the buffer and incrementally updates embeddings.
    #[command(after_help = r#"Examples:
  cat updated.txt | rlm-cli update main-source   # Update from stdin
  rlm-cli update my-buffer "new content"         # Update with inline content
  rlm-cli update my-buffer --embed               # Update and generate embeddings
  rlm-cli update my-buffer --chunk-size 500      # Custom chunk size"#)]
    #[command(alias = "update")]
    UpdateBuffer {
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
    ExportBuffers {
        /// Output file path (stdout if not specified).
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Pretty-print if JSON format.
        #[arg(short, long)]
        pretty: bool,
    },

    /// Set or get context variables.
    #[command(name = "var")]
    Variable {
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

    /// Search chunks using hybrid semantic + BM25 search.
    ///
    /// Returns chunk IDs and scores. Use `chunk get <id>` to retrieve content.
    #[command(after_help = r#"Examples:
  rlm-cli search "error handling"                  # Hybrid search (default)
  rlm-cli search "authentication" -k 5             # Top 5 results
  rlm-cli search "config" --mode bm25              # BM25 keyword search only
  rlm-cli search "API" --mode semantic             # Semantic search only
  rlm-cli search "bug fix" --buffer main-source    # Filter by buffer
  rlm-cli search "auth" --preview                  # Include content preview
  rlm-cli --format json search "test" | jq '.results[].chunk_id'
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

    /// Aggregate findings from analyst subagents.
    ///
    /// Reads JSON findings from stdin or a buffer, groups by relevance,
    /// deduplicates, and outputs a synthesizer-ready report.
    #[command(after_help = r#"Examples:
  cat findings.json | rlm-cli aggregate           # Aggregate from stdin
  rlm-cli aggregate --buffer findings             # Read from buffer
  rlm-cli aggregate --min-relevance medium        # Filter low relevance
  rlm-cli --format json aggregate | jq '.findings'

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

    /// Dispatch chunks for parallel subagent processing.
    ///
    /// Splits chunks into batches suitable for parallel subagent analysis.
    /// Returns batch assignments with chunk IDs and metadata.
    #[command(after_help = r#"Examples:
  rlm-cli dispatch my-buffer                     # Dispatch all chunks
  rlm-cli dispatch my-buffer --batch-size 5      # 5 chunks per batch
  rlm-cli dispatch my-buffer --workers 4         # Split into 4 batches
  rlm-cli dispatch my-buffer --query "error"     # Only relevant chunks
  rlm-cli --format json dispatch my-buffer       # JSON for orchestrator"#)]
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

    /// Chunk operations (get, list, embed).
    #[command(subcommand)]
    Chunk(ChunkCommands),
}

/// Chunk subcommands for pass-by-reference retrieval.
#[derive(Subcommand, Debug)]
pub enum ChunkCommands {
    /// Get a chunk by ID.
    ///
    /// Returns the chunk content and metadata. This is the primary
    /// pass-by-reference retrieval mechanism for subagents.
    #[command(after_help = r#"Examples:
  rlm-cli chunk get 42                    # Get chunk content
  rlm-cli chunk get 42 --metadata         # Include byte range, token count
  rlm-cli --format json chunk get 42      # JSON output for programmatic use
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
  rlm-cli chunk list main-source          # List chunk IDs
  rlm-cli chunk list 1 --preview          # Show content preview
  rlm-cli --format json chunk list 1 | jq '.[].id'
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
  rlm-cli chunk embed main-source         # Generate embeddings
  rlm-cli chunk embed 1 --force           # Re-embed existing chunks
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
