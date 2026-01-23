//! CLI command implementations.
//!
//! Contains the business logic for each CLI command.

use crate::chunking::{ChunkerMetadata, create_chunker};
use crate::cli::output::{
    GrepMatch, OutputFormat, format_buffer, format_buffer_list, format_chunk_indices,
    format_grep_matches, format_peek, format_status, format_write_chunks_result,
};
use crate::cli::parser::{ChunkCommands, Cli, Commands};
use crate::core::{Buffer, Context, ContextValue};
use crate::embedding::create_embedder;
use crate::error::{CommandError, Result, StorageError};
use crate::io::{read_file, write_file};
use crate::search::{SearchConfig, SearchResult, embed_buffer_chunks, hybrid_search};
use crate::storage::{SqliteStorage, Storage};
use regex::RegexBuilder;
use std::fmt::Write as FmtWrite;
use std::io::{self, Read, Write as IoWrite};

/// Executes the CLI command.
///
/// # Arguments
///
/// * `cli` - Parsed CLI arguments.
///
/// # Returns
///
/// Result with output string on success.
///
/// # Errors
///
/// Returns an error if the command fails to execute.
#[allow(clippy::too_many_lines)]
pub fn execute(cli: &Cli) -> Result<String> {
    let format = OutputFormat::parse(&cli.format);
    let db_path = cli.get_db_path();

    match &cli.command {
        Commands::Init { force } => cmd_init(&db_path, *force, format),
        Commands::Status => cmd_status(&db_path, format),
        Commands::Reset { yes } => cmd_reset(&db_path, *yes, format),
        Commands::Load {
            file,
            name,
            chunker,
            chunk_size,
            overlap,
        } => cmd_load(
            &db_path,
            file,
            name.as_deref(),
            chunker,
            *chunk_size,
            *overlap,
            format,
        ),
        Commands::ListBuffers => cmd_list_buffers(&db_path, format),
        Commands::ShowBuffer { buffer, chunks } => {
            cmd_show_buffer(&db_path, buffer, *chunks, format)
        }
        Commands::DeleteBuffer { buffer, yes } => cmd_delete_buffer(&db_path, buffer, *yes, format),
        Commands::Peek { buffer, start, end } => cmd_peek(&db_path, buffer, *start, *end, format),
        Commands::Grep {
            buffer,
            pattern,
            max_matches,
            window,
            ignore_case,
        } => cmd_grep(
            &db_path,
            buffer,
            pattern,
            *max_matches,
            *window,
            *ignore_case,
            format,
        ),
        Commands::ChunkIndices {
            buffer,
            chunk_size,
            overlap,
        } => cmd_chunk_indices(&db_path, buffer, *chunk_size, *overlap, format),
        Commands::WriteChunks {
            buffer,
            out_dir,
            chunk_size,
            overlap,
            prefix,
        } => cmd_write_chunks(
            &db_path,
            buffer,
            out_dir,
            *chunk_size,
            *overlap,
            prefix,
            format,
        ),
        Commands::AddBuffer { name, content } => {
            cmd_add_buffer(&db_path, name, content.as_deref(), format)
        }
        Commands::ExportBuffers { output, pretty } => {
            cmd_export_buffers(&db_path, output.as_deref(), *pretty, format)
        }
        Commands::Variable {
            name,
            value,
            delete,
        } => cmd_variable(&db_path, name, value.as_deref(), *delete, format),
        Commands::Global {
            name,
            value,
            delete,
        } => cmd_global(&db_path, name, value.as_deref(), *delete, format),
        Commands::Search {
            query,
            top_k,
            threshold,
            mode,
            rrf_k,
            buffer,
        } => cmd_search(
            &db_path,
            query,
            *top_k,
            *threshold,
            mode,
            *rrf_k,
            buffer.as_deref(),
            format,
        ),
        Commands::Chunk(chunk_cmd) => match chunk_cmd {
            ChunkCommands::Get { id, metadata } => cmd_chunk_get(&db_path, *id, *metadata, format),
            ChunkCommands::List {
                buffer,
                preview,
                preview_len,
            } => cmd_chunk_list(&db_path, buffer, *preview, *preview_len, format),
            ChunkCommands::Embed { buffer, force } => {
                cmd_chunk_embed(&db_path, buffer, *force, format)
            }
            ChunkCommands::Status => cmd_chunk_status(&db_path, format),
        },
        #[cfg(feature = "fuse")]
        Commands::Mount { mount_point } => cmd_mount(&db_path, mount_point),
    }
}

