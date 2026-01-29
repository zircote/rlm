//! Primary (planning) agent.
//!
//! Analyzes the user query and buffer metadata to produce an
//! [`AnalysisPlan`] that guides the orchestrator's dispatch strategy.

use async_trait::async_trait;

use super::config::AgentConfig;
use super::finding::AnalysisPlan;
use super::provider::LlmProvider;
use super::traits::{Agent, AgentResponse};
use crate::error::AgentError;

/// Agent that plans the analysis strategy for a query.
///
/// Given a query and buffer metadata, determines the optimal search mode,
/// batch sizing, relevance threshold, and focus areas.
pub struct PrimaryAgent {
    model: String,
    max_tokens: u32,
    system_prompt: String,
}

impl PrimaryAgent {
    /// Creates a new primary agent with the given configuration and system prompt.
    #[must_use]
    pub fn new(config: &AgentConfig, system_prompt: String) -> Self {
        Self {
            model: config.primary_model.clone(),
            max_tokens: config.primary_max_tokens,
            system_prompt,
        }
    }

    /// Executes the agent and parses the analysis plan.
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::ResponseParse`] if the response is not valid JSON.
    /// Falls back to [`AnalysisPlan::default`] on parse failure if `lenient` is true.
    pub async fn plan(
        &self,
        provider: &dyn LlmProvider,
        user_msg: &str,
        lenient: bool,
    ) -> Result<(AnalysisPlan, AgentResponse), AgentError> {
        let response = self.execute(provider, user_msg).await?;
        let plan = Self::parse_plan(&response.content, lenient)?;
        Ok((plan, response))
    }

    /// Parses the agent's JSON response into an analysis plan.
    fn parse_plan(content: &str, lenient: bool) -> Result<AnalysisPlan, AgentError> {
        let trimmed = content.trim();

        // Handle markdown code blocks
        let json_str = if trimmed.starts_with("```") {
            trimmed
                .trim_start_matches("```json")
                .trim_start_matches("```")
                .trim_end_matches("```")
                .trim()
        } else {
            trimmed
        };

        match serde_json::from_str::<AnalysisPlan>(json_str) {
            Ok(plan) => Ok(plan),
            Err(e) => {
                if lenient {
                    Ok(AnalysisPlan::default())
                } else {
                    Err(AgentError::ResponseParse {
                        message: format!("Failed to parse analysis plan: {e}"),
                        content: content.to_string(),
                    })
                }
            }
        }
    }
}

#[async_trait]
impl Agent for PrimaryAgent {
    fn name(&self) -> &'static str {
        "primary"
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn system_prompt(&self) -> &str {
        &self.system_prompt
    }

    fn json_mode(&self) -> bool {
        true
    }

    fn temperature(&self) -> f32 {
        0.0
    }

    fn max_tokens(&self) -> u32 {
        self.max_tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_plan_valid() {
        let json = r#"{"search_mode": "semantic", "batch_size": 5, "threshold": 0.4, "focus_areas": ["errors"], "max_chunks": 100}"#;
        let plan = PrimaryAgent::parse_plan(json, false);
        assert!(plan.is_ok());
        let plan = plan.unwrap_or_default();
        assert_eq!(plan.search_mode, "semantic");
        assert_eq!(plan.batch_size, Some(5));
    }

    #[test]
    fn test_parse_plan_lenient_fallback() {
        let plan = PrimaryAgent::parse_plan("invalid json", true);
        assert!(plan.is_ok());
        let plan = plan.unwrap_or_default();
        assert_eq!(plan.search_mode, "hybrid");
    }

    #[test]
    fn test_parse_plan_strict_failure() {
        let result = PrimaryAgent::parse_plan("invalid json", false);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_plan_code_block() {
        let json = "```json\n{\"search_mode\": \"bm25\"}\n```";
        let plan = PrimaryAgent::parse_plan(json, false);
        assert!(plan.is_ok());
        assert_eq!(plan.unwrap_or_default().search_mode, "bm25");
    }

    #[test]
    fn test_agent_properties() {
        use super::super::prompt::PRIMARY_SYSTEM_PROMPT;
        let config = AgentConfig::builder()
            .api_key("test")
            .primary_model("gpt-4o-mini")
            .build()
            .unwrap_or_else(|_| unreachable!());
        let agent = PrimaryAgent::new(&config, PRIMARY_SYSTEM_PROMPT.to_string());
        assert_eq!(agent.name(), "primary");
        assert!(agent.json_mode());
    }
}
