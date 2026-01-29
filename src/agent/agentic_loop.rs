//! Agentic tool-calling loop.
//!
//! Drives the LLM ↔ tool execution round-trip: sends a request to the model,
//! executes any tool calls in the response, appends results, and repeats
//! until the model produces a final text response or the iteration limit
//! is reached.

use tracing::debug;

use super::executor::ToolExecutor;
use super::message::{ChatRequest, ChatResponse, assistant_tool_calls_message, tool_message};
use super::provider::LlmProvider;
use crate::error::AgentError;

/// Runs an agentic loop: model → tool calls → tool results → model → …
///
/// Continues until the model responds without tool calls (i.e., it produces
/// a final text answer) or `max_iterations` is reached.
///
/// # Arguments
///
/// * `provider` - LLM provider to call.
/// * `request` - Initial chat request (mutated in-place with tool messages).
/// * `executor` - Dispatches tool calls to internal functions.
/// * `max_iterations` - Safety limit on round-trips.
///
/// # Returns
///
/// The final [`ChatResponse`] containing the model's text answer and
/// cumulative usage from the last call. Earlier usage is not aggregated
/// (the orchestrator tracks total tokens separately).
///
/// # Errors
///
/// Returns [`AgentError::ToolLoopExceeded`] if the model keeps requesting
/// tools beyond `max_iterations`. Propagates any provider errors.
#[allow(clippy::future_not_send)]
pub async fn agentic_loop(
    provider: &dyn LlmProvider,
    request: &mut ChatRequest,
    executor: &ToolExecutor<'_>,
    max_iterations: usize,
) -> Result<ChatResponse, AgentError> {
    for iteration in 0..max_iterations {
        let response = provider.chat(request).await?;

        // If no tool calls, we have a final answer
        if response.tool_calls.is_empty() {
            debug!(iteration, "agentic loop completed with final text response");
            return Ok(response);
        }

        debug!(
            iteration,
            tool_count = response.tool_calls.len(),
            "executing tool calls"
        );

        // Append the assistant message with tool calls
        request
            .messages
            .push(assistant_tool_calls_message(response.tool_calls.clone()));

        // Execute each tool call and append results
        for call in &response.tool_calls {
            let result = executor.execute(call);
            debug!(
                tool = call.name,
                call_id = call.id,
                is_error = result.is_error,
                "tool execution complete"
            );
            request
                .messages
                .push(tool_message(&result.tool_call_id, &result.content));
        }
    }

    Err(AgentError::ToolLoopExceeded { max_iterations })
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;
    use crate::agent::message::{
        ChatRequest, ChatResponse, TokenUsage, system_message, user_message,
    };
    use crate::agent::tool::ToolCall;
    use crate::error::AgentError;
    use crate::storage::{SqliteStorage, Storage};

    use std::pin::Pin;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;
    use futures_util::Stream;

    /// Mock provider that returns tool calls on the first N calls,
    /// then a final text response.
    struct MockToolProvider {
        call_count: AtomicUsize,
        tool_rounds: usize,
    }

    impl MockToolProvider {
        fn new(tool_rounds: usize) -> Self {
            Self {
                call_count: AtomicUsize::new(0),
                tool_rounds,
            }
        }
    }

    #[async_trait]
    impl LlmProvider for MockToolProvider {
        fn name(&self) -> &'static str {
            "mock"
        }

        async fn chat(&self, _request: &ChatRequest) -> Result<ChatResponse, AgentError> {
            let count = self.call_count.fetch_add(1, Ordering::SeqCst);

            if count < self.tool_rounds {
                // Return a tool call
                Ok(ChatResponse {
                    content: String::new(),
                    usage: TokenUsage::default(),
                    tool_calls: vec![ToolCall {
                        id: format!("call_{count}"),
                        name: "storage_stats".to_string(),
                        arguments: "{}".to_string(),
                    }],
                    finish_reason: Some("tool_calls".to_string()),
                })
            } else {
                // Return final text
                Ok(ChatResponse {
                    content: "Final answer based on tool results.".to_string(),
                    usage: TokenUsage {
                        prompt_tokens: 100,
                        completion_tokens: 20,
                        total_tokens: 120,
                    },
                    tool_calls: Vec::new(),
                    finish_reason: Some("stop".to_string()),
                })
            }
        }

        async fn chat_stream(
            &self,
            _request: &ChatRequest,
        ) -> Result<Pin<Box<dyn Stream<Item = Result<String, AgentError>> + Send>>, AgentError>
        {
            Err(AgentError::Stream {
                message: "not implemented".to_string(),
            })
        }
    }

    fn setup_storage() -> SqliteStorage {
        let mut storage =
            SqliteStorage::in_memory().unwrap_or_else(|e| panic!("in_memory failed: {e}"));
        storage
            .init()
            .unwrap_or_else(|e| panic!("init failed: {e}"));
        storage
    }

    #[tokio::test]
    async fn test_agentic_loop_single_tool_round() {
        let storage = setup_storage();
        let executor = ToolExecutor::new(&storage);
        let provider = MockToolProvider::new(1);

        let mut request = ChatRequest {
            model: "test".to_string(),
            messages: vec![
                system_message("You are a test agent."),
                user_message("Get storage stats."),
            ],
            temperature: Some(0.0),
            max_tokens: Some(1024),
            json_mode: false,
            stream: false,
            tools: Vec::new(),
        };

        let response = agentic_loop(&provider, &mut request, &executor, 10)
            .await
            .unwrap_or_else(|e| panic!("agentic_loop failed: {e}"));

        assert_eq!(response.content, "Final answer based on tool results.");
        // Should have: system + user + assistant(tool_calls) + tool(result) = 4 messages
        assert_eq!(request.messages.len(), 4);
    }

    #[tokio::test]
    async fn test_agentic_loop_multiple_rounds() {
        let storage = setup_storage();
        let executor = ToolExecutor::new(&storage);
        let provider = MockToolProvider::new(3);

        let mut request = ChatRequest {
            model: "test".to_string(),
            messages: vec![system_message("test"), user_message("query")],
            temperature: Some(0.0),
            max_tokens: Some(1024),
            json_mode: false,
            stream: false,
            tools: Vec::new(),
        };

        let response = agentic_loop(&provider, &mut request, &executor, 10)
            .await
            .unwrap_or_else(|e| panic!("agentic_loop failed: {e}"));

        assert_eq!(response.content, "Final answer based on tool results.");
        // 2 initial + 3 rounds * 2 (assistant + tool) = 8 messages
        assert_eq!(request.messages.len(), 8);
    }

    #[tokio::test]
    async fn test_agentic_loop_exceeds_max() {
        let storage = setup_storage();
        let executor = ToolExecutor::new(&storage);
        // Provider always returns tool calls (100 rounds > max of 2)
        let provider = MockToolProvider::new(100);

        let mut request = ChatRequest {
            model: "test".to_string(),
            messages: vec![system_message("test"), user_message("query")],
            temperature: Some(0.0),
            max_tokens: Some(1024),
            json_mode: false,
            stream: false,
            tools: Vec::new(),
        };

        let result = agentic_loop(&provider, &mut request, &executor, 2).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, AgentError::ToolLoopExceeded { max_iterations: 2 }),
            "Expected ToolLoopExceeded, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_agentic_loop_no_tools() {
        let storage = setup_storage();
        let executor = ToolExecutor::new(&storage);
        // Provider returns text immediately (0 tool rounds)
        let provider = MockToolProvider::new(0);

        let mut request = ChatRequest {
            model: "test".to_string(),
            messages: vec![system_message("test"), user_message("query")],
            temperature: Some(0.0),
            max_tokens: Some(1024),
            json_mode: false,
            stream: false,
            tools: Vec::new(),
        };

        let response = agentic_loop(&provider, &mut request, &executor, 10)
            .await
            .unwrap_or_else(|e| panic!("agentic_loop failed: {e}"));

        assert_eq!(response.content, "Final answer based on tool results.");
        // No tool rounds, so messages unchanged
        assert_eq!(request.messages.len(), 2);
    }
}