/// Opens storage and ensures it's initialized.
fn open_storage(db_path: &std::path::Path) -> Result<SqliteStorage> {
    let storage = SqliteStorage::open(db_path)?;

    if !storage.is_initialized()? {
        return Err(StorageError::NotInitialized.into());
    }

    Ok(storage)
}

/// Resolves a buffer identifier (ID or name) to a buffer.
fn resolve_buffer(storage: &SqliteStorage, identifier: &str) -> Result<Buffer> {
    // Try as ID first
    if let Ok(id) = identifier.parse::<i64>()
        && let Some(buffer) = storage.get_buffer(id)?
    {
        return Ok(buffer);
    }

    // Try as name
    if let Some(buffer) = storage.get_buffer_by_name(identifier)? {
        return Ok(buffer);
    }

    Err(StorageError::BufferNotFound {
        identifier: identifier.to_string(),
    }
    .into())
}

// ==================== Command Implementations ====================

fn cmd_init(db_path: &std::path::Path, force: bool, _format: OutputFormat) -> Result<String> {
    // Check if already exists
    if db_path.exists() && !force {
        return Err(CommandError::ExecutionFailed(
            "Database already exists. Use --force to reinitialize.".to_string(),
        )
        .into());
    }

    // Create parent directory if needed
    if let Some(parent) = db_path.parent()
        && !parent.exists()
    {
        std::fs::create_dir_all(parent).map_err(|e| {
            CommandError::ExecutionFailed(format!("Failed to create directory: {e}"))
        })?;
    }

    // If force, delete existing
    if force && db_path.exists() {
        std::fs::remove_file(db_path).map_err(|e| {
            CommandError::ExecutionFailed(format!("Failed to remove existing database: {e}"))
        })?;
    }

    let mut storage = SqliteStorage::open(db_path)?;
    storage.init()?;

    // Initialize empty context
    let context = Context::new();
    storage.save_context(&context)?;

    Ok(format!(
        "Initialized RLM database at: {}\n",
        db_path.display()
    ))
}

fn cmd_status(db_path: &std::path::Path, format: OutputFormat) -> Result<String> {
    let storage = open_storage(db_path)?;
    let stats = storage.stats()?;
    Ok(format_status(&stats, format))
}

fn cmd_reset(db_path: &std::path::Path, yes: bool, _format: OutputFormat) -> Result<String> {
    if !yes {
        // In a real implementation, we'd prompt the user
        // For now, require --yes flag
        return Err(CommandError::ExecutionFailed(
            "Use --yes to confirm reset. This will delete all data.".to_string(),
        )
        .into());
    }

    let mut storage = open_storage(db_path)?;
    storage.reset()?;

    // Reinitialize with empty context
    let context = Context::new();
    storage.save_context(&context)?;

    Ok("RLM state reset successfully.\n".to_string())
}

fn cmd_load(
    db_path: &std::path::Path,
    file: &std::path::Path,
    name: Option<&str>,
    chunker_name: &str,
    chunk_size: usize,
    overlap: usize,
    format: OutputFormat,
) -> Result<String> {
    let mut storage = open_storage(db_path)?;

    // Read file content
    let content = read_file(file)?;

    // Create buffer
    let buffer_name = name
        .map(String::from)
        .or_else(|| file.file_name().and_then(|n| n.to_str()).map(String::from));

    let mut buffer = Buffer::from_file(file.to_path_buf(), content.clone());
    buffer.name = buffer_name;
    buffer.compute_hash();

    // Add buffer to storage
    let buffer_id = storage.add_buffer(&buffer)?;

    // Chunk the content
    let chunker = create_chunker(chunker_name)?;
    let meta = ChunkerMetadata::with_size_and_overlap(chunk_size, overlap);
    let chunks = chunker.chunk(buffer_id, &content, Some(&meta))?;

    // Store chunks
    storage.add_chunks(buffer_id, &chunks)?;

    // Generate embeddings for semantic search (automatic during load)
    let embedder = create_embedder()?;
    let embedded_count = embed_buffer_chunks(&mut storage, embedder.as_ref(), buffer_id)?;

    // Update buffer with chunk count
    let mut updated_buffer =
        storage
            .get_buffer(buffer_id)?
            .ok_or_else(|| StorageError::BufferNotFound {
                identifier: buffer_id.to_string(),
            })?;
    updated_buffer.set_chunk_count(chunks.len());
    storage.update_buffer(&updated_buffer)?;

    // Update context
    if let Some(mut context) = storage.load_context()? {
        context.add_buffer(buffer_id);
        storage.save_context(&context)?;
    }

    match format {
        OutputFormat::Text => Ok(format!(
            "Loaded buffer {} (ID: {}) with {} chunks ({} embedded) from {}\n",
            updated_buffer.name.as_deref().unwrap_or("unnamed"),
            buffer_id,
            chunks.len(),
            embedded_count,
            file.display()
        )),
        OutputFormat::Json => {
            let result = serde_json::json!({
                "buffer_id": buffer_id,
                "name": updated_buffer.name,
                "chunk_count": chunks.len(),
                "embedded_count": embedded_count,
                "size": content.len(),
                "source": file.to_string_lossy()
            });
            Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
        }
    }
}

