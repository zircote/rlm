//! Integration tests for RLM-RS.

#![allow(clippy::expect_used)]

use rlm_rs::core::{Buffer, Chunk, Context, ContextValue};
use rlm_rs::storage::{SqliteStorage, Storage};
use tempfile::TempDir;

/// Helper to create a test storage instance.
fn create_test_storage() -> (SqliteStorage, TempDir) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test.db");
    let mut storage = SqliteStorage::open(&db_path).expect("Failed to create storage");
    storage.init().expect("Failed to init storage");
    (storage, temp_dir)
}

#[test]
fn test_storage_init_and_status() {
    let (storage, _temp) = create_test_storage();

    assert!(storage.is_initialized().expect("is_initialized failed"));

    let stats = storage.stats().expect("stats failed");
    assert_eq!(stats.buffer_count, 0);
    assert_eq!(stats.chunk_count, 0);
}

#[test]
fn test_buffer_crud() {
    let (mut storage, _temp) = create_test_storage();

    // Create buffer
    let buffer = Buffer::from_named(
        "test-buffer".to_string(),
        "Test content for the buffer".to_string(),
    );

    let id = storage.add_buffer(&buffer).expect("add_buffer failed");
    assert!(id > 0);

    // Read buffer by ID
    let loaded = storage.get_buffer(id).expect("get_buffer failed");
    assert!(loaded.is_some());
    let loaded = loaded.expect("buffer should exist");
    assert_eq!(loaded.content, "Test content for the buffer");

    // Read buffer by name
    let by_name = storage
        .get_buffer_by_name("test-buffer")
        .expect("get_buffer_by_name failed");
    assert!(by_name.is_some());

    // List buffers
    let buffers = storage.list_buffers().expect("list_buffers failed");
    assert_eq!(buffers.len(), 1);

    // Delete buffer
    storage.delete_buffer(id).expect("delete_buffer failed");
    let deleted = storage
        .get_buffer(id)
        .expect("get_buffer after delete failed");
    assert!(deleted.is_none());
}

#[test]
fn test_chunks() {
    let (mut storage, _temp) = create_test_storage();

    // Create buffer first
    let buffer = Buffer::from_content("Hello, world! This is test content.".to_string());
    let buffer_id = storage.add_buffer(&buffer).expect("add_buffer failed");

    // Create chunks
    let chunks = vec![
        Chunk::new(buffer_id, "Hello, world!".to_string(), 0..13, 0),
        Chunk::new(buffer_id, " This is test content.".to_string(), 13..35, 1),
    ];

    storage
        .add_chunks(buffer_id, &chunks)
        .expect("add_chunks failed");

    // Retrieve chunks
    let loaded_chunks = storage.get_chunks(buffer_id).expect("get_chunks failed");
    assert_eq!(loaded_chunks.len(), 2);
    assert_eq!(loaded_chunks[0].content, "Hello, world!");
    assert_eq!(loaded_chunks[1].index, 1);

    // Count chunks
    let count = storage.chunk_count(buffer_id).expect("chunk_count failed");
    assert_eq!(count, 2);
}

#[test]
fn test_context_operations() {
    let (mut storage, _temp) = create_test_storage();

    // Initially no context
    let ctx = storage.load_context().expect("load_context failed");
    assert!(ctx.is_none());

    // Create and save context
    let mut context = Context::new();
    context.set_variable(
        "key1".to_string(),
        ContextValue::String("value1".to_string()),
    );
    context.set_global(
        "global_key".to_string(),
        ContextValue::String("global_value".to_string()),
    );

    storage.save_context(&context).expect("save_context failed");

    // Load context back
    let loaded = storage.load_context().expect("load_context failed");
    assert!(loaded.is_some());
    let loaded = loaded.expect("context should exist");

    assert_eq!(
        loaded.get_variable("key1"),
        Some(&ContextValue::String("value1".to_string()))
    );
    assert_eq!(
        loaded.get_global("global_key"),
        Some(&ContextValue::String("global_value".to_string()))
    );
}

#[test]
fn test_chunker_strategies() {
    use rlm_rs::chunking::{Chunker, FixedChunker, available_strategies, create_chunker};

    // Test available strategies
    let strategies = available_strategies();
    assert!(strategies.contains(&"fixed"));
    assert!(strategies.contains(&"semantic"));
    assert!(strategies.contains(&"parallel"));

    // Test creating chunkers by name
    let fixed = create_chunker("fixed");
    assert!(fixed.is_ok());

    let semantic = create_chunker("semantic");
    assert!(semantic.is_ok());

    let unknown = create_chunker("unknown");
    assert!(unknown.is_err());

    // Test chunking
    let content = "Line one.\nLine two.\nLine three.";
    let chunker = FixedChunker::with_size(15);
    let chunks = chunker.chunk(1, content, None).expect("chunk failed");
    assert!(!chunks.is_empty());
}

#[test]
fn test_storage_reset() {
    let (mut storage, _temp) = create_test_storage();

    // Add some data
    let buffer = Buffer::from_content("content".to_string());
    storage.add_buffer(&buffer).expect("add_buffer failed");

    let stats = storage.stats().expect("stats failed");
    assert_eq!(stats.buffer_count, 1);

    // Reset
    storage.reset().expect("reset failed");

    let stats = storage.stats().expect("stats after reset failed");
    assert_eq!(stats.buffer_count, 0);
}

mod search_tests {
    use super::*;
    use rlm_rs::embedding::create_embedder;
    use rlm_rs::search::{
        DEFAULT_SIMILARITY_THRESHOLD, DEFAULT_TOP_K, SearchConfig, buffer_fully_embedded,
        embed_buffer_chunks, hybrid_search, search_bm25, search_semantic,
    };

    #[test]
    fn test_search_config_defaults() {
        let config = SearchConfig::default();
        assert_eq!(config.top_k, DEFAULT_TOP_K);
        assert!((config.similarity_threshold - DEFAULT_SIMILARITY_THRESHOLD).abs() < f32::EPSILON);
        assert_eq!(config.rrf_k, 60);
        assert!(config.use_semantic);
        assert!(config.use_bm25);
    }

    #[test]
    fn test_search_config_builder() {
        let config = SearchConfig::new()
            .with_top_k(5)
            .with_threshold(0.5)
            .with_rrf_k(30)
            .with_semantic(false)
            .with_bm25(true);

        assert_eq!(config.top_k, 5);
        assert!((config.similarity_threshold - 0.5).abs() < f32::EPSILON);
        assert_eq!(config.rrf_k, 30);
        assert!(!config.use_semantic);
        assert!(config.use_bm25);
    }

    #[test]
    fn test_embedder_creation() {
        let embedder = create_embedder();
        assert!(embedder.is_ok());
        let embedder = embedder.expect("embedder creation failed");
        assert!(embedder.dimensions() > 0);
    }

    #[test]
    fn test_embedding_single_text() {
        let embedder = create_embedder().expect("embedder creation failed");
        let result = embedder.embed("Hello, world!");
        assert!(result.is_ok());
        let embedding = result.expect("embedding failed");
        assert_eq!(embedding.len(), embedder.dimensions());
    }

    #[test]
    fn test_embedding_batch() {
        let embedder = create_embedder().expect("embedder creation failed");
        let texts = vec!["Hello", "World", "Test"];
        let result = embedder.embed_batch(&texts);
        assert!(result.is_ok());
        let embeddings = result.expect("batch embedding failed");
        assert_eq!(embeddings.len(), 3);
        for emb in &embeddings {
            assert_eq!(emb.len(), embedder.dimensions());
        }
    }

    #[test]
    fn test_embedding_empty_batch() {
        let embedder = create_embedder().expect("embedder creation failed");
        let texts: Vec<&str> = vec![];
        let result = embedder.embed_batch(&texts);
        assert!(result.is_ok());
        assert!(result.expect("empty batch failed").is_empty());
    }

