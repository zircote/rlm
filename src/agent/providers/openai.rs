//! `OpenAI` provider implementation using the `async-openai` crate.
//!
//! Supports any `OpenAI`-compatible API (`OpenAI`, Azure, local proxies)
//! via the base URL override in [`AgentConfig`].

use std::pin::Pin;

use async_openai::Client;
use async_openai::config::OpenAIConfig;
use async_openai::types::{
    ChatCompletionMessageToolCall, ChatCompletionRequestAssistantMessage,
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
    ChatCompletionRequestToolMessage, ChatCompletionRequestUserMessage, ChatCompletionTool,
    ChatCompletionToolType, CreateChatCompletionRequest, CreateChatCompletionStreamResponse,
    FunctionCall, FunctionObject, ResponseFormat,
};
use async_trait::async_trait;
use futures_util::{Stream, StreamExt};

use crate::agent::config::AgentConfig;
use crate::agent::message::{ChatMessage, ChatRequest, ChatResponse, Role, TokenUsage};
use crate::agent::provider::LlmProvider;
use crate::agent::tool::ToolCall;
use crate::error::AgentError;

/// `OpenAI`-compatible LLM provider.
///
/// Wraps the `async-openai` client for chat completions. Compatible
/// with any API that follows the `OpenAI` chat completion spec.
pub struct OpenAiProvider {
    client: Client<OpenAIConfig>,
}

impl OpenAiProvider {
    /// Creates a new provider from agent configuration.
    #[must_use]
    pub fn new(config: &AgentConfig) -> Self {
        let mut openai_config = OpenAIConfig::new().with_api_key(&config.api_key);

        if let Some(ref base_url) = config.base_url {
            openai_config = openai_config.with_api_base(base_url);
        }

        Self {
            client: Client::with_config(openai_config),
        }
    }

    /// Converts our message type to the `OpenAI` SDK type.
    fn convert_message(msg: &ChatMessage) -> ChatCompletionRequestMessage {
        match msg.role {
            Role::System => {
                ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
                    content: async_openai::types::ChatCompletionRequestSystemMessageContent::Text(
                        msg.content.clone(),
                    ),
                    name: None,
                })
            }
            Role::User => ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
                content: async_openai::types::ChatCompletionRequestUserMessageContent::Text(
                    msg.content.clone(),
                ),
                name: None,
            }),
            Role::Assistant => {
                let tool_calls = if msg.tool_calls.is_empty() {
                    None
                } else {
                    Some(
                        msg.tool_calls
                            .iter()
                            .map(|tc| ChatCompletionMessageToolCall {
                                id: tc.id.clone(),
                                r#type: ChatCompletionToolType::Function,
                                function: FunctionCall {
                                    name: tc.name.clone(),
                                    arguments: tc.arguments.clone(),
                                },
                            })
                            .collect(),
                    )
                };

                let content = if msg.content.is_empty() {
                    None
                } else {
                    Some(
                        async_openai::types::ChatCompletionRequestAssistantMessageContent::Text(
                            msg.content.clone(),
                        ),
                    )
                };

                #[allow(deprecated)]
                ChatCompletionRequestMessage::Assistant(ChatCompletionRequestAssistantMessage {
                    content,
                    name: None,
                    tool_calls,
                    refusal: None,
                    audio: None,
                    function_call: None,
                })
            }
            Role::Tool => ChatCompletionRequestMessage::Tool(ChatCompletionRequestToolMessage {
                content: async_openai::types::ChatCompletionRequestToolMessageContent::Text(
                    msg.content.clone(),
                ),
                tool_call_id: msg.tool_call_id.clone().unwrap_or_default(),
            }),
        }
    }

    /// Builds an `OpenAI` chat completion request from our generic request.
    fn build_request(request: &ChatRequest) -> CreateChatCompletionRequest {
        let messages: Vec<_> = request.messages.iter().map(Self::convert_message).collect();

        let response_format = if request.json_mode {
            Some(ResponseFormat::JsonObject)
        } else {
            None
        };

        let tools = if request.tools.is_empty() {
            None
        } else {
            Some(
                request
                    .tools
                    .iter()
                    .map(|td| ChatCompletionTool {
                        r#type: ChatCompletionToolType::Function,
                        function: FunctionObject {
                            name: td.name.clone(),
                            description: Some(td.description.clone()),
                            parameters: Some(td.parameters.clone()),
                            strict: None,
                        },
                    })
                    .collect(),
            )
        };

        CreateChatCompletionRequest {
            model: request.model.clone(),
            messages,
            temperature: request.temperature.filter(|&t| t != 0.0),
            max_completion_tokens: request.max_tokens,
            stream: if request.stream { Some(true) } else { None },
            response_format,
            tools,
            ..Default::default()
        }
    }
}

