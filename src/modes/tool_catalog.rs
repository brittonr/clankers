//! Capability-specific tool catalog owners.
//!
//! `modes::common` keeps the public compatibility functions and shared `ToolSet`
//! types; this module owns concrete tool construction by family so new tools have
//! an explicit catalog owner instead of growing one monolithic constructor.

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;

use clankers_plugin::PluginManager;
use tracing::info;

use super::common::ToolEnv;
use super::common::ToolTier;
use crate::tools::Tool;
use crate::tools::ToolDefinition;
use crate::tools::plugin_tool::PluginTool;
use crate::tools::validator_tool::ValidatorTool;

/// Static owner inventory for architecture rails and documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ToolFamilyOwner {
    pub family: &'static str,
    pub builder: &'static str,
    pub tier: Option<ToolTier>,
}

const TOOL_FAMILY_OWNERS: &[ToolFamilyOwner] = &[
    ToolFamilyOwner {
        family: "core",
        builder: "build_core_tools",
        tier: Some(ToolTier::Core),
    },
    ToolFamilyOwner {
        family: "orchestration",
        builder: "build_orchestration_tools",
        tier: Some(ToolTier::Orchestration),
    },
    ToolFamilyOwner {
        family: "specialty",
        builder: "build_specialty_tools",
        tier: Some(ToolTier::Specialty),
    },
    ToolFamilyOwner {
        family: "daemon-session",
        builder: "build_daemon_session_tools",
        tier: Some(ToolTier::Specialty),
    },
    ToolFamilyOwner {
        family: "matrix",
        builder: "build_matrix_tools",
        tier: Some(ToolTier::Matrix),
    },
    ToolFamilyOwner {
        family: "plugin",
        builder: "build_plugin_tools",
        tier: Some(ToolTier::Specialty),
    },
    ToolFamilyOwner {
        family: "extension-runtime",
        builder: "build_extension_runtime_tools",
        tier: Some(ToolTier::Specialty),
    },
    ToolFamilyOwner {
        family: "mcp",
        builder: "build_mcp_tools",
        tier: Some(ToolTier::Specialty),
    },
];

#[cfg(test)]
pub(crate) fn tool_family_owners() -> &'static [ToolFamilyOwner] {
    TOOL_FAMILY_OWNERS
}

pub(crate) fn build_builtin_tiered_tools(env: &ToolEnv) -> Vec<(ToolTier, Arc<dyn Tool>)> {
    debug_assert!(TOOL_FAMILY_OWNERS.iter().any(|owner| owner.family == "core"));
    let mut tools = Vec::new();
    tools.extend(build_core_tools(env));
    tools.extend(build_orchestration_tools(env));
    tools.extend(build_specialty_tools(env));
    tools.extend(build_daemon_session_tools(env));
    tools.extend(build_matrix_tools());
    tools.extend(build_extension_runtime_tools(env));
    tools
}

fn build_core_tools(env: &ToolEnv) -> Vec<(ToolTier, Arc<dyn Tool>)> {
    let process_monitor = env.process_monitor.clone();
    let mut bash_tool = if let Some(tx) = env.bash_confirm_tx.clone() {
        crate::tools::bash::BashTool::with_confirm(tx)
    } else {
        crate::tools::bash::BashTool::new()
    };
    if let Some(ref pm) = process_monitor {
        bash_tool = bash_tool.with_process_monitor(pm.clone());
    }

    let mut process_tool = crate::tools::process::ProcessTool::new();
    if let Some(ref pm) = process_monitor {
        process_tool = process_tool.with_process_monitor(pm.clone());
    }

    vec![
        (ToolTier::Core, Arc::new(crate::tools::read::ReadTool::new())),
        (ToolTier::Core, Arc::new(crate::tools::write::WriteTool::new())),
        (ToolTier::Core, Arc::new(crate::tools::edit::EditTool::new())),
        (ToolTier::Core, Arc::new(crate::tools::patch::PatchTool::new())),
        (ToolTier::Core, Arc::new(crate::tools::execute_code::ExecuteCodeTool::new())),
        (ToolTier::Core, Arc::new(process_tool)),
        (ToolTier::Core, Arc::new(bash_tool)),
        (ToolTier::Core, Arc::new(crate::tools::grep::GrepTool::new())),
        (ToolTier::Core, Arc::new(crate::tools::find::FindTool::new())),
        (ToolTier::Core, Arc::new(crate::tools::ls::LsTool::new())),
    ]
}

