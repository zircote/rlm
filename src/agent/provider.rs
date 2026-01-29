//! Pluggable LLM provider trait.
//!
//! Implementations translate provider-agnostic [`ChatRequest`]/[`ChatResponse`]
//! into provider-specific SDK calls. This keeps all agent logic decoupled
//! from any particular LLM vendor.

use std::pin::Pin;

use async_trait::async_trait;
use futures_util::Stream;

use super::message::{ChatRequest, ChatResponse};
use crate::error::AgentError;

/// Trait for LLM provider backends.
///
/// Implementations handle the transport layer (HTTP, SDK calls, retries)
/// for a specific provider while presenting a uniform interface to agents.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Provider name (e.g., `"openai"`, `"anthropic"`).
    fn name(&self) -> &'static str;

    /// Executes a chat completion request.
    ///
    /// # Errors
    ///
    /// Returns [`AgentError`] on API failures, timeouts, or parse errors.
    async fn chat(&self, request: &ChatRequest) -> Result<ChatResponse, AgentError>;

    /// Executes a streaming chat completion request.
    ///
    /// Returns a stream of text chunks as they arrive from the provider.
    ///
    /// # Errors
    ///
    /// Returns [`AgentError`] on connection or streaming failures.
    async fn chat_stream(
        &self,
        request: &ChatRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, AgentError>> + Send>>, AgentError>;
}
