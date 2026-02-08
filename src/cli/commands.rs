//! CLI command implementations.
//!
//! Contains the business logic for each CLI command.

// Allow style choices for clarity
#![allow(clippy::format_push_string)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::option_if_let_else)]
#![allow(clippy::manual_div_ceil)]
#![allow(clippy::redundant_closure_for_method_calls)]
#![allow(clippy::if_not_else)]

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
        Commands::UpdateBuffer {
            buffer,
            content,
            embed,
            strategy,
            chunk_size,
            overlap,
        } => cmd_update_buffer(
            &db_path,
            buffer,
            content.as_deref(),
            *embed,
            strategy,
            *chunk_size,
            *overlap,
            format,
        ),
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
            preview,
            preview_len,
        } => cmd_search(
            &db_path,
            query,
            *top_k,
            *threshold,
            mode,
            *rrf_k,
            buffer.as_deref(),
            *preview,
            *preview_len,
            format,
        ),
        Commands::Aggregate {
            buffer,
            min_relevance,
            group_by,
            sort_by,
            output_buffer,
        } => cmd_aggregate(
            &db_path,
            buffer.as_deref(),
            min_relevance,
            group_by,
            sort_by,
            output_buffer.as_deref(),
            format,
        ),
        Commands::Dispatch {
            buffer,
            batch_size,
            workers,
            query,
            mode,
            threshold,
        } => cmd_dispatch(
            &db_path,
            buffer,
            *batch_size,
            *workers,
            query.as_deref(),
            mode,
            *threshold,
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
        OutputFormat::Json | OutputFormat::Ndjson => {
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
        OutputFormat::Json | OutputFormat::Ndjson => {
            let result = serde_json::json!({
                "buffer_id": buffer_id,
                "name": name,
                "size": content.len()
            });
            Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::redundant_clone)]
fn cmd_update_buffer(
    db_path: &std::path::Path,
    identifier: &str,
    content: Option<&str>,
    embed: bool,
    strategy: &str,
    chunk_size: usize,
    overlap: usize,
    format: OutputFormat,
) -> Result<String> {
    let mut storage = open_storage(db_path)?;
    let buffer = resolve_buffer(&storage, identifier)?;
    let buffer_id = buffer
        .id
        .ok_or_else(|| CommandError::ExecutionFailed("Buffer has no ID".to_string()))?;
    let buffer_name = buffer.name.clone().unwrap_or_else(|| buffer_id.to_string());

    // Read content from stdin if not provided
    let new_content = if let Some(c) = content {
        c.to_string()
    } else {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf).map_err(|e| {
            CommandError::ExecutionFailed(format!("Failed to read from stdin: {e}"))
        })?;
        buf
    };

    let content_size = new_content.len();

    // Get old chunk count for comparison
    let old_chunk_count = storage.chunk_count(buffer_id)?;

    // Delete existing chunks (this cascades to embeddings)
    storage.delete_chunks(buffer_id)?;

    // Update buffer content
    let updated_buffer = Buffer {
        id: Some(buffer_id),
        name: buffer.name.clone(),
        content: new_content.clone(),
        source: buffer.source.clone(),
        metadata: buffer.metadata.clone(),
    };
    storage.update_buffer(&updated_buffer)?;

    // Re-chunk the content
    let chunker = create_chunker(strategy)?;
    let meta = ChunkerMetadata::with_size_and_overlap(chunk_size, overlap);
    let chunks = chunker.chunk(buffer_id, &new_content, Some(&meta))?;
    let new_chunk_count = chunks.len();
    storage.add_chunks(buffer_id, &chunks)?;

    // Optionally embed the new chunks
    let embed_result = if embed {
        let embedder = create_embedder()?;
        let result = crate::search::embed_buffer_chunks_incremental(
            &mut storage,
            embedder.as_ref(),
            buffer_id,
            false,
        )?;
        Some(result)
    } else {
        None
    };

    match format {
        OutputFormat::Text => {
            let mut output = String::new();
            output.push_str(&format!(
                "Updated buffer '{}' ({} bytes)\n",
                buffer_name, content_size
            ));
            output.push_str(&format!(
                "Chunks: {} -> {} (using {} strategy)\n",
                old_chunk_count, new_chunk_count, strategy
            ));
            if let Some(ref result) = embed_result {
                output.push_str(&format!(
                    "Embedded {} chunks using model '{}'\n",
                    result.embedded_count, result.model_name
                ));
            }
            Ok(output)
        }
        OutputFormat::Json | OutputFormat::Ndjson => {
            let json = serde_json::json!({
                "buffer_id": buffer_id,
                "buffer_name": buffer_name,
                "content_size": content_size,
                "old_chunk_count": old_chunk_count,
                "new_chunk_count": new_chunk_count,
                "strategy": strategy,
                "embedded": embed_result.as_ref().map(|r| serde_json::json!({
                    "count": r.embedded_count,
                    "model": r.model_name
                }))
            });
            Ok(serde_json::to_string_pretty(&json).unwrap_or_default())
        }
    }
}

