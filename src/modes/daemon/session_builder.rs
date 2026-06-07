//! Socketless daemon session construction plans.
//!
//! The control socket and chat transports are imperative shells: they accept
//! requests, open sockets, and send frames. This module owns the deterministic
//! session-construction decisions that can be tested without binding a Unix
//! socket or spawning an actor.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use clanker_tui_types::SubagentEvent;
use clankers_controller::SessionController;
use clankers_controller::config::ControllerConfig;
use clankers_controller::transport::SessionHandle;
use clankers_controller::transport::session_socket_path;
use clankers_protocol::DaemonEvent;
use clankers_protocol::SerializedMessage;
use clankers_protocol::SessionCommand;
use clankers_protocol::SessionKey;
use tokio::sync::broadcast;
use tokio::sync::mpsc;

use super::session_plugins::DaemonSessionTickService;
use super::session_plugins::tool_rebuilder_for_factory;
use super::session_store::SessionCatalogEntry;
use super::session_store::SessionLifecycle;
use super::socket_bridge::SessionFactory;

/// User/control-plane inputs for a local daemon session create request.
pub(crate) struct CreateSessionPlanRequest {
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    pub resume_id: Option<String>,
    pub continue_last: bool,
    pub cwd: Option<String>,
    pub thinking_level: Option<String>,
}

/// Inputs passed to `agent_process::spawn_agent_process`.
pub(crate) struct AgentSpawnPlan {
    pub session_id: String,
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    pub capabilities: Option<Vec<clankers_ucan::Capability>>,
    pub public_auth: Option<crate::capability_gate::PublicUcanToolAuthorization>,
}

/// Socketless construction result for a daemon session.
pub(crate) struct SessionBuildPlan {
    pub session_id: String,
    pub resolved_model: String,
    pub socket_path: PathBuf,
    pub spawn: AgentSpawnPlan,
    pub seed_messages: Vec<SerializedMessage>,
    pub thinking_level: Option<String>,
    pub key: Option<SessionKey>,
}

/// Actor-ready runtime inputs assembled without binding daemon sockets.
pub(crate) struct DaemonSessionRuntime {
    pub session_id: String,
    pub controller: SessionController,
    pub cmd_tx: mpsc::UnboundedSender<SessionCommand>,
    pub cmd_rx: mpsc::UnboundedReceiver<SessionCommand>,
    pub event_tx: broadcast::Sender<DaemonEvent>,
    pub panel_rx: mpsc::UnboundedReceiver<SubagentEvent>,
    pub bash_confirm_rx: crate::tools::bash::ConfirmRx,
    pub actor_tick_service: DaemonSessionTickService,
    pub automerge_path: Option<PathBuf>,
}

pub(crate) struct DaemonSessionRuntimeRequest<'a> {
    pub factory: &'a SessionFactory,
    pub session_id: String,
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    pub capabilities: Option<Vec<clankers_ucan::Capability>>,
    pub public_auth: Option<crate::capability_gate::PublicUcanToolAuthorization>,
}

/// Builder that owns session-construction policy while callers own IO/spawn.
/// Build a hook pipeline for a daemon session from settings.
///
/// This is session assembly policy rather than actor-loop multiplexing. Tests
/// can exercise it without binding daemon sockets or spawning an actor.
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(unbounded_loop, reason = "bounded upward directory walk looking for .git")
)]
pub(crate) fn build_session_hook_pipeline(
    settings: &clankers_config::settings::Settings,
    plugin_manager: Option<&Arc<Mutex<clankers_plugin::PluginManager>>>,
) -> Option<Arc<clankers_hooks::HookPipeline>> {
    if !settings.hooks.enabled {
        return None;
    }

    let cwd = std::env::current_dir().unwrap_or_default();
    let mut pipeline = clankers_hooks::HookPipeline::new();
    pipeline.set_disabled_hooks(settings.hooks.disabled_hooks.iter().cloned());

    let hooks_dir = settings.hooks.resolve_hooks_dir(&cwd);
    let timeout = Duration::from_secs(settings.hooks.script_timeout_secs);
    pipeline.register(Arc::new(clankers_hooks::script::ScriptHookHandler::new(hooks_dir, timeout)));

    if settings.hooks.manage_git_hooks {
        let mut current = cwd.as_path();
        loop {
            if current.join(".git").exists() {
                pipeline.register(Arc::new(clankers_hooks::git::GitHookHandler::new(current.to_path_buf())));
                break;
            }
            match current.parent() {
                Some(parent) => current = parent,
                None => break,
            }
        }
    }

    if let Some(plugin_manager) = plugin_manager {
        pipeline.register(Arc::new(clankers_plugin::hooks::PluginHookHandler::new(Arc::clone(plugin_manager))));
    }

    Some(Arc::new(pipeline))
}

