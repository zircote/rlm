//! Agentic query system for RLM-RS.
//!
//! Provides an LLM-powered workflow that fans out analysis across chunks
//! and synthesizes results. Uses a pluggable provider abstraction backed
//! by OpenAI-compatible APIs.
//!
//! # Architecture
//!
//! ```text
//! User query → Orchestrator
//!   ├── PrimaryAgent (plans analysis strategy)
//!   ├── Search (existing hybrid_search)
//!   ├── Fan-out → N concurrent SubcallAgents
//!   │   └── Each analyzes a batch of chunks → Vec<Finding>
//!   ├── Collect all findings
//!   └── SynthesizerAgent → final markdown response
//! ```
//!
//! # Feature Gate
//!
//! This module requires the `agent` feature flag:
//! ```toml
//! [dependencies]
//! rlm-rs = { version = "...", features = ["agent"] }
//! ```

pub mod agentic_loop;
pub mod client;
pub mod config;
pub mod executor;
pub mod finding;
pub mod message;
pub mod orchestrator;
pub mod primary;
pub mod prompt;
pub mod provider;
pub mod providers;
pub mod subcall;
pub mod synthesizer;
pub mod tool;
pub mod traits;

// Re-export key types
pub use config::AgentConfig;
pub use finding::{Finding, LoadedChunk, QueryResult, Relevance, SubagentResult};
pub use message::{ChatMessage, ChatRequest, ChatResponse, Role, TokenUsage};
pub use orchestrator::Orchestrator;
pub use primary::PrimaryAgent;
pub use prompt::PromptSet;
pub use provider::LlmProvider;
pub use subcall::SubcallAgent;
pub use synthesizer::SynthesizerAgent;
pub use tool::{ToolCall, ToolDefinition, ToolResult, ToolSet};
pub use traits::{Agent, execute_with_tools};
