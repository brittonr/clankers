//! Builder for Agent with automatic routing and cost tracking setup

use std::path::PathBuf;
use std::sync::Arc;

use clanker_message::ThinkingConfig;
use clankers_config::settings::Settings;
use clankers_db::Db;
use clankers_model_selection::cost_tracker::CostTracker;
use clankers_model_selection::cost_tracker::pricing_from_models;
use clankers_model_selection::policy::RoutingPolicy;
use clankers_provider::Provider;

use crate::Agent;
use crate::AgentSettings;
use crate::tool::Tool;

/// Builder for constructing an Agent with automatic routing and cost tracking setup.
///
/// This unifies the duplicated bootstrap logic found in daemon.rs and interactive.rs.
/// The builder automatically wires routing policy and cost tracker from settings when
/// `build()` is called.
pub struct AgentBuilder {
    provider: Arc<dyn Provider>,
    settings: Settings,
    model: String,
    system_prompt: String,
    tools: Vec<Arc<dyn Tool>>,
    db: Option<Db>,
    pricing_config_dir: Option<PathBuf>,
    thinking: Option<ThinkingConfig>,
    capability_gate: Option<Arc<dyn crate::tool::CapabilityGate>>,
}

impl AgentBuilder {
    /// Create a new AgentBuilder with required parameters
    pub fn new(provider: Arc<dyn Provider>, settings: Settings, model: String, system_prompt: String) -> Self {
        Self {
            provider,
            settings,
            model,
            system_prompt,
            tools: Vec::new(),
            db: None,
            pricing_config_dir: None,
            thinking: None,
            capability_gate: None,
        }
    }

    /// Set the tools for this agent
    pub fn with_tools(mut self, tools: Vec<Arc<dyn Tool>>) -> Self {
        self.tools = tools;
        self
    }

    /// Attach a database handle to this agent
    pub fn with_db(mut self, db: Db) -> Self {
        self.db = Some(db);
        self
    }

    /// Enable extended thinking with the given config.
    pub fn with_thinking(mut self, config: ThinkingConfig) -> Self {
        self.thinking = Some(config);
        self
    }

    /// Attach a capability gate for tool call authorization.
    pub fn with_capability_gate(mut self, gate: Arc<dyn crate::tool::CapabilityGate>) -> Self {
        self.capability_gate = Some(gate);
        self
    }

    /// Set the optional config directory for resolving pricing data.
    pub fn with_pricing_config_dir(mut self, path: PathBuf) -> Self {
        self.pricing_config_dir = Some(path);
        self
    }

    /// Build the Agent, automatically wiring routing policy and cost tracking from settings
    pub fn build(self) -> Agent {
        // Snapshot model pricing before moving the provider into the agent
        let provider_models: Vec<clankers_provider::Model> = self.provider.models().to_vec();

        let agent_settings = AgentSettings::from_config(&self.settings);
        let mut agent =
            Agent::new_with_agent_settings(self.provider, self.tools, agent_settings, self.model, self.system_prompt);

        // Attach database if provided
        if let Some(db) = self.db {
            agent = agent.with_db(db);
        }

        // Wire routing policy from settings
        if let Some(routing_config) = self.settings.routing.as_ref()
            && routing_config.enabled
        {
            let policy = RoutingPolicy::new(routing_config.clone());
            agent = agent.with_routing_policy(policy).with_model_roles(self.settings.model_roles.clone());
        }

        // Wire cost tracking from settings
        if let Some(cost_config) = self.settings.cost_tracking.as_ref() {
            let pricing = pricing_from_models(&provider_models, self.pricing_config_dir.as_deref());
            let tracker = Arc::new(CostTracker::new(pricing, cost_config.clone()));
            agent = agent.with_cost_tracker(tracker);
        }

        // Enable extended thinking from settings by default, with explicit
        // builder overrides taking precedence.
        let thinking_level = if let Some(ref thinking) = self.thinking {
            if thinking.enabled {
                clanker_message::ThinkingLevel::from_budget(
                    u32::try_from(thinking.budget_tokens.unwrap_or(128_000)).unwrap_or(u32::MAX),
                )
            } else {
                clanker_message::ThinkingLevel::Off
            }
        } else {
            self.settings.parsed_thinking_level()
        };
        agent.set_thinking_level(thinking_level);

        // Attach capability gate if provided
        if let Some(gate) = self.capability_gate {
            agent = agent.with_capability_gate(gate);
        }

        agent
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use clanker_message::streaming::StreamEvent;
    use clankers_config::settings::Settings;
    use clankers_provider::CompletionRequest;
    use clankers_provider::Model;
    use clankers_provider::Provider;
    use clankers_provider::error::Result;
    use tokio::sync::mpsc;

    use super::AgentBuilder;

    struct MockProvider;

    #[async_trait]
    impl Provider for MockProvider {
        async fn complete(&self, _request: CompletionRequest, _tx: mpsc::Sender<StreamEvent>) -> Result<()> {
            Ok(())
        }

        fn models(&self) -> &[Model] {
            &[]
        }

        fn name(&self) -> &str {
            "mock"
        }
    }

    #[test]
    fn builder_enables_max_thinking_from_default_settings() {
        let agent = AgentBuilder::new(
            Arc::new(MockProvider),
            Settings::default(),
            "test-model".to_string(),
            "system".to_string(),
        )
        .build();

        assert_eq!(agent.thinking_level(), clanker_message::ThinkingLevel::Max);
        assert!(agent.is_thinking_enabled());
    }

    #[test]
    fn builder_honors_settings_thinking_off() {
        let settings = Settings {
            thinking_level: "off".to_string(),
            ..Settings::default()
        };
        let agent =
            AgentBuilder::new(Arc::new(MockProvider), settings, "test-model".to_string(), "system".to_string()).build();

        assert_eq!(agent.thinking_level(), clanker_message::ThinkingLevel::Off);
        assert!(!agent.is_thinking_enabled());
    }
}