fn cmd_list_buffers(db_path: &std::path::Path, format: OutputFormat) -> Result<String> {
    let storage = open_storage(db_path)?;
    let buffers = storage.list_buffers()?;
    Ok(format_buffer_list(&buffers, format))
}

fn cmd_show_buffer(
    db_path: &std::path::Path,
    identifier: &str,
    show_chunks: bool,
    format: OutputFormat,
) -> Result<String> {
    let storage = open_storage(db_path)?;
    let buffer = resolve_buffer(&storage, identifier)?;

    let chunks = if show_chunks {
        Some(storage.get_chunks(buffer.id.unwrap_or(0))?)
    } else {
        None
    };

    Ok(format_buffer(&buffer, chunks.as_deref(), format))
}

fn cmd_delete_buffer(
    db_path: &std::path::Path,
    identifier: &str,
    yes: bool,
    _format: OutputFormat,
) -> Result<String> {
    if !yes {
        return Err(
            CommandError::ExecutionFailed("Use --yes to confirm deletion.".to_string()).into(),
        );
    }

    let mut storage = open_storage(db_path)?;
    let buffer = resolve_buffer(&storage, identifier)?;
    let buffer_id = buffer.id.unwrap_or(0);
    let buffer_name = buffer.name.unwrap_or_else(|| format!("{buffer_id}"));

    storage.delete_buffer(buffer_id)?;

    // Update context
    if let Some(mut context) = storage.load_context()? {
        context.remove_buffer(buffer_id);
        storage.save_context(&context)?;
    }

    Ok(format!("Deleted buffer: {buffer_name}\n"))
}

fn cmd_peek(
    db_path: &std::path::Path,
    identifier: &str,
    start: usize,
    end: Option<usize>,
    format: OutputFormat,
) -> Result<String> {
    let storage = open_storage(db_path)?;
    let buffer = resolve_buffer(&storage, identifier)?;

    let end = end.unwrap_or(start + 3000).min(buffer.content.len());
    let start = start.min(buffer.content.len());

    let content = buffer.slice(start, end).unwrap_or("");
    Ok(format_peek(content, start, end, format))
}

fn cmd_grep(
    db_path: &std::path::Path,
    identifier: &str,
    pattern: &str,
    max_matches: usize,
    window: usize,
    ignore_case: bool,
    format: OutputFormat,
) -> Result<String> {
    let storage = open_storage(db_path)?;
    let buffer = resolve_buffer(&storage, identifier)?;

    let regex = RegexBuilder::new(pattern)
        .case_insensitive(ignore_case)
        .build()
        .map_err(|e| CommandError::InvalidArgument(format!("Invalid regex: {e}")))?;

    let mut matches = Vec::new();
    for m in regex.find_iter(&buffer.content) {
        if matches.len() >= max_matches {
            break;
        }

        let start = m.start().saturating_sub(window);
        let end = (m.end() + window).min(buffer.content.len());

        // Find valid UTF-8 boundaries
        let start = crate::io::find_char_boundary(&buffer.content, start);
        let end = crate::io::find_char_boundary(&buffer.content, end);

        matches.push(GrepMatch {
            offset: m.start(),
            matched: m.as_str().to_string(),
            snippet: buffer.content[start..end].to_string(),
        });
    }

    Ok(format_grep_matches(&matches, pattern, format))
}

fn cmd_chunk_indices(
    db_path: &std::path::Path,
    identifier: &str,
    chunk_size: usize,
    overlap: usize,
    format: OutputFormat,
) -> Result<String> {
    let storage = open_storage(db_path)?;
    let buffer = resolve_buffer(&storage, identifier)?;

    let content_len = buffer.content.len();
    let mut indices = Vec::new();

    if chunk_size == 0 || overlap >= chunk_size {
        return Err(
            CommandError::InvalidArgument("Invalid chunk_size or overlap".to_string()).into(),
        );
    }

    let step = chunk_size - overlap;
    let mut start = 0;

    while start < content_len {
        let end = (start + chunk_size).min(content_len);
        indices.push((start, end));
        if end >= content_len {
            break;
        }
        start += step;
    }

    Ok(format_chunk_indices(&indices, format))
}

