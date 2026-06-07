//! Builder for Agent with automatic routing and cost tracking setup

use std::any::Any;
use std::sync::Arc;

use clanker_message::ThinkingConfig;
use clanker_message::ThinkingLevel;
use clankers_tool_host::ToolHookService;
use clankers_tool_host::ToolSearchService;

use crate::Agent;
use crate::AgentCostRecorder;
use crate::AgentMemoryContextProvider;
use crate::AgentModelRoles;
use crate::AgentModelService;
use crate::AgentRoutingPolicy;
use crate::AgentSettings;
use crate::tool::Tool;

/// Builder for constructing an Agent with automatic routing and cost tracking setup.
///
/// This unifies the duplicated bootstrap logic found in daemon.rs and interactive.rs.
/// The builder automatically wires routing policy and cost tracker from settings when
/// `build()` is called.
pub struct AgentBuilder {
    provider: Arc<dyn AgentModelService>,
    config: AgentBuilderConfig,
    model: String,
    system_prompt: String,
    tools: Vec<Arc<dyn Tool>>,
    memory_context_provider: Option<Arc<dyn AgentMemoryContextProvider>>,
    tool_context_services: Vec<Arc<dyn Any + Send + Sync>>,
    tool_search_service: Option<Arc<dyn ToolSearchService>>,
    tool_hook_service: Option<Arc<dyn ToolHookService>>,
    thinking: Option<ThinkingConfig>,
    capability_gate: Option<Arc<dyn crate::tool::CapabilityGate>>,
}

/// Agent-owned configuration for the desktop compatibility builder.
#[derive(Clone)]
pub struct AgentBuilderConfig {
    pub agent_settings: AgentSettings,
    pub model_roles: AgentModelRoles,
    pub routing_policy: Option<Arc<dyn AgentRoutingPolicy>>,
    pub cost_recorder: Option<Arc<dyn AgentCostRecorder>>,
    pub cost_provider: Option<Arc<dyn clanker_message::CostProvider>>,
    pub thinking_level: ThinkingLevel,
}

impl Default for AgentBuilderConfig {
    fn default() -> Self {
        Self {
            agent_settings: AgentSettings::default(),
            model_roles: AgentModelRoles::default(),
            routing_policy: None,
            cost_recorder: None,
            cost_provider: None,
            thinking_level: ThinkingLevel::Max,
        }
    }
}

impl AgentBuilder {
    /// Create a new AgentBuilder with required parameters.
    pub fn new(
        provider: Arc<dyn AgentModelService>,
        config: AgentBuilderConfig,
        model: String,
        system_prompt: String,
    ) -> Self {
        Self {
            provider,
            config,
            model,
            system_prompt,
            tools: Vec::new(),
            memory_context_provider: None,
            tool_context_services: Vec::new(),
            tool_search_service: None,
            tool_hook_service: None,
            thinking: None,
            capability_gate: None,
        }
    }

    /// Set the tools for this agent
    pub fn with_tools(mut self, tools: Vec<Arc<dyn Tool>>) -> Self {
        self.tools = tools;
        self
    }

    /// Attach a memory context provider to this agent.
    pub fn with_memory_context_provider(mut self, provider: Arc<dyn AgentMemoryContextProvider>) -> Self {
        self.memory_context_provider = Some(provider);
        self
    }

    /// Attach a concrete legacy tool-context service to this agent.
    pub fn with_tool_context_service<T>(mut self, service: Arc<T>) -> Self
    where
        T: Any + Send + Sync + 'static,
    {
        self.tool_context_services.push(service);
        self
    }

    /// Attach a neutral search service for migrated tools.
    pub fn with_tool_search_service(mut self, service: Arc<dyn ToolSearchService>) -> Self {
        self.tool_search_service = Some(service);
        self
    }

    /// Attach a neutral hook service for migrated tool hooks.
    pub fn with_tool_hook_service(mut self, service: Arc<dyn ToolHookService>) -> Self {
        self.tool_hook_service = Some(service);
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

    /// Build the Agent, automatically wiring routing policy and cost tracking from the builder
    /// config.
    pub fn build(self) -> Agent {
        let mut agent = Agent::new_with_agent_settings(
            self.provider,
            self.tools,
            self.config.agent_settings,
            self.model,
            self.system_prompt,
        );

        // Attach optional memory/tool services supplied by the application edge.
        if let Some(provider) = self.memory_context_provider {
            agent = agent.with_memory_context_provider(provider);
        }
        agent = agent.with_tool_context_services(self.tool_context_services);
        if let Some(service) = self.tool_search_service {
            agent = agent.with_tool_search_service(service);
        }
        if let Some(service) = self.tool_hook_service {
            agent = agent.with_tool_hook_service(service);
        }

        // Wire routing policy from the app-edge adapter.
        if let Some(policy) = self.config.routing_policy {
            agent = agent.with_routing_policy(policy).with_agent_model_roles(self.config.model_roles.clone());
        }

        // Wire cost tracking from the app-edge adapter.
        if let Some(recorder) = self.config.cost_recorder {
            agent = agent.with_cost_recorder(recorder);
        }
        if let Some(provider) = self.config.cost_provider {
            agent = agent.with_cost_provider(provider);
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
            self.config.thinking_level
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
    use clankers_provider::CompletionRequest;
    use clankers_provider::Model;
    use clankers_provider::Provider;
    use clankers_provider::error::Result;
    use tokio::sync::mpsc;

    use super::AgentBuilder;
    use super::AgentBuilderConfig;

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
    fn builder_enables_max_thinking_from_default_config() {
        let agent = AgentBuilder::new(
            Arc::new(MockProvider),
            AgentBuilderConfig::default(),
            "test-model".to_string(),
            "system".to_string(),
        )
        .build();

        assert_eq!(agent.thinking_level(), clanker_message::ThinkingLevel::Max);
        assert!(agent.is_thinking_enabled());
    }

    #[test]
    fn builder_honors_config_thinking_off() {
        let config = AgentBuilderConfig {
            thinking_level: clanker_message::ThinkingLevel::Off,
            ..AgentBuilderConfig::default()
        };
        let agent =
            AgentBuilder::new(Arc::new(MockProvider), config, "test-model".to_string(), "system".to_string()).build();

        assert_eq!(agent.thinking_level(), clanker_message::ThinkingLevel::Off);
        assert!(!agent.is_thinking_enabled());
    }
}