pub(crate) fn assemble_session_runtime(request: DaemonSessionRuntimeRequest<'_>) -> DaemonSessionRuntime {
    let paths = clankers_config::ClankersPaths::get();
    assemble_session_runtime_in_dir(request, paths.global_sessions_dir.clone(), Some(paths.global_config_dir.clone()))
}

fn assemble_session_runtime_in_dir(
    request: DaemonSessionRuntimeRequest<'_>,
    sessions_dir: PathBuf,
    pricing_config_dir: Option<PathBuf>,
) -> DaemonSessionRuntime {
    let factory = request.factory;
    let session_id = request.session_id;
    let model = request.model.unwrap_or_else(|| factory.default_model.clone());
    let system_prompt = request.system_prompt.unwrap_or_else(|| factory.default_system_prompt.clone());

    let (panel_tx, panel_rx) = mpsc::unbounded_channel::<SubagentEvent>();
    let (bash_confirm_tx, bash_confirm_rx) = crate::tools::bash::confirm_channel();
    let tools = factory.build_tools_with_panel_tx(panel_tx, Some(bash_confirm_tx));
    let effective_caps =
        merge_session_capabilities(request.capabilities.as_deref(), factory.settings.default_capabilities.as_deref());

    let builder_config = crate::agent_config::agent_builder_config_from_settings(
        &factory.settings,
        factory.provider.models(),
        pricing_config_dir.as_deref(),
    );
    let model_service: Arc<dyn clankers_agent::AgentModelService> = Arc::new(
        crate::agent_runtime_adapters::ProviderModelServiceAdapter::new(Arc::clone(&factory.provider)),
    );
    let mut builder = clankers_agent::builder::AgentBuilder::new(
        model_service,
        builder_config,
        model.clone(),
        system_prompt.clone(),
    )
    .with_tools(tools);

    if let Some(public_auth) = request.public_auth {
        let gate = Arc::new(crate::capability_gate::PublicUcanCapabilityGate::new(public_auth));
        builder = builder.with_capability_gate(gate);
    } else if let Some(caps) = &effective_caps {
        let gate = Arc::new(crate::capability_gate::UcanCapabilityGate::new(caps.clone()));
        builder = builder.with_capability_gate(gate);
    }

    let hook_pipeline = build_session_hook_pipeline(&factory.settings, factory.plugin_manager.as_ref());
    let mut agent = builder.build();
    if let Some(ref pipeline) = hook_pipeline {
        agent = agent
            .with_hook_service(Arc::new(crate::agent_runtime_adapters::HookPipelineAgentHookService::new(Arc::clone(
                pipeline,
            ))))
            .with_tool_hook_service(Arc::new(crate::agent_runtime_adapters::HookPipelineToolHookService::new(
                Arc::clone(pipeline),
                session_id.clone(),
            )))
            .with_tool_context_service(Arc::clone(pipeline));
    }
    let tool_patterns = effective_caps.as_deref().and_then(crate::capability_gate::extract_tool_patterns);
    let (session_manager, automerge_path) = build_session_manager(&sessions_dir, &session_id, &model);

    let config = ControllerConfig {
        session_id: session_id.clone(),
        model,
        system_prompt: Some(system_prompt),
        capabilities: tool_patterns.clone(),
        capability_ceiling: tool_patterns,
        session_manager,
        hook_pipeline,
        initial_thinking_level: crate::modes::common::core_thinking_level(factory.settings.parsed_thinking_level()),
        auto_test_command: factory.settings.auto_test_command.clone(),
        auto_test_enabled: factory.settings.auto_test_command.is_some(),
    };

    let mut controller = SessionController::new(agent, config);
    controller.set_tool_rebuilder(tool_rebuilder_for_factory(Arc::new(SessionFactory {
        provider: Arc::clone(&factory.provider),
        tools: factory.tools.clone(),
        settings: factory.settings.clone(),
        default_model: factory.default_model.clone(),
        default_system_prompt: factory.default_system_prompt.clone(),
        registry: None,
        catalog: None,
        schedule_engine: factory.schedule_engine.clone(),
        plugin_manager: factory.plugin_manager.clone(),
    })));

    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<SessionCommand>();
    let (event_tx, _) = broadcast::channel::<DaemonEvent>(256);

    DaemonSessionRuntime {
        session_id,
        controller,
        cmd_tx,
        cmd_rx,
        event_tx,
        panel_rx,
        bash_confirm_rx,
        actor_tick_service: DaemonSessionTickService::for_plugin_manager(factory.plugin_manager.clone()),
        automerge_path,
    }
}