fn cmd_write_chunks(
    db_path: &std::path::Path,
    identifier: &str,
    out_dir: &std::path::Path,
    chunk_size: usize,
    overlap: usize,
    prefix: &str,
    format: OutputFormat,
) -> Result<String> {
    let mut storage = open_storage(db_path)?;
    let buffer = resolve_buffer(&storage, identifier)?;
    let buffer_id = buffer.id.unwrap_or(0);

    // Create chunker and chunk the content
    let chunker = create_chunker("semantic")?;
    let meta = ChunkerMetadata::with_size_and_overlap(chunk_size, overlap);
    let chunks = chunker.chunk(buffer_id, &buffer.content, Some(&meta))?;

    // Store chunks in SQLite
    storage.add_chunks(buffer_id, &chunks)?;

    // Update buffer with chunk count
    let mut updated_buffer =
        storage
            .get_buffer(buffer_id)?
            .ok_or_else(|| StorageError::BufferNotFound {
                identifier: buffer_id.to_string(),
            })?;
    updated_buffer.set_chunk_count(chunks.len());
    storage.update_buffer(&updated_buffer)?;

    // Write chunks to files
    let chunks_iter = chunks
        .iter()
        .enumerate()
        .map(|(i, c)| (i, c.content.as_str()));
    let paths = crate::io::reader::write_chunks(out_dir, chunks_iter, prefix)?;

    Ok(format_write_chunks_result(&paths, format))
}

fn cmd_add_buffer(
    db_path: &std::path::Path,
    name: &str,
    content: Option<&str>,
    format: OutputFormat,
) -> Result<String> {
    let mut storage = open_storage(db_path)?;

    // Read content from stdin if not provided
    let content = if let Some(c) = content {
        c.to_string()
    } else {
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer).map_err(|e| {
            CommandError::ExecutionFailed(format!("Failed to read from stdin: {e}"))
        })?;
        buffer
    };

    let buffer = Buffer::from_named(name.to_string(), content.clone());
    let buffer_id = storage.add_buffer(&buffer)?;

    // Update context
    if let Some(mut context) = storage.load_context()? {
        context.add_buffer(buffer_id);
        storage.save_context(&context)?;
    }

    match format {
        OutputFormat::Text => Ok(format!(
            "Added buffer '{}' (ID: {}, {} bytes)\n",
            name,
            buffer_id,
            content.len()
        )),
        OutputFormat::Json => {
            let result = serde_json::json!({
                "buffer_id": buffer_id,
                "name": name,
                "size": content.len()
            });
            Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
        }
    }
}

fn cmd_export_buffers(
    db_path: &std::path::Path,
    output: Option<&std::path::Path>,
    _pretty: bool,
    _format: OutputFormat,
) -> Result<String> {
    let storage = open_storage(db_path)?;
    let content = storage.export_buffers()?;

    if let Some(path) = output {
        write_file(path, &content)?;
        Ok(format!("Exported buffers to: {}\n", path.display()))
    } else {
        // Write to stdout
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        handle.write_all(content.as_bytes()).map_err(|e| {
            CommandError::ExecutionFailed(format!("Failed to write to stdout: {e}"))
        })?;
        Ok(String::new()) // Content already written
    }
}

fn cmd_variable(
    db_path: &std::path::Path,
    name: &str,
    value: Option<&str>,
    delete: bool,
    format: OutputFormat,
) -> Result<String> {
    let mut storage = open_storage(db_path)?;
    let mut context = storage.load_context()?.unwrap_or_else(Context::new);

    if delete {
        context.remove_variable(name);
        storage.save_context(&context)?;
        return Ok(format!("Deleted variable: {name}\n"));
    }

    if let Some(v) = value {
        context.set_variable(name.to_string(), ContextValue::String(v.to_string()));
        storage.save_context(&context)?;
        Ok(format!("Set variable: {name} = {v}\n"))
    } else {
        context.get_variable(name).map_or_else(
            || Ok(format!("Variable '{name}' not found\n")),
            |v| match format {
                OutputFormat::Text => Ok(format!("{name} = {v:?}\n")),
                OutputFormat::Json => Ok(serde_json::to_string_pretty(v).unwrap_or_default()),
            },
        )
    }
}

