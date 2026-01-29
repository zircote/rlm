//! Provider registry and factory.
//!
//! Maps provider names to concrete [`LlmProvider`] implementations.

use crate::agent::config::AgentConfig;
use crate::agent::provider::LlmProvider;
use crate::agent::providers::OpenAiProvider;
use crate::error::AgentError;

/// Creates an [`LlmProvider`] based on the configured provider name.
///
/// # Supported Providers
///
/// - `"openai"` (default) — OpenAI-compatible APIs via `async-openai`
///
/// # Errors
///
/// Returns [`AgentError::UnsupportedProvider`] for unknown provider names.
pub fn create_provider(config: &AgentConfig) -> Result<Box<dyn LlmProvider>, AgentError> {
    match config.provider.as_str() {
        "openai" => Ok(Box::new(OpenAiProvider::new(config))),
        other => Err(AgentError::UnsupportedProvider {
            name: other.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_openai_provider() {
        let config = AgentConfig::builder()
            .api_key("test")
            .provider("openai")
            .build()
            .unwrap_or_else(|_| unreachable!());
        let provider = create_provider(&config);
        assert!(provider.is_ok());
        assert_eq!(provider.unwrap_or_else(|_| unreachable!()).name(), "openai");
    }

    #[test]
    fn test_create_unknown_provider() {
        let config = AgentConfig::builder()
            .api_key("test")
            .provider("unknown")
            .build()
            .unwrap_or_else(|_| unreachable!());
        let result = create_provider(&config);
        assert!(result.is_err());
    }
}
