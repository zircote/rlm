//! CLI layer for RLM-RS.
//!
//! Provides the command-line interface using clap, with commands
//! for initializing, managing, and querying RLM state.

pub mod commands;
pub mod output;
pub mod parser;

pub use commands::execute;
pub use output::OutputFormat;
#[cfg(feature = "agent")]
pub use parser::AgentCommands;
pub use parser::{BufferCommands, ChunkCommands, Cli, Commands, ContextCommands};
