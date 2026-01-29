//! Subcall agent for chunk analysis.
//!
//! Analyzes batches of chunks against a user query, returning
//! structured [`Finding`]s in JSON format.

use async_trait::async_trait;

use super::config::AgentConfig;
use super::finding::Finding;
use super::provider::LlmProvider;
use super::traits::{Agent, AgentResponse};
use crate::error::AgentError;

/// Maximum number of findings a single subcall batch may return.
const MAX_FINDINGS_PER_BATCH: usize = 200;

/// Maximum byte length of any single finding text string.
const MAX_FINDING_TEXT_LEN: usize = 5_000;

/// Maximum number of follow-up suggestions per finding.
const MAX_FOLLOW_UPS: usize = 10;

/// Agent that analyzes document chunks and extracts relevant findings.
///
/// Each subcall agent processes a batch of chunks and returns structured
/// JSON findings. The orchestrator fans out many of these concurrently.
pub struct SubcallAgent {
    model: String,
    max_tokens: u32,
    system_prompt: String,
}

impl SubcallAgent {
    /// Creates a new subcall agent with the given configuration and system prompt.
    #[must_use]
    pub fn new(config: &AgentConfig, system_prompt: String) -> Self {
        Self {
            model: config.subcall_model.clone(),
            max_tokens: config.subcall_max_tokens,
            system_prompt,
        }
    }

    /// Executes the agent and parses findings from the JSON response.
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::ResponseParse`] if the response is not valid JSON
    /// or does not match the expected finding schema. When the response was
    /// truncated (finish\_reason `"length"`), the error message includes a
    /// diagnostic hint suggesting `--subcall-max-tokens` or `--batch-size`
    /// adjustments.
    pub async fn execute_and_parse(
        &self,
        provider: &dyn LlmProvider,
        user_msg: &str,
    ) -> Result<(Vec<Finding>, AgentResponse), AgentError> {
        let response = self.execute(provider, user_msg).await?;
        let truncated = response
            .finish_reason
            .as_deref()
            .is_some_and(|r| r == "length");
        match Self::parse_findings(&response.content) {
            Ok(findings) => Ok((Self::sanitize_findings(findings), response)),
            Err(_) if truncated => Err(AgentError::ResponseParse {
                message: format!(
                    "Response truncated (finish_reason=length, max_tokens={}). \
                     Consider increasing --subcall-max-tokens or reducing --batch-size.",
                    self.max_tokens
                ),
                content: response.content,
            }),
            Err(e) => Err(e),
        }
    }

    /// Caps counts and truncates oversized text fields in parsed findings.
    fn sanitize_findings(mut findings: Vec<Finding>) -> Vec<Finding> {
        findings.truncate(MAX_FINDINGS_PER_BATCH);
        for f in &mut findings {
            for text in &mut f.findings {
                if text.len() > MAX_FINDING_TEXT_LEN {
                    text.truncate(MAX_FINDING_TEXT_LEN);
                }
            }
            if let Some(ref mut s) = f.summary
                && s.len() > MAX_FINDING_TEXT_LEN
            {
                s.truncate(MAX_FINDING_TEXT_LEN);
            }
            f.follow_up.truncate(MAX_FOLLOW_UPS);
            for text in &mut f.follow_up {
                if text.len() > MAX_FINDING_TEXT_LEN {
                    text.truncate(MAX_FINDING_TEXT_LEN);
                }
            }
        }
        findings
    }

    /// Parses the agent's JSON response into findings.
    fn parse_findings(content: &str) -> Result<Vec<Finding>, AgentError> {
        let trimmed = content.trim();

        // Strip delimiters: XML <findings> tags or markdown code blocks
        let json_str = trimmed
            .strip_prefix("<findings>")
            .and_then(|s| s.strip_suffix("</findings>"))
            .map_or_else(
                || {
                    if trimmed.starts_with("```") {
                        trimmed
                            .trim_start_matches("```json")
                            .trim_start_matches("```")
                            .trim_end_matches("```")
                            .trim()
                    } else {
                        trimmed
                    }
                },
                str::trim,
            );

        // Try as array first
        let array_err = match serde_json::from_str::<Vec<Finding>>(json_str) {
            Ok(findings) => return Ok(findings),
            Err(e) => e,
        };

        // Try as wrapper object: {"findings": [...]}
        if let Ok(wrapper) = serde_json::from_str::<serde_json::Value>(json_str) {
            if let Some(arr) = wrapper.get("findings").and_then(|v| v.as_array()) {
                let json_arr = serde_json::Value::Array(arr.clone());
                if let Ok(findings) = serde_json::from_value::<Vec<Finding>>(json_arr) {
                    return Ok(findings);
                }
            }
            // Try as a single finding object
            if let Ok(finding) = serde_json::from_value::<Finding>(wrapper) {
                return Ok(vec![finding]);
            }
        }

        // Build diagnostic message
        let preview_len = json_str.len().min(200);
        let preview = &json_str[..preview_len];
        let message = format!(
            "Failed to parse findings JSON: {array_err}. \
             Response length: {} bytes, preview: {preview:?}",
            json_str.len(),
        );

        Err(AgentError::ResponseParse {
            message,
            content: content.to_string(),
        })
    }
}

