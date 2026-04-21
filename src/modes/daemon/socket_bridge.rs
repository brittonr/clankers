//! Bridge between Unix domain sockets and SessionController instances.
//!
//! Wires `clankers-controller`'s transport layer into the daemon. The
//! control socket handles session creation; per-session sockets relay
//! commands and events between clients and their SessionController.

use std::sync::Arc;

use clanker_actor::ProcessRegistry;
use clankers_controller::SessionController;
use clankers_controller::transport::DaemonState;
use clankers_controller::transport::SessionHandle;
use clankers_controller::transport::session_socket_path;
use clankers_protocol::DaemonEvent;
use clankers_protocol::SessionCommand;
use clankers_protocol::control::ControlCommand;
use clankers_protocol::control::ControlResponse;
use clankers_protocol::frame::{self};
use clankers_tui_types::SubagentEvent;
use tokio::net::UnixListener;
use tokio::sync::Mutex;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;

use crate::config::settings::Settings;
use crate::provider::Provider;
use crate::tools::Tool;

/// Resources needed to create new sessions.
pub struct SessionFactory {
    pub provider: Arc<dyn Provider>,
    pub tools: Vec<Arc<dyn Tool>>,
    pub settings: Settings,
    pub default_model: String,
    pub default_system_prompt: String,
    pub registry: Option<ProcessRegistry>,
    pub catalog: Option<Arc<super::session_store::SessionCatalog>>,
    /// Shared schedule engine — persists across sessions.
    pub schedule_engine: Option<std::sync::Arc<clanker_scheduler::ScheduleEngine>>,
    /// Shared plugin manager — plugins loaded once at daemon startup.
    pub plugin_manager: Option<std::sync::Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
}

impl SessionFactory {
    fn child_actor_factory(&self) -> Option<Arc<Self>> {
        self.registry.as_ref().map(|_| {
            Arc::new(Self {
                provider: Arc::clone(&self.provider),
                tools: self.tools.clone(),
                settings: self.settings.clone(),
                default_model: self.default_model.clone(),
                default_system_prompt: self.default_system_prompt.clone(),
                // Don't recurse — child agents use subprocess fallback
                registry: None,
                catalog: None,
                schedule_engine: self.schedule_engine.clone(),
                plugin_manager: None, // child factories skip plugins
            })
        })
    }

    /// Rebuild tools with a panel_tx for subagent event routing.
    ///
    /// Clones all tools, injecting the panel sender into SubagentTool,
    /// DelegateTool, and ValidatorTool. Other tools are passed through.
    pub fn build_tools_with_panel_tx(
        &self,
        panel_tx: mpsc::UnboundedSender<SubagentEvent>,
        bash_confirm_tx: Option<crate::tools::bash::ConfirmTx>,
    ) -> Vec<Arc<dyn Tool>> {
        let child_factory = self.child_actor_factory();
        let actor_ctx =
            self.registry
                .as_ref()
                .zip(child_factory)
                .map(|(reg, factory)| crate::tools::subagent::ActorContext {
                    registry: reg.clone(),
                    factory,
                });
        let env = crate::modes::common::ToolEnv {
            panel_tx: Some(panel_tx),
            bash_confirm_tx,
            actor_ctx,
            schedule_engine: self.schedule_engine.clone(),
            ..Default::default()
        };
        let tiered = crate::modes::common::build_all_tiered_tools(&env, self.plugin_manager.as_ref());
        let tool_set = crate::modes::common::ToolSet::new(tiered, [
            crate::modes::common::ToolTier::Core,
            crate::modes::common::ToolTier::Orchestration,
            crate::modes::common::ToolTier::Specialty,
            crate::modes::common::ToolTier::Matrix,
        ]);
        tool_set.active_tools()
    }
}