fn build_orchestration_tools(env: &ToolEnv) -> Vec<(ToolTier, Arc<dyn Tool>)> {
    let panel_tx = env.panel_tx.clone();
    let process_monitor = env.process_monitor.clone();

    let mut subagent_tool = crate::tools::subagent::SubagentTool::new();
    if let Some(ref ptx) = panel_tx {
        subagent_tool = subagent_tool.with_panel_tx(ptx.clone());
    }
    if let Some(ref pm) = process_monitor {
        subagent_tool = subagent_tool.with_process_monitor(pm.clone());
    }
    if let Some(ref actx) = env.actor_ctx {
        subagent_tool = subagent_tool.with_actor_ctx(actx.clone());
    }

    let mut delegate_tool = crate::tools::delegate::DelegateTool::new();
    if let Some(ref ptx) = panel_tx {
        delegate_tool = delegate_tool.with_panel_tx(ptx.clone());
    }
    {
        let paths = clankers_config::ClankersPaths::get();
        let registry_path = crate::modes::rpc::peers::registry_path(paths);
        let identity_path = crate::modes::rpc::iroh::identity_path(paths);
        delegate_tool = delegate_tool.with_peer_routing(registry_path, identity_path);
    }
    if let Some(ref pm) = process_monitor {
        delegate_tool = delegate_tool.with_process_monitor(pm.clone());
    }
    if let Some(ref actx) = env.actor_ctx {
        delegate_tool = delegate_tool.with_actor_ctx(actx.clone());
    }

    let mut procmon_tool = crate::tools::procmon::ProcmonTool::new();
    if let Some(ref pm) = process_monitor {
        procmon_tool = procmon_tool.with_monitor(pm.clone());
    }

    vec![
        (ToolTier::Orchestration, Arc::new(subagent_tool)),
        (ToolTier::Orchestration, Arc::new(delegate_tool)),
        (ToolTier::Orchestration, Arc::new(crate::tools::signal_loop::SignalLoopTool::new())),
        (ToolTier::Orchestration, Arc::new(procmon_tool)),
    ]
}

fn build_specialty_tools(env: &ToolEnv) -> Vec<(ToolTier, Arc<dyn Tool>)> {
    let mut todo_tool = crate::tools::todo::TodoTool::new();
    if let Some(tx) = env.todo_tx.clone() {
        todo_tool = todo_tool.with_tx(tx);
    }

    vec![
        (ToolTier::Specialty, Arc::new(todo_tool)),
        (ToolTier::Specialty, Arc::new(crate::tools::nix::NixTool::new())),
        (ToolTier::Specialty, Arc::new(crate::tools::web::WebTool::new())),
        (ToolTier::Specialty, Arc::new(crate::tools::checkpoint::CheckpointTool::new())),
        (ToolTier::Specialty, Arc::new(crate::tools::tool_gateway::ToolGatewayTool::new())),
        (ToolTier::Specialty, Arc::new(crate::tools::voice_mode::VoiceModeTool::new())),
        (ToolTier::Specialty, Arc::new(crate::tools::soul_personality::SoulPersonalityTool::new())),
        (ToolTier::Specialty, Arc::new(crate::tools::commit::CommitTool::new())),
        (ToolTier::Specialty, Arc::new(crate::tools::review::ReviewTool::new())),
        (ToolTier::Specialty, Arc::new(crate::tools::ask::AskTool::new())),
        (ToolTier::Specialty, Arc::new(crate::tools::image_gen::ImageGenTool::new())),
        (ToolTier::Specialty, Arc::new(crate::tools::tts::TtsTool::new())),
        (ToolTier::Specialty, Arc::new(crate::tools::autoresearch::InitExperimentTool::new())),
        (ToolTier::Specialty, Arc::new(crate::tools::autoresearch::RunExperimentTool::new())),
        (ToolTier::Specialty, Arc::new(crate::tools::autoresearch::LogExperimentTool::new())),
        (
            ToolTier::Specialty,
            Arc::new(crate::tools::memory::MemoryTool::new(clankers_config::settings::MemoryLimits::default())),
        ),
        (
            ToolTier::Specialty,
            Arc::new(crate::tools::skill_manage::SkillManageTool::new(
                clankers_config::ClankersPaths::get().global_skills_dir.clone(),
            )),
        ),
        (
            ToolTier::Specialty,
            Arc::new(crate::tools::skill_view::SkillsListTool::new(
                clankers_config::ClankersPaths::get().global_skills_dir.clone(),
                crate::tools::skill_view::project_skills_dir_from_cwd(),
            )),
        ),
        (
            ToolTier::Specialty,
            Arc::new(crate::tools::skill_view::SkillViewTool::new(
                clankers_config::ClankersPaths::get().global_skills_dir.clone(),
                crate::tools::skill_view::project_skills_dir_from_cwd(),
            )),
        ),
        (
            ToolTier::Specialty,
            Arc::new(crate::tools::session_search::SessionSearchTool::new(
                clankers_config::ClankersPaths::get().global_sessions_dir.clone(),
                100,
            )),
        ),
        (
            ToolTier::Specialty,
            Arc::new(crate::tools::compress::CompressTool::new(
                crate::tools::compress::compression_slot(),
                env.settings
                    .as_ref()
                    .map(|settings| settings.compression.keep_recent)
                    .unwrap_or(clankers_config::settings::CompressionSettings::default().keep_recent),
                env.settings
                    .as_ref()
                    .map(|settings| settings.compression.min_messages)
                    .unwrap_or(clankers_config::settings::CompressionSettings::default().min_messages),
            )),
        ),
    ]
}

