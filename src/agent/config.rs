//! Agent configuration with builder pattern and environment variable support.
//!
//! Configuration is resolved in order: explicit values → environment variables → defaults.

use std::path::PathBuf;
use std::time::Duration;

use crate::error::AgentError;

/// Default maximum concurrent API calls.
const DEFAULT_MAX_CONCURRENCY: usize = 50;
/// Default chunks per batch.
const DEFAULT_BATCH_SIZE: usize = 10;
/// Default subcall max tokens. Set high to avoid truncating exhaustive
/// extraction output from dense content (financial data, logs, regulatory text).
const DEFAULT_SUBCALL_MAX_TOKENS: u32 = 16384;
/// Default synthesizer max tokens.
const DEFAULT_SYNTHESIZER_MAX_TOKENS: u32 = 4096;
/// Default primary agent max tokens.
const DEFAULT_PRIMARY_MAX_TOKENS: u32 = 1024;
/// Default request timeout in seconds.
const DEFAULT_TIMEOUT_SECS: u64 = 120;
/// Default max retries.
const DEFAULT_MAX_RETRIES: u32 = 3;
/// Default maximum tool-calling loop iterations.
const DEFAULT_MAX_TOOL_ITERATIONS: usize = 10;
/// Default search top-k results to retrieve.
const DEFAULT_SEARCH_TOP_K: usize = 200;

/// Configuration for the agent system.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// LLM provider name (e.g., "openai").
    pub provider: String,
    /// API key for the provider.
    pub api_key: String,
    /// Optional base URL override (for proxies or compatible APIs).
    pub base_url: Option<String>,
    /// Model for subcall (chunk analysis) agents.
    pub subcall_model: String,
    /// Model for the synthesizer agent.
    pub synthesizer_model: String,
    /// Model for the primary (planning) agent.
    pub primary_model: String,
    /// Maximum concurrent API requests.
    pub max_concurrency: usize,
    /// Number of chunks per batch.
    pub batch_size: usize,
    /// Maximum tokens for subcall responses.
    pub subcall_max_tokens: u32,
    /// Maximum tokens for synthesizer responses.
    pub synthesizer_max_tokens: u32,
    /// Maximum tokens for primary agent responses.
    pub primary_max_tokens: u32,
    /// Request timeout.
    pub timeout: Duration,
    /// Maximum retry attempts per request.
    pub max_retries: u32,
    /// Maximum tool-calling loop iterations before aborting.
    pub max_tool_iterations: usize,
    /// Maximum search results to retrieve before chunking and fan-out.
    ///
    /// Controls the `top_k` parameter passed to the search layer. Higher
    /// values surface more chunks for analysis at the cost of including
    /// lower-relevance results.
    pub search_top_k: usize,
    /// Directory containing prompt template files.
    ///
    /// When set, the agent system loads system prompts from markdown files
    /// in this directory, falling back to compiled-in defaults for any
    /// missing files.
    pub prompt_dir: Option<PathBuf>,
    /// Minimum delay between API requests per task.
    ///
    /// Applied after acquiring the concurrency semaphore permit.
    /// Set to `Duration::ZERO` (default) to disable rate limiting
    /// beyond what the concurrency semaphore provides.
    pub request_delay: Duration,
}

impl AgentConfig {
    /// Creates a new builder for `AgentConfig`.
    #[must_use]
    pub fn builder() -> AgentConfigBuilder {
        AgentConfigBuilder::default()
    }