fn cmd_global(
    db_path: &std::path::Path,
    name: &str,
    value: Option<&str>,
    delete: bool,
    format: OutputFormat,
) -> Result<String> {
    let mut storage = open_storage(db_path)?;
    let mut context = storage.load_context()?.unwrap_or_else(Context::new);

    if delete {
        context.remove_global(name);
        storage.save_context(&context)?;
        return Ok(format!("Deleted global: {name}\n"));
    }

    if let Some(v) = value {
        context.set_global(name.to_string(), ContextValue::String(v.to_string()));
        storage.save_context(&context)?;
        Ok(format!("Set global: {name} = {v}\n"))
    } else {
        context.get_global(name).map_or_else(
            || Ok(format!("Global '{name}' not found\n")),
            |v| match format {
                OutputFormat::Text => Ok(format!("{name} = {v:?}\n")),
                OutputFormat::Json => Ok(serde_json::to_string_pretty(v).unwrap_or_default()),
            },
        )
    }
}

// ==================== Search Commands ====================

#[allow(clippy::too_many_arguments)]
fn cmd_search(
    db_path: &std::path::Path,
    query: &str,
    top_k: usize,
    threshold: f32,
    mode: &str,
    rrf_k: u32,
    buffer_filter: Option<&str>,
    format: OutputFormat,
) -> Result<String> {
    let storage = open_storage(db_path)?;
    let embedder = create_embedder()?;

    // Determine search mode
    let (use_semantic, use_bm25) = match mode.to_lowercase().as_str() {
        "semantic" => (true, false),
        "bm25" => (false, true),
        _ => (true, true), // hybrid is default
    };

    let config = SearchConfig::new()
        .with_top_k(top_k)
        .with_threshold(threshold)
        .with_rrf_k(rrf_k)
        .with_semantic(use_semantic)
        .with_bm25(use_bm25);

    // If buffer filter is specified, validate it exists
    let buffer_id = if let Some(identifier) = buffer_filter {
        let buffer = resolve_buffer(&storage, identifier)?;
        buffer.id
    } else {
        None
    };

    let results = hybrid_search(&storage, embedder.as_ref(), query, &config)?;

    // Filter by buffer if specified
    let results: Vec<SearchResult> = if let Some(bid) = buffer_id {
        let buffer_chunks: std::collections::HashSet<i64> = storage
            .get_chunks(bid)?
            .iter()
            .filter_map(|c| c.id)
            .collect();
        results
            .into_iter()
            .filter(|r| buffer_chunks.contains(&r.chunk_id))
            .collect()
    } else {
        results
    };

    Ok(format_search_results(&results, query, mode, format))
}

/// Formats a score for display, using scientific notation for very small values.
fn format_score(score: f64) -> String {
    if score == 0.0 {
        "0".to_string()
    } else if score.abs() < 0.0001 {
        format!("{score:.2e}")
    } else {
        format!("{score:.4}")
    }
}

fn format_search_results(
    results: &[SearchResult],
    query: &str,
    mode: &str,
    format: OutputFormat,
) -> String {
    match format {
        OutputFormat::Text => {
            if results.is_empty() {
                return format!("No results found for query: \"{query}\"\n");
            }

            let mut output = String::new();
            let _ = writeln!(
                output,
                "Search results for \"{query}\" ({mode} mode, {} results):\n",
                results.len()
            );
            let _ = writeln!(
                output,
                "{:<10} {:<12} {:<12} {:<12}",
                "Chunk ID", "Score", "Semantic", "BM25"
            );
            output.push_str(&"-".repeat(50));
            output.push('\n');

            for result in results {
                let semantic = result
                    .semantic_score
                    .map_or_else(|| "-".to_string(), |s| format_score(f64::from(s)));
                let bm25 = result
                    .bm25_score
                    .map_or_else(|| "-".to_string(), format_score);

                let _ = writeln!(
                    output,
                    "{:<10} {:<12.4} {:<12} {:<12}",
                    result.chunk_id, result.score, semantic, bm25
                );
            }

            output.push_str("\nUse 'rlm-rs chunk get <id>' to retrieve chunk content.\n");
            output
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "query": query,
                "mode": mode,
                "count": results.len(),
                "results": results.iter().map(|r| {
                    serde_json::json!({
                        "chunk_id": r.chunk_id,
                        "buffer_id": r.buffer_id,
                        "index": r.index,
                        "score": r.score,
                        "semantic_score": r.semantic_score,
                        "bm25_score": r.bm25_score
                    })
                }).collect::<Vec<_>>()
            });
            serde_json::to_string_pretty(&json).unwrap_or_default()
        }
    }
}

// ==================== FUSE Mount Command ====================