#[async_trait]
impl Agent for SubcallAgent {
    fn name(&self) -> &'static str {
        "subcall"
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
    fn test_parse_findings_valid() {
        let json = r#"[
            {"chunk_id": 1, "relevance": "high", "findings": ["found it"], "summary": "yes"},
            {"chunk_id": 2, "relevance": "none"}
        ]"#;
        let findings = SubcallAgent::parse_findings(json);
        assert!(findings.is_ok());
        let findings = findings.unwrap_or_default();
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].chunk_id, 1);
    }

    #[test]
    fn test_parse_findings_code_block() {
        let json = "```json\n[{\"chunk_id\": 1, \"relevance\": \"low\"}]\n```";
        let findings = SubcallAgent::parse_findings(json);
        assert!(findings.is_ok());
    }

    #[test]
    fn test_parse_findings_invalid() {
        let result = SubcallAgent::parse_findings("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_sanitize_findings_limits() {
        let long_text = "x".repeat(MAX_FINDING_TEXT_LEN + 1000);
        let many_follow_ups: Vec<String> =
            (0..MAX_FOLLOW_UPS + 5).map(|i| format!("q{i}")).collect();
        let finding = Finding {
            chunk_id: 1,
            relevance: crate::core::Relevance::High,
            findings: vec![long_text.clone()],
            summary: Some(long_text),
            follow_up: many_follow_ups,
            chunk_index: None,
            chunk_buffer_id: None,
        };
        // More than MAX_FINDINGS_PER_BATCH findings
        let findings: Vec<Finding> =
            std::iter::repeat_n(finding, MAX_FINDINGS_PER_BATCH + 50).collect();
        let sanitized = SubcallAgent::sanitize_findings(findings);
        assert_eq!(sanitized.len(), MAX_FINDINGS_PER_BATCH);
        assert_eq!(sanitized[0].findings[0].len(), MAX_FINDING_TEXT_LEN);
        assert_eq!(
            sanitized[0].summary.as_ref().map(String::len),
            Some(MAX_FINDING_TEXT_LEN)
        );
        assert_eq!(sanitized[0].follow_up.len(), MAX_FOLLOW_UPS);
    }

    #[test]
    fn test_agent_properties() {
        use super::super::prompt::SUBCALL_SYSTEM_PROMPT;
        let config = AgentConfig::builder()
            .api_key("test")
            .subcall_model("gpt-5-mini-2025-08-07")
            .subcall_max_tokens(1024)
            .build()
            .unwrap_or_else(|_| unreachable!());
        let agent = SubcallAgent::new(&config, SUBCALL_SYSTEM_PROMPT.to_string());
        assert_eq!(agent.name(), "subcall");
        assert_eq!(agent.model(), "gpt-5-mini-2025-08-07");
        assert!(agent.json_mode());
        assert!((agent.temperature() - 0.0).abs() < f32::EPSILON);
        assert_eq!(agent.max_tokens(), 1024);
    }
}