fn build_session_manager(
    sessions_dir: &Path,
    session_id: &str,
    model: &str,
) -> (Option<clankers_session::SessionManager>, Option<PathBuf>) {
    let cwd = std::env::current_dir().unwrap_or_default().to_string_lossy().into_owned();
    match clankers_session::SessionManager::create(sessions_dir, &cwd, model, None, None, None) {
        Ok(mgr) => {
            let path = mgr.file_path().to_path_buf();
            tracing::info!("session {session_id}: persistence enabled at {path:?}");
            (Some(mgr), Some(path))
        }
        Err(error) => {
            tracing::warn!("session {session_id}: failed to create session file: {error}");
            (None, None)
        }
    }
}

pub(crate) struct SessionBuilder {
    default_model: String,
    sessions_dir: PathBuf,
}

impl SessionBuilder {
    pub(crate) fn from_global_paths(default_model: impl Into<String>) -> Self {
        Self {
            default_model: default_model.into(),
            sessions_dir: clankers_config::ClankersPaths::get().global_sessions_dir.clone(),
        }
    }

    #[cfg(test)]
    pub(crate) fn for_sessions_dir(default_model: impl Into<String>, sessions_dir: PathBuf) -> Self {
        Self {
            default_model: default_model.into(),
            sessions_dir,
        }
    }

    pub(crate) fn plan_create_session(&self, request: CreateSessionPlanRequest) -> SessionBuildPlan {
        let effective_cwd = request.cwd.as_deref().unwrap_or(".");
        let (session_id, seed_messages) = resolve_session_resume_in_dir(
            &self.sessions_dir,
            request.resume_id.as_deref(),
            request.continue_last,
            effective_cwd,
        );
        let resolved_model = request.model.clone().unwrap_or_else(|| self.default_model.clone());
        let socket_path = session_socket_path(&session_id);
        let spawn = AgentSpawnPlan {
            session_id: session_id.clone(),
            model: request.model,
            system_prompt: request.system_prompt,
            capabilities: None,
            public_auth: None,
        };

        SessionBuildPlan {
            session_id,
            resolved_model,
            socket_path,
            spawn,
            seed_messages,
            thinking_level: request.thinking_level.filter(|level| !level.trim().is_empty()),
            key: None,
        }
    }

    pub(crate) fn plan_new_keyed_session(
        &self,
        key: &SessionKey,
        capabilities: Option<Vec<clankers_ucan::Capability>>,
        public_auth: Option<crate::capability_gate::PublicUcanToolAuthorization>,
    ) -> SessionBuildPlan {
        let session_id = clanker_message::transcript::generate_id();
        let socket_path = session_socket_path(&session_id);
        let spawn = AgentSpawnPlan {
            session_id: session_id.clone(),
            model: None,
            system_prompt: None,
            capabilities,
            public_auth,
        };

        SessionBuildPlan {
            session_id,
            resolved_model: self.default_model.clone(),
            socket_path,
            spawn,
            seed_messages: Vec::new(),
            thinking_level: None,
            key: Some(key.clone()),
        }
    }