/// Run the control socket with session creation support.
///
/// Unlike `transport::run_control_socket`, this version handles
/// `CreateSession` by constructing a `SessionController` and spawning
/// the session socket + driver tasks.
pub async fn run_control_socket_with_factory(
    state: Arc<Mutex<DaemonState>>,
    factory: Arc<SessionFactory>,
    registry: ProcessRegistry,
    shutdown: tokio::sync::watch::Receiver<bool>,
) {
    let path = clankers_controller::transport::control_socket_path();
    std::fs::remove_file(&path).ok();

    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => {
            error!("failed to bind control socket: {e}");
            return;
        }
    };
    info!("control socket listening at {}", path.display());

    loop {
        tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok((stream, _)) => {
                        let state = Arc::clone(&state);
                        let factory = Arc::clone(&factory);
                        let registry = registry.clone();
                        let shutdown = shutdown.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_control(stream, state, factory, registry, shutdown).await {
                                debug!("control connection ended: {e}");
                            }
                        });
                    }
                    Err(e) => {
                        warn!("control socket accept error: {e}");
                    }
                }
            }
            () = shutdown_signal(&shutdown) => {
                info!("control socket shutting down");
                break;
            }
        }
    }
}

#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(function_length, reason = "sequential dispatch logic")
)]
async fn handle_control(
    mut stream: tokio::net::UnixStream,
    state: Arc<Mutex<DaemonState>>,
    factory: Arc<SessionFactory>,
    registry: ProcessRegistry,
    shutdown: tokio::sync::watch::Receiver<bool>,
) -> Result<(), clankers_protocol::frame::FrameError> {
    let (mut reader, mut writer) = stream.split();
    let cmd: ControlCommand = frame::read_frame(&mut reader).await?;
    debug!("control command: {cmd:?}");

    let response = match cmd {
        ControlCommand::CreateSession {
            model,
            system_prompt,
            token: _,
            resume_id,
            continue_last,
            cwd,
        } => {
            // If resuming, try to load seed messages from the session file.
            let (session_id, seed_messages) =
                resolve_session_resume(resume_id.as_deref(), continue_last, cwd.as_deref());
            let resolved_model = model.clone().unwrap_or_else(|| factory.default_model.clone());

            // Spawn as an actor process in the registry
            let spawned = super::agent_process::spawn_agent_process(
                &registry,
                &factory,
                session_id.clone(),
                model,
                system_prompt,
                None,
                None, // local sessions get full access
            );
            let cmd_tx = spawned.cmd_tx;
            let event_tx = spawned.event_tx;

            let socket_path = session_socket_path(&session_id);

            // Register in daemon state
            {
                let mut st = state.lock().await;
                st.sessions.insert(session_id.clone(), SessionHandle {
                    session_id: session_id.clone(),
                    model: resolved_model.clone(),
                    turn_count: 0,
                    last_active: chrono::Utc::now().to_rfc3339(),
                    client_count: 0,
                    cmd_tx: Some(cmd_tx.clone()),
                    event_tx: Some(event_tx.clone()),
                    socket_path: socket_path.clone(),
                    state: "active".to_string(),
                });
            }

            // Write catalog entry for recovery
            if let Some(ref catalog) = factory.catalog {
                let now = chrono::Utc::now().to_rfc3339();
                catalog.insert_session(&super::session_store::SessionCatalogEntry {
                    session_id: session_id.clone(),
                    automerge_path: spawned.automerge_path.clone().unwrap_or_default(),
                    model: resolved_model.clone(),
                    created_at: now.clone(),
                    last_active: now,
                    turn_count: 0,
                    state: super::session_store::SessionLifecycle::Active,
                });
            }

            // Bind the session socket before replying so attaches cannot race it.
            let listener = match clankers_controller::transport::bind_session_socket(&session_id) {
                Ok(listener) => listener,
                Err(e) => {
                    {
                        let mut st = state.lock().await;
                        st.remove_session(&session_id);
                    }
                    if let Some(ref catalog) = factory.catalog {
                        catalog.set_state(&session_id, super::session_store::SessionLifecycle::Tombstoned);
                    }
                    return frame::write_frame(&mut writer, &ControlResponse::Error {
                        message: format!("failed to bind session socket for {session_id}: {e}"),
                    })
                    .await;
                }
            };
            let sock_shutdown = shutdown.clone();
            let sock_cmd_tx = cmd_tx.clone();
            let sock_event_tx = event_tx.clone();
            let sock_session_id = session_id.clone();
            tokio::spawn(async move {
                clankers_controller::transport::run_session_socket_with_listener(
                    listener,
                    sock_session_id,
                    sock_cmd_tx,
                    sock_event_tx,
                    sock_shutdown,
                )
                .await;
            });

            // Seed resumed messages if any
            if !seed_messages.is_empty() {
                let count = seed_messages.len();
                cmd_tx
                    .send(SessionCommand::SeedMessages {
                        messages: seed_messages,
                    })
                    .ok();
                info!("created session {session_id} (model: {resolved_model}, resumed {count} messages)");
            } else {
                info!("created session {session_id} (model: {resolved_model})");
            }

            ControlResponse::Created {
                session_id,
                socket_path: socket_path.to_string_lossy().into_owned(),
            }
        }

        ControlCommand::AttachSession { session_id } => {
            let mut st = state.lock().await;
            let needs_recovery = st.sessions.get(&session_id).is_some_and(|h| h.cmd_tx.is_none());

            if needs_recovery {
                match super::agent_process::recover_session(&session_id, &registry, &factory, &mut st, &shutdown) {
                    Ok(_) => {
                        let socket_path = st
                            .sessions
                            .get(&session_id)
                            .map(|h| h.socket_path.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        ControlResponse::Attached { socket_path }
                    }
                    Err(e) => ControlResponse::Error {
                        message: format!("recovery failed: {e}"),
                    },
                }
            } else if let Some(handle) = st.sessions.get(&session_id) {
                ControlResponse::Attached {
                    socket_path: handle.socket_path.to_string_lossy().into_owned(),
                }
            } else {
                ControlResponse::Error {
                    message: format!("session '{session_id}' not found"),
                }
            }
        }

        // Delegate non-creation commands to the standard handler
        other => dispatch_control_command(other, &state, &factory).await,
    };

    frame::write_frame(&mut writer, &response).await?;
    Ok(())
}

/// Dispatch non-creation control commands.
async fn dispatch_control_command(
    cmd: ControlCommand,
    state: &Arc<Mutex<DaemonState>>,
    factory: &SessionFactory,
) -> ControlResponse {
    let st = state.lock().await;
    match cmd {
        ControlCommand::ListSessions => ControlResponse::Sessions(st.session_summaries()),
        ControlCommand::Status => ControlResponse::Status(st.status()),
        ControlCommand::ProcessTree => ControlResponse::Tree(vec![]),
        ControlCommand::KillSession { session_id } => {
            if let Some(handle) = st.sessions.get(&session_id) {
                if let Some(ref tx) = handle.cmd_tx {
                    tx.send(SessionCommand::Disconnect).ok();
                }
                if let Some(ref catalog) = factory.catalog {
                    catalog.set_state(&session_id, super::session_store::SessionLifecycle::Tombstoned);
                }
                ControlResponse::Killed
            } else {
                ControlResponse::Error {
                    message: format!("session '{session_id}' not found"),
                }
            }
        }
        ControlCommand::AttachSession { .. } => {
            // Handled in caller (needs mutable state for recovery)
            ControlResponse::Error {
                message: "internal error: AttachSession routed to dispatch".to_string(),
            }
        }
        ControlCommand::CreateSession { .. } => {
            // Should not reach here — handled in the caller
            ControlResponse::Error {
                message: "internal error: CreateSession routed to dispatch".to_string(),
            }
        }
        ControlCommand::Shutdown => {
            // Trigger daemon shutdown — runs checkpoint sequence.
            // Use kill(getpid()) not raise() — raise sends to the calling
            // *thread* in multi-threaded programs, but tokio's signal handler
            // is process-level.
            unsafe {
                libc::kill(libc::getpid(), libc::SIGTERM);
            }
            ControlResponse::ShuttingDown
        }
        ControlCommand::RestartDaemon => {
            super::RESTART_REQUESTED.store(true, std::sync::atomic::Ordering::SeqCst);
            unsafe {
                libc::kill(libc::getpid(), libc::SIGTERM);
            }
            ControlResponse::Restarting
        }
        ControlCommand::ListPlugins => {
            let summaries = if let Some(ref pm) = factory.plugin_manager {
                crate::plugin::build_protocol_plugin_summaries(pm)
            } else {
                Vec::new()
            };
            ControlResponse::Plugins(summaries)
        }
    }
}

/// Drain controller events and subagent panel events, broadcasting all as DaemonEvents.
pub fn drain_and_broadcast(
    controller: &mut SessionController,
    event_tx: &broadcast::Sender<DaemonEvent>,
    panel_rx: &mut mpsc::UnboundedReceiver<SubagentEvent>,
    plugin_manager: Option<&std::sync::Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
) {
    // Drain controller events
    let events = controller.drain_events();

    // Dispatch to plugins before broadcasting (plugins may produce UI actions)
    if let Some(pm) = plugin_manager.filter(|_| !events.is_empty()) {
        let result = crate::modes::plugin_dispatch::dispatch_daemon_events_to_plugins(pm, &events);

        // Convert plugin display messages to SystemMessage events
        for (plugin_name, message) in result.messages {
            event_tx
                .send(DaemonEvent::SystemMessage {
                    text: format!("\u{1f50c} {}: {}", plugin_name, message),
                    is_error: false,
                })
                .ok();
        }

        // Convert plugin UI actions to protocol events
        for action in result.ui_actions {
            event_tx.send(crate::modes::plugin_dispatch::ui_action_to_daemon_event(action)).ok();
        }
    }

    for event in events {
        event_tx.send(event).ok();
    }

    // Drain subagent panel events → DaemonEvent
    while let Ok(panel_event) = panel_rx.try_recv() {
        let daemon_event = match panel_event {
            SubagentEvent::Started { id, name, task, pid } => DaemonEvent::SubagentStarted { id, name, task, pid },
            SubagentEvent::Output { id, line } => DaemonEvent::SubagentOutput { id, line },
            SubagentEvent::Done { id } => DaemonEvent::SubagentDone { id },
            SubagentEvent::Error { id, message } => DaemonEvent::SubagentError { id, message },
            // KillRequest and InputRequest are TUI→tool direction, not relevant here
            SubagentEvent::KillRequest { .. } | SubagentEvent::InputRequest { .. } => continue,
        };
        event_tx.send(daemon_event).ok();
    }
}

/// Resolve session resume: look up an existing session file by ID or
/// most-recent, returning seed messages to inject. Falls back to a
/// fresh session (new ID, empty messages) if nothing matches.
fn resolve_session_resume(
    resume_id: Option<&str>,
    continue_last: bool,
    cwd: Option<&str>,
) -> (String, Vec<clankers_protocol::SerializedMessage>) {
    let paths = crate::config::ClankersPaths::get();
    let sessions_dir = &paths.global_sessions_dir;
    let effective_cwd = cwd.unwrap_or(".");

    let try_open = |file: std::path::PathBuf| -> Option<(String, Vec<clankers_protocol::SerializedMessage>)> {
        let mgr = clankers_session::SessionManager::open(file).ok()?;
        let msgs = mgr.build_context().ok()?;
        let session_id = mgr.session_id().to_string();
        let serialized = msgs
            .iter()
            .map(|m| {
                let (role, content, model) = match m {
                    clankers_message::AgentMessage::User(u) => {
                        let text = u
                            .content
                            .iter()
                            .filter_map(|c| match c {
                                clankers_message::Content::Text { text } => Some(text.as_str()),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        ("user", text, None)
                    }
                    clankers_message::AgentMessage::Assistant(a) => {
                        let text = a
                            .content
                            .iter()
                            .filter_map(|c| match c {
                                clankers_message::Content::Text { text } => Some(text.as_str()),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        ("assistant", text, Some(a.model.clone()))
                    }
                    _ => {
                        return clankers_protocol::SerializedMessage {
                            role: "user".to_string(),
                            content: String::new(),
                            model: None,
                            timestamp: None,
                        };
                    }
                };
                clankers_protocol::SerializedMessage {
                    role: role.to_string(),
                    content,
                    model,
                    timestamp: None,
                }
            })
            .filter(|m| !m.content.is_empty())
            .collect();
        Some((session_id, serialized))
    };

    // Try resume by ID
    if let Some(id) = resume_id {
        let files = clankers_session::store::list_sessions(sessions_dir, effective_cwd);
        if let Some(file) =
            files.into_iter().find(|f| f.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.contains(id)))
            && let Some(result) = try_open(file)
        {
            return result;
        }
    }

    // Try continue last
    if continue_last {
        let files = clankers_session::store::list_sessions(sessions_dir, effective_cwd);
        if let Some(file) = files.into_iter().next()
            && let Some(result) = try_open(file)
        {
            return result;
        }
    }

    // New session
    (clankers_message::generate_id(), Vec::new())
}

async fn shutdown_signal(shutdown: &tokio::sync::watch::Receiver<bool>) {
    let mut rx = shutdown.clone();
    while !*rx.borrow_and_update() {
        if rx.changed().await.is_err() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::time::Duration;

    use clanker_actor::ProcessRegistry;
    use clankers_protocol::control::ControlResponse;
    use tempfile::tempdir;

    use super::*;

    struct StubProvider;

    #[async_trait::async_trait]
    impl crate::provider::Provider for StubProvider {
        async fn complete(
            &self,
            _req: crate::provider::CompletionRequest,
            _tx: tokio::sync::mpsc::Sender<crate::provider::streaming::StreamEvent>,
        ) -> std::result::Result<(), crate::provider::error::ProviderError> {
            Ok(())
        }

        fn models(&self) -> &[crate::provider::Model] {
            &[]
        }

        fn name(&self) -> &str {
            "stub"
        }
    }

    fn make_factory(
        plugin_manager: Option<Arc<Mutex<crate::plugin::PluginManager>>>,
        registry: Option<ProcessRegistry>,
    ) -> SessionFactory {
        SessionFactory {
            provider: Arc::new(StubProvider),
            tools: vec![],
            settings: crate::config::settings::Settings::default(),
            default_model: "test".to_string(),
            default_system_prompt: String::new(),
            registry,
            catalog: None,
            schedule_engine: None,
            plugin_manager,
        }
    }

    fn write_plugin_manifest(dir: &Path, name: &str, manifest: serde_json::Value) {
        let plugin_dir = dir.join(name);
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(plugin_dir.join("plugin.json"), serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
    }

    #[test]
    fn child_factory_for_actor_ctx_skips_plugin_host_and_registry() {
        let plugins_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins");
        let plugin_manager = crate::modes::common::init_plugin_manager(&plugins_dir, None, &[]);
        let parent = make_factory(Some(plugin_manager), Some(ProcessRegistry::new()));

        let child = parent.child_actor_factory().expect("child factory available when registry exists");
        assert!(child.registry.is_none());
        assert!(child.plugin_manager.is_none());

        let (parent_panel_tx, _parent_panel_rx) = mpsc::unbounded_channel();
        let parent_tools = parent.build_tools_with_panel_tx(parent_panel_tx, None);
        let parent_tool_names: Vec<String> = parent_tools.iter().map(|tool| tool.definition().name.clone()).collect();
        assert!(parent_tool_names.contains(&"test_echo".to_string()));

        let (child_panel_tx, _child_panel_rx) = mpsc::unbounded_channel();
        let child_tools = child.build_tools_with_panel_tx(child_panel_tx, None);
        let child_tool_names: Vec<String> = child_tools.iter().map(|tool| tool.definition().name.clone()).collect();
        assert!(!child_tool_names.contains(&"test_echo".to_string()));
        assert!(!child_tool_names.contains(&"test_reverse".to_string()));
        assert!(child_tool_names.contains(&"read".to_string()));
    }

    #[tokio::test]
    async fn list_plugins_control_command_returns_empty_for_empty_host() {
        let dir = tempdir().unwrap();
        let plugin_manager = crate::modes::common::init_plugin_manager_for_mode(
            dir.path(),
            None,
            &[],
            crate::plugin::PluginRuntimeMode::Daemon,
            dir.path(),
        );
        let state = Arc::new(tokio::sync::Mutex::new(DaemonState::new()));
        let factory = make_factory(Some(plugin_manager), None);

        let response = dispatch_control_command(ControlCommand::ListPlugins, &state, &factory).await;
        match response {
            ControlResponse::Plugins(plugins) => assert!(plugins.is_empty()),
            other => panic!("expected plugins response, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn list_plugins_control_command_reports_live_and_error_stdio_plugins() {
        let dir = tempdir().unwrap();
        crate::plugin::tests::stdio_runtime::write_stdio_plugin_manifest(
            dir.path(),
            "stdio-list-active",
            "ready_register",
            "daemon",
            None,
            None,
        );
        write_plugin_manifest(
            dir.path(),
            "stdio-list-invalid",
            serde_json::json!({
                "name": "stdio-list-invalid",
                "version": "0.1.0",
                "kind": "stdio",
                "stdio": {
                    "args": ["plugin.py"],
                    "sandbox": "inherit"
                }
            }),
        );

        let plugin_manager = crate::plugin::tests::stdio_runtime::init_manager_with_restart_delays(
            dir.path(),
            crate::plugin::PluginRuntimeMode::Daemon,
            "5,10,15,20,25",
        );
        crate::plugin::tests::stdio_runtime::wait_for_plugin_state(
            &plugin_manager,
            "stdio-list-active",
            Duration::from_secs(2),
            |state| matches!(state, crate::plugin::PluginState::Active),
        )
        .await;
        crate::plugin::tests::stdio_runtime::wait_for_live_tool(
            &plugin_manager,
            "stdio-list-active",
            "stdio_list_active_tool",
            Duration::from_secs(2),
        )
        .await;

        let state = Arc::new(tokio::sync::Mutex::new(DaemonState::new()));
        let factory = make_factory(Some(Arc::clone(&plugin_manager)), None);
        let response = dispatch_control_command(ControlCommand::ListPlugins, &state, &factory).await;

        match response {
            ControlResponse::Plugins(plugins) => {
                let active = plugins.iter().find(|plugin| plugin.name == "stdio-list-active").unwrap();
                assert_eq!(active.kind.as_deref(), Some("stdio"));
                assert_eq!(active.state, "Active");
                assert_eq!(active.permissions, vec!["ui".to_string()]);
                assert!(active.tools.iter().any(|tool| tool == "stdio_list_active_tool"));

                let invalid = plugins.iter().find(|plugin| plugin.name == "stdio-list-invalid").unwrap();
                assert_eq!(invalid.kind.as_deref(), Some("stdio"));
                assert_eq!(invalid.state, "Error");
                assert!(invalid.last_error.as_deref().is_some_and(|error| error.contains("stdio.command")));
            }
            other => panic!("expected plugins response, got {other:?}"),
        }

        crate::plugin::shutdown_plugin_runtime(&plugin_manager, "test shutdown").await;
    }
}