/// Mounts the RLM database as a FUSE filesystem.
///
/// This function blocks until the filesystem is unmounted (via `fusermount -u`).
#[cfg(feature = "fuse")]
fn cmd_mount(db_path: &std::path::Path, mount_point: &std::path::Path) -> Result<String> {
    use std::io::Write as _;

    // Verify mount point exists and is a directory
    if !mount_point.exists() {
        return Err(CommandError::ExecutionFailed(format!(
            "Mount point does not exist: {}",
            mount_point.display()
        ))
        .into());
    }

    if !mount_point.is_dir() {
        return Err(CommandError::ExecutionFailed(format!(
            "Mount point is not a directory: {}",
            mount_point.display()
        ))
        .into());
    }

    // Open storage
    let storage = open_storage(db_path)?;

    // Print startup message (before blocking)
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    let _ = writeln!(
        handle,
        "Mounting RLM filesystem at: {}",
        mount_point.display()
    );
    let _ = writeln!(handle, "Database: {}", db_path.display());
    let _ = writeln!(
        handle,
        "Press Ctrl+C or run 'fusermount -u {}' to unmount.",
        mount_point.display()
    );
    drop(handle);

    // Mount the filesystem (blocks until unmounted)
    crate::fuse::mount(storage, mount_point)?;

    Ok(format!(
        "Unmounted RLM filesystem from: {}\n",
        mount_point.display()
    ))
}

// ==================== Chunk Commands ====================

fn cmd_chunk_get(
    db_path: &std::path::Path,
    chunk_id: i64,
    include_metadata: bool,
    format: OutputFormat,
) -> Result<String> {
    let storage = open_storage(db_path)?;

    let chunk = storage
        .get_chunk(chunk_id)?
        .ok_or(StorageError::ChunkNotFound { id: chunk_id })?;

    match format {
        OutputFormat::Text => {
            if include_metadata {
                let mut output = String::new();
                let _ = writeln!(output, "Chunk ID: {}", chunk.id.unwrap_or(0));
                let _ = writeln!(output, "Buffer ID: {}", chunk.buffer_id);
                let _ = writeln!(output, "Index: {}", chunk.index);
                let _ = writeln!(
                    output,
                    "Byte range: {}..{}",
                    chunk.byte_range.start, chunk.byte_range.end
                );
                let _ = writeln!(output, "Size: {} bytes", chunk.size());
                output.push_str("---\n");
                output.push_str(&chunk.content);
                if !chunk.content.ends_with('\n') {
                    output.push('\n');
                }
                Ok(output)
            } else {
                // Plain content output for pass-by-reference use case
                Ok(chunk.content)
            }
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "chunk_id": chunk.id,
                "buffer_id": chunk.buffer_id,
                "index": chunk.index,
                "byte_range": {
                    "start": chunk.byte_range.start,
                    "end": chunk.byte_range.end
                },
                "size": chunk.size(),
                "content": chunk.content
            });
            Ok(serde_json::to_string_pretty(&json).unwrap_or_default())
        }
    }
}

fn cmd_chunk_list(
    db_path: &std::path::Path,
    identifier: &str,
    show_preview: bool,
    preview_len: usize,
    format: OutputFormat,
) -> Result<String> {
    let storage = open_storage(db_path)?;
    let buffer = resolve_buffer(&storage, identifier)?;
    let buffer_id = buffer.id.unwrap_or(0);

    let chunks = storage.get_chunks(buffer_id)?;

    match format {
        OutputFormat::Text => {
            if chunks.is_empty() {
                return Ok(format!(
                    "No chunks found for buffer: {}\n",
                    buffer.name.as_deref().unwrap_or(&buffer_id.to_string())
                ));
            }

            let mut output = String::new();
            let _ = writeln!(
                output,
                "Chunks for buffer '{}' ({} chunks):\n",
                buffer.name.as_deref().unwrap_or(&buffer_id.to_string()),
                chunks.len()
            );

            if show_preview {
                let _ = writeln!(
                    output,
                    "{:<8} {:<6} {:<12} {:<12} Preview",
                    "ID", "Index", "Start", "Size"
                );
                output.push_str(&"-".repeat(70));
                output.push('\n');

                for chunk in &chunks {
                    let preview: String = chunk
                        .content
                        .chars()
                        .take(preview_len)
                        .map(|c| if c == '\n' { ' ' } else { c })
                        .collect();
                    let preview = if chunk.content.len() > preview_len {
                        format!("{preview}...")
                    } else {
                        preview
                    };

                    let _ = writeln!(
                        output,
                        "{:<8} {:<6} {:<12} {:<12} {}",
                        chunk.id.unwrap_or(0),
                        chunk.index,
                        chunk.byte_range.start,
                        chunk.size(),
                        preview
                    );
                }
            } else {
                let _ = writeln!(
                    output,
                    "{:<8} {:<6} {:<12} {:<12}",
                    "ID", "Index", "Start", "Size"
                );
                output.push_str(&"-".repeat(40));
                output.push('\n');

                for chunk in &chunks {
                    let _ = writeln!(
                        output,
                        "{:<8} {:<6} {:<12} {:<12}",
                        chunk.id.unwrap_or(0),
                        chunk.index,
                        chunk.byte_range.start,
                        chunk.size()
                    );
                }
            }

            Ok(output)
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "buffer_id": buffer_id,
                "buffer_name": buffer.name,
                "chunk_count": chunks.len(),
                "chunks": chunks.iter().map(|c| {
                    let mut obj = serde_json::json!({
                        "id": c.id,
                        "index": c.index,
                        "byte_range": {
                            "start": c.byte_range.start,
                            "end": c.byte_range.end
                        },
                        "size": c.size()
                    });
                    if show_preview {
                        let preview: String = c.content.chars().take(preview_len).collect();
                        obj["preview"] = serde_json::Value::String(preview);
                    }
                    obj
                }).collect::<Vec<_>>()
            });
            Ok(serde_json::to_string_pretty(&json).unwrap_or_default())
        }
    }
}