fn build_daemon_session_tools(env: &ToolEnv) -> Vec<(ToolTier, Arc<dyn Tool>)> {
    let mut tools: Vec<(ToolTier, Arc<dyn Tool>)> = Vec::new();
    if let Some(ref engine) = env.schedule_engine {
        tools.push((ToolTier::Specialty, Arc::new(crate::tools::schedule::ScheduleTool::new(Arc::clone(engine)))));
    }
    tools
}

fn build_matrix_tools() -> Vec<(ToolTier, Arc<dyn Tool>)> {
    #[cfg(feature = "matrix-bridge")]
    {
        return vec![
            (ToolTier::Matrix, Arc::new(crate::tools::matrix::MatrixSendTool::new())),
            (ToolTier::Matrix, Arc::new(crate::tools::matrix::MatrixReadTool::new())),
            (ToolTier::Matrix, Arc::new(crate::tools::matrix::MatrixRoomsTool::new())),
            (ToolTier::Matrix, Arc::new(crate::tools::matrix::MatrixPeersTool::new())),
            (ToolTier::Matrix, Arc::new(crate::tools::matrix::MatrixJoinTool::new())),
            (ToolTier::Matrix, Arc::new(crate::tools::matrix::MatrixRpcTool::new())),
        ];
    }

    #[cfg(not(feature = "matrix-bridge"))]
    Vec::new()
}

fn build_extension_runtime_tools(env: &ToolEnv) -> Vec<(ToolTier, Arc<dyn Tool>)> {
    let mut tools: Vec<(ToolTier, Arc<dyn Tool>)> = Vec::new();

    if let Some(settings) = env.settings.as_ref()
        && let Some(tool) = crate::tools::browser::build_browser_tool_from_settings(&settings.browser_automation)
    {
        tools.push((ToolTier::Specialty, tool));
    }

    if let Some(settings) = env.settings.as_ref()
        && let Some(tool) =
            crate::tools::external_memory::build_external_memory_tool_from_settings(&settings.external_memory)
    {
        tools.push((ToolTier::Specialty, tool));
    }

    if let Some(settings) = env.settings.as_ref()
        && settings.steel_eval.enabled
    {
        tools.push((
            ToolTier::Specialty,
            Arc::new(crate::tools::steel_eval::SteelEvalTool::new(steel_eval_tool_config(&settings.steel_eval))),
        ));
    }

    if std::process::Command::new("nix")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
    {
        tools.push((ToolTier::Specialty, Arc::new(crate::tools::nix::eval_tool::NixEvalTool::new())));
    }

    #[cfg(feature = "tui-validate")]
    tools.push((ToolTier::Specialty, Arc::new(crate::tools::devtools::validate_tui::ValidateTuiTool::new())));

    tools
}

