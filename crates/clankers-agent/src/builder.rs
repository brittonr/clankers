//! Builder for Agent with automatic routing and cost tracking setup

use std::sync::Arc;

use clankers_config::paths::ClankersPaths;
use clankers_config::settings::Settings;
use clankers_db::Db;
use clankers_model_selection::cost_tracker::CostTracker;
use clankers_model_selection::cost_tracker::pricing_from_models;
use clankers_model_selection::policy::RoutingPolicy;
use clankers_provider::Provider;

use crate::Agent;
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
    paths: Option<ClankersPaths>,
    thinking: Option<clankers_provider::ThinkingConfig>,
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
            paths: None,
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
    pub fn with_thinking(mut self, config: clankers_provider::ThinkingConfig) -> Self {
        self.thinking = Some(config);
        self
    }

    /// Attach a capability gate for tool call authorization.
    pub fn with_capability_gate(mut self, gate: Arc<dyn crate::tool::CapabilityGate>) -> Self {
        self.capability_gate = Some(gate);
        self
    }

    /// Set the ClankersPaths for resolving pricing data
    ///
    /// If not provided, `build()` will call `ClankersPaths::resolve()` automatically
    /// when cost tracking is enabled.
    pub fn with_paths(mut self, paths: ClankersPaths) -> Self {
        self.paths = Some(paths);
        self
    }

    /// Build the Agent, automatically wiring routing policy and cost tracking from settings
    pub fn build(self) -> Agent {
        // Snapshot model pricing before moving the provider into the agent
        let provider_models: Vec<clanker_router::Model> = self.provider.models().to_vec();

        let mut agent = Agent::new(self.provider, self.tools, self.settings.clone(), self.model, self.system_prompt);

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
            let paths = self.paths.unwrap_or_else(|| ClankersPaths::get().clone());
            let pricing = pricing_from_models(&provider_models, Some(&paths.global_config_dir));
            let tracker = Arc::new(CostTracker::new(pricing, cost_config.clone()));
            agent = agent.with_cost_tracker(tracker);
        }

        // Enable extended thinking if requested
        if let Some(ref thinking) = self.thinking
            && thinking.enabled
        {
            agent.toggle_thinking(thinking.budget_tokens.unwrap_or(10_000));
        }

        // Attach capability gate if provided
        if let Some(gate) = self.capability_gate {
            agent = agent.with_capability_gate(gate);
        }

        agent
    }
}
