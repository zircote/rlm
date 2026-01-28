//! Core domain models for RLM-RS.
//!
//! This module contains the fundamental data structures used throughout the
//! RLM system: contexts, buffers, and chunks. These are pure domain models
//! with no I/O dependencies.

pub mod buffer;
pub mod chunk;
pub mod context;

pub use buffer::{Buffer, BufferMetadata};
pub use chunk::{Chunk, ChunkMetadata, estimate_tokens_for_text};
pub use context::{Context, ContextValue};