impl std::fmt::Debug for OpenAiProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenAiProvider")
            .field("client", &"<async-openai::Client>")
            .finish()
    }
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    fn name(&self) -> &'static str {
        "openai"
    }

    async fn chat(&self, request: &ChatRequest) -> Result<ChatResponse, AgentError> {
        let openai_request = Self::build_request(request);

        let response = self
            .client
            .chat()
            .create(openai_request)
            .await
            .map_err(|e| AgentError::ApiRequest {
                message: e.to_string(),
                status: None,
            })?;

        let choice = response.choices.first();

        let content = choice
            .and_then(|c| c.message.content.as_ref())
            .cloned()
            .unwrap_or_default();

        let tool_calls = choice
            .and_then(|c| c.message.tool_calls.as_ref())
            .map(|tcs| {
                tcs.iter()
                    .map(|tc| ToolCall {
                        id: tc.id.clone(),
                        name: tc.function.name.clone(),
                        arguments: tc.function.arguments.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        let finish_reason = choice.and_then(|c| {
            c.finish_reason
                .as_ref()
                .map(|fr| format!("{fr:?}").to_lowercase())
        });

        let usage = response
            .usage
            .map_or_else(TokenUsage::default, |u| TokenUsage {
                prompt_tokens: u.prompt_tokens,
                completion_tokens: u.completion_tokens,
                total_tokens: u.total_tokens,
            });

        Ok(ChatResponse {
            content,
            usage,
            tool_calls,
            finish_reason,
        })
    }

    async fn chat_stream(
        &self,
        request: &ChatRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, AgentError>> + Send>>, AgentError> {
        let mut stream_request = request.clone();
        stream_request.stream = true;
        let openai_request = Self::build_request(&stream_request);

        let stream = self
            .client
            .chat()
            .create_stream(openai_request)
            .await
            .map_err(|e| AgentError::ApiRequest {
                message: e.to_string(),
                status: None,
            })?;

        let mapped = stream.map(
            |result: Result<
                CreateChatCompletionStreamResponse,
                async_openai::error::OpenAIError,
            >| {
                match result {
                    Ok(response) => {
                        let text = response
                            .choices
                            .first()
                            .and_then(|c| c.delta.content.as_ref())
                            .cloned()
                            .unwrap_or_default();
                        Ok(text)
                    }
                    Err(e) => Err(AgentError::Stream {
                        message: e.to_string(),
                    }),
                }
            },
        );

        Ok(Box::pin(mapped))
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;
    use crate::agent::message;
    use crate::agent::tool::ToolDefinition;

    #[test]
    fn test_convert_system_message() {
        let msg = message::system_message("test");
        let converted = OpenAiProvider::convert_message(&msg);
        assert!(matches!(converted, ChatCompletionRequestMessage::System(_)));
    }

    #[test]
    fn test_convert_user_message() {
        let msg = message::user_message("hello");
        let converted = OpenAiProvider::convert_message(&msg);
        assert!(matches!(converted, ChatCompletionRequestMessage::User(_)));
    }

    #[test]
    fn test_convert_tool_message() {
        let msg = message::tool_message("call_123", "result data");
        let converted = OpenAiProvider::convert_message(&msg);
        assert!(matches!(converted, ChatCompletionRequestMessage::Tool(_)));
    }

    #[test]
    fn test_convert_assistant_with_tool_calls() {
        let msg = message::assistant_tool_calls_message(vec![ToolCall {
            id: "call_1".to_string(),
            name: "get_chunks".to_string(),
            arguments: r#"{"chunk_ids":[1]}"#.to_string(),
        }]);
        let converted = OpenAiProvider::convert_message(&msg);
        if let ChatCompletionRequestMessage::Assistant(a) = converted {
            assert!(a.tool_calls.is_some());
            let tcs = a.tool_calls.as_ref().map_or(0, Vec::len);
            assert_eq!(tcs, 1);
        } else {
            panic!("Expected Assistant message");
        }
    }

    #[test]
    fn test_build_request_json_mode() {
        let request = ChatRequest {
            model: "gpt-5.2-2025-12-11".to_string(),
            messages: vec![message::user_message("test")],
            temperature: Some(0.0),
            max_tokens: Some(100),
            json_mode: true,
            stream: false,
            tools: Vec::new(),
        };
        let built = OpenAiProvider::build_request(&request);
        assert!(built.response_format.is_some());
        assert!(built.tools.is_none());
    }

    #[test]
    fn test_build_request_streaming() {
        let request = ChatRequest {
            model: "gpt-5.2-2025-12-11".to_string(),
            messages: vec![message::user_message("test")],
            temperature: None,
            max_tokens: None,
            json_mode: false,
            stream: true,
            tools: Vec::new(),
        };
        let built = OpenAiProvider::build_request(&request);
        assert_eq!(built.stream, Some(true));
    }

    #[test]
    fn test_build_request_with_tools() {
        let request = ChatRequest {
            model: "gpt-5.2-2025-12-11".to_string(),
            messages: vec![message::user_message("test")],
            temperature: Some(0.0),
            max_tokens: Some(100),
            json_mode: false,
            stream: false,
            tools: vec![ToolDefinition {
                name: "get_chunks".to_string(),
                description: "Get chunks by ID".to_string(),
                parameters: serde_json::json!({"type": "object", "properties": {}}),
            }],
        };
        let built = OpenAiProvider::build_request(&request);
        assert!(built.tools.is_some());
        let tools = built.tools.as_ref().map_or(0, Vec::len);
        assert_eq!(tools, 1);
    }
}
