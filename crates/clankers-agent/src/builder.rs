//! Builder for Agent with automatic routing and cost tracking setup

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use clanker_message::ThinkingConfig;
use clankers_config::model_roles::ModelRoles;
use clankers_config::settings::Settings;
use clankers_db::Db;
use clankers_model_selection::cost_tracker::CostTracker;
use clankers_model_selection::cost_tracker::pricing_from_models;
use clankers_model_selection::policy::RoutingPolicy;
use clankers_provider::Provider;

use crate::Agent;
use crate::AgentMemorySettings;
use crate::AgentModelRoles;
use crate::AgentSettings;
use crate::AgentSkillSettings;
use crate::compaction;
use crate::tool::Tool;
use crate::turn;

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

/// Convert desktop Clankers settings into agent-owned runtime settings.
pub fn agent_settings_from_config(settings: &Settings) -> AgentSettings {
    AgentSettings {
        max_tokens: settings.max_tokens,
        max_output_lines: settings.max_output_lines,
        max_output_bytes: settings.max_output_bytes,
        no_cache: settings.no_cache,
        cache_ttl: settings.cache_ttl.clone(),
        memory: AgentMemorySettings {
            global_char_limit: settings.memory.global_char_limit,
            project_char_limit: settings.memory.project_char_limit,
        },
        skills: AgentSkillSettings {
            creation_nudge_interval: settings.skills.creation_nudge_interval,
        },
        compression: auto_compact_settings_from_config(&settings.compression),
        steel_turn_planning: agent_steel_turn_planning_settings_from_config(&settings.steel_turn_planning),
        steel_tool_substrate: agent_tool_steel_substrate_settings_from_config(&settings.steel_tool_substrate),
    }
}

/// Convert desktop model-role settings into an agent-owned resolver.
pub fn agent_model_roles_from_config(roles: &ModelRoles) -> AgentModelRoles {
    let role_models: HashMap<String, Option<String>> =
        roles.all().map(|role| (role.name.clone(), role.model.clone())).collect();
    AgentModelRoles::from_role_models(role_models)
}

fn auto_compact_settings_from_config(
    settings: &clankers_config::settings::CompressionSettings,
) -> compaction::AutoCompactSettings {
    let summary_model = settings.summary_model.trim();
    compaction::AutoCompactSettings {
        tail_budget_fraction: settings.tail_budget_fraction,
        keep_recent: settings.keep_recent,
        summary_model: (!summary_model.is_empty()).then(|| summary_model.to_string()),
    }
}

fn agent_tool_steel_substrate_settings_from_config(
    settings: &clankers_config::settings::SteelToolSubstrateSettings,
) -> turn::AgentToolSteelSubstrateSettings {
    turn::AgentToolSteelSubstrateSettings {
        enabled: settings.enabled,
        rollout_stage: settings.rollout_stage.map(agent_tool_steel_substrate_rollout_stage_from_config),
        fallback_mode: settings.fallback_mode.map(agent_tool_steel_substrate_fallback_mode_from_config),
        session_capabilities: settings.session_capabilities.clone(),
        granted_ucan_abilities: settings.granted_ucan_abilities.clone(),
        disabled_executors: settings.disabled_executors.clone(),
        disabled_actions: settings.disabled_actions.clone(),
        receipt_prefix: settings.receipt_prefix.clone(),
        max_input_bytes: settings.max_input_bytes,
        max_source_bytes: settings.max_source_bytes,
    }
}

fn agent_tool_steel_substrate_rollout_stage_from_config(
    stage: clankers_config::settings::SteelToolSubstrateRolloutStage,
) -> turn::AgentToolSteelSubstrateRolloutStage {
    match stage {
        clankers_config::settings::SteelToolSubstrateRolloutStage::Disabled => {
            turn::AgentToolSteelSubstrateRolloutStage::Disabled
        }
        clankers_config::settings::SteelToolSubstrateRolloutStage::Comparison => {
            turn::AgentToolSteelSubstrateRolloutStage::Comparison
        }
        clankers_config::settings::SteelToolSubstrateRolloutStage::Default => {
            turn::AgentToolSteelSubstrateRolloutStage::Default
        }
        clankers_config::settings::SteelToolSubstrateRolloutStage::Block => {
            turn::AgentToolSteelSubstrateRolloutStage::Block
        }
    }
}