fn cmd_chunk_embed(
    db_path: &std::path::Path,
    identifier: &str,
    force: bool,
    format: OutputFormat,
) -> Result<String> {
    let mut storage = open_storage(db_path)?;
    let buffer = resolve_buffer(&storage, identifier)?;
    let buffer_id = buffer.id.unwrap_or(0);
    let buffer_name = buffer.name.unwrap_or_else(|| buffer_id.to_string());

    // Check if already embedded (unless force)
    if !force {
        let chunks = storage.get_chunks(buffer_id)?;
        let mut all_embedded = true;
        for chunk in &chunks {
            if let Some(cid) = chunk.id
                && !storage.has_embedding(cid)?
            {
                all_embedded = false;
                break;
            }
        }
        if all_embedded && !chunks.is_empty() {
            return match format {
                OutputFormat::Text => Ok(format!(
                    "Buffer '{buffer_name}' already has embeddings. Use --force to re-embed.\n"
                )),
                OutputFormat::Json => {
                    let json = serde_json::json!({
                        "buffer_id": buffer_id,
                        "buffer_name": buffer_name,
                        "chunks_embedded": 0,
                        "already_embedded": true
                    });
                    Ok(serde_json::to_string_pretty(&json).unwrap_or_default())
                }
            };
        }
    }

    let embedder = create_embedder()?;
    let count = embed_buffer_chunks(&mut storage, embedder.as_ref(), buffer_id)?;

    match format {
        OutputFormat::Text => Ok(format!(
            "Generated embeddings for {count} chunks in buffer '{buffer_name}'.\n"
        )),
        OutputFormat::Json => {
            let json = serde_json::json!({
                "buffer_id": buffer_id,
                "buffer_name": buffer_name,
                "chunks_embedded": count
            });
            Ok(serde_json::to_string_pretty(&json).unwrap_or_default())
        }
    }
}

fn cmd_chunk_status(db_path: &std::path::Path, format: OutputFormat) -> Result<String> {
    let storage = open_storage(db_path)?;
    let buffers = storage.list_buffers()?;

    let mut buffer_stats: Vec<(String, i64, usize, usize)> = Vec::new();

    for buffer in &buffers {
        let buffer_id = buffer.id.unwrap_or(0);
        let buffer_name = buffer.name.clone().unwrap_or_else(|| buffer_id.to_string());
        let chunks = storage.get_chunks(buffer_id)?;
        let chunk_count = chunks.len();

        let mut embedded_count = 0;
        for chunk in &chunks {
            if let Some(cid) = chunk.id
                && storage.has_embedding(cid)?
            {
                embedded_count += 1;
            }
        }

        buffer_stats.push((buffer_name, buffer_id, chunk_count, embedded_count));
    }

    let total_chunks: usize = buffer_stats.iter().map(|(_, _, c, _)| c).sum();
    let total_embedded: usize = buffer_stats.iter().map(|(_, _, _, e)| e).sum();

    match format {
        OutputFormat::Text => {
            let mut output = String::new();
            output.push_str("Embedding Status\n");
            output.push_str("================\n\n");
            let _ = writeln!(
                output,
                "Total: {total_embedded}/{total_chunks} chunks embedded\n"
            );

            if !buffer_stats.is_empty() {
                let _ = writeln!(
                    output,
                    "{:<6} {:<20} {:<10} {:<10} Status",
                    "ID", "Name", "Chunks", "Embedded"
                );
                output.push_str(&"-".repeat(60));
                output.push('\n');

                for (name, id, chunks, embedded) in &buffer_stats {
                    let status = if *embedded == *chunks {
                        "✓ complete"
                    } else if *embedded > 0 {
                        "◐ partial"
                    } else {
                        "○ none"
                    };

                    let _ = writeln!(
                        output,
                        "{:<6} {:<20} {:<10} {:<10} {}",
                        id,
                        truncate_str(name, 20),
                        chunks,
                        embedded,
                        status
                    );
                }
            }

            Ok(output)
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "total_chunks": total_chunks,
                "total_embedded": total_embedded,
                "buffers": buffer_stats.iter().map(|(name, id, chunks, embedded)| {
                    serde_json::json!({
                        "buffer_id": id,
                        "name": name,
                        "chunk_count": chunks,
                        "embedded_count": embedded,
                        "fully_embedded": chunks == embedded
                    })
                }).collect::<Vec<_>>()
            });
            Ok(serde_json::to_string_pretty(&json).unwrap_or_default())
        }
    }
}

