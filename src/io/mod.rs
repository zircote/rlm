//! I/O utilities for RLM-RS.
//!
//! Provides file reading with memory mapping support for efficient
//! handling of large files, along with Unicode utilities.

pub mod reader;
pub mod unicode;

pub use reader::{FileReader, read_file, read_file_mmap, write_chunks, write_file};
pub use unicode::{current_timestamp, find_char_boundary, validate_utf8};
