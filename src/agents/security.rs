//! Agent scope trust model and security policies

use super::definition::AgentConfig;
use super::definition::AgentSource;

/// Check if a project agent should be trusted.
///
/// # Trust Model
/// - User agents (from ~/.clankers/agent/agents/) are always trusted
/// - Project agents (from .clankers/agents/) are trusted only if confirmation is disabled
///
/// When `confirm_project_agents` is true, project agents require explicit user approval
/// before being used (similar to how project skills work in pi).
pub fn should_trust_project_agent(agent: &AgentConfig, confirm_project_agents: bool) -> bool {
    match agent.source {
        AgentSource::User => true,                       // always trusted
        AgentSource::Project => !confirm_project_agents, // trusted only if confirmation disabled
    }
}

/// Get the trust level description for an agent
pub fn trust_level(agent: &AgentConfig) -> &'static str {
    match agent.source {
        AgentSource::User => "trusted (user-level)",
        AgentSource::Project => "untrusted (project-level)",
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn make_agent(source: AgentSource) -> AgentConfig {
        AgentConfig {
            name: "test".to_string(),
            description: "Test agent".to_string(),
            model: None,
            tools: None,
            system_prompt: "Test".to_string(),
            source,
            file_path: PathBuf::from("/test.md"),
        }
    }

    #[test]
    fn test_user_agents_always_trusted() {
        let agent = make_agent(AgentSource::User);
        assert!(should_trust_project_agent(&agent, true));
        assert!(should_trust_project_agent(&agent, false));
    }

    #[test]
    fn test_project_agents_require_confirmation() {
        let agent = make_agent(AgentSource::Project);

        // With confirmation enabled, project agents are untrusted
        assert!(!should_trust_project_agent(&agent, true));

        // With confirmation disabled, project agents are trusted
        assert!(should_trust_project_agent(&agent, false));
    }

    #[test]
    fn test_trust_level_strings() {
        let user_agent = make_agent(AgentSource::User);
        let project_agent = make_agent(AgentSource::Project);

        assert_eq!(trust_level(&user_agent), "trusted (user-level)");
        assert_eq!(trust_level(&project_agent), "untrusted (project-level)");
    }
}
