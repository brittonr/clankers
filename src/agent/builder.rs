//! Builder for Agent with automatic routing and cost tracking setup

use std::sync::Arc;

use crate::agent::Agent;
use crate::config::paths::ClankersPaths;
use crate::config::settings::Settings;
use crate::db::Db;
use crate::provider::Provider;
use crate::routing::cost_tracker::{load_pricing, CostTracker};
use crate::routing::policy::RoutingPolicy;
use crate::tools::Tool;

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
}

impl AgentBuilder {
    /// Create a new AgentBuilder with required parameters
    pub fn new(
        provider: Arc<dyn Provider>,
        settings: Settings,
        model: String,
        system_prompt: String,
    ) -> Self {
        Self {
            provider,
            settings,
            model,
            system_prompt,
            tools: Vec::new(),
            db: None,
            paths: None,
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
        let mut agent = Agent::new(
            self.provider,
            self.tools,
            self.settings.clone(),
            self.model,
            self.system_prompt,
        );

        // Attach database if provided
        if let Some(db) = self.db {
            agent = agent.with_db(db);
        }

        // Wire routing policy from settings
        if let Some(routing_config) = self.settings.routing.as_ref() && routing_config.enabled {
            let policy = RoutingPolicy::new(routing_config.clone());
            agent = agent
                .with_routing_policy(policy)
                .with_model_roles(self.settings.model_roles.clone());
        }

        // Wire cost tracking from settings
        if let Some(cost_config) = self.settings.cost_tracking.as_ref() {
            let paths = self.paths.unwrap_or_else(|| ClankersPaths::get().clone());
            let pricing = load_pricing(Some(&paths.global_config_dir));
            let tracker = Arc::new(CostTracker::new(pricing, cost_config.clone()));
            agent = agent.with_cost_tracker(tracker);
        }

        agent
    }
}
