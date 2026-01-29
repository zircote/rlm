//! Synthesizer agent for aggregating findings.
//!
//! Takes the collected findings from all subcall agents and produces
//! a coherent markdown narrative response. Has access to tools for
//! verifying findings against storage.

use async_trait::async_trait;

use super::config::AgentConfig;
use super::tool::{ToolDefinition, ToolSet};
use super::traits::Agent;

/// Agent that synthesizes findings into a final response.
///
/// Receives aggregated findings from all subcall agents and produces
/// a well-structured markdown response addressing the original query.
/// Has tool-calling access to all six internal tools for verification.
pub struct SynthesizerAgent {
    model: String,
    max_tokens: u32,
    max_tool_iterations: usize,
    system_prompt: String,
}

impl SynthesizerAgent {
    /// Creates a new synthesizer agent with the given configuration and system prompt.
    #[must_use]
    pub fn new(config: &AgentConfig, system_prompt: String) -> Self {
        Self {
            model: config.synthesizer_model.clone(),
            max_tokens: config.synthesizer_max_tokens,
            max_tool_iterations: config.max_tool_iterations,
            system_prompt,
        }
    }
}

#[async_trait]
impl Agent for SynthesizerAgent {
    fn name(&self) -> &'static str {
        "synthesizer"
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn system_prompt(&self) -> &str {
        &self.system_prompt
    }

    fn json_mode(&self) -> bool {
        false
    }

    fn temperature(&self) -> f32 {
        0.1
    }

    fn max_tokens(&self) -> u32 {
        self.max_tokens
    }

    fn tools(&self) -> Vec<ToolDefinition> {
        ToolSet::synthesizer_tools().definitions().to_vec()
    }

    fn max_tool_iterations(&self) -> usize {
        self.max_tool_iterations
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_properties() {
        use super::super::prompt::SYNTHESIZER_SYSTEM_PROMPT;
        let config = AgentConfig::builder()
            .api_key("test")
            .synthesizer_model("gpt-4o")
            .synthesizer_max_tokens(8192)
            .build()
            .unwrap_or_else(|_| unreachable!());
        let agent = SynthesizerAgent::new(&config, SYNTHESIZER_SYSTEM_PROMPT.to_string());
        assert_eq!(agent.name(), "synthesizer");
        assert_eq!(agent.model(), "gpt-4o");
        assert!(!agent.json_mode());
        assert!((agent.temperature() - 0.1).abs() < f32::EPSILON);
        assert_eq!(agent.max_tokens(), 8192);
    }

    #[test]
    fn test_synthesizer_has_tools() {
        let config = AgentConfig::builder()
            .api_key("test")
            .build()
            .unwrap_or_else(|_| unreachable!());
        let agent = SynthesizerAgent::new(&config, "test prompt".to_string());
        let tools = agent.tools();
        assert_eq!(tools.len(), 6);
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"get_chunks"));
        assert!(names.contains(&"search"));
        assert!(names.contains(&"grep_chunks"));
        assert!(names.contains(&"get_buffer"));
        assert!(names.contains(&"list_buffers"));
        assert!(names.contains(&"storage_stats"));
    }

    #[test]
    fn test_max_tool_iterations() {
        let config = AgentConfig::builder()
            .api_key("test")
            .max_tool_iterations(5)
            .build()
            .unwrap_or_else(|_| unreachable!());
        let agent = SynthesizerAgent::new(&config, "test".to_string());
        assert_eq!(agent.max_tool_iterations(), 5);
    }
}