    /// Creates configuration from environment variables with defaults.
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::ApiKeyMissing`] if no API key is found.
    pub fn from_env() -> Result<Self, AgentError> {
        Self::builder().from_env().build()
    }
}

/// Builder for [`AgentConfig`].
#[derive(Debug, Clone, Default)]
pub struct AgentConfigBuilder {
    provider: Option<String>,
    api_key: Option<String>,
    base_url: Option<String>,
    subcall_model: Option<String>,
    synthesizer_model: Option<String>,
    primary_model: Option<String>,
    max_concurrency: Option<usize>,
    batch_size: Option<usize>,
    subcall_max_tokens: Option<u32>,
    synthesizer_max_tokens: Option<u32>,
    primary_max_tokens: Option<u32>,
    timeout: Option<Duration>,
    max_retries: Option<u32>,
    max_tool_iterations: Option<usize>,
    search_top_k: Option<usize>,
    prompt_dir: Option<PathBuf>,
    request_delay: Option<Duration>,
}

impl AgentConfigBuilder {
    /// Populates unset fields from environment variables.
    #[must_use]
    pub fn from_env(mut self) -> Self {
        if self.provider.is_none() {
            self.provider = std::env::var("RLM_PROVIDER").ok();
        }
        if self.api_key.is_none() {
            self.api_key = std::env::var("OPENAI_API_KEY")
                .or_else(|_| std::env::var("RLM_API_KEY"))
                .ok();
        }
        if self.base_url.is_none() {
            self.base_url = std::env::var("OPENAI_BASE_URL")
                .or_else(|_| std::env::var("RLM_BASE_URL"))
                .ok();
        }
        if self.subcall_model.is_none() {
            self.subcall_model = std::env::var("RLM_SUBCALL_MODEL").ok();
        }
        if self.synthesizer_model.is_none() {
            self.synthesizer_model = std::env::var("RLM_SYNTHESIZER_MODEL").ok();
        }
        if self.primary_model.is_none() {
            self.primary_model = std::env::var("RLM_PRIMARY_MODEL").ok();
        }
        if self.max_concurrency.is_none() {
            self.max_concurrency = std::env::var("RLM_MAX_CONCURRENCY")
                .ok()
                .and_then(|v| v.parse().ok());
        }
        if self.batch_size.is_none() {
            self.batch_size = std::env::var("RLM_BATCH_SIZE")
                .ok()
                .and_then(|v| v.parse().ok());
        }
        if self.search_top_k.is_none() {
            self.search_top_k = std::env::var("RLM_SEARCH_TOP_K")
                .ok()
                .and_then(|v| v.parse().ok());
        }
        if self.prompt_dir.is_none() {
            self.prompt_dir = std::env::var("RLM_PROMPT_DIR").ok().map(PathBuf::from);
        }
        self
    }

    /// Sets the LLM provider name.
    #[must_use]
    pub fn provider(mut self, provider: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self
    }

    /// Sets the API key.
    #[must_use]
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Sets the base URL override.
    #[must_use]
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Sets the subcall model.
    #[must_use]
    pub fn subcall_model(mut self, model: impl Into<String>) -> Self {
        self.subcall_model = Some(model.into());
        self
    }

    /// Sets the synthesizer model.
    #[must_use]
    pub fn synthesizer_model(mut self, model: impl Into<String>) -> Self {
        self.synthesizer_model = Some(model.into());
        self
    }

    /// Sets the primary agent model.
    #[must_use]
    pub fn primary_model(mut self, model: impl Into<String>) -> Self {
        self.primary_model = Some(model.into());
        self
    }

    /// Sets the maximum concurrency.
    #[must_use]
    pub const fn max_concurrency(mut self, n: usize) -> Self {
        self.max_concurrency = Some(n);
        self
    }

    /// Sets the batch size.
    #[must_use]
    pub const fn batch_size(mut self, n: usize) -> Self {
        self.batch_size = Some(n);
        self
    }

    /// Sets the subcall max tokens.
    #[must_use]
    pub const fn subcall_max_tokens(mut self, n: u32) -> Self {
        self.subcall_max_tokens = Some(n);
        self
    }

    /// Sets the synthesizer max tokens.
    #[must_use]
    pub const fn synthesizer_max_tokens(mut self, n: u32) -> Self {
        self.synthesizer_max_tokens = Some(n);
        self
    }

    /// Sets the request timeout.
    #[must_use]
    pub const fn timeout(mut self, duration: Duration) -> Self {
        self.timeout = Some(duration);
        self
    }

    /// Sets the max retries.
    #[must_use]
    pub const fn max_retries(mut self, n: u32) -> Self {
        self.max_retries = Some(n);
        self
    }

    /// Sets the maximum tool-calling loop iterations.
    #[must_use]
    pub const fn max_tool_iterations(mut self, n: usize) -> Self {
        self.max_tool_iterations = Some(n);
        self
    }

    /// Sets the search top-k (maximum search results to retrieve).
    #[must_use]
    pub const fn search_top_k(mut self, n: usize) -> Self {
        self.search_top_k = Some(n);
        self
    }

    /// Sets the prompt template directory.
    #[must_use]
    pub fn prompt_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.prompt_dir = Some(dir.into());
        self
    }

    /// Sets the minimum delay between API requests per task.
    #[must_use]
    pub const fn request_delay(mut self, delay: Duration) -> Self {
        self.request_delay = Some(delay);
        self
    }

    /// Builds the [`AgentConfig`].
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::ApiKeyMissing`] if no API key was set.
    pub fn build(self) -> Result<AgentConfig, AgentError> {
        let api_key = self.api_key.ok_or(AgentError::ApiKeyMissing)?;

        Ok(AgentConfig {
            provider: self.provider.unwrap_or_else(|| "openai".to_string()),
            api_key,
            base_url: self.base_url,
            subcall_model: self
                .subcall_model
                .unwrap_or_else(|| "gpt-5-mini-2025-08-07".to_string()),
            synthesizer_model: self
                .synthesizer_model
                .unwrap_or_else(|| "gpt-5.2-2025-12-11".to_string()),
            primary_model: self
                .primary_model
                .unwrap_or_else(|| "gpt-5.2-2025-12-11".to_string()),
            max_concurrency: self.max_concurrency.unwrap_or(DEFAULT_MAX_CONCURRENCY),
            batch_size: self.batch_size.unwrap_or(DEFAULT_BATCH_SIZE),
            subcall_max_tokens: self
                .subcall_max_tokens
                .unwrap_or(DEFAULT_SUBCALL_MAX_TOKENS),
            synthesizer_max_tokens: self
                .synthesizer_max_tokens
                .unwrap_or(DEFAULT_SYNTHESIZER_MAX_TOKENS),
            primary_max_tokens: self
                .primary_max_tokens
                .unwrap_or(DEFAULT_PRIMARY_MAX_TOKENS),
            timeout: self
                .timeout
                .unwrap_or(Duration::from_secs(DEFAULT_TIMEOUT_SECS)),
            max_retries: self.max_retries.unwrap_or(DEFAULT_MAX_RETRIES),
            max_tool_iterations: self
                .max_tool_iterations
                .unwrap_or(DEFAULT_MAX_TOOL_ITERATIONS),
            search_top_k: self.search_top_k.unwrap_or(DEFAULT_SEARCH_TOP_K),
            prompt_dir: self.prompt_dir,
            request_delay: self.request_delay.unwrap_or(Duration::ZERO),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_defaults() {
        let config = AgentConfig::builder()
            .api_key("test-key")
            .build()
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(config.provider, "openai");
        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.max_concurrency, DEFAULT_MAX_CONCURRENCY);
        assert_eq!(config.batch_size, DEFAULT_BATCH_SIZE);
        assert_eq!(config.subcall_model, "gpt-5-mini-2025-08-07");
    }

    #[test]
    fn test_builder_missing_api_key() {
        let result = AgentConfig::builder().build();
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_custom_values() {
        let config = AgentConfig::builder()
            .api_key("key")
            .provider("custom")
            .subcall_model("gpt-3.5-turbo")
            .max_concurrency(10)
            .batch_size(5)
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(config.provider, "custom");
        assert_eq!(config.subcall_model, "gpt-3.5-turbo");
        assert_eq!(config.max_concurrency, 10);
        assert_eq!(config.batch_size, 5);
        assert_eq!(config.timeout, Duration::from_secs(30));
    }
}