pub(crate) fn build_all_tiered_tools(
    env: &ToolEnv,
    plugin_manager: Option<&Arc<Mutex<PluginManager>>>,
) -> Vec<(ToolTier, Arc<dyn Tool>)> {
    let mut tiered = build_builtin_tiered_tools(env);
    if let Some(manager) = plugin_manager {
        let flat_tools: Vec<Arc<dyn Tool>> = tiered.iter().map(|(_, t)| t.clone()).collect();
        let plugin_tools = build_plugin_tools(&flat_tools, manager, env.panel_tx.as_ref());
        for tool in plugin_tools {
            tiered.push((ToolTier::Specialty, tool));
        }
    }
    tiered.extend(build_mcp_tools(env, &tiered));
    tiered
}

fn build_mcp_tools(env: &ToolEnv, existing: &[(ToolTier, Arc<dyn Tool>)]) -> Vec<(ToolTier, Arc<dyn Tool>)> {
    if let (Some(settings), Some(registry)) = (&env.settings, &env.mcp_registry)
        && !settings.mcp.servers.is_empty()
    {
        let mut seen_names: HashSet<String> = existing.iter().map(|(_, tool)| tool.definition().name.clone()).collect();
        return crate::tools::mcp::build_tools_from_settings(&settings.mcp, &mut seen_names, Arc::clone(registry))
            .into_iter()
            .map(|tool| (ToolTier::Specialty, tool))
            .collect();
    }
    Vec::new()
}

/// Build tools provided by loaded plugins. Each tool declared in a plugin's
/// manifest becomes a `PluginTool` that the agent can invoke. Validator plugins
/// (those with "exec" permission and validation tools) get the `ValidatorTool`
/// adapter that can spawn subprocess validators.
pub(crate) fn build_plugin_tools(
    builtin_tools: &[Arc<dyn Tool>],
    manager: &Arc<Mutex<PluginManager>>,
    panel_tx: Option<&tokio::sync::mpsc::UnboundedSender<clankers_tui::components::subagent_event::SubagentEvent>>,
) -> Vec<Arc<dyn Tool>> {
    let host = clankers_plugin::PluginHostFacade::new(Arc::clone(manager));
    let mut tools: Vec<Arc<dyn Tool>> = Vec::new();

    let mut seen_names: HashSet<String> = builtin_tools.iter().map(|t| t.definition().name.clone()).collect();

    let mut active_plugins = host.active_plugins();
    active_plugins.sort_by(|left, right| {
        let left_rank = i32::from(!left.manifest.kind.uses_wasm_runtime());
        let right_rank = i32::from(!right.manifest.kind.uses_wasm_runtime());
        left_rank.cmp(&right_rank).then(left.name.cmp(&right.name))
    });

    for plugin_info in active_plugins {
        if plugin_info.manifest.kind.uses_wasm_runtime() {
            if !plugin_info.manifest.tool_definitions.is_empty() {
                build_detailed_tools(&plugin_info, manager, &mut seen_names, panel_tx, &mut tools);
            } else {
                build_bare_tools(&plugin_info, manager, &mut seen_names, &mut tools);
            }
        } else {
            build_stdio_tools(&plugin_info, manager, &mut seen_names, &mut tools);
        }
    }

    if !tools.is_empty() {
        info!("Registered {} plugin tool(s)", tools.len());
    }

    tools
}

fn build_detailed_tools(
    plugin_info: &clankers_plugin::PluginInfo,
    manager: &Arc<Mutex<PluginManager>>,
    seen_names: &mut HashSet<String>,
    panel_tx: Option<&tokio::sync::mpsc::UnboundedSender<clankers_tui::components::subagent_event::SubagentEvent>>,
    tools: &mut Vec<Arc<dyn Tool>>,
) {
    let is_validator = plugin_info.manifest.permissions.iter().any(|p| p == "exec" || p == "all");

    for tool_def in &plugin_info.manifest.tool_definitions {
        if !seen_names.insert(tool_def.name.clone()) {
            continue;
        }

        let definition = ToolDefinition {
            name: tool_def.name.clone(),
            description: tool_def.description.clone(),
            input_schema: tool_def.input_schema.clone(),
        };

        if is_validator && tool_def.name.starts_with("validate") {
            let mut vtool =
                ValidatorTool::new(definition, plugin_info.name.clone(), tool_def.handler.clone(), Arc::clone(manager));
            if let Some(ptx) = panel_tx {
                vtool = vtool.with_panel_tx(ptx.clone());
            }
            tools.push(Arc::new(vtool));
        } else {
            tools.push(Arc::new(PluginTool::new(
                definition,
                plugin_info.name.clone(),
                tool_def.handler.clone(),
                Arc::clone(manager),
            )));
        }
    }
}