fn agent_tool_steel_substrate_fallback_mode_from_config(
    mode: clankers_config::settings::SteelToolSubstrateFallbackMode,
) -> turn::AgentToolSteelSubstrateFallbackMode {
    match mode {
        clankers_config::settings::SteelToolSubstrateFallbackMode::RustNative => {
            turn::AgentToolSteelSubstrateFallbackMode::RustNative
        }
        clankers_config::settings::SteelToolSubstrateFallbackMode::Block => {
            turn::AgentToolSteelSubstrateFallbackMode::Block
        }
    }
}

fn agent_steel_turn_planning_settings_from_config(
    settings: &clankers_config::settings::SteelTurnPlanningSettings,
) -> turn::AgentSteelTurnPlanningSettings {
    turn::AgentSteelTurnPlanningSettings {
        enabled: settings.enabled,
        profile_path: settings.profile_path.clone(),
        script_path: settings.script_path.clone(),
        script_blake3: settings.script_blake3.clone(),
        profile_blake3: settings.profile_blake3.clone(),
        rollout_stage: settings.rollout_stage.map(agent_steel_turn_planning_rollout_stage_from_config),
        fallback_mode: settings.fallback_mode.map(agent_steel_turn_planning_fallback_mode_from_config),
        planning_seam: settings.planning_seam.clone(),
        session_capabilities: settings.session_capabilities.clone(),
        granted_ucan_abilities: settings.granted_ucan_abilities.clone(),
        ucan_authority_grants: settings
            .ucan_authority_grants
            .iter()
            .map(agent_steel_turn_planning_authority_grant_from_config)
            .collect(),
        disabled_actions: settings.disabled_actions.clone(),
        receipt_prefix: settings.receipt_prefix.clone(),
        max_input_bytes: settings.max_input_bytes,
        max_source_bytes: settings.max_source_bytes,
    }
}

fn agent_steel_turn_planning_authority_grant_from_config(
    grant: &clankers_config::settings::SteelTurnPlanningAuthorityGrantSettings,
) -> turn::AgentSteelTurnPlanningAuthorityGrantSettings {
    turn::AgentSteelTurnPlanningAuthorityGrantSettings {
        resource: grant.resource.clone(),
        ability: grant.ability.clone(),
        audience: grant.audience.clone(),
        proof_reference: grant.proof_reference.clone(),
        expires_at: grant.expires_at,
        revoked: grant.revoked,
        caveats: grant.caveats.clone(),
    }
}

fn agent_steel_turn_planning_rollout_stage_from_config(
    stage: clankers_config::settings::SteelTurnPlanningRolloutStage,
) -> turn::AgentSteelTurnPlanningRolloutStage {
    match stage {
        clankers_config::settings::SteelTurnPlanningRolloutStage::Disabled => {
            turn::AgentSteelTurnPlanningRolloutStage::Disabled
        }
        clankers_config::settings::SteelTurnPlanningRolloutStage::Comparison => {
            turn::AgentSteelTurnPlanningRolloutStage::Comparison
        }
        clankers_config::settings::SteelTurnPlanningRolloutStage::Default => {
            turn::AgentSteelTurnPlanningRolloutStage::Default
        }
    }
}

