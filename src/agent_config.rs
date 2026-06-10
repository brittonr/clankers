//! Root-shell adapters from desktop settings to agent-owned DTOs.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use clanker_message::BudgetEvent;
use clanker_message::CostProvider;
use clanker_message::cost::CostMicros;
use clankers_agent::AgentCostRecorder;
use clankers_agent::AgentMemorySettings;
use clankers_agent::AgentModelRoles;
use clankers_agent::AgentModelSelection;
use clankers_agent::AgentOrchestrationPhase;
use clankers_agent::AgentOrchestrationPlan;
use clankers_agent::AgentRoutingPolicy;
use clankers_agent::AgentRoutingSignals;
use clankers_agent::AgentSettings;
use clankers_agent::AgentSkillSettings;
use clankers_agent::builder::AgentBuilderConfig;
use clankers_agent::compaction;
use clankers_agent::turn;
use clankers_config::model_roles::ModelRoles;
use clankers_config::settings::Settings;
use clankers_model_selection::cost_tracker::CostTracker;
use clankers_model_selection::cost_tracker::pricing_from_models;
use clankers_model_selection::policy::RoutingPolicy;
use clankers_model_selection::signals::ComplexitySignals;
use clankers_model_selection::signals::ToolCallSummary;
use clankers_provider::Model;

/// Convert desktop Clankers settings into agent-owned builder configuration.
#[must_use]
pub fn agent_builder_config_from_settings(
    settings: &Settings,
    provider_models: &[Model],
    pricing_config_dir: Option<&Path>,
) -> AgentBuilderConfig {
    let cost_tracker = settings.cost_tracking.as_ref().map(|cost_config| {
        let pricing = pricing_from_models(provider_models, pricing_config_dir);
        Arc::new(CostTracker::new(pricing, cost_config.clone()))
    });

    AgentBuilderConfig {
        agent_settings: agent_settings_from_config(settings),
        model_roles: agent_model_roles_from_config(&settings.model_roles),
        routing_policy: settings
            .routing
            .as_ref()
            .filter(|routing| routing.enabled)
            .cloned()
            .map(RoutingPolicy::new)
            .map(|policy| Arc::new(ModelSelectionRoutingPolicyAdapter { policy }) as Arc<dyn AgentRoutingPolicy>),
        cost_recorder: cost_tracker
            .as_ref()
            .map(|tracker| Arc::new(ModelSelectionCostRecorder::new(Arc::clone(tracker))) as Arc<dyn AgentCostRecorder>),
        cost_provider: cost_tracker.map(|tracker| tracker as Arc<dyn clanker_message::CostProvider>),
        thinking_level: settings.parsed_thinking_level(),
    }
}

struct ModelSelectionRoutingPolicyAdapter {
    policy: RoutingPolicy,
}

impl AgentRoutingPolicy for ModelSelectionRoutingPolicyAdapter {
    fn select_model(&self, signals: &AgentRoutingSignals) -> AgentModelSelection {
        let prompt_text = signals.prompt_text.clone().unwrap_or_default();
        let selection = self.policy.select_model(&ComplexitySignals {
            token_count: signals.token_count,
            recent_tools: signals
                .recent_tools
                .iter()
                .map(|tool| ToolCallSummary {
                    tool_name: tool.tool_name.clone(),
                    complexity: self.policy.classify_tool(&tool.tool_name),
                })
                .collect(),
            keywords: self.policy.extract_keywords(&prompt_text),
            user_hint: self.policy.parse_user_hint(&prompt_text),
            current_cost: signals.current_cost,
            prompt_text: Some(prompt_text),
        });

        AgentModelSelection {
            role: selection.role,
            reason: selection.reason.to_string(),
            orchestration: selection.orchestration.map(agent_orchestration_plan_from_model_selection),
        }
    }
}

fn agent_orchestration_plan_from_model_selection(
    plan: clankers_model_selection::orchestration::OrchestrationPlan,
) -> AgentOrchestrationPlan {
    AgentOrchestrationPlan {
        pattern: plan.pattern.to_string(),
        phases: plan
            .phases
            .into_iter()
            .map(|phase| AgentOrchestrationPhase {
                role: phase.role,
                label: phase.label,
                system_suffix: phase.system_suffix,
            })
            .collect(),
    }
}

struct ModelSelectionCostRecorder {
    tracker: Arc<CostTracker>,
}

impl ModelSelectionCostRecorder {
    fn new(tracker: Arc<CostTracker>) -> Self {
        Self { tracker }
    }
}

impl AgentCostRecorder for ModelSelectionCostRecorder {
    fn record_usage(
        &self,
        model_id: &str,
        input_tokens: u64,
        output_tokens: u64,
    ) -> (CostMicros, Vec<BudgetEvent>) {
        let (_total_cost, budget_events) = self.tracker.record_usage(model_id, input_tokens, output_tokens);
        (self.total_cost(), budget_events)
    }

    fn total_cost(&self) -> CostMicros {
        CostProvider::total_cost(self.tracker.as_ref())
    }
}

/// Convert desktop Clankers settings into agent-owned runtime settings.
#[must_use]
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
#[must_use]
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
        tail_context_fraction: settings.tail_context_fraction,
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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(agent_settings.rollout_stage, Some(turn::AgentToolSteelSubstrateRolloutStage::Comparison));
        assert_eq!(agent_settings.fallback_mode, Some(turn::AgentToolSteelSubstrateFallbackMode::Block));
        assert_eq!(agent_settings.max_input_bytes, Some(42));
        assert_eq!(agent_settings.max_source_bytes, 256);
        assert_eq!(agent_settings.disabled_executors, vec!["subagent"]);

        let config = turn::steel_tool_substrate_config_from_settings(&agent_settings)
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
                tail_context_fraction: 0.25,
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
        assert!((agent_settings.compression.tail_context_fraction - 0.25).abs() < f64::EPSILON);
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
    fn agent_builder_config_constructs_routing_and_cost_at_app_edge() {
        let settings = Settings {
            routing: Some(clankers_model_selection::config::RoutingPolicyConfig::default()),
            cost_tracking: Some(clankers_model_selection::cost_tracker::CostTrackerConfig::default()),
            ..Settings::default()
        };

        let config = agent_builder_config_from_settings(&settings, &[], None);

        assert!(config.routing_policy.is_some());
        assert!(config.cost_recorder.is_some());
        assert!(config.cost_provider.is_some());
    }
}
