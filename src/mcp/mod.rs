//! MCP (Model Context Protocol) server for rlm-rs.
//!
//! Exposes the RLM agentic query pipeline and storage as an MCP server,
//! allowing external agents to delegate chunking, analysis, and synthesis
//! to rlm-rs.
//!
//! # Feature Gate
//!
//! This module requires the `mcp` feature flag:
//! ```toml
//! [dependencies]
//! rlm-rs = { version = "...", features = ["mcp"] }
//! ```
//!
//! # Architecture
//!
//! ```text
//! MCP Client (Claude agent)
//!   ↓ query(query, buffer_name)
//! RlmMcpServer
//!   ↓ spawn_blocking (SqliteStorage is !Send)
//! Orchestrator::query()
//!   ├── PrimaryAgent (plan)
//!   ├── Search (hybrid semantic + BM25)
//!   ├── Fan-out → N SubcallAgents
//!   └── SynthesizerAgent → response
//!   ↓
//! QueryResult JSON → MCP Client
//! ```

pub mod params;
pub mod server;
pub mod transport;

pub use params::QueryParams;
pub use server::RlmMcpServer;
pub use transport::{serve_sse, serve_stdio};