    pub(crate) fn plan_recovered_catalog_session(
        &self,
        entry: &SessionCatalogEntry,
        public_auth: Option<crate::capability_gate::PublicUcanToolAuthorization>,
    ) -> SessionBuildPlan {
        let seed_messages = load_recovery_seed_messages(entry);
        let socket_path = session_socket_path(&entry.session_id);
        let spawn = AgentSpawnPlan {
            session_id: entry.session_id.clone(),
            model: Some(entry.model.clone()),
            system_prompt: None,
            capabilities: None,
            public_auth,
        };

        SessionBuildPlan {
            session_id: entry.session_id.clone(),
            resolved_model: entry.model.clone(),
            socket_path,
            spawn,
            seed_messages,
            thinking_level: None,
            key: None,
        }
    }

    pub(crate) fn plan_recovered_keyed_session(
        &self,
        key: &SessionKey,
        entry: &SessionCatalogEntry,
        public_auth: Option<crate::capability_gate::PublicUcanToolAuthorization>,
    ) -> SessionBuildPlan {
        let mut plan = self.plan_recovered_catalog_session(entry, public_auth);
        plan.key = Some(key.clone());
        plan
    }

    pub(crate) fn plan_ephemeral_child_session(
        &self,
        session_id: String,
        model: Option<String>,
        system_prompt: Option<String>,
    ) -> SessionBuildPlan {
        let resolved_model = model.clone().unwrap_or_else(|| self.default_model.clone());
        let socket_path = session_socket_path(&session_id);
        let spawn = AgentSpawnPlan {
            session_id: session_id.clone(),
            model,
            system_prompt,
            capabilities: None,
            public_auth: None,
        };

        SessionBuildPlan {
            session_id,
            resolved_model,
            socket_path,
            spawn,
            seed_messages: Vec::new(),
            thinking_level: None,
            key: None,
        }
    }
}

impl SessionBuildPlan {
    pub(crate) fn session_handle(
        &self,
        cmd_tx: mpsc::UnboundedSender<SessionCommand>,
        event_tx: broadcast::Sender<DaemonEvent>,
    ) -> SessionHandle {
        SessionHandle {
            session_id: self.session_id.clone(),
            model: self.resolved_model.clone(),
            turn_count: 0,
            last_active: chrono::Utc::now().to_rfc3339(),
            client_count: 0,
            cmd_tx: Some(cmd_tx),
            event_tx: Some(event_tx),
            socket_path: self.socket_path.clone(),
            state: "active".to_string(),
        }
    }

    pub(crate) fn catalog_entry(&self, automerge_path: PathBuf, now: String) -> SessionCatalogEntry {
        SessionCatalogEntry {
            session_id: self.session_id.clone(),
            automerge_path,
            model: self.resolved_model.clone(),
            created_at: now.clone(),
            last_active: now,
            turn_count: 0,
            state: SessionLifecycle::Active,
        }
    }

    pub(crate) fn thinking_command(&self) -> Option<SessionCommand> {
        self.thinking_level.as_ref().map(|level| SessionCommand::SetThinkingLevel { level: level.clone() })
    }

    pub(crate) fn seed_command(&self) -> Option<SessionCommand> {
        if self.seed_messages.is_empty() {
            None
        } else {
            Some(SessionCommand::SeedMessages {
                messages: self.seed_messages.clone(),
            })
        }
    }
}

/// Merge UCAN token capabilities with settings `default_capabilities` before spawning a session.
///
/// When both are present, the settings caps act as an outer boundary: only UCAN capabilities
/// that the settings also authorize are kept. When only one source is present, use it. When
/// neither exists, return `None` to mean full local access.
pub(crate) fn merge_session_capabilities(
    ucan_caps: Option<&[clankers_ucan::Capability]>,
    settings_caps: Option<&[clankers_ucan::Capability]>,
) -> Option<Vec<clankers_ucan::Capability>> {
    match (ucan_caps, settings_caps) {
        (None, None) => None,
        (Some(ucan), None) => Some(ucan.to_vec()),
        (None, Some(settings)) => Some(settings.to_vec()),
        (Some(ucan), Some(settings)) => Some(
            ucan.iter()
                .filter(|capability| settings.iter().any(|setting| setting.contains(capability)))
                .cloned()
                .collect(),
        ),
    }
}

