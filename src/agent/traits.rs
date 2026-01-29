//! Agent trait definition.
//!
//! All agents (subcall, synthesizer, primary) implement this trait,
//! which provides a uniform interface for the orchestrator.

use async_trait::async_trait;

use super::executor::ToolExecutor;
use super::message::{ChatRequest, ChatResponse, system_message, user_message};
use super::provider::LlmProvider;
use super::tool::ToolDefinition;
use crate::error::AgentError;

/// Response from an agent execution.
#[derive(Debug, Clone)]
pub struct AgentResponse {
    /// The agent's text output.
    pub content: String,
    /// Token usage for this call.
    pub usage: super::message::TokenUsage,
    /// Why the model stopped generating (e.g. `"stop"`, `"length"`).
    pub finish_reason: Option<String>,
}

/// Trait implemented by all agents in the system.
///
/// Agents encapsulate a specific role (analysis, synthesis, planning)
/// with a fixed system prompt and model configuration. The orchestrator
/// calls [`Agent::execute`] to run the agent against a provider.
///
/// Agents that support tool-calling override [`Agent::tools`] to return
/// their available tool definitions and use [`execute_with_tools`]
/// for agentic loop execution.
#[async_trait]
pub trait Agent: Send + Sync {
    /// Agent name for logging and identification.
    fn name(&self) -> &'static str;

    /// Model identifier to use for this agent.
    fn model(&self) -> &str;

    /// System prompt that defines the agent's role and behavior.
    fn system_prompt(&self) -> &str;

    /// Whether to request JSON-formatted output.
    fn json_mode(&self) -> bool {
        false
    }

    /// Sampling temperature (0.0 = deterministic, higher = more creative).
    fn temperature(&self) -> f32 {
        0.0
    }

    /// Maximum tokens for the response.
    fn max_tokens(&self) -> u32 {
        2048
    }

    /// Tool definitions available to this agent.
    ///
    /// Returns an empty vec by default (no tools). Override to enable
    /// tool-calling for an agent.
    fn tools(&self) -> Vec<ToolDefinition> {
        Vec::new()
    }

    /// Maximum tool-calling loop iterations before aborting.
    fn max_tool_iterations(&self) -> usize {
        10
    }

    /// Executes the agent with the given user message (no tools).
    ///
    /// Builds a [`ChatRequest`] from the agent's configuration and
    /// delegates to the provider.
    ///
    /// # Errors
    ///
    /// Returns [`AgentError`] on API failures or response parsing errors.
    async fn execute(
        &self,
        provider: &dyn LlmProvider,
        user_msg: &str,
    ) -> Result<AgentResponse, AgentError> {
        let request = ChatRequest {
            model: self.model().to_string(),
            messages: vec![system_message(self.system_prompt()), user_message(user_msg)],
            temperature: Some(self.temperature()),
            max_tokens: Some(self.max_tokens()),
            json_mode: self.json_mode(),
            stream: false,
            tools: Vec::new(),
        };

        let response: ChatResponse = provider.chat(&request).await?;

        Ok(AgentResponse {
            content: response.content,
            usage: response.usage,
            finish_reason: response.finish_reason,
        })
    }
}

/// Executes an agent with tool-calling support.
///
/// If the agent's [`Agent::tools`] returns definitions, builds a tool-enabled
/// request and runs the agentic loop. Otherwise falls through to
/// [`Agent::execute`].
///
/// This is a free function (not on the `Agent` trait) because `ToolExecutor`
/// holds a `&SqliteStorage` which is `!Sync`, making futures that capture
/// it `!Send`. The orchestrator calls this on its own thread where `!Send`
/// is acceptable.
///
/// # Errors
///
/// Returns [`AgentError`] on API failures, tool execution errors,
/// or if the tool loop exceeds the agent's max iterations.
#[allow(clippy::future_not_send)]
pub async fn execute_with_tools(
    agent: &dyn Agent,
    provider: &dyn LlmProvider,
    user_msg: &str,
    executor: &ToolExecutor<'_>,
) -> Result<AgentResponse, AgentError> {
    let tool_defs = agent.tools();

    // If no tools, fall back to standard execute
    if tool_defs.is_empty() {
        return agent.execute(provider, user_msg).await;
    }

    let mut request = ChatRequest {
        model: agent.model().to_string(),
        messages: vec![
            system_message(agent.system_prompt()),
            user_message(user_msg),
        ],
        temperature: Some(agent.temperature()),
        max_tokens: Some(agent.max_tokens()),
        json_mode: agent.json_mode(),
        stream: false,
        tools: tool_defs,
    };

    let response = super::agentic_loop::agentic_loop(
        provider,
        &mut request,
        executor,
        agent.max_tool_iterations(),
    )
    .await?;

    Ok(AgentResponse {
        content: response.content,
        usage: response.usage,
        finish_reason: response.finish_reason,
    })
}