/// Analyst finding from a subagent.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
struct AnalystFinding {
    chunk_id: i64,
    relevance: String,
    #[serde(default)]
    findings: Vec<String>,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    follow_up: Vec<String>,
}

/// Relevance level for sorting.
fn relevance_order(relevance: &str) -> u8 {
    match relevance.to_lowercase().as_str() {
        "high" => 0,
        "medium" => 1,
        "low" => 2,
        "none" => 3,
        _ => 4,
    }
}

/// Check if relevance meets minimum threshold.
fn meets_relevance_threshold(relevance: &str, min_relevance: &str) -> bool {
    relevance_order(relevance) <= relevance_order(min_relevance)
}

fn cmd_aggregate(
    db_path: &std::path::Path,
    buffer: Option<&str>,
    min_relevance: &str,
    group_by: &str,
    sort_by: &str,
    output_buffer: Option<&str>,
    format: OutputFormat,
) -> Result<String> {
    let mut storage = open_storage(db_path)?;

    // Read findings from buffer or stdin
    let input = if let Some(buffer_name) = buffer {
        let buf = resolve_buffer(&storage, buffer_name)?;
        buf.content
    } else {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf).map_err(|e| {
            CommandError::ExecutionFailed(format!("Failed to read from stdin: {e}"))
        })?;
        buf
    };

    // Parse findings
    let findings: Vec<AnalystFinding> = serde_json::from_str(&input)
        .map_err(|e| CommandError::ExecutionFailed(format!("Invalid JSON input: {e}")))?;

    // Filter by relevance
    let filtered: Vec<_> = findings
        .into_iter()
        .filter(|f| meets_relevance_threshold(&f.relevance, min_relevance))
        .collect();

    // Sort findings
    let mut sorted = filtered;
    match sort_by {
        "relevance" => sorted.sort_by_key(|f| relevance_order(&f.relevance)),
        "chunk_id" => sorted.sort_by_key(|f| f.chunk_id),
        "findings_count" => sorted.sort_by_key(|f| std::cmp::Reverse(f.findings.len())),
        _ => {}
    }

    // Group findings
    let grouped: std::collections::BTreeMap<String, Vec<&AnalystFinding>> = match group_by {
        "relevance" => {
            let mut map = std::collections::BTreeMap::new();
            for f in &sorted {
                map.entry(f.relevance.clone())
                    .or_insert_with(Vec::new)
                    .push(f);
            }
            map
        }
        "chunk_id" => {
            let mut map = std::collections::BTreeMap::new();
            for f in &sorted {
                map.entry(f.chunk_id.to_string())
                    .or_insert_with(Vec::new)
                    .push(f);
            }
            map
        }
        _ => {
            let mut map = std::collections::BTreeMap::new();
            map.insert("all".to_string(), sorted.iter().collect());
            map
        }
    };

    // Collect all unique findings (deduplicated)
    let mut all_findings: Vec<&str> = Vec::new();
    for f in &sorted {
        for finding in &f.findings {
            if !all_findings.contains(&finding.as_str()) {
                all_findings.push(finding);
            }
        }
    }

    // Build summary stats
    let total_findings = sorted.len();
    let high_count = sorted.iter().filter(|f| f.relevance == "high").count();
    let medium_count = sorted.iter().filter(|f| f.relevance == "medium").count();
    let low_count = sorted.iter().filter(|f| f.relevance == "low").count();
    let unique_findings_count = all_findings.len();

    // Store in output buffer if requested
    if let Some(out_name) = output_buffer {
        let output_content = serde_json::to_string_pretty(&sorted).unwrap_or_default();
        let out_buffer = Buffer::from_named(out_name.to_string(), output_content);
        storage.add_buffer(&out_buffer)?;
    }

    match format {
        OutputFormat::Text => {
            let mut output = String::new();
            output.push_str(&format!("Aggregated {} analyst findings\n", total_findings));
            output.push_str(&format!(
                "Relevance: {} high, {} medium, {} low\n",
                high_count, medium_count, low_count
            ));
            output.push_str(&format!("Unique findings: {}\n\n", unique_findings_count));

            for (group, items) in &grouped {
                output.push_str(&format!("## {} ({} chunks)\n", group, items.len()));
                for f in items {
                    output.push_str(&format!("  Chunk {}: ", f.chunk_id));
                    if let Some(ref summary) = f.summary {
                        output.push_str(&truncate_str(summary, 80));
                    } else if !f.findings.is_empty() {
                        output.push_str(&truncate_str(&f.findings[0], 80));
                    }
                    output.push('\n');
                }
                output.push('\n');
            }

            if output_buffer.is_some() {
                output.push_str(&format!(
                    "Results stored in buffer '{}'\n",
                    output_buffer.unwrap_or("")
                ));
            }

            Ok(output)
        }
        OutputFormat::Json | OutputFormat::Ndjson => {
            let json = serde_json::json!({
                "summary": {
                    "total_findings": total_findings,
                    "high_relevance": high_count,
                    "medium_relevance": medium_count,
                    "low_relevance": low_count,
                    "unique_findings": unique_findings_count
                },
                "grouped": grouped,
                "findings": sorted,
                "all_findings_deduplicated": all_findings,
                "output_buffer": output_buffer
            });
            Ok(serde_json::to_string_pretty(&json).unwrap_or_default())
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
                OutputFormat::Json | OutputFormat::Ndjson => {
                    Ok(serde_json::to_string_pretty(v).unwrap_or_default())
                }
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
                OutputFormat::Json | OutputFormat::Ndjson => {
                    Ok(serde_json::to_string_pretty(v).unwrap_or_default())
                }
            },
        )
    }
}