pub(crate) fn load_recovery_seed_messages(entry: &SessionCatalogEntry) -> Vec<SerializedMessage> {
    if !entry.automerge_path.exists() {
        tracing::warn!("recovery automerge file missing at {:?} — starting fresh", entry.automerge_path);
        return Vec::new();
    }

    match clankers_session::SessionManager::open(entry.automerge_path.clone()) {
        Ok(manager) => match manager.build_context() {
            Ok(messages) => {
                let serialized = serialize_seed_messages(&messages);
                tracing::info!("loaded {} recovery messages from {:?}", serialized.len(), entry.automerge_path);
                serialized
            }
            Err(error) => {
                tracing::warn!(
                    "failed to build recovery context from {:?}: {error} — starting fresh",
                    entry.automerge_path
                );
                Vec::new()
            }
        },
        Err(error) => {
            tracing::warn!("failed to open recovery automerge at {:?}: {error} — starting fresh", entry.automerge_path);
            Vec::new()
        }
    }
}

pub(crate) fn serialize_seed_messages(
    messages: &[clanker_message::transcript::AgentMessage],
) -> Vec<SerializedMessage> {
    crate::modes::session_ledger::desktop_messages_to_serialized_seed_messages(messages)
}

fn resolve_session_resume_in_dir(
    sessions_dir: &Path,
    resume_id: Option<&str>,
    continue_last: bool,
    cwd: &str,
) -> (String, Vec<SerializedMessage>) {
    let try_open = |file: PathBuf| -> Option<(String, Vec<SerializedMessage>)> {
        let manager = clankers_session::SessionManager::open(file).ok()?;
        let messages = manager.build_context().ok()?;
        Some((manager.session_id().to_string(), serialize_seed_messages(&messages)))
    };

    if let Some(id) = resume_id {
        let files = clankers_session::store::list_sessions(sessions_dir, cwd);
        if let Some(file) = files
            .into_iter()
            .find(|file| file.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.contains(id)))
            && let Some(result) = try_open(file)
        {
            return result;
        }
    }

    if continue_last {
        let files = clankers_session::store::list_sessions(sessions_dir, cwd);
        if let Some(file) = files.into_iter().next()
            && let Some(result) = try_open(file)
        {
            return result;
        }
    }

    (clanker_message::transcript::generate_id(), Vec::new())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use chrono::Utc;
    use clanker_message::Content;
    use clanker_message::StopReason;
    use clanker_message::Usage;
    use clanker_message::transcript::AssistantMessage;
    use clanker_message::transcript::MessageId;
    use clanker_message::transcript::UserMessage;
    use clankers_ucan::Capability;
    use tempfile::tempdir;

    use super::*;

    struct StubProvider;

    #[async_trait::async_trait]
    impl clankers_provider::Provider for StubProvider {
        async fn complete(
            &self,
            _req: clankers_provider::CompletionRequest,
            _tx: tokio::sync::mpsc::Sender<clanker_message::streaming::StreamEvent>,
        ) -> std::result::Result<(), clankers_provider::error::ProviderError> {
            Ok(())
        }

        fn models(&self) -> &[clankers_provider::Model] {
            &[]
        }

        fn name(&self) -> &str {
            "session-builder-stub"
        }
    }

    fn make_factory() -> SessionFactory {
        SessionFactory {
            provider: Arc::new(StubProvider),
            tools: vec![],
            settings: clankers_config::settings::Settings::default(),
            default_model: "default-model".to_string(),
            default_system_prompt: "default prompt".to_string(),
            registry: None,
            catalog: None,
            schedule_engine: None,
            plugin_manager: None,
        }
    }

    fn append_resume_fixture(session: &mut clankers_session::SessionManager) {
        let user_id = MessageId::new("user-1");
        session
            .append_message(
                clanker_message::transcript::AgentMessage::User(UserMessage {
                    id: user_id.clone(),
                    content: vec![Content::Text {
                        text: "hello from resume".to_string(),
                    }],
                    timestamp: Utc::now(),
                }),
                None,
            )
            .unwrap();
        session
            .append_message(
                clanker_message::transcript::AgentMessage::Assistant(AssistantMessage {
                    id: MessageId::new("assistant-1"),
                    content: vec![Content::Text {
                        text: "hello back".to_string(),
                    }],
                    model: "fixture-model".to_string(),
                    usage: Usage::default(),
                    stop_reason: StopReason::Stop,
                    timestamp: Utc::now(),
                }),
                Some(user_id),
            )
            .unwrap();
    }

    #[test]
    fn session_capability_merge_is_socketless_and_settings_bounded() {
        assert!(merge_session_capabilities(None, None).is_none());

        let settings = vec![Capability::ToolUse {
            tool_pattern: "read,bash,grep".to_string(),
        }];
        let ucan = vec![
            Capability::ToolUse {
                tool_pattern: "read".to_string(),
            },
            Capability::ToolUse {
                tool_pattern: "write".to_string(),
            },
        ];
        let result = merge_session_capabilities(Some(&ucan), Some(&settings)).unwrap();
        assert_eq!(result.len(), 1);
        assert!(matches!(&result[0], Capability::ToolUse { tool_pattern } if tool_pattern == "read"));

        let no_overlap_settings = vec![Capability::ToolUse {
            tool_pattern: "todo".to_string(),
        }];
        let blocked = merge_session_capabilities(Some(&ucan), Some(&no_overlap_settings)).unwrap();
        assert!(blocked.is_empty());
    }

    #[test]
    fn create_plan_for_new_session_has_spawn_and_handle_data_without_socket() {
        let dir = tempdir().unwrap();
        let builder = SessionBuilder::for_sessions_dir("default-model", dir.path().join("sessions"));

        let plan = builder.plan_create_session(CreateSessionPlanRequest {
            model: None,
            system_prompt: Some("system".to_string()),
            resume_id: None,
            continue_last: false,
            cwd: Some("/tmp/project".to_string()),
            thinking_level: Some("max".to_string()),
        });

        assert_eq!(plan.resolved_model, "default-model");
        assert_eq!(plan.spawn.session_id, plan.session_id);
        assert_eq!(plan.spawn.model, None);
        assert_eq!(plan.spawn.system_prompt.as_deref(), Some("system"));
        let expected_socket_name = format!("session-{}.sock", plan.session_id);
        assert_eq!(plan.socket_path.file_name().and_then(|name| name.to_str()), Some(expected_socket_name.as_str()));
        assert!(matches!(plan.thinking_command(), Some(SessionCommand::SetThinkingLevel { level }) if level == "max"));
        assert!(plan.seed_command().is_none());
    }

    #[test]
    fn create_plan_resolves_resume_messages_without_socket() {
        let dir = tempdir().unwrap();
        let sessions_dir = dir.path().join("sessions");
        let cwd = dir.path().join("project");
        let cwd_text = cwd.to_string_lossy().to_string();
        let mut session =
            clankers_session::SessionManager::create(&sessions_dir, &cwd_text, "fixture-model", None, None, None)
                .unwrap();
        append_resume_fixture(&mut session);
        let resume_id = session.session_id().to_string();

        let builder = SessionBuilder::for_sessions_dir("default-model", sessions_dir);
        let plan = builder.plan_create_session(CreateSessionPlanRequest {
            model: Some("requested-model".to_string()),
            system_prompt: None,
            resume_id: Some(resume_id.clone()),
            continue_last: false,
            cwd: Some(cwd_text),
            thinking_level: None,
        });

        assert_eq!(plan.session_id, resume_id);
        assert_eq!(plan.resolved_model, "requested-model");
        assert_eq!(plan.seed_messages.len(), 2);
        assert_eq!(plan.seed_messages[0].role, "user");
        assert_eq!(plan.seed_messages[0].content, "hello from resume");
        assert_eq!(plan.seed_messages[1].role, "assistant");
        assert_eq!(plan.seed_messages[1].model.as_deref(), Some("fixture-model"));
        assert!(matches!(plan.seed_command(), Some(SessionCommand::SeedMessages { messages }) if messages.len() == 2));
    }

    #[test]
    fn keyed_plans_prepare_new_and_recovered_actor_inputs_without_socket() {
        let dir = tempdir().unwrap();
        let sessions_dir = dir.path().join("sessions");
        let cwd = dir.path().join("project");
        let cwd_text = cwd.to_string_lossy().to_string();
        let mut session =
            clankers_session::SessionManager::create(&sessions_dir, &cwd_text, "catalog-model", None, None, None)
                .unwrap();
        append_resume_fixture(&mut session);
        let entry = SessionCatalogEntry {
            session_id: session.session_id().to_string(),
            automerge_path: PathBuf::from(session.file_path()),
            model: "catalog-model".to_string(),
            created_at: "now".to_string(),
            last_active: "now".to_string(),
            turn_count: 7,
            state: SessionLifecycle::Suspended,
        };
        let key = SessionKey::Matrix {
            user_id: "@user:example".to_string(),
            room_id: "!room:example".to_string(),
        };
        let builder = SessionBuilder::for_sessions_dir("default-model", sessions_dir);

        let new_plan = builder.plan_new_keyed_session(&key, None, None);
        assert_eq!(new_plan.resolved_model, "default-model");
        assert_eq!(new_plan.key.as_ref(), Some(&key));
        assert!(new_plan.spawn.model.is_none());
        assert!(new_plan.seed_messages.is_empty());

        let recovered = builder.plan_recovered_keyed_session(&key, &entry, None);
        assert_eq!(recovered.session_id, entry.session_id);
        assert_eq!(recovered.resolved_model, "catalog-model");
        assert_eq!(recovered.key.as_ref(), Some(&key));
        assert_eq!(recovered.spawn.model.as_deref(), Some("catalog-model"));
        assert_eq!(recovered.seed_messages.len(), 2);
    }

    #[test]
    fn ephemeral_plan_prepares_child_actor_inputs_without_socket() {
        let dir = tempdir().unwrap();
        let builder = SessionBuilder::for_sessions_dir("default-model", dir.path().join("sessions"));
        let plan = builder.plan_ephemeral_child_session(
            "ephemeral-child".to_string(),
            Some("child-model".to_string()),
            Some("child system".to_string()),
        );

        assert_eq!(plan.session_id, "ephemeral-child");
        assert_eq!(plan.resolved_model, "child-model");
        assert_eq!(plan.spawn.session_id, "ephemeral-child");
        assert_eq!(plan.spawn.model.as_deref(), Some("child-model"));
        assert_eq!(plan.spawn.system_prompt.as_deref(), Some("child system"));
        assert!(plan.spawn.capabilities.is_none());
        assert!(plan.spawn.public_auth.is_none());
        assert!(plan.seed_messages.is_empty());
        assert!(plan.key.is_none());
        let expected_socket_name = "session-ephemeral-child.sock";
        assert_eq!(plan.socket_path.file_name().and_then(|name| name.to_str()), Some(expected_socket_name));
    }

    #[test]
    fn runtime_bundle_assembles_controller_channels_and_tick_service_without_actor_or_socket() {
        let dir = tempdir().unwrap();
        let factory = make_factory();
        let runtime = assemble_session_runtime_in_dir(
            DaemonSessionRuntimeRequest {
                factory: &factory,
                session_id: "runtime-bundle".to_string(),
                model: Some("runtime-model".to_string()),
                system_prompt: Some("runtime system".to_string()),
                capabilities: None,
                public_auth: None,
            },
            dir.path().join("sessions"),
            None,
        );

        assert_eq!(runtime.session_id, "runtime-bundle");
        assert!(runtime.cmd_tx.send(SessionCommand::GetToolList).is_ok());
        assert_eq!(runtime.event_tx.receiver_count(), 0);
        assert!(runtime.automerge_path.as_ref().is_some_and(|path| path.starts_with(dir.path())));
        assert!(runtime.actor_tick_service.plugin_summaries().is_empty());
        assert!(runtime.bash_confirm_rx.is_empty());
        assert!(!runtime.controller.current_tool_infos().is_empty());
    }
}