fn build_bare_tools(
    plugin_info: &clankers_plugin::PluginInfo,
    manager: &Arc<Mutex<PluginManager>>,
    seen_names: &mut HashSet<String>,
    tools: &mut Vec<Arc<dyn Tool>>,
) {
    for tool_name in &plugin_info.manifest.tools {
        if !seen_names.insert(tool_name.clone()) {
            continue;
        }

        let definition = ToolDefinition {
            name: tool_name.clone(),
            description: format!("Tool '{}' provided by plugin '{}'", tool_name, plugin_info.name),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string",
                        "description": "Input to pass to the tool"
                    }
                }
            }),
        };
        tools.push(Arc::new(PluginTool::new(
            definition,
            plugin_info.name.clone(),
            "handle_tool_call".to_string(),
            Arc::clone(manager),
        )));
    }
}

fn build_stdio_tools(
    plugin_info: &clankers_plugin::PluginInfo,
    manager: &Arc<Mutex<PluginManager>>,
    seen_names: &mut HashSet<String>,
    tools: &mut Vec<Arc<dyn Tool>>,
) {
    let registered = {
        let manager = manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        manager.live_registered_tools(&plugin_info.name)
    };

    for tool in registered {
        if !seen_names.insert(tool.name.clone()) {
            continue;
        }
        tools.push(Arc::new(PluginTool::new_stdio(
            ToolDefinition {
                name: tool.name,
                description: tool.description,
                input_schema: tool.input_schema,
            },
            plugin_info.name.clone(),
            Arc::clone(manager),
        )));
    }
}

fn steel_eval_tool_config(
    settings: &clankers_config::settings::SteelEvalSettings,
) -> crate::tools::steel_eval::SteelEvalToolConfig {
    let mut default_profile = steel_eval_profile_config(&settings.profile);
    default_profile.id.clone_from(&settings.default_profile);
    let profiles = settings.profiles.iter().map(steel_eval_profile_config).collect();
    crate::tools::steel_eval::SteelEvalToolConfig::new(default_profile, profiles)
}

fn steel_eval_profile_config(
    profile: &clankers_config::settings::SteelEvalProfileSettings,
) -> crate::tools::steel_eval::SteelEvalProfileConfig {
    crate::tools::steel_eval::SteelEvalProfileConfig {
        id: profile.id.clone(),
        max_source_bytes: profile.max_source_bytes,
        max_output_bytes: profile.max_output_bytes,
        max_host_calls: profile.max_host_calls,
        max_steps: profile.max_steps,
        session_capabilities: profile.session_capabilities.clone(),
        host_functions: profile.host_functions.iter().map(steel_eval_host_function).collect(),
    }
}

fn steel_eval_host_function(
    host: &clankers_config::settings::SteelEvalHostFunctionSettings,
) -> clankers_runtime::steel_runtime::SteelHostFunctionRegistration {
    clankers_runtime::steel_runtime::SteelHostFunctionRegistration {
        name: host.name.clone(),
        required_capability: host.required_capability.clone(),
        output: host.output.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_catalog_owner_inventory_names_all_required_families() {
        let owners = tool_family_owners();
        let families = owners.iter().map(|owner| owner.family).collect::<std::collections::BTreeSet<_>>();

        for required in [
            "core",
            "orchestration",
            "specialty",
            "daemon-session",
            "matrix",
            "plugin",
            "extension-runtime",
            "mcp",
        ] {
            assert!(families.contains(required), "missing tool family owner: {required}");
        }
    }

    #[test]
    fn builtin_catalog_groups_publish_expected_tools() {
        let env = ToolEnv::default();

        let core = build_core_tools(&env);
        let orchestration = build_orchestration_tools(&env);
        let specialty = build_specialty_tools(&env);

        assert!(core.iter().any(|(_, tool)| tool.definition().name == "read"));
        assert!(core.iter().any(|(_, tool)| tool.definition().name == "bash"));
        assert!(orchestration.iter().any(|(_, tool)| tool.definition().name == "subagent"));
        assert!(orchestration.iter().any(|(_, tool)| tool.definition().name == "delegate_task"));
        assert!(specialty.iter().any(|(_, tool)| tool.definition().name == "tool_gateway"));
        assert!(specialty.iter().any(|(_, tool)| tool.definition().name == "checkpoint"));
    }
}
