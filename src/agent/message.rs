//! Provider-agnostic message types for LLM communication.
//!
//! These types decouple agent logic from any specific LLM SDK,
//! allowing the same agents to work across `OpenAI`, Anthropic, etc.

use serde::{Deserialize, Serialize};

use super::tool::{ToolCall, ToolDefinition};

/// Role of a chat message participant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// System instructions.
    System,
    /// User input.
    User,
    /// Assistant response.
    Assistant,
    /// Tool result.
    Tool,
}

/// A single chat message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Role of the message sender.
    pub role: Role,
    /// Message content.
    pub content: String,
    /// Tool calls requested by the assistant (only for `Role::Assistant`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    /// Tool call ID this message responds to (only for `Role::Tool`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// A chat completion request (provider-agnostic).
#[derive(Debug, Clone)]
pub struct ChatRequest {
    /// Model identifier (e.g., "gpt-5.2-2025-12-11").
    pub model: String,
    /// Ordered conversation messages.
    pub messages: Vec<ChatMessage>,
    /// Sampling temperature (0.0–2.0).
    pub temperature: Option<f32>,
    /// Maximum tokens to generate.
    pub max_tokens: Option<u32>,
    /// Request JSON-formatted output.
    pub json_mode: bool,
    /// Stream the response.
    pub stream: bool,
    /// Tool definitions available to the model.
    pub tools: Vec<ToolDefinition>,
}

/// Token usage statistics from a completion.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Tokens consumed by the prompt.
    pub prompt_tokens: u32,
    /// Tokens generated in the completion.
    pub completion_tokens: u32,
    /// Total tokens used.
    pub total_tokens: u32,
}

/// A chat completion response (provider-agnostic).
#[derive(Debug, Clone)]
pub struct ChatResponse {
    /// Generated text content.
    pub content: String,
    /// Token usage statistics.
    pub usage: TokenUsage,
    /// Tool calls requested by the model.
    pub tool_calls: Vec<ToolCall>,
    /// Finish reason from the model (e.g., `"stop"`, `"tool_calls"`).
    pub finish_reason: Option<String>,
}

/// Creates a system message.
#[must_use]
pub fn system_message(content: &str) -> ChatMessage {
    ChatMessage {
        role: Role::System,
        content: content.to_string(),
        tool_calls: Vec::new(),
        tool_call_id: None,
    }
}

/// Creates a user message.
#[must_use]
pub fn user_message(content: &str) -> ChatMessage {
    ChatMessage {
        role: Role::User,
        content: content.to_string(),
        tool_calls: Vec::new(),
        tool_call_id: None,
    }
}

/// Creates an assistant message with tool calls (no text content).
#[must_use]
pub const fn assistant_tool_calls_message(tool_calls: Vec<ToolCall>) -> ChatMessage {
    ChatMessage {
        role: Role::Assistant,
        content: String::new(),
        tool_calls,
        tool_call_id: None,
    }
}

/// Creates a tool result message.
#[must_use]
pub fn tool_message(tool_call_id: &str, content: &str) -> ChatMessage {
    ChatMessage {
        role: Role::Tool,
        content: content.to_string(),
        tool_calls: Vec::new(),
        tool_call_id: Some(tool_call_id.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_message() {
        let msg = system_message("You are helpful.");
        assert_eq!(msg.role, Role::System);
        assert_eq!(msg.content, "You are helpful.");
        assert!(msg.tool_calls.is_empty());
        assert!(msg.tool_call_id.is_none());
    }

    #[test]
    fn test_user_message() {
        let msg = user_message("Hello");
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content, "Hello");
    }

    #[test]
    fn test_tool_message() {
        let msg = tool_message("call_123", "result data");
        assert_eq!(msg.role, Role::Tool);
        assert_eq!(msg.content, "result data");
        assert_eq!(msg.tool_call_id.as_deref(), Some("call_123"));
    }

    #[test]
    fn test_assistant_tool_calls_message() {
        let calls = vec![ToolCall {
            id: "call_1".to_string(),
            name: "get_chunks".to_string(),
            arguments: r#"{"chunk_ids":[1]}"#.to_string(),
        }];
        let msg = assistant_tool_calls_message(calls);
        assert_eq!(msg.role, Role::Assistant);
        assert!(msg.content.is_empty());
        assert_eq!(msg.tool_calls.len(), 1);
        assert_eq!(msg.tool_calls[0].name, "get_chunks");
    }

    #[test]
    fn test_role_serialization() {
        let json = serde_json::to_string(&Role::System).unwrap_or_default();
        assert_eq!(json, "\"system\"");

        let json = serde_json::to_string(&Role::Tool).unwrap_or_default();
        assert_eq!(json, "\"tool\"");
    }

    #[test]
    fn test_chat_message_serialization() {
        let msg = user_message("test");
        let json = serde_json::to_string(&msg).unwrap_or_default();
        assert!(json.contains("\"user\""));
        assert!(json.contains("\"test\""));
        // tool_calls and tool_call_id should be omitted when empty/None
        assert!(!json.contains("tool_calls"));
        assert!(!json.contains("tool_call_id"));
    }
}