// ==================== Dispatch Command ====================

#[allow(clippy::too_many_arguments)]
fn cmd_dispatch(
    db_path: &std::path::Path,
    identifier: &str,
    batch_size: usize,
    workers: Option<usize>,
    query: Option<&str>,
    mode: &str,
    threshold: f32,
    format: OutputFormat,
) -> Result<String> {
    let storage = open_storage(db_path)?;
    let buffer = resolve_buffer(&storage, identifier)?;
    let buffer_id = buffer.id.unwrap_or(0);
    let buffer_name = buffer.name.unwrap_or_else(|| buffer_id.to_string());

    // Get all chunks for this buffer
    let chunks = storage.get_chunks(buffer_id)?;

    if chunks.is_empty() {
        return Ok(format!("No chunks found in buffer '{}'\n", buffer_name));
    }

    // Get chunk IDs, optionally filtered by search query
    let chunk_ids: Vec<i64> = if let Some(query_str) = query {
        // Filter chunks by search relevance
        let embedder = create_embedder()?;

        let (use_semantic, use_bm25) = match mode.to_lowercase().as_str() {
            "semantic" => (true, false),
            "bm25" => (false, true),
            _ => (true, true),
        };

        let config = SearchConfig::new()
            .with_top_k(chunks.len()) // Get all matches
            .with_threshold(threshold)
            .with_semantic(use_semantic)
            .with_bm25(use_bm25);

        let results = hybrid_search(&storage, embedder.as_ref(), query_str, &config)?;

        // Filter to only chunks from this buffer
        let buffer_chunk_ids: std::collections::HashSet<i64> =
            chunks.iter().filter_map(|c| c.id).collect();

        results
            .into_iter()
            .filter(|r| buffer_chunk_ids.contains(&r.chunk_id))
            .map(|r| r.chunk_id)
            .collect()
    } else {
        chunks.iter().filter_map(|c| c.id).collect()
    };

    if chunk_ids.is_empty() {
        return Ok(format!(
            "No matching chunks found in buffer '{}' for query\n",
            buffer_name
        ));
    }

    // Calculate batch assignments
    let effective_batch_size = if let Some(num_workers) = workers {
        // Divide chunks evenly among workers
        (chunk_ids.len() + num_workers - 1) / num_workers
    } else {
        batch_size
    };

    // Create batches
    let batches: Vec<Vec<i64>> = chunk_ids
        .chunks(effective_batch_size)
        .map(|chunk| chunk.to_vec())
        .collect();

    match format {
        OutputFormat::Text => {
            let mut output = String::new();
            let _ = writeln!(
                output,
                "Dispatch plan for buffer '{}' ({} chunks -> {} batches):\n",
                buffer_name,
                chunk_ids.len(),
                batches.len()
            );

            for (i, batch) in batches.iter().enumerate() {
                let _ = writeln!(
                    output,
                    "Batch {}: {} chunks (IDs: {})",
                    i,
                    batch.len(),
                    batch
                        .iter()
                        .take(5)
                        .map(|id| id.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                        + if batch.len() > 5 { ", ..." } else { "" }
                );
            }

            output
                .push_str("\nUsage: Feed each batch to a subagent with 'rlm-cli chunk get <id>'\n");
            Ok(output)
        }
        OutputFormat::Json | OutputFormat::Ndjson => {
            let json = serde_json::json!({
                "buffer_id": buffer_id,
                "buffer_name": buffer_name,
                "total_chunks": chunk_ids.len(),
                "batch_count": batches.len(),
                "batch_size": effective_batch_size,
                "query_filter": query,
                "batches": batches.iter().enumerate().map(|(i, batch)| {
                    serde_json::json!({
                        "batch_index": i,
                        "chunk_count": batch.len(),
                        "chunk_ids": batch
                    })
                }).collect::<Vec<_>>()
            });
            Ok(serde_json::to_string_pretty(&json).unwrap_or_default())
        }
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
    preview: bool,
    preview_len: usize,
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
    let mut results: Vec<SearchResult> = if let Some(bid) = buffer_id {
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

    // Populate content previews if requested
    if preview {
        crate::search::populate_previews(&storage, &mut results, preview_len)?;
    }

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

                // Show content preview if available
                if let Some(ref preview) = result.content_preview {
                    let _ = writeln!(output, "  Preview: {preview}");
                }
            }

            output.push_str("\nUse 'rlm-cli chunk get <id>' to retrieve chunk content.\n");
            output
        }
        OutputFormat::Json | OutputFormat::Ndjson => {
            let json = serde_json::json!({
                "query": query,
                "mode": mode,
                "count": results.len(),
                "results": results.iter().map(|r| {
                    let mut obj = serde_json::json!({
                        "chunk_id": r.chunk_id,
                        "buffer_id": r.buffer_id,
                        "index": r.index,
                        "score": r.score,
                        "semantic_score": r.semantic_score,
                        "bm25_score": r.bm25_score
                    });
                    if let Some(ref preview) = r.content_preview {
                        obj["content_preview"] = serde_json::json!(preview);
                    }
                    obj
                }).collect::<Vec<_>>()
            });
            serde_json::to_string_pretty(&json).unwrap_or_default()
        }
    }
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
        OutputFormat::Json | OutputFormat::Ndjson => {
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
        OutputFormat::Json | OutputFormat::Ndjson => {
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

    let embedder = create_embedder()?;

    // Use incremental embedding (force_reembed = force flag)
    let result = crate::search::embed_buffer_chunks_incremental(
        &mut storage,
        embedder.as_ref(),
        buffer_id,
        force,
    )?;

    // Check for model version mismatch warning
    let model_warning = if !force {
        if let Some(existing_model) =
            crate::search::check_model_mismatch(&storage, buffer_id, &result.model_name)?
        {
            Some(format!(
                "Warning: Some embeddings use model '{existing_model}', current model is '{}'. \
                 Use --force to regenerate with the new model.",
                result.model_name
            ))
        } else {
            None
        }
    } else {
        None
    };

    match format {
        OutputFormat::Text => {
            let mut output = String::new();
            if let Some(warning) = &model_warning {
                output.push_str(warning);
                output.push('\n');
            }

            if !result.had_changes() {
                output.push_str(&format!(
                    "Buffer '{buffer_name}' already fully embedded ({} chunks). Use --force to re-embed.\n",
                    result.total_chunks
                ));
            } else {
                if result.embedded_count > 0 {
                    output.push_str(&format!(
                        "Embedded {} new chunks in buffer '{buffer_name}' using model '{}'.\n",
                        result.embedded_count, result.model_name
                    ));
                }
                if result.replaced_count > 0 {
                    output.push_str(&format!(
                        "Re-embedded {} chunks with updated model.\n",
                        result.replaced_count
                    ));
                }
                if result.skipped_count > 0 {
                    output.push_str(&format!(
                        "Skipped {} chunks (already embedded with current model).\n",
                        result.skipped_count
                    ));
                }
            }
            Ok(output)
        }
        OutputFormat::Json | OutputFormat::Ndjson => {
            let json = serde_json::json!({
                "buffer_id": buffer_id,
                "buffer_name": buffer_name,
                "embedded_count": result.embedded_count,
                "replaced_count": result.replaced_count,
                "skipped_count": result.skipped_count,
                "total_chunks": result.total_chunks,
                "model": result.model_name,
                "had_changes": result.had_changes(),
                "completion_percentage": result.completion_percentage(),
                "model_warning": model_warning
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
        OutputFormat::Json | OutputFormat::Ndjson => {
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