fn agent_steel_turn_planning_fallback_mode_from_config(
    mode: clankers_config::settings::SteelTurnPlanningFallbackMode,
) -> turn::AgentSteelTurnPlanningFallbackMode {
    match mode {
        clankers_config::settings::SteelTurnPlanningFallbackMode::RustNative => {
            turn::AgentSteelTurnPlanningFallbackMode::RustNative
        }
        clankers_config::settings::SteelTurnPlanningFallbackMode::Block => {
            turn::AgentSteelTurnPlanningFallbackMode::Block
        }
    }
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

        let agent_settings = agent_settings_from_config(&self.settings);
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
            agent = agent
                .with_routing_policy(policy)
                .with_agent_model_roles(agent_model_roles_from_config(&self.settings.model_roles));
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
    use super::agent_model_roles_from_config;
    use super::agent_settings_from_config;
    use super::agent_tool_steel_substrate_settings_from_config;

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
    fn steel_tool_substrate_settings_adapter_preserves_config_policy_at_agent_edge() {
        let settings = clankers_config::settings::SteelToolSubstrateSettings {
            rollout_stage: Some(clankers_config::settings::SteelToolSubstrateRolloutStage::Comparison),
            fallback_mode: Some(clankers_config::settings::SteelToolSubstrateFallbackMode::Block),
            session_capabilities: vec!["steel-tool-substrate".to_string(), "tool-dispatch".to_string()],
            granted_ucan_abilities: vec!["clankers/steel/tool.call".to_string()],
            disabled_executors: vec!["subagent".to_string()],
            disabled_actions: vec!["steel.host.tool.call".to_string()],
            receipt_prefix: Some("target/steel-tool-substrate".to_string()),
            max_input_bytes: Some(42),
            max_source_bytes: 256,
            ..Settings::default().steel_tool_substrate
        };

        let agent_settings = agent_tool_steel_substrate_settings_from_config(&settings);
        assert_eq!(agent_settings.rollout_stage, Some(crate::turn::AgentToolSteelSubstrateRolloutStage::Comparison));
        assert_eq!(agent_settings.fallback_mode, Some(crate::turn::AgentToolSteelSubstrateFallbackMode::Block));
        assert_eq!(agent_settings.max_input_bytes, Some(42));
        assert_eq!(agent_settings.max_source_bytes, 256);
        assert_eq!(agent_settings.disabled_executors, vec!["subagent"]);

        let config = crate::turn::steel_tool_substrate_config_from_settings(&agent_settings)
            .expect("neutral settings activate")
            .expect("enabled substrate config");
        assert_eq!(config.profile.rollout_stage, clankers_runtime::SteelToolSubstrateRolloutStage::Comparison);
        assert_eq!(config.profile.fallback_mode, clankers_runtime::SteelToolSubstrateFallbackMode::Block);
        assert!(!config.profile.allowed_executor_kinds.contains(&clankers_runtime::SteelToolExecutorKind::Subagent));
    }

    #[test]
    fn agent_settings_adapter_preserves_runtime_config_at_agent_edge() {
        let settings = Settings {
            max_tokens: 12_345,
            max_output_lines: 17,
            max_output_bytes: 4096,
            no_cache: true,
            cache_ttl: Some("1h".to_string()),
            memory: clankers_config::settings::MemoryLimits {
                global_char_limit: 111,
                project_char_limit: 222,
            },
            skills: clankers_config::settings::SkillSettings {
                creation_nudge_interval: 3,
            },
            compression: clankers_config::settings::CompressionSettings {
                keep_recent: 6,
                tail_budget_fraction: 0.25,
                summary_model: "compact-model".to_string(),
                ..Settings::default().compression
            },
            ..Settings::default()
        };

        let agent_settings = agent_settings_from_config(&settings);

        assert_eq!(agent_settings.max_tokens, 12_345);
        assert_eq!(agent_settings.max_output_lines, 17);
        assert_eq!(agent_settings.max_output_bytes, 4096);
        assert!(agent_settings.no_cache);
        assert_eq!(agent_settings.cache_ttl.as_deref(), Some("1h"));
        assert_eq!(agent_settings.memory.global_char_limit, 111);
        assert_eq!(agent_settings.memory.project_char_limit, 222);
        assert_eq!(agent_settings.skills.creation_nudge_interval, 3);
        assert_eq!(agent_settings.compression.keep_recent, 6);
        assert_eq!(agent_settings.compression.tail_budget_fraction, 0.25);
        assert_eq!(agent_settings.compression.summary_model.as_deref(), Some("compact-model"));
    }

    #[test]
    fn agent_model_roles_adapter_preserves_alias_and_default_resolution() {
        let mut roles = clankers_config::model_roles::ModelRoles::default();
        roles.set_model("default", "default-model".to_string());
        roles.set_model("slow", "slow-model".to_string());

        let agent_roles = agent_model_roles_from_config(&roles);

        assert_eq!(agent_roles.resolve("thinking", "fallback-model"), "slow-model");
        assert_eq!(agent_roles.resolve("unknown", "fallback-model"), "default-model");
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