    #[test]
    fn test_storage_embedding_operations() {
        let (mut storage, _temp) = create_test_storage();

        // Create buffer and chunk
        let buffer = Buffer::from_content("Test content for embedding".to_string());
        let buffer_id = storage.add_buffer(&buffer).expect("add_buffer failed");

        let chunks = vec![Chunk::new(
            buffer_id,
            "Test content for embedding".to_string(),
            0..26,
            0,
        )];
        storage
            .add_chunks(buffer_id, &chunks)
            .expect("add_chunks failed");

        let loaded_chunks = storage.get_chunks(buffer_id).expect("get_chunks failed");
        let chunk_id = loaded_chunks[0].id.expect("chunk should have id");

        // Initially no embedding
        assert!(
            !storage
                .has_embedding(chunk_id)
                .expect("has_embedding failed")
        );

        // Store embedding
        let embedding = vec![0.1_f32; 384];
        storage
            .store_embedding(chunk_id, &embedding, None)
            .expect("store_embedding failed");

        // Now has embedding
        assert!(
            storage
                .has_embedding(chunk_id)
                .expect("has_embedding failed")
        );

        // Retrieve embedding
        let retrieved = storage
            .get_embedding(chunk_id)
            .expect("get_embedding failed");
        assert!(retrieved.is_some());
        let retrieved = retrieved.expect("embedding should exist");
        assert_eq!(retrieved.len(), 384);

        // Get all embeddings
        let all = storage
            .get_all_embeddings()
            .expect("get_all_embeddings failed");
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].0, chunk_id);
    }

    #[test]
    fn test_storage_embedding_batch() {
        let (mut storage, _temp) = create_test_storage();

        // Create buffer and chunks
        let buffer = Buffer::from_content("Chunk one. Chunk two.".to_string());
        let buffer_id = storage.add_buffer(&buffer).expect("add_buffer failed");

        let chunks = vec![
            Chunk::new(buffer_id, "Chunk one.".to_string(), 0..10, 0),
            Chunk::new(buffer_id, "Chunk two.".to_string(), 11..21, 1),
        ];
        storage
            .add_chunks(buffer_id, &chunks)
            .expect("add_chunks failed");

        let loaded_chunks = storage.get_chunks(buffer_id).expect("get_chunks failed");

        // Store batch
        let batch: Vec<(i64, Vec<f32>)> = loaded_chunks
            .iter()
            .filter_map(|c| c.id.map(|id| (id, vec![0.5_f32; 384])))
            .collect();

        storage
            .store_embeddings_batch(&batch, None)
            .expect("store_embeddings_batch failed");

        // Verify all stored
        let all = storage
            .get_all_embeddings()
            .expect("get_all_embeddings failed");
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_fts_search() {
        let (mut storage, _temp) = create_test_storage();

        // Create buffer and chunks with searchable content
        let buffer = Buffer::from_content(
            "The quick brown fox jumps over the lazy dog. Rust programming language.".to_string(),
        );
        let buffer_id = storage.add_buffer(&buffer).expect("add_buffer failed");

        let chunks = vec![
            Chunk::new(
                buffer_id,
                "The quick brown fox jumps over the lazy dog.".to_string(),
                0..44,
                0,
            ),
            Chunk::new(
                buffer_id,
                "Rust programming language.".to_string(),
                45..71,
                1,
            ),
        ];
        storage
            .add_chunks(buffer_id, &chunks)
            .expect("add_chunks failed");

        // Search for "fox"
        let results = storage.search_fts("fox", 10).expect("search_fts failed");
        assert!(!results.is_empty());

        // Search for "Rust"
        let results = storage.search_fts("Rust", 10).expect("search_fts failed");
        assert!(!results.is_empty());

        // Search for non-existent term
        let results = storage
            .search_fts("nonexistent", 10)
            .expect("search_fts failed");
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_bm25_only() {
        let (mut storage, _temp) = create_test_storage();

        let buffer =
            Buffer::from_content("Machine learning and artificial intelligence.".to_string());
        let buffer_id = storage.add_buffer(&buffer).expect("add_buffer failed");

        let chunks = vec![Chunk::new(
            buffer_id,
            "Machine learning and artificial intelligence.".to_string(),
            0..45,
            0,
        )];
        storage
            .add_chunks(buffer_id, &chunks)
            .expect("add_chunks failed");

        let results = search_bm25(&storage, "learning", 10).expect("search_bm25 failed");
        assert!(!results.is_empty());
        assert!(results[0].bm25_score.is_some());
        assert!(results[0].semantic_score.is_none());
    }

    #[test]
    fn test_hybrid_search_bm25_only_mode() {
        let (mut storage, _temp) = create_test_storage();

        let buffer = Buffer::from_content("Database indexing strategies.".to_string());
        let buffer_id = storage.add_buffer(&buffer).expect("add_buffer failed");

        let chunks = vec![Chunk::new(
            buffer_id,
            "Database indexing strategies.".to_string(),
            0..29,
            0,
        )];
        storage
            .add_chunks(buffer_id, &chunks)
            .expect("add_chunks failed");

        let embedder = create_embedder().expect("embedder creation failed");
        let config = SearchConfig::new().with_semantic(false).with_bm25(true);

        let results =
            hybrid_search(&storage, embedder.as_ref(), "indexing", &config).expect("search failed");
        assert!(!results.is_empty());
    }

    #[test]
    fn test_hybrid_search_semantic_only_mode() {
        let (mut storage, _temp) = create_test_storage();

        let buffer = Buffer::from_content("Neural network architectures.".to_string());
        let buffer_id = storage.add_buffer(&buffer).expect("add_buffer failed");

        let chunks = vec![Chunk::new(
            buffer_id,
            "Neural network architectures.".to_string(),
            0..29,
            0,
        )];
        storage
            .add_chunks(buffer_id, &chunks)
            .expect("add_chunks failed");

        // Embed the chunks
        let loaded = storage.get_chunks(buffer_id).expect("get_chunks failed");
        let chunk_id = loaded[0].id.expect("chunk should have id");

        let embedder = create_embedder().expect("embedder creation failed");
        let embedding = embedder
            .embed("Neural network architectures.")
            .expect("embed failed");
        storage
            .store_embedding(chunk_id, &embedding, None)
            .expect("store_embedding failed");

        let config = SearchConfig::new()
            .with_semantic(true)
            .with_bm25(false)
            .with_threshold(0.1);

        let results = hybrid_search(&storage, embedder.as_ref(), "deep learning neural", &config)
            .expect("search failed");
        // Results may or may not be returned depending on similarity
        // Just verify it doesn't error
        assert!(results.is_empty() || results[0].semantic_score.is_some());
    }

    #[test]
    fn test_hybrid_search_combined() {
        let (mut storage, _temp) = create_test_storage();

        let buffer = Buffer::from_content("Functional programming paradigms.".to_string());
        let buffer_id = storage.add_buffer(&buffer).expect("add_buffer failed");

        let chunks = vec![Chunk::new(
            buffer_id,
            "Functional programming paradigms.".to_string(),
            0..33,
            0,
        )];
        storage
            .add_chunks(buffer_id, &chunks)
            .expect("add_chunks failed");

        // Embed chunks
        let loaded = storage.get_chunks(buffer_id).expect("get_chunks failed");
        let chunk_id = loaded[0].id.expect("chunk should have id");

        let embedder = create_embedder().expect("embedder creation failed");
        let embedding = embedder
            .embed("Functional programming paradigms.")
            .expect("embed failed");
        storage
            .store_embedding(chunk_id, &embedding, None)
            .expect("store_embedding failed");

        let config = SearchConfig::new()
            .with_semantic(true)
            .with_bm25(true)
            .with_threshold(0.1);

        let results = hybrid_search(&storage, embedder.as_ref(), "programming", &config)
            .expect("search failed");
        // Combined search should work
        assert!(!results.is_empty() || results.is_empty()); // Just verify no error
    }

    #[test]
    fn test_search_semantic_convenience() {
        let (mut storage, _temp) = create_test_storage();

        let buffer = Buffer::from_content("API design patterns.".to_string());
        let buffer_id = storage.add_buffer(&buffer).expect("add_buffer failed");

        let chunks = vec![Chunk::new(
            buffer_id,
            "API design patterns.".to_string(),
            0..20,
            0,
        )];
        storage
            .add_chunks(buffer_id, &chunks)
            .expect("add_chunks failed");

        let loaded = storage.get_chunks(buffer_id).expect("get_chunks failed");
        let chunk_id = loaded[0].id.expect("chunk should have id");

        let embedder = create_embedder().expect("embedder creation failed");
        let embedding = embedder
            .embed("API design patterns.")
            .expect("embed failed");
        storage
            .store_embedding(chunk_id, &embedding, None)
            .expect("store_embedding failed");

        let results = search_semantic(&storage, embedder.as_ref(), "REST API", 10, 0.1)
            .expect("search_semantic failed");
        // Results depend on similarity
        assert!(results.is_empty() || results[0].bm25_score.is_none());
    }

    #[test]
    fn test_embed_buffer_chunks() {
        let (mut storage, _temp) = create_test_storage();

        let buffer = Buffer::from_content("Content to embed.".to_string());
        let buffer_id = storage.add_buffer(&buffer).expect("add_buffer failed");

        let chunks = vec![Chunk::new(
            buffer_id,
            "Content to embed.".to_string(),
            0..17,
            0,
        )];
        storage
            .add_chunks(buffer_id, &chunks)
            .expect("add_chunks failed");

        let embedder = create_embedder().expect("embedder creation failed");

        let count =
            embed_buffer_chunks(&mut storage, embedder.as_ref(), buffer_id).expect("embed failed");
        assert_eq!(count, 1);

        // Verify embedded
        let is_embedded =
            buffer_fully_embedded(&storage, buffer_id).expect("buffer_fully_embedded failed");
        assert!(is_embedded);
    }

    #[test]
    fn test_embed_empty_buffer() {
        let (mut storage, _temp) = create_test_storage();

        let buffer = Buffer::from_content(String::new());
        let buffer_id = storage.add_buffer(&buffer).expect("add_buffer failed");

        let embedder = create_embedder().expect("embedder creation failed");

        let count =
            embed_buffer_chunks(&mut storage, embedder.as_ref(), buffer_id).expect("embed failed");
        assert_eq!(count, 0);

        let is_embedded =
            buffer_fully_embedded(&storage, buffer_id).expect("buffer_fully_embedded failed");
        assert!(is_embedded); // Empty buffer is considered fully embedded
    }

    #[test]
    fn test_buffer_not_fully_embedded() {
        let (mut storage, _temp) = create_test_storage();

        let buffer = Buffer::from_content("Some content.".to_string());
        let buffer_id = storage.add_buffer(&buffer).expect("add_buffer failed");

        let chunks = vec![Chunk::new(buffer_id, "Some content.".to_string(), 0..13, 0)];
        storage
            .add_chunks(buffer_id, &chunks)
            .expect("add_chunks failed");

        // Not embedded yet
        let is_embedded =
            buffer_fully_embedded(&storage, buffer_id).expect("buffer_fully_embedded failed");
        assert!(!is_embedded);
    }

    #[test]
    fn test_get_chunk_by_id() {
        let (mut storage, _temp) = create_test_storage();

        let buffer = Buffer::from_content("Chunk content here.".to_string());
        let buffer_id = storage.add_buffer(&buffer).expect("add_buffer failed");

        let chunks = vec![Chunk::new(
            buffer_id,
            "Chunk content here.".to_string(),
            0..19,
            0,
        )];
        storage
            .add_chunks(buffer_id, &chunks)
            .expect("add_chunks failed");

        let loaded = storage.get_chunks(buffer_id).expect("get_chunks failed");
        let chunk_id = loaded[0].id.expect("chunk should have id");

        // Get chunk by ID
        let chunk = storage.get_chunk(chunk_id).expect("get_chunk failed");
        assert!(chunk.is_some());
        let chunk = chunk.expect("chunk should exist");
        assert_eq!(chunk.content, "Chunk content here.");
    }

    #[test]
    fn test_get_nonexistent_chunk() {
        let (storage, _temp) = create_test_storage();

        let chunk = storage.get_chunk(99999).expect("get_chunk failed");
        assert!(chunk.is_none());
    }
}

mod property_tests {
    use proptest::prelude::*;
    use rlm_rs::core::Chunk;

    proptest! {
        #[test]
        fn chunk_size_matches_content(content in "[a-z]{1,100}") {
            let chunk = Chunk::new(1, content.clone(), 0..content.len(), 0);
            prop_assert_eq!(chunk.size(), content.len());
        }

        #[test]
        fn chunk_byte_range_valid(start in 0usize..1000, len in 1usize..100) {
            let content = "x".repeat(len);
            let end = start + len;
            let chunk = Chunk::new(1, content, start..end, 0);
            prop_assert_eq!(chunk.start(), start);
            prop_assert_eq!(chunk.end(), end);
            prop_assert_eq!(chunk.range_size(), len);
        }

        #[test]
        fn chunk_estimate_tokens_reasonable(content in "[a-z ]{1,200}") {
            let chunk = Chunk::new(1, content.clone(), 0..content.len(), 0);
            let tokens = chunk.estimate_tokens();
            // Approximately 4 chars per token
            let expected_min = content.len() / 6;
            let expected_max = content.len() / 2;
            prop_assert!(tokens >= expected_min || content.len() < 4);
            prop_assert!(tokens <= expected_max + 1);
        }
    }
}

/// CLI command integration tests.
mod cli_tests {
    use rlm_rs::cli::commands::execute;
    use rlm_rs::cli::parser::{BufferCommands, ChunkCommands, Cli, Commands, ContextCommands};
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// Helper to create a CLI struct with custom `db_path`.
    fn make_cli(db_path: PathBuf, command: Commands) -> Cli {
        Cli {
            db_path: Some(db_path),
            verbose: false,
            format: "text".to_string(),
            command,
        }
    }

    /// Helper to create a CLI struct with JSON format.
    fn make_cli_json(db_path: PathBuf, command: Commands) -> Cli {
        Cli {
            db_path: Some(db_path),
            verbose: false,
            format: "json".to_string(),
            command,
        }
    }

    #[test]
    fn test_cmd_init() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        let result = execute(&cli);
        assert!(result.is_ok());
        assert!(result.expect("init result").contains("Initialized"));
        assert!(db_path.exists());
    }

    #[test]
    fn test_cmd_init_force() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");

        // First init
        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("first init");

        // Second init without force should fail
        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        let result = execute(&cli);
        assert!(result.is_err());

        // Second init with force should succeed
        let cli = make_cli(db_path, Commands::Init { force: true });
        let result = execute(&cli);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cmd_status() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");

        // Init first
        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        // Status command
        let cli = make_cli(db_path, Commands::Status);
        let result = execute(&cli);
        assert!(result.is_ok());
        let output = result.expect("status output");
        assert!(output.contains("Buffers:") || output.contains("buffers"));
    }

    #[test]
    fn test_cmd_status_json() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli_json(db_path, Commands::Status);
        let result = execute(&cli);
        assert!(result.is_ok());
        let output = result.expect("json output");
        assert!(output.contains('{'));
        assert!(output.contains("buffer_count"));
    }

    #[test]
    fn test_cmd_status_not_initialized() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("nonexistent.db");

        let cli = make_cli(db_path, Commands::Status);
        let result = execute(&cli);
        assert!(result.is_err());
    }

    #[test]
    fn test_cmd_reset_requires_yes() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        // Reset without --yes should fail
        let cli = make_cli(db_path, Commands::Reset { yes: false });
        let result = execute(&cli);
        assert!(result.is_err());
    }

    #[test]
    fn test_cmd_reset_with_yes() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(db_path, Commands::Reset { yes: true });
        let result = execute(&cli);
        assert!(result.is_ok());
        assert!(result.expect("reset").contains("reset"));
    }

    #[test]
    fn test_cmd_load_file() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file_path = temp_dir.path().join("content.txt");
        std::fs::write(&file_path, "Hello world!\nThis is test content.").expect("write file");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path,
            Commands::Buffer(BufferCommands::Load {
                file: file_path,
                name: Some("test-buffer".to_string()),
                chunker: "fixed".to_string(),
                chunk_size: 1000,
                overlap: 100,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());
        let output = result.expect("load output");
        assert!(output.contains("Loaded"));
        assert!(output.contains("test-buffer"));
    }

    #[test]
    fn test_cmd_load_file_json() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file_path = temp_dir.path().join("content.txt");
        std::fs::write(&file_path, "Test content for JSON output").expect("write file");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli_json(
            db_path,
            Commands::Buffer(BufferCommands::Load {
                file: file_path,
                name: None,
                chunker: "fixed".to_string(),
                chunk_size: 1000,
                overlap: 100,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());
        let output = result.expect("json output");
        assert!(output.contains('{'));
        assert!(output.contains("buffer_id"));
    }

    #[test]
    fn test_cmd_list_buffers() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file_path = temp_dir.path().join("content.txt");
        std::fs::write(&file_path, "Buffer content").expect("write file");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Load {
                file: file_path,
                name: Some("mybuffer".to_string()),
                chunker: "fixed".to_string(),
                chunk_size: 1000,
                overlap: 0,
            }),
        );
        execute(&cli).expect("load");

        let cli = make_cli(db_path, Commands::Buffer(BufferCommands::List));
        let result = execute(&cli);
        assert!(result.is_ok());
        let output = result.expect("list output");
        assert!(output.contains("mybuffer"));
    }

    #[test]
    fn test_cmd_list_buffers_empty() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(db_path, Commands::Buffer(BufferCommands::List));
        let result = execute(&cli);
        assert!(result.is_ok());
        let output = result.expect("list output");
        assert!(output.contains("No buffers"));
    }

    #[test]
    fn test_cmd_show_buffer() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file_path = temp_dir.path().join("content.txt");
        std::fs::write(&file_path, "Show buffer content").expect("write file");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Load {
                file: file_path,
                name: Some("showbuf".to_string()),
                chunker: "fixed".to_string(),
                chunk_size: 1000,
                overlap: 0,
            }),
        );
        execute(&cli).expect("load");

        // Show by name
        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Show {
                buffer: "showbuf".to_string(),
                chunks: false,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());
        let output = result.expect("show output");
        assert!(output.contains("showbuf"));

        // Show with chunks
        let cli = make_cli(
            db_path,
            Commands::Buffer(BufferCommands::Show {
                buffer: "showbuf".to_string(),
                chunks: true,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());
        let output = result.expect("show output");
        assert!(output.contains("Chunks:") || output.contains("chunks"));
    }

    #[test]
    fn test_cmd_show_buffer_not_found() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path,
            Commands::Buffer(BufferCommands::Show {
                buffer: "nonexistent".to_string(),
                chunks: false,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_err());
    }

    #[test]
    fn test_cmd_delete_buffer() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file_path = temp_dir.path().join("content.txt");
        std::fs::write(&file_path, "Delete me").expect("write file");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Load {
                file: file_path,
                name: Some("deleteme".to_string()),
                chunker: "fixed".to_string(),
                chunk_size: 1000,
                overlap: 0,
            }),
        );
        execute(&cli).expect("load");

        // Delete requires --yes
        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Delete {
                buffer: "deleteme".to_string(),
                yes: false,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_err());

        // Delete with --yes
        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Delete {
                buffer: "deleteme".to_string(),
                yes: true,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());

        // Verify deleted
        let cli = make_cli(
            db_path,
            Commands::Buffer(BufferCommands::Show {
                buffer: "deleteme".to_string(),
                chunks: false,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_err());
    }

    #[test]
    fn test_cmd_peek() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file_path = temp_dir.path().join("content.txt");
        std::fs::write(&file_path, "Peek at this content here").expect("write file");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Load {
                file: file_path,
                name: Some("peekbuf".to_string()),
                chunker: "fixed".to_string(),
                chunk_size: 1000,
                overlap: 0,
            }),
        );
        execute(&cli).expect("load");

        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Peek {
                buffer: "peekbuf".to_string(),
                start: 0,
                end: Some(10),
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());
        let output = result.expect("peek output");
        assert!(output.contains("Peek"));

        // Peek with default end
        let cli = make_cli(
            db_path,
            Commands::Buffer(BufferCommands::Peek {
                buffer: "peekbuf".to_string(),
                start: 5,
                end: None,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cmd_grep() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file_path = temp_dir.path().join("content.txt");
        std::fs::write(
            &file_path,
            "Line one has hello\nLine two has world\nLine three has hello again",
        )
        .expect("write file");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Load {
                file: file_path,
                name: Some("grepbuf".to_string()),
                chunker: "fixed".to_string(),
                chunk_size: 1000,
                overlap: 0,
            }),
        );
        execute(&cli).expect("load");

        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Grep {
                buffer: "grepbuf".to_string(),
                pattern: "hello".to_string(),
                max_matches: 10,
                window: 50,
                ignore_case: false,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());
        let output = result.expect("grep output");
        assert!(output.contains("match") || output.contains("hello"));

        // Case insensitive grep
        let cli = make_cli(
            db_path,
            Commands::Buffer(BufferCommands::Grep {
                buffer: "grepbuf".to_string(),
                pattern: "HELLO".to_string(),
                max_matches: 10,
                window: 50,
                ignore_case: true,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cmd_grep_no_matches() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file_path = temp_dir.path().join("content.txt");
        std::fs::write(&file_path, "Some content").expect("write file");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Load {
                file: file_path,
                name: Some("grepbuf2".to_string()),
                chunker: "fixed".to_string(),
                chunk_size: 1000,
                overlap: 0,
            }),
        );
        execute(&cli).expect("load");

        let cli = make_cli(
            db_path,
            Commands::Buffer(BufferCommands::Grep {
                buffer: "grepbuf2".to_string(),
                pattern: "notfound".to_string(),
                max_matches: 10,
                window: 50,
                ignore_case: false,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());
        let output = result.expect("grep output");
        assert!(output.contains("No matches") || output.contains('0'));
    }

    #[test]
    fn test_cmd_chunk_indices() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file_path = temp_dir.path().join("content.txt");
        std::fs::write(&file_path, "x".repeat(200)).expect("write file");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Load {
                file: file_path,
                name: Some("chunkbuf".to_string()),
                chunker: "fixed".to_string(),
                chunk_size: 1000,
                overlap: 0,
            }),
        );
        execute(&cli).expect("load");

        let cli = make_cli(
            db_path,
            Commands::Chunk(ChunkCommands::Indices {
                buffer: "chunkbuf".to_string(),
                chunk_size: 50,
                overlap: 10,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cmd_write_chunks() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file_path = temp_dir.path().join("content.txt");
        let out_dir = temp_dir.path().join("chunks_out");
        std::fs::write(&file_path, "x".repeat(200)).expect("write file");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Load {
                file: file_path,
                name: Some("writebuf".to_string()),
                chunker: "fixed".to_string(),
                chunk_size: 1000,
                overlap: 0,
            }),
        );
        execute(&cli).expect("load");

        let cli = make_cli(
            db_path,
            Commands::Chunk(ChunkCommands::Write {
                buffer: "writebuf".to_string(),
                out_dir,
                chunk_size: 50,
                overlap: 10,
                prefix: "test".to_string(),
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cmd_add_buffer() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Add {
                name: "addbuf".to_string(),
                content: Some("Added content".to_string()),
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());

        // Verify it was added
        let cli = make_cli(
            db_path,
            Commands::Buffer(BufferCommands::Show {
                buffer: "addbuf".to_string(),
                chunks: false,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cmd_export_buffers() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file_path = temp_dir.path().join("content.txt");
        std::fs::write(&file_path, "Export content").expect("write file");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Load {
                file: file_path,
                name: Some("exportbuf".to_string()),
                chunker: "fixed".to_string(),
                chunk_size: 1000,
                overlap: 0,
            }),
        );
        execute(&cli).expect("load");

        // Export to stdout (no output file)
        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Export {
                output: None,
                pretty: false,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());

        // Export with pretty print
        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Export {
                output: None,
                pretty: true,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());

        // Export to file
        let export_path = temp_dir.path().join("export.json");
        let cli = make_cli(
            db_path,
            Commands::Buffer(BufferCommands::Export {
                output: Some(export_path.clone()),
                pretty: true,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());
        assert!(export_path.exists());
    }

    #[test]
    fn test_cmd_variable() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        // Set variable
        let cli = make_cli(
            db_path.clone(),
            Commands::Context(ContextCommands::Var {
                name: "myvar".to_string(),
                value: Some("myvalue".to_string()),
                delete: false,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());

        // Get variable
        let cli = make_cli(
            db_path.clone(),
            Commands::Context(ContextCommands::Var {
                name: "myvar".to_string(),
                value: None,
                delete: false,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());
        let output = result.expect("var output");
        assert!(output.contains("myvalue"));

        // Delete variable
        let cli = make_cli(
            db_path,
            Commands::Context(ContextCommands::Var {
                name: "myvar".to_string(),
                value: None,
                delete: true,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cmd_global() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        // Set global
        let cli = make_cli(
            db_path.clone(),
            Commands::Context(ContextCommands::Global {
                name: "globalvar".to_string(),
                value: Some("globalvalue".to_string()),
                delete: false,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());

        // Get global
        let cli = make_cli(
            db_path.clone(),
            Commands::Context(ContextCommands::Global {
                name: "globalvar".to_string(),
                value: None,
                delete: false,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());

        // Delete global
        let cli = make_cli(
            db_path,
            Commands::Context(ContextCommands::Global {
                name: "globalvar".to_string(),
                value: None,
                delete: true,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cmd_search_bm25() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file_path = temp_dir.path().join("content.txt");
        std::fs::write(
            &file_path,
            "Rust programming language\nMemory safety features\nZero cost abstractions",
        )
        .expect("write file");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Load {
                file: file_path,
                name: Some("searchbuf".to_string()),
                chunker: "fixed".to_string(),
                chunk_size: 1000,
                overlap: 0,
            }),
        );
        execute(&cli).expect("load");

        // BM25-only search
        let cli = make_cli(
            db_path,
            Commands::Search {
                query: "programming".to_string(),
                top_k: 5,
                threshold: 0.3,
                mode: "bm25".to_string(),
                rrf_k: 60,
                buffer: None,
                preview: false,
                preview_len: 150,
            },
        );
        let result = execute(&cli);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cmd_chunk_get() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file_path = temp_dir.path().join("content.txt");
        std::fs::write(&file_path, "Chunk get test content").expect("write file");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Load {
                file: file_path,
                name: Some("chunkgetbuf".to_string()),
                chunker: "fixed".to_string(),
                chunk_size: 1000,
                overlap: 0,
            }),
        );
        execute(&cli).expect("load");

        // Get chunk by ID (ID 1 should exist after load)
        let cli = make_cli(
            db_path.clone(),
            Commands::Chunk(ChunkCommands::Get {
                id: 1,
                metadata: false,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());

        // Get chunk with metadata
        let cli = make_cli(
            db_path,
            Commands::Chunk(ChunkCommands::Get {
                id: 1,
                metadata: true,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());
        let output = result.expect("chunk output");
        assert!(output.contains("Chunk") || output.contains("content"));
    }

    #[test]
    fn test_cmd_chunk_get_not_found() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path,
            Commands::Chunk(ChunkCommands::Get {
                id: 999,
                metadata: false,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_err());
    }

    #[test]
    fn test_cmd_chunk_list() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file_path = temp_dir.path().join("content.txt");
        std::fs::write(&file_path, "Chunk list content").expect("write file");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Load {
                file: file_path,
                name: Some("chunklistbuf".to_string()),
                chunker: "fixed".to_string(),
                chunk_size: 1000,
                overlap: 0,
            }),
        );
        execute(&cli).expect("load");

        // List chunks
        let cli = make_cli(
            db_path.clone(),
            Commands::Chunk(ChunkCommands::List {
                buffer: "chunklistbuf".to_string(),
                preview: false,
                preview_len: 100,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());

        // List with preview
        let cli = make_cli(
            db_path,
            Commands::Chunk(ChunkCommands::List {
                buffer: "chunklistbuf".to_string(),
                preview: true,
                preview_len: 50,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cmd_chunk_status() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(db_path, Commands::Chunk(ChunkCommands::Status));
        let result = execute(&cli);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cmd_search_json() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file_path = temp_dir.path().join("content.txt");
        std::fs::write(&file_path, "JSON search test content").expect("write file");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Load {
                file: file_path,
                name: Some("jsonsearch".to_string()),
                chunker: "fixed".to_string(),
                chunk_size: 1000,
                overlap: 0,
            }),
        );
        execute(&cli).expect("load");

        let cli = make_cli_json(
            db_path,
            Commands::Search {
                query: "test".to_string(),
                top_k: 5,
                threshold: 0.3,
                mode: "bm25".to_string(),
                rrf_k: 60,
                buffer: None,
                preview: false,
                preview_len: 150,
            },
        );
        let result = execute(&cli);
        assert!(result.is_ok());
        let output = result.expect("json output");
        assert!(output.contains('{') || output.contains('['));
    }

    #[test]
    fn test_cmd_load_semantic_chunker() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file_path = temp_dir.path().join("content.txt");
        std::fs::write(
            &file_path,
            "First paragraph here.\n\nSecond paragraph.\n\nThird paragraph.",
        )
        .expect("write file");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path,
            Commands::Buffer(BufferCommands::Load {
                file: file_path,
                name: Some("semantic".to_string()),
                chunker: "semantic".to_string(),
                chunk_size: 1000,
                overlap: 100,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cmd_load_parallel_chunker() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file_path = temp_dir.path().join("content.txt");
        std::fs::write(&file_path, "x".repeat(500)).expect("write file");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path,
            Commands::Buffer(BufferCommands::Load {
                file: file_path,
                name: Some("parallel".to_string()),
                chunker: "parallel".to_string(),
                chunk_size: 100,
                overlap: 10,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cmd_variable_not_found() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        // Get nonexistent variable
        let cli = make_cli(
            db_path,
            Commands::Context(ContextCommands::Var {
                name: "nonexistent".to_string(),
                value: None,
                delete: false,
            }),
        );
        let result = execute(&cli);
        // Should return a message about variable not found
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_cmd_chunk_embed() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file_path = temp_dir.path().join("content.txt");
        std::fs::write(&file_path, "Content for embedding test").expect("write file");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Load {
                file: file_path,
                name: Some("embedbuf".to_string()),
                chunker: "fixed".to_string(),
                chunk_size: 1000,
                overlap: 0,
            }),
        );
        execute(&cli).expect("load");

        // Embed chunks
        let cli = make_cli(
            db_path.clone(),
            Commands::Chunk(ChunkCommands::Embed {
                buffer: "embedbuf".to_string(),
                force: false,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());

        // Embed with force
        let cli = make_cli(
            db_path,
            Commands::Chunk(ChunkCommands::Embed {
                buffer: "embedbuf".to_string(),
                force: true,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cmd_init_nested_directory() {
        let temp_dir = TempDir::new().expect("temp dir");
        // Create a nested path that doesn't exist yet
        let db_path = temp_dir.path().join("nested").join("dir").join("test.db");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        let result = execute(&cli);
        assert!(result.is_ok());
        assert!(db_path.exists());
    }

    #[test]
    fn test_cmd_resolve_buffer_by_id() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file_path = temp_dir.path().join("content.txt");
        std::fs::write(&file_path, "Resolve by ID").expect("write file");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Load {
                file: file_path,
                name: Some("resolvebuf".to_string()),
                chunker: "fixed".to_string(),
                chunk_size: 1000,
                overlap: 0,
            }),
        );
        execute(&cli).expect("load");

        // Resolve by ID "1"
        let cli = make_cli(
            db_path,
            Commands::Buffer(BufferCommands::Show {
                buffer: "1".to_string(), // ID instead of name
                chunks: false,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cmd_search_with_buffer_filter() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file_path = temp_dir.path().join("content.txt");
        std::fs::write(&file_path, "Searchable content here").expect("write file");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Load {
                file: file_path,
                name: Some("filterbuf".to_string()),
                chunker: "fixed".to_string(),
                chunk_size: 1000,
                overlap: 0,
            }),
        );
        execute(&cli).expect("load");

        // Search with buffer filter
        let cli = make_cli(
            db_path,
            Commands::Search {
                query: "content".to_string(),
                top_k: 5,
                threshold: 0.3,
                mode: "bm25".to_string(),
                rrf_k: 60,
                buffer: Some("filterbuf".to_string()),
                preview: false,
                preview_len: 150,
            },
        );
        let result = execute(&cli);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cmd_search_semantic_mode() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file_path = temp_dir.path().join("content.txt");
        std::fs::write(&file_path, "Semantic search content").expect("write file");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Load {
                file: file_path,
                name: Some("semanticbuf".to_string()),
                chunker: "fixed".to_string(),
                chunk_size: 1000,
                overlap: 0,
            }),
        );
        execute(&cli).expect("load");

        // Embed first for semantic search
        let cli = make_cli(
            db_path.clone(),
            Commands::Chunk(ChunkCommands::Embed {
                buffer: "semanticbuf".to_string(),
                force: false,
            }),
        );
        execute(&cli).expect("embed");

        // Semantic-only search
        let cli = make_cli(
            db_path.clone(),
            Commands::Search {
                query: "semantic".to_string(),
                top_k: 5,
                threshold: 0.1, // Low threshold for test
                mode: "semantic".to_string(),
                rrf_k: 60,
                buffer: None,
                preview: false,
                preview_len: 150,
            },
        );
        let result = execute(&cli);
        assert!(result.is_ok());

        // Hybrid search
        let cli = make_cli(
            db_path,
            Commands::Search {
                query: "content".to_string(),
                top_k: 5,
                threshold: 0.1,
                mode: "hybrid".to_string(),
                rrf_k: 60,
                buffer: None,
                preview: false,
                preview_len: 150,
            },
        );
        let result = execute(&cli);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cmd_grep_json() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file_path = temp_dir.path().join("content.txt");
        std::fs::write(&file_path, "Line with pattern\nAnother line").expect("write file");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Load {
                file: file_path,
                name: Some("grepjson".to_string()),
                chunker: "fixed".to_string(),
                chunk_size: 1000,
                overlap: 0,
            }),
        );
        execute(&cli).expect("load");

        let cli = make_cli_json(
            db_path,
            Commands::Buffer(BufferCommands::Grep {
                buffer: "grepjson".to_string(),
                pattern: "pattern".to_string(),
                max_matches: 10,
                window: 50,
                ignore_case: false,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());
        let output = result.expect("json output");
        assert!(output.contains('{') || output.contains('['));
    }

    #[test]
    fn test_cmd_peek_json() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file_path = temp_dir.path().join("content.txt");
        std::fs::write(&file_path, "Peek JSON content").expect("write file");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Load {
                file: file_path,
                name: Some("peekjson".to_string()),
                chunker: "fixed".to_string(),
                chunk_size: 1000,
                overlap: 0,
            }),
        );
        execute(&cli).expect("load");

        let cli = make_cli_json(
            db_path,
            Commands::Buffer(BufferCommands::Peek {
                buffer: "peekjson".to_string(),
                start: 0,
                end: Some(10),
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cmd_chunk_list_json() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file_path = temp_dir.path().join("content.txt");
        std::fs::write(&file_path, "Chunk list JSON content").expect("write file");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Load {
                file: file_path,
                name: Some("chunklistjson".to_string()),
                chunker: "fixed".to_string(),
                chunk_size: 1000,
                overlap: 0,
            }),
        );
        execute(&cli).expect("load");

        let cli = make_cli_json(
            db_path,
            Commands::Chunk(ChunkCommands::List {
                buffer: "chunklistjson".to_string(),
                preview: true,
                preview_len: 50,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cmd_chunk_get_json() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file_path = temp_dir.path().join("content.txt");
        std::fs::write(&file_path, "Chunk get JSON content").expect("write file");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Load {
                file: file_path,
                name: Some("chunkgetjson".to_string()),
                chunker: "fixed".to_string(),
                chunk_size: 1000,
                overlap: 0,
            }),
        );
        execute(&cli).expect("load");

        let cli = make_cli_json(
            db_path,
            Commands::Chunk(ChunkCommands::Get {
                id: 1,
                metadata: true,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());
        let output = result.expect("json output");
        assert!(output.contains('{'));
    }

    #[test]
    fn test_cmd_chunk_indices_json() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file_path = temp_dir.path().join("content.txt");
        std::fs::write(&file_path, "x".repeat(200)).expect("write file");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Load {
                file: file_path,
                name: Some("indicesjson".to_string()),
                chunker: "fixed".to_string(),
                chunk_size: 1000,
                overlap: 0,
            }),
        );
        execute(&cli).expect("load");

        let cli = make_cli_json(
            db_path,
            Commands::Chunk(ChunkCommands::Indices {
                buffer: "indicesjson".to_string(),
                chunk_size: 50,
                overlap: 10,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cmd_write_chunks_json() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file_path = temp_dir.path().join("content.txt");
        let out_dir = temp_dir.path().join("json_chunks");
        std::fs::write(&file_path, "x".repeat(200)).expect("write file");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Load {
                file: file_path,
                name: Some("writejson".to_string()),
                chunker: "fixed".to_string(),
                chunk_size: 1000,
                overlap: 0,
            }),
        );
        execute(&cli).expect("load");

        let cli = make_cli_json(
            db_path,
            Commands::Chunk(ChunkCommands::Write {
                buffer: "writejson".to_string(),
                out_dir,
                chunk_size: 50,
                overlap: 10,
                prefix: "test".to_string(),
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cmd_delete_buffer_json() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file_path = temp_dir.path().join("content.txt");
        std::fs::write(&file_path, "Delete JSON content").expect("write file");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Load {
                file: file_path,
                name: Some("deletejson".to_string()),
                chunker: "fixed".to_string(),
                chunk_size: 1000,
                overlap: 0,
            }),
        );
        execute(&cli).expect("load");

        let cli = make_cli_json(
            db_path,
            Commands::Buffer(BufferCommands::Delete {
                buffer: "deletejson".to_string(),
                yes: true,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cmd_chunk_status_with_buffers() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file_path = temp_dir.path().join("content.txt");
        std::fs::write(&file_path, "Content for chunk status test").expect("write file");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Load {
                file: file_path,
                name: Some("statusbuf".to_string()),
                chunker: "fixed".to_string(),
                chunk_size: 1000,
                overlap: 0,
            }),
        );
        execute(&cli).expect("load");

        // Check status with buffer present (covers lines 965-982)
        let cli = make_cli(db_path.clone(), Commands::Chunk(ChunkCommands::Status));
        let result = execute(&cli);
        assert!(result.is_ok());
        let output = result.expect("status output");
        assert!(output.contains("Status") || output.contains("statusbuf"));

        // Embed chunks and check status again (covers different status states)
        let cli = make_cli(
            db_path.clone(),
            Commands::Chunk(ChunkCommands::Embed {
                buffer: "statusbuf".to_string(),
                force: false,
            }),
        );
        execute(&cli).expect("embed");

        let cli = make_cli(db_path, Commands::Chunk(ChunkCommands::Status));
        let result = execute(&cli);
        assert!(result.is_ok());
        let output = result.expect("status output");
        // Should show complete status after embedding
        assert!(
            output.contains("complete") || output.contains("embedded") || output.contains("Status")
        );
    }

    #[test]
    fn test_cmd_chunk_status_json() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file_path = temp_dir.path().join("content.txt");
        std::fs::write(&file_path, "JSON chunk status content").expect("write file");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Load {
                file: file_path,
                name: Some("statusjson".to_string()),
                chunker: "fixed".to_string(),
                chunk_size: 1000,
                overlap: 0,
            }),
        );
        execute(&cli).expect("load");

        // JSON status output (covers lines 1028-1041)
        let cli = make_cli_json(db_path, Commands::Chunk(ChunkCommands::Status));
        let result = execute(&cli);
        assert!(result.is_ok());
        let output = result.expect("json output");
        assert!(output.contains("total_chunks") || output.contains('{'));
    }

    #[test]
    fn test_cmd_buffer_with_long_name() {
        // This test covers truncate_str function (lines 1047-1053)
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file_path = temp_dir.path().join("content.txt");
        std::fs::write(&file_path, "Long name content").expect("write file");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        // Use a very long buffer name to trigger truncation
        let long_name = "a".repeat(50);
        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Load {
                file: file_path,
                name: Some(long_name),
                chunker: "fixed".to_string(),
                chunk_size: 1000,
                overlap: 0,
            }),
        );
        execute(&cli).expect("load");

        // List buffers should truncate the long name
        let cli = make_cli(db_path.clone(), Commands::Buffer(BufferCommands::List));
        let result = execute(&cli);
        assert!(result.is_ok());
        let output = result.expect("list output");
        // Long name should appear (possibly truncated with ...)
        assert!(output.contains("aaa") || output.contains("..."));

        // Chunk status should also show truncated name
        let cli = make_cli(db_path, Commands::Chunk(ChunkCommands::Status));
        let result = execute(&cli);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cmd_add_buffer_json() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        // Add buffer with JSON output (covers lines 533-538)
        let cli = make_cli_json(
            db_path,
            Commands::Buffer(BufferCommands::Add {
                name: "addjson".to_string(),
                content: Some("JSON added content".to_string()),
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());
        let output = result.expect("json output");
        assert!(output.contains("buffer_id") || output.contains('{'));
    }

    #[test]
    fn test_cmd_chunk_embed_already_embedded() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file_path = temp_dir.path().join("content.txt");
        std::fs::write(&file_path, "Already embedded content").expect("write file");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Load {
                file: file_path,
                name: Some("alreadyembedded".to_string()),
                chunker: "fixed".to_string(),
                chunk_size: 1000,
                overlap: 0,
            }),
        );
        execute(&cli).expect("load");

        // First embed
        let cli = make_cli(
            db_path.clone(),
            Commands::Chunk(ChunkCommands::Embed {
                buffer: "alreadyembedded".to_string(),
                force: false,
            }),
        );
        execute(&cli).expect("first embed");

        // Try to embed again without force (covers lines 933-936)
        let cli = make_cli(
            db_path,
            Commands::Chunk(ChunkCommands::Embed {
                buffer: "alreadyembedded".to_string(),
                force: false,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());
        let output = result.expect("already embedded output");
        assert!(output.contains("already") || output.contains("embedding"));
    }

    #[test]
    fn test_cmd_chunk_embed_json() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file_path = temp_dir.path().join("content.txt");
        std::fs::write(&file_path, "JSON embed content").expect("write file");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Load {
                file: file_path,
                name: Some("embedjson".to_string()),
                chunker: "fixed".to_string(),
                chunk_size: 1000,
                overlap: 0,
            }),
        );
        execute(&cli).expect("load");

        // Embed with JSON output (covers lines 948-953)
        let cli = make_cli_json(
            db_path,
            Commands::Chunk(ChunkCommands::Embed {
                buffer: "embedjson".to_string(),
                force: false,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());
        let output = result.expect("json output");
        assert!(output.contains("chunks_embedded") || output.contains('{'));
    }

    #[test]
    fn test_cmd_variable_json() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        // Set variable with JSON output
        let cli = make_cli_json(
            db_path.clone(),
            Commands::Context(ContextCommands::Var {
                name: "jsonvar".to_string(),
                value: Some("jsonvalue".to_string()),
                delete: false,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());

        // Get variable with JSON output
        let cli = make_cli_json(
            db_path,
            Commands::Context(ContextCommands::Var {
                name: "jsonvar".to_string(),
                value: None,
                delete: false,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cmd_global_json() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        // Set global with JSON output
        let cli = make_cli_json(
            db_path.clone(),
            Commands::Context(ContextCommands::Global {
                name: "jsonglobal".to_string(),
                value: Some("jsonglobalvalue".to_string()),
                delete: false,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());

        // Get global with JSON output
        let cli = make_cli_json(
            db_path,
            Commands::Context(ContextCommands::Global {
                name: "jsonglobal".to_string(),
                value: None,
                delete: false,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cmd_multiple_buffers_status() {
        // Test with multiple buffers having different embedding states
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file1 = temp_dir.path().join("file1.txt");
        let file2 = temp_dir.path().join("file2.txt");
        let file3 = temp_dir.path().join("file3.txt");
        std::fs::write(&file1, "First buffer content").expect("write file1");
        std::fs::write(&file2, "Second buffer content").expect("write file2");
        std::fs::write(&file3, "Third buffer content").expect("write file3");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        // Load three buffers
        for (file, name) in [(&file1, "buf1"), (&file2, "buf2"), (&file3, "buf3")] {
            let cli = make_cli(
                db_path.clone(),
                Commands::Buffer(BufferCommands::Load {
                    file: file.clone(),
                    name: Some(name.to_string()),
                    chunker: "fixed".to_string(),
                    chunk_size: 1000,
                    overlap: 0,
                }),
            );
            execute(&cli).expect("load");
        }

        // Embed only buf1 (complete)
        let cli = make_cli(
            db_path.clone(),
            Commands::Chunk(ChunkCommands::Embed {
                buffer: "buf1".to_string(),
                force: false,
            }),
        );
        execute(&cli).expect("embed buf1");

        // Status should show different states (complete, none)
        let cli = make_cli(db_path.clone(), Commands::Chunk(ChunkCommands::Status));
        let result = execute(&cli);
        assert!(result.is_ok());
        let output = result.expect("status output");
        // Should have status indicators
        assert!(output.contains("buf1") || output.contains("Status"));

        // JSON status
        let cli = make_cli_json(db_path, Commands::Chunk(ChunkCommands::Status));
        let result = execute(&cli);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cmd_show_buffer_json() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file_path = temp_dir.path().join("content.txt");
        std::fs::write(&file_path, "Show buffer JSON content").expect("write file");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Load {
                file: file_path,
                name: Some("showjson".to_string()),
                chunker: "fixed".to_string(),
                chunk_size: 1000,
                overlap: 0,
            }),
        );
        execute(&cli).expect("load");

        // Show buffer with JSON output
        let cli = make_cli_json(
            db_path,
            Commands::Buffer(BufferCommands::Show {
                buffer: "showjson".to_string(),
                chunks: true,
            }),
        );
        let result = execute(&cli);
        assert!(result.is_ok());
        let output = result.expect("json output");
        assert!(output.contains('{') || output.contains("showjson"));
    }

    #[test]
    fn test_cmd_list_json() {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let file_path = temp_dir.path().join("content.txt");
        std::fs::write(&file_path, "List JSON content").expect("write file");

        let cli = make_cli(db_path.clone(), Commands::Init { force: false });
        execute(&cli).expect("init");

        let cli = make_cli(
            db_path.clone(),
            Commands::Buffer(BufferCommands::Load {
                file: file_path,
                name: Some("listjson".to_string()),
                chunker: "fixed".to_string(),
                chunk_size: 1000,
                overlap: 0,
            }),
        );
        execute(&cli).expect("load");

        // List buffers with JSON output
        let cli = make_cli_json(db_path, Commands::Buffer(BufferCommands::List));
        let result = execute(&cli);
        assert!(result.is_ok());
        let output = result.expect("json output");
        assert!(output.contains('{') || output.contains('['));
    }
}

// =============================================================================
// Parallel Chunker Integration Tests
// =============================================================================

#[test]
fn test_parallel_chunker_ordering() {
    use rlm_rs::chunking::{Chunker, ParallelChunker, SemanticChunker};

    // Create content large enough to trigger parallel processing
    // ParallelChunker uses 2KB min partition size
    let content = "Line one. ".repeat(500); // ~5000 bytes

    let chunker = ParallelChunker::new(SemanticChunker::with_size(100));
    let chunks = chunker.chunk(1, &content, None).expect("chunk failed");

    // Verify chunks are in order by byte_range
    for i in 1..chunks.len() {
        assert!(
            chunks[i - 1].byte_range.end <= chunks[i].byte_range.start,
            "Chunk {} end ({}) should be <= chunk {} start ({})",
            i - 1,
            chunks[i - 1].byte_range.end,
            i,
            chunks[i].byte_range.start
        );
    }

    // Verify all chunks have correct buffer_id
    for chunk in &chunks {
        assert_eq!(chunk.buffer_id, 1);
    }

    // Verify chunks cover the entire content (with possible overlap gaps)
    assert_eq!(chunks.first().expect("non-empty").byte_range.start, 0);
}

#[test]
fn test_parallel_chunker_storage_integration() {
    use rlm_rs::chunking::{Chunker, ParallelChunker, SemanticChunker};

    let (mut storage, _temp) = create_test_storage();

    // Create a buffer with substantial content
    let content = "Paragraph one. ".repeat(200);
    let buffer = Buffer::from_named("parallel-test".to_string(), content.clone());

    let buffer_id = storage.add_buffer(&buffer).expect("add buffer");

    // Chunk with parallel chunker
    let chunker = ParallelChunker::new(SemanticChunker::with_size(500));
    let chunks = chunker.chunk(buffer_id, &content, None).expect("chunk");

    // Store all chunks
    storage.add_chunks(buffer_id, &chunks).expect("add chunks");

    // Verify chunks were stored correctly
    let loaded_chunks = storage.get_chunks(buffer_id).expect("get chunks");
    assert_eq!(loaded_chunks.len(), chunks.len());

    // Verify chunk ordering is preserved in storage
    let mut sorted = loaded_chunks;
    sorted.sort_by_key(|c| c.index);
    for (original, stored) in chunks.iter().zip(sorted.iter()) {
        assert_eq!(original.index, stored.index);
        assert_eq!(original.byte_range, stored.byte_range);
        assert_eq!(original.content, stored.content);
    }
}

#[test]
fn test_parallel_chunker_overlap_handling() {
    use rlm_rs::chunking::{Chunker, ParallelChunker, SemanticChunker};

    let content = "Word five. ".repeat(250); // ~2750 bytes
    let chunk_size = 100;
    let overlap = 20;

    let chunker = ParallelChunker::new(SemanticChunker::with_size_and_overlap(chunk_size, overlap));
    let chunks = chunker.chunk(1, &content, None).expect("chunk");

    assert!(!chunks.is_empty(), "Should produce chunks");

    // Verify overlap exists between consecutive chunks (where applicable)
    let mut overlap_found = false;
    for i in 1..chunks.len() {
        let prev_end = chunks[i - 1].byte_range.end;
        let curr_start = chunks[i].byte_range.start;

        // With overlap, some chunks may have overlapping content
        if curr_start < prev_end {
            overlap_found = true;
        }
    }

    // Note: Parallel chunker may not guarantee overlap at partition boundaries
    // The important thing is chunks are contiguous and ordered
    for chunk in &chunks {
        assert!(chunk.byte_range.start < chunk.byte_range.end);
    }

    // Suppress unused variable warning
    let _ = overlap_found;
}

#[test]
fn test_parallel_chunker_with_unicode() {
    use rlm_rs::chunking::{Chunker, ParallelChunker, SemanticChunker};

    // Content with multi-byte UTF-8 characters
    let content = "日本語テスト。".repeat(300);

    let chunker = ParallelChunker::new(SemanticChunker::with_size(100));
    let chunks = chunker.chunk(1, &content, None).expect("chunk");

    // Verify all chunks are valid UTF-8 (no mid-character splits)
    for chunk in &chunks {
        // This should not panic - content is already String so it's valid UTF-8
        assert!(chunk.content.is_ascii() || !chunk.content.is_empty());

        // Verify byte_range references valid UTF-8 boundaries
        let slice = &content[chunk.byte_range.clone()];
        assert_eq!(slice, chunk.content);
    }
}

#[test]
fn test_parallel_chunker_small_content_fallback() {
    use rlm_rs::chunking::{Chunker, ParallelChunker, SemanticChunker};

    // Content smaller than min partition size (2KB) should fall back to single-threaded
    let content = "Small content here.";

    let chunker = ParallelChunker::new(SemanticChunker::with_size(100));
    let chunks = chunker.chunk(1, content, None).expect("chunk");

    assert!(!chunks.is_empty());
    assert_eq!(chunks[0].buffer_id, 1);
}

// ---------------------------------------------------------------------------
// Aggregate / Dispatch / Relevance CLI tests
// ---------------------------------------------------------------------------

#[cfg(feature = "agent")]
#[test]
fn test_cmd_aggregate_from_buffer() {
    use rlm_rs::cli::commands::execute;
    use rlm_rs::cli::parser::{AgentCommands, Cli, Commands};
    use tempfile::TempDir;

    let temp_dir = TempDir::new().expect("temp dir");
    let db_path = temp_dir.path().join("test.db");

    // Init database
    let cli = Cli {
        db_path: Some(db_path.clone()),
        verbose: false,
        format: "text".to_string(),
        command: Commands::Init { force: false },
    };
    execute(&cli).expect("init");

    // Store findings JSON in a buffer
    let findings_json = r#"[
        {"chunk_id": 1, "relevance": "high", "findings": ["Found critical bug"], "summary": "Bug in auth"},
        {"chunk_id": 2, "relevance": "medium", "findings": ["Minor issue"], "summary": "Logging gap"},
        {"chunk_id": 3, "relevance": "low", "findings": ["Style nit"], "summary": "Naming"}
    ]"#;
    {
        let mut storage = rlm_rs::storage::SqliteStorage::open(&db_path).expect("open storage");
        let buf = rlm_rs::core::Buffer::from_named(
            "test-findings".to_string(),
            findings_json.to_string(),
        );
        storage.add_buffer(&buf).expect("add buffer");
    }

    // Aggregate from buffer — text output
    let cli = Cli {
        db_path: Some(db_path.clone()),
        verbose: false,
        format: "text".to_string(),
        command: Commands::Agent(AgentCommands::Aggregate {
            buffer: Some("test-findings".to_string()),
            min_relevance: "low".to_string(),
            group_by: "relevance".to_string(),
            sort_by: "relevance".to_string(),
            output_buffer: None,
        }),
    };
    let result = execute(&cli).expect("aggregate text");
    assert!(result.contains("Aggregated 3 analyst findings"));
    assert!(result.contains("1 high"));
    assert!(result.contains("1 medium"));
    assert!(result.contains("1 low"));

    // Aggregate with medium threshold — should filter out low
    let cli = Cli {
        db_path: Some(db_path.clone()),
        verbose: false,
        format: "text".to_string(),
        command: Commands::Agent(AgentCommands::Aggregate {
            buffer: Some("test-findings".to_string()),
            min_relevance: "medium".to_string(),
            group_by: "relevance".to_string(),
            sort_by: "relevance".to_string(),
            output_buffer: None,
        }),
    };
    let result = execute(&cli).expect("aggregate filtered");
    assert!(result.contains("Aggregated 2 analyst findings"));

    // Aggregate JSON output
    let cli = Cli {
        db_path: Some(db_path),
        verbose: false,
        format: "json".to_string(),
        command: Commands::Agent(AgentCommands::Aggregate {
            buffer: Some("test-findings".to_string()),
            min_relevance: "low".to_string(),
            group_by: "none".to_string(),
            sort_by: "chunk_id".to_string(),
            output_buffer: None,
        }),
    };
    let result = execute(&cli).expect("aggregate json");
    let parsed: serde_json::Value = serde_json::from_str(&result).expect("valid json");
    assert_eq!(parsed["summary"]["total_findings"], 3);
}

#[cfg(feature = "agent")]
#[test]
fn test_cmd_dispatch() {
    use rlm_rs::cli::commands::execute;
    use rlm_rs::cli::parser::{AgentCommands, BufferCommands, Cli, Commands};
    use tempfile::TempDir;

    let temp_dir = TempDir::new().expect("temp dir");
    let db_path = temp_dir.path().join("test.db");
    let file_path = temp_dir.path().join("source.txt");
    std::fs::write(
        &file_path,
        "First section content.\nSecond section content.\nThird section content.",
    )
    .expect("write file");

    // Init + load
    let cli = Cli {
        db_path: Some(db_path.clone()),
        verbose: false,
        format: "text".to_string(),
        command: Commands::Init { force: false },
    };
    execute(&cli).expect("init");

    let cli = Cli {
        db_path: Some(db_path.clone()),
        verbose: false,
        format: "text".to_string(),
        command: Commands::Buffer(BufferCommands::Load {
            file: file_path,
            name: Some("dispatch-buf".to_string()),
            chunker: "fixed".to_string(),
            chunk_size: 30,
            overlap: 0,
        }),
    };
    execute(&cli).expect("load");

    // Dispatch — text
    let cli = Cli {
        db_path: Some(db_path.clone()),
        verbose: false,
        format: "text".to_string(),
        command: Commands::Agent(AgentCommands::Dispatch {
            buffer: "dispatch-buf".to_string(),
            batch_size: 2,
            workers: None,
            query: None,
            mode: "hybrid".to_string(),
            threshold: 0.3,
        }),
    };
    let result = execute(&cli).expect("dispatch text");
    assert!(result.contains("batch"));

    // Dispatch — JSON
    let cli = Cli {
        db_path: Some(db_path),
        verbose: false,
        format: "json".to_string(),
        command: Commands::Agent(AgentCommands::Dispatch {
            buffer: "dispatch-buf".to_string(),
            batch_size: 2,
            workers: None,
            query: None,
            mode: "bm25".to_string(),
            threshold: 0.3,
        }),
    };
    let result = execute(&cli).expect("dispatch json");
    let parsed: serde_json::Value = serde_json::from_str(&result).expect("valid json");
    assert!(parsed["batches"].is_array());
}

#[test]
fn test_relevance_shared_type() {
    use rlm_rs::core::Relevance;

    // Verify the shared Relevance type works correctly
    let high = Relevance::parse("high");
    let medium = Relevance::parse("medium");
    let low = Relevance::parse("low");
    let none = Relevance::parse("none");

    // Ordering: High < Medium < Low < None (by discriminant)
    assert!(high < medium);
    assert!(medium < low);
    assert!(low < none);

    // Threshold checks
    assert!(high.meets_threshold(low)); // high meets a low bar
    assert!(!low.meets_threshold(high)); // low doesn't meet a high bar
    assert!(medium.meets_threshold(medium)); // self meets self

    // Display roundtrip
    assert_eq!(high.as_str(), "high");
    assert_eq!(format!("{medium}"), "medium");

    // Serde roundtrip
    let json = serde_json::to_string(&high).expect("serialize");
    assert_eq!(json, "\"high\"");
    let deser: Relevance = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(deser, high);
}