/// Truncates a string to max length with ellipsis.
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len <= 3 {
        s[..max_len].to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, std::path::PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        (temp_dir, db_path)
    }

    #[test]
    fn test_cmd_init() {
        let (_temp_dir, db_path) = setup();
        let result = cmd_init(&db_path, false, OutputFormat::Text);
        assert!(result.is_ok());
        assert!(db_path.exists());
    }

    #[test]
    fn test_cmd_init_already_exists() {
        let (_temp_dir, db_path) = setup();

        // First init
        cmd_init(&db_path, false, OutputFormat::Text).unwrap();

        // Second init should fail without force
        let result = cmd_init(&db_path, false, OutputFormat::Text);
        assert!(result.is_err());

        // With force should succeed
        let result = cmd_init(&db_path, true, OutputFormat::Text);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cmd_status() {
        let (_temp_dir, db_path) = setup();
        cmd_init(&db_path, false, OutputFormat::Text).unwrap();

        let result = cmd_status(&db_path, OutputFormat::Text);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("Buffers"));
    }

    #[test]
    fn test_cmd_reset() {
        let (_temp_dir, db_path) = setup();
        cmd_init(&db_path, false, OutputFormat::Text).unwrap();

        // Without --yes should fail
        let result = cmd_reset(&db_path, false, OutputFormat::Text);
        assert!(result.is_err());

        // With --yes should succeed
        let result = cmd_reset(&db_path, true, OutputFormat::Text);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cmd_add_buffer() {
        let (_temp_dir, db_path) = setup();
        cmd_init(&db_path, false, OutputFormat::Text).unwrap();

        let result = cmd_add_buffer(
            &db_path,
            "test-buffer",
            Some("Hello, world!"),
            OutputFormat::Text,
        );
        assert!(result.is_ok());
        assert!(result.unwrap().contains("test-buffer"));
    }

    #[test]
    fn test_cmd_list_buffers() {
        let (_temp_dir, db_path) = setup();
        cmd_init(&db_path, false, OutputFormat::Text).unwrap();

        // Empty list
        let result = cmd_list_buffers(&db_path, OutputFormat::Text);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("No buffers"));

        // Add a buffer
        cmd_add_buffer(&db_path, "test", Some("content"), OutputFormat::Text).unwrap();

        let result = cmd_list_buffers(&db_path, OutputFormat::Text);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("test"));
    }

    #[test]
    fn test_cmd_variable() {
        let (_temp_dir, db_path) = setup();
        cmd_init(&db_path, false, OutputFormat::Text).unwrap();

        // Set variable
        let result = cmd_variable(&db_path, "key", Some("value"), false, OutputFormat::Text);
        assert!(result.is_ok());

        // Get variable
        let result = cmd_variable(&db_path, "key", None, false, OutputFormat::Text);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("value"));

        // Delete variable
        let result = cmd_variable(&db_path, "key", None, true, OutputFormat::Text);
        assert!(result.is_ok());
    }

    #[test]
    fn test_truncate_str_short() {
        // String shorter than max_len should be returned as-is
        let result = truncate_str("hello", 10);
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_truncate_str_exact() {
        // String exactly at max_len should be returned as-is
        let result = truncate_str("hello", 5);
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_truncate_str_long() {
        // String longer than max_len should be truncated with ...
        let result = truncate_str("hello world", 8);
        assert_eq!(result, "hello...");
    }

    #[test]
    fn test_truncate_str_very_short_max() {
        // max_len <= 3 should just truncate without ellipsis
        let result = truncate_str("hello", 3);
        assert_eq!(result, "hel");
    }

    #[test]
    fn test_truncate_str_edge_case() {
        // max_len of 4 should show 1 char + ...
        let result = truncate_str("hello", 4);
        assert_eq!(result, "h...");
    }
}
