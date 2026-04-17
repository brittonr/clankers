//! AgentProcess — wraps a SessionController as an actor in the ProcessRegistry.
//!
//! Each `AgentProcess` runs one agent session. It receives signals
//! (prompts, commands, shutdown) via the actor system and emits
//! `DaemonEvent`s to connected clients via a broadcast channel.
//!
//! Used by:
//! - Daemon session creation (replaces raw `tokio::spawn` of driver)
//! - SubagentTool (in-process subagents instead of `clankers -p`)
//! - DelegateTool (in-process workers instead of `clankers -p`)

use std::sync::Arc;

use clanker_actor::DeathReason;
use clanker_actor::ProcessId;
use clanker_actor::ProcessRegistry;
use clanker_actor::Signal;
use clankers_controller::SessionController;
use clankers_controller::config::ControllerConfig;
use clankers_protocol::DaemonEvent;
use clankers_protocol::SessionCommand;
use clankers_tui_types::SubagentEvent;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::debug;
use tracing::info;
use tracing::warn;

use super::socket_bridge::SessionFactory;

/// Maximum bytes collected from agent text output.
const MAX_COLLECTED_BYTES: usize = 512 * 1024;

/// Spawn a session controller as a named actor process.
///
/// Returns the process ID, command sender, and event broadcaster.
/// The session socket listener is NOT started here — call
/// `transport::run_session_socket` separately if clients need
/// to connect via Unix socket.
///
/// `capabilities` — if set, tool calls are checked against these UCAN
/// capabilities. `None` means full access (local sessions, root tokens).
/// Info returned from `spawn_agent_process` for wiring up channels and catalog.
pub struct SpawnedSession {
    pub pid: ProcessId,
    pub cmd_tx: mpsc::UnboundedSender<SessionCommand>,
    pub event_tx: broadcast::Sender<DaemonEvent>,
    /// Path to the automerge session file (if persistence succeeded).
    pub automerge_path: Option<std::path::PathBuf>,
}

pub fn spawn_agent_process(
    registry: &ProcessRegistry,
    factory: &SessionFactory,
    session_id: String,
    model: Option<String>,
    system_prompt: Option<String>,
    parent: Option<ProcessId>,
    capabilities: Option<Vec<clankers_ucan::Capability>>,
) -> SpawnedSession {
    let model = model.unwrap_or_else(|| factory.default_model.clone());
    let system_prompt = system_prompt.unwrap_or_else(|| factory.default_system_prompt.clone());

    // Create subagent event channel
    let (panel_tx, panel_rx) = mpsc::unbounded_channel::<SubagentEvent>();

    // Create bash confirm channel so dangerous commands can be approved
    // by attached clients via the ConfirmRequest/ConfirmBash protocol.
    let (bash_confirm_tx, bash_confirm_rx) = crate::tools::bash::confirm_channel();

    // Build tools with panel_tx for subagent event routing
    let tools = factory.build_tools_with_panel_tx(panel_tx, Some(bash_confirm_tx));

    // Build the agent, attaching capability gate from UCAN token and/or settings.
    //
    // Three cases:
    //   1. UCAN caps only (remote peer)  → gate from token
    //   2. Settings caps only (local)    → gate from defaultCapabilities
    //   3. Both                          → merge (both sets must authorize)
    //   4. Neither                       → no gate, full access
    let effective_caps = merge_capabilities(
        capabilities.as_deref(),
        factory.settings.default_capabilities.as_deref(),
    );

    let mut builder = crate::agent::builder::AgentBuilder::new(
        Arc::clone(&factory.provider),
        factory.settings.clone(),
        model.clone(),
        system_prompt.clone(),
    )
    .with_tools(tools);

    if let Some(caps) = &effective_caps {
        let gate = std::sync::Arc::new(crate::capability_gate::UcanCapabilityGate::new(caps.clone()));
        builder = builder.with_capability_gate(gate);
    }

    let agent = builder.build();

    let tool_patterns = effective_caps.as_deref().and_then(crate::capability_gate::extract_tool_patterns);

    // ── Session persistence ──────────────────────────────────────────
    // Create a SessionManager so daemon sessions get JSONL persistence
    // (same as standalone mode). Without this, conversation history is
    // lost when the daemon stops.
    let (session_manager, automerge_path) = {
        let paths = crate::config::ClankersPaths::get();
        let cwd = std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        match clankers_session::SessionManager::create(
            &paths.global_sessions_dir,
            &cwd,
            &model,
            None,
            None,
            None,
        ) {
            Ok(mgr) => {
                let path = mgr.file_path().to_path_buf();
                info!("session {session_id}: persistence enabled at {path:?}");
                (Some(mgr), Some(path))
            }
            Err(e) => {
                warn!("session {session_id}: failed to create session file: {e}");
                (None, None)
            }
        }
    };

    // ── Hook pipeline ────────────────────────────────────────────────
    let hook_pipeline = build_session_hook_pipeline(&factory.settings, factory.plugin_manager.as_ref());

    let config = ControllerConfig {
        session_id: session_id.clone(),
        model: model.clone(),
        system_prompt: Some(system_prompt),
        capabilities: tool_patterns.clone(),
        capability_ceiling: tool_patterns,
        session_manager,
        hook_pipeline,
        auto_test_command: factory.settings.auto_test_command.clone(),
        auto_test_enabled: factory.settings.auto_test_command.is_some(),
    };

    let mut controller = SessionController::new(agent, config);

    // Wire tool rebuilder so SetDisabledTools can hot-reload the agent's tools.
    let rebuilder = DaemonToolRebuilder {
        factory: Arc::new(SessionFactory {
            provider: Arc::clone(&factory.provider),
            tools: factory.tools.clone(),
            settings: factory.settings.clone(),
            default_model: factory.default_model.clone(),
            default_system_prompt: factory.default_system_prompt.clone(),
            registry: None, // child tools use subprocess fallback
            catalog: None,
            schedule_engine: factory.schedule_engine.clone(),
            plugin_manager: factory.plugin_manager.clone(),
        }),
    };
    controller.set_tool_rebuilder(Arc::new(rebuilder));

    // Create channels for external command/event access
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<SessionCommand>();
    let (event_tx, _) = broadcast::channel::<DaemonEvent>(256);

    let driver_event_tx = event_tx.clone();
    let driver_plugin_manager = factory.plugin_manager.clone();
    let process_name = format!("session:{session_id}");

    let pid = registry.spawn(
        Some(process_name),
        parent,
        move |_pid, mut signal_rx| async move {
            Box::pin(run_agent_actor(
                controller,
                cmd_rx,
                driver_event_tx,
                session_id,
                panel_rx,
                bash_confirm_rx,
                &mut signal_rx,
                driver_plugin_manager,
            ))
            .await
        },
    );

    SpawnedSession { pid, cmd_tx, event_tx, automerge_path }
}

/// The actor loop: multiplexes session commands, actor signals,
/// bash confirm requests, and periodic event draining.
async fn run_agent_actor(
    mut controller: SessionController,
    mut cmd_rx: mpsc::UnboundedReceiver<SessionCommand>,
    event_tx: broadcast::Sender<DaemonEvent>,
    session_id: String,
    mut panel_rx: mpsc::UnboundedReceiver<SubagentEvent>,
    mut bash_confirm_rx: crate::tools::bash::ConfirmRx,
    signal_rx: &mut mpsc::UnboundedReceiver<Signal>,
    plugin_manager: Option<std::sync::Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
) -> DeathReason {
    info!("agent process started: {session_id}");

    // Fire plugin_init so plugins can set up initial UI state
    if let Some(ref pm) = plugin_manager {
        for action in crate::modes::common::fire_plugin_init(pm) {
            event_tx.send(crate::modes::plugin_dispatch::ui_action_to_daemon_event(action)).ok();
        }
    }

    loop {
        tokio::select! {
            // Actor signals (Kill, Shutdown, LinkDied, etc.)
            signal = signal_rx.recv() => {
                match signal {
                    Some(Signal::Kill) => {
                        info!("agent process killed: {session_id}");
                        controller.shutdown().await;
                        return DeathReason::Killed;
                    }
                    Some(Signal::Shutdown { .. }) => {
                        info!("agent process shutting down: {session_id}");
                        controller.shutdown().await;
                        return DeathReason::Shutdown;
                    }
                    Some(Signal::LinkDied { process_id, reason, .. }) => {
                        debug!("agent process {session_id}: linked process {process_id} died: {reason}");
                        // Child died — not fatal for the session itself
                    }
                    None => {
                        // Registry dropped — clean exit
                        controller.shutdown().await;
                        return DeathReason::Normal;
                    }
                    _ => {}
                }
            }

            // Session commands from clients
            cmd = cmd_rx.recv() => {
                let Some(cmd) = cmd else { break };
                let is_disconnect = matches!(cmd, SessionCommand::Disconnect);

                // Handle plugin queries locally (controller doesn't know about plugins)
                if matches!(cmd, SessionCommand::GetPlugins) {
                    let summaries = build_plugin_summaries(plugin_manager.as_ref());
                    event_tx.send(DaemonEvent::PluginList { plugins: summaries }).ok();
                    if is_disconnect { break; }
                    continue;
                }

                controller.handle_command(cmd).await;
                super::socket_bridge::drain_and_broadcast(
                    &mut controller, &event_tx, &mut panel_rx,
                    plugin_manager.as_ref(),
                );

                // Post-prompt actions (auto-test, loop continuation)
                if !controller.is_busy() {
                    if let Some(auto_prompt) = controller.maybe_auto_test() {
                        controller
                            .handle_command(SessionCommand::Prompt {
                                text: auto_prompt,
                                images: vec![],
                            })
                            .await;
                        super::socket_bridge::drain_and_broadcast(
                            &mut controller, &event_tx, &mut panel_rx,
                            plugin_manager.as_ref(),
                        );
                    }
                    controller.clear_auto_test();
                }

                if is_disconnect { break; }
            }

            // Bash tool requesting confirmation for a dangerous command.
            // Bridge it into the protocol: register in ConfirmStore, emit
            // ConfirmRequest, and spawn a task that forwards the client's
            // response back to the bash tool's oneshot.
            Some(req) = bash_confirm_rx.recv() => {
                let cwd = std::env::current_dir()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                let (request_id, confirm_result_rx) =
                    controller.register_bash_confirm();
                event_tx.send(DaemonEvent::ConfirmRequest {
                    request_id,
                    command: req.command,
                    working_dir: cwd,
                }).ok();
                // Bridge ConfirmStore's oneshot → bash tool's oneshot.
                // Times out after 60s with no client response → command blocked.
                tokio::spawn(async move {
                    match tokio::time::timeout(
                        tokio::time::Duration::from_secs(60),
                        confirm_result_rx,
                    )
                    .await
                    {
                        Ok(Ok(approved)) => {
                            req.resp_tx.send(approved).ok();
                        }
                        _ => {
                            // Timeout or channel dropped — block the command
                            req.resp_tx.send(false).ok();
                        }
                    }
                });
            }

            // Periodic drain of background agent events
            () = tokio::time::sleep(tokio::time::Duration::from_millis(50)) => {
                super::socket_bridge::drain_and_broadcast(
                    &mut controller, &event_tx, &mut panel_rx,
                    plugin_manager.as_ref(),
                );
            }
        }
    }

    controller.shutdown().await;
    info!("agent process stopped: {session_id}");
    DeathReason::Normal
}

// ── Ephemeral agent runner ──────────────────────────────────────────────────

/// Spawn an in-process agent, send a single prompt, collect text output,
/// and return when the agent finishes.
///
/// Used by SubagentTool and DelegateTool in daemon mode instead of
/// forking a subprocess.
///
/// `panel_tx` receives `SubagentEvent`s for TUI streaming.
/// `signal` cancels the agent on parent abort.
/// `parent_capabilities` — if set, child capabilities are clamped to this set.
pub async fn run_ephemeral_agent(
    registry: &ProcessRegistry,
    factory: &SessionFactory,
    task: &str,
    agent_def: Option<&str>,
    parent_pid: Option<ProcessId>,
    panel_tx: Option<&mpsc::UnboundedSender<SubagentEvent>>,
    sub_id: &str,
    signal: CancellationToken,
) -> Result<String, String> {
    let session_id = clankers_message::generate_id();

    // Resolve agent definition to model + system prompt overrides
    let (model, system_prompt) = resolve_agent_def(agent_def, factory);

    let spawned = spawn_agent_process(
        registry,
        factory,
        session_id.clone(),
        model,
        system_prompt,
        parent_pid,
        None, // ephemeral agents inherit parent's capabilities via actor links
    );
    let pid = spawned.pid;
    let cmd_tx = spawned.cmd_tx;
    let event_tx = spawned.event_tx;

    let mut event_rx = event_tx.subscribe();

    // Send the prompt
    cmd_tx.send(SessionCommand::Prompt {
        text: task.to_string(),
        images: vec![],
    }).ok();

    // Collect text from DaemonEvent stream
    let mut collected = String::new();

    loop {
        tokio::select! {
            event = event_rx.recv() => {
                match event {
                    Ok(DaemonEvent::TextDelta { text, .. }) => {
                        if let Some(tx) = panel_tx {
                            tx.send(SubagentEvent::Output {
                                id: sub_id.to_string(),
                                line: text.clone(),
                            }).ok();
                        }
                        if collected.len() < MAX_COLLECTED_BYTES {
                            collected.push_str(&text);
                        }
                    }
                    Ok(DaemonEvent::AgentEnd) => {
                        if let Some(tx) = panel_tx {
                            tx.send(SubagentEvent::Done {
                                id: sub_id.to_string(),
                            }).ok();
                        }
                        break;
                    }
                    Ok(DaemonEvent::PromptDone { error: Some(msg) }) => {
                        if let Some(tx) = panel_tx {
                            tx.send(SubagentEvent::Error {
                                id: sub_id.to_string(),
                                message: msg.clone(),
                            }).ok();
                        }
                        cmd_tx.send(SessionCommand::Disconnect).ok();
                        return Err(msg);
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("ephemeral agent {session_id}: skipped {n} events");
                    }
                    _ => {}
                }
            }
            () = signal.cancelled() => {
                // Parent cancelled — kill the agent actor
                registry.send(pid, Signal::Kill);
                if let Some(tx) = panel_tx {
                    tx.send(SubagentEvent::Error {
                        id: sub_id.to_string(),
                        message: "Cancelled".into(),
                    }).ok();
                }
                return Err("Cancelled".to_string());
            }
        }
    }

    // Disconnect cleanly
    cmd_tx.send(SessionCommand::Disconnect).ok();

    Ok(collected)
}

// ── Prompt-and-collect for non-TUI consumers ────────────────────────────────

/// Maximum bytes collected from agent text output for prompt_and_collect.
const MAX_PROMPT_COLLECT_BYTES: usize = 512 * 1024;

/// Send a prompt to an actor session and collect the full text response.
///
/// Subscribes to the session's broadcast channel, sends the prompt via
/// `cmd_tx`, and collects `DaemonEvent::TextDelta` until `AgentEnd` or
/// `PromptDone`. Used by chat/1 and Matrix bridge instead of the old
/// clone-seed-prompt pattern.
///
/// If `update_last_active` is false (proactive prompts like heartbeats),
/// the session handle's timestamp is not updated, so idle reaping still
/// works correctly.
#[cfg_attr(dylint_lib = "tigerstyle", allow(unbounded_loop, reason = "event loop; bounded by channel close"))]
pub async fn prompt_and_collect(
    cmd_tx: &tokio::sync::mpsc::UnboundedSender<SessionCommand>,
    event_tx: &tokio::sync::broadcast::Sender<DaemonEvent>,
    text: String,
    images: Vec<clankers_protocol::ImageData>,
) -> String {
    let mut event_rx = event_tx.subscribe();

    // Send the prompt
    if cmd_tx
        .send(SessionCommand::Prompt {
            text,
            images,
        })
        .is_err()
    {
        return String::new();
    }

    // Collect text until the agent finishes
    let mut collected = String::new();
    loop {
        match event_rx.recv().await {
            Ok(DaemonEvent::TextDelta { text, .. }) => {
                if collected.len() < MAX_PROMPT_COLLECT_BYTES {
                    collected.push_str(&text);
                }
            }
            Ok(DaemonEvent::AgentEnd) => break,
            Ok(DaemonEvent::PromptDone { .. }) => break,
            Err(broadcast::error::RecvError::Closed) => break,
            Err(broadcast::error::RecvError::Lagged(n)) => {
                warn!("prompt_and_collect: skipped {n} events");
            }
            _ => {}
        }
    }

    collected
}

fn load_recovery_seed_messages(
    entry: &super::session_store::SessionCatalogEntry,
) -> Vec<clankers_protocol::SerializedMessage> {
    if entry.automerge_path.exists() {
        match clankers_session::SessionManager::open(entry.automerge_path.clone()) {
            Ok(mgr) => match mgr.build_context() {
                Ok(msgs) => {
                    let serialized: Vec<_> = msgs
                        .iter()
                        .filter_map(|m| {
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
                                _ => return None,
                            };
                            if content.is_empty() {
                                return None;
                            }
                            Some(clankers_protocol::SerializedMessage {
                                role: role.to_string(),
                                content,
                                model,
                                timestamp: None,
                            })
                        })
                        .collect();
                    info!(
                        "loaded {} recovery messages from {:?}",
                        serialized.len(),
                        entry.automerge_path
                    );
                    serialized
                }
                Err(e) => {
                    warn!(
                        "failed to build recovery context from {:?}: {e} — starting fresh",
                        entry.automerge_path
                    );
                    Vec::new()
                }
            },
            Err(e) => {
                warn!(
                    "failed to open recovery automerge at {:?}: {e} — starting fresh",
                    entry.automerge_path
                );
                Vec::new()
            }
        }
    } else {
        warn!(
            "recovery automerge file missing at {:?} — starting fresh",
            entry.automerge_path
        );
        Vec::new()
    }
}

/// Get or create an actor session for a transport key.
///
/// Looks up the session in `DaemonState` by key. If not found, spawns a
/// new actor process via `spawn_agent_process`, registers it in the state
/// with the key index, and returns the session's command/event channels.
///
/// `capabilities` — UCAN capabilities for the session (None = full access).
/// Only used when creating a new session; ignored if session already exists.
///
/// Returns `(session_id, cmd_tx, event_tx)`.
pub async fn get_or_create_keyed_session(
    state: &tokio::sync::Mutex<clankers_controller::transport::DaemonState>,
    registry: &clanker_actor::ProcessRegistry,
    factory: &super::socket_bridge::SessionFactory,
    key: &clankers_protocol::SessionKey,
    capabilities: Option<Vec<clankers_ucan::Capability>>,
) -> (
    String,
    tokio::sync::mpsc::UnboundedSender<SessionCommand>,
    tokio::sync::broadcast::Sender<DaemonEvent>,
) {
    let mut suspended_session_id = None;

    // Fast path: session already exists. If the key points at a suspended
    // placeholder, remember it so we can revive the existing session instead
    // of silently forking a fresh one.
    {
        let st = state.lock().await;
        if let Some(handle) = st.session_by_key(key) {
            if let Some(ref cmd_tx) = handle.cmd_tx
                && let Some(ref event_tx) = handle.event_tx
            {
                return (
                    handle.session_id.clone(),
                    cmd_tx.clone(),
                    event_tx.clone(),
                );
            }
            suspended_session_id = Some(handle.session_id.clone());
        }
    }

    if let Some(session_id) = suspended_session_id
        && let Some(catalog) = factory.catalog.as_ref()
        && let Some(entry) = catalog.get_session(&session_id)
    {
        let seed_messages = load_recovery_seed_messages(&entry);
        let spawned = spawn_agent_process(
            registry,
            factory,
            session_id.clone(),
            Some(entry.model.clone()),
            None,
            None,
            None,
        );
        let cmd_tx = spawned.cmd_tx;
        let event_tx = spawned.event_tx;

        if !seed_messages.is_empty() {
            cmd_tx
                .send(SessionCommand::SeedMessages {
                    messages: seed_messages,
                })
                .ok();
        }

        {
            let mut st = state.lock().await;
            if let Some(handle) = st.sessions.get_mut(&session_id) {
                handle.model = entry.model.clone();
                handle.cmd_tx = Some(cmd_tx.clone());
                handle.event_tx = Some(event_tx.clone());
                handle.state = "active".to_string();
            }
        }

        catalog.set_state(&session_id, super::session_store::SessionLifecycle::Active);
        info!("recovered keyed session {} for {}", session_id, key);
        return (session_id, cmd_tx, event_tx);
    }

    // Slow path: create a new session
    let session_id = clankers_message::generate_id();
    let spawned = spawn_agent_process(
        registry,
        factory,
        session_id.clone(),
        None,
        None,
        None,
        capabilities,
    );
    let cmd_tx = spawned.cmd_tx;
    let event_tx = spawned.event_tx;

    let socket_path =
        clankers_controller::transport::session_socket_path(&session_id);

    {
        let mut st = state.lock().await;
        st.sessions.insert(
            session_id.clone(),
            clankers_controller::transport::SessionHandle {
                session_id: session_id.clone(),
                model: factory.default_model.clone(),
                turn_count: 0,
                last_active: chrono::Utc::now().to_rfc3339(),
                client_count: 0,
                cmd_tx: Some(cmd_tx.clone()),
                event_tx: Some(event_tx.clone()),
                socket_path,
                state: "active".to_string(),
            },
        );
        st.register_key(key.clone(), session_id.clone());
    }

    // Write catalog entry + key mapping
    if let Some(ref catalog) = factory.catalog {
        let now = chrono::Utc::now().to_rfc3339();
        catalog.insert_session(&super::session_store::SessionCatalogEntry {
            session_id: session_id.clone(),
            automerge_path: spawned.automerge_path.clone().unwrap_or_default(),
            model: factory.default_model.clone(),
            created_at: now.clone(),
            last_active: now,
            turn_count: 0,
            state: super::session_store::SessionLifecycle::Active,
        });
        catalog.insert_key(key, &session_id);
    }

    info!("created keyed session {} for {}", session_id, key);
    (session_id, cmd_tx, event_tx)
}

/// Lazily recover a suspended session: open the automerge file, spawn
/// an actor, seed its messages, and upgrade the placeholder handle.
///
/// Returns `(cmd_tx, event_tx)` on success, or an error message.
#[cfg_attr(dylint_lib = "tigerstyle", allow(nested_conditionals, reason = "complex control flow — extracting helpers would obscure logic"))]
pub fn recover_session(
    session_id: &str,
    registry: &ProcessRegistry,
    factory: &super::socket_bridge::SessionFactory,
    state: &mut clankers_controller::transport::DaemonState,
    shutdown: &tokio::sync::watch::Receiver<bool>,
) -> Result<
    (mpsc::UnboundedSender<SessionCommand>, broadcast::Sender<DaemonEvent>),
    String,
> {
    // Look up catalog entry
    let catalog = factory.catalog.as_ref().ok_or("no session catalog")?;
    let entry = catalog.get_session(session_id)
        .ok_or_else(|| format!("session '{session_id}' not in catalog"))?;

    // Load seed messages from automerge
    let seed_messages = load_recovery_seed_messages(&entry);

    // Spawn the actor
    let spawned = spawn_agent_process(
        registry,
        factory,
        session_id.to_string(),
        Some(entry.model.clone()),
        None,
        None,
        None,
    );
    let cmd_tx = spawned.cmd_tx;
    let event_tx = spawned.event_tx;

    // Seed messages
    if !seed_messages.is_empty() {
        cmd_tx.send(SessionCommand::SeedMessages { messages: seed_messages }).ok();
    }

    // Start session socket
    let _socket_path = clankers_controller::transport::session_socket_path(session_id);
    let sock_shutdown = shutdown.clone();
    let sock_cmd_tx = cmd_tx.clone();
    let sock_event_tx = event_tx.clone();
    let sock_session_id = session_id.to_string();
    tokio::spawn(async move {
        clankers_controller::transport::run_session_socket(
            sock_session_id,
            sock_cmd_tx,
            sock_event_tx,
            sock_shutdown,
        ).await;
    });

    // Upgrade placeholder handle
    if let Some(handle) = state.sessions.get_mut(session_id) {
        handle.cmd_tx = Some(cmd_tx.clone());
        handle.event_tx = Some(event_tx.clone());
        handle.state = "active".to_string();
    }

    // Update catalog
    catalog.set_state(session_id, super::session_store::SessionLifecycle::Active);

    info!("Recovered session {session_id}");
    Ok((cmd_tx, event_tx))
}

/// Resolve agent definition name to (model, system_prompt) overrides.
fn resolve_agent_def(
    agent_def: Option<&str>,
    _factory: &SessionFactory,
) -> (Option<String>, Option<String>) {
    let Some(name) = agent_def else {
        return (None, None);
    };

    let paths = clankers_config::paths::ClankersPaths::resolve();
    let cwd = std::env::current_dir().unwrap_or_default();
    let project_paths = clankers_config::paths::ProjectPaths::resolve(&cwd);

    let registry = crate::agent_defs::discovery::discover_agents(
        &paths.global_agents_dir,
        Some(&project_paths.agents_dir),
        &crate::agent_defs::definition::AgentScope::Both,
    );

    if let Some(def) = registry.get(name) {
        (def.model.clone(), Some(def.system_prompt.clone()))
    } else {
        debug!("agent definition '{name}' not found, using defaults");
        (None, None)
    }
}

/// Tool rebuilder that uses the daemon's SessionFactory to rebuild
/// the filtered tool set when disabled tools change.
struct DaemonToolRebuilder {
    factory: Arc<SessionFactory>,
}

impl clankers_controller::ToolRebuilder for DaemonToolRebuilder {
    fn rebuild_filtered(&self, disabled: &[String]) -> Vec<Arc<dyn crate::tools::Tool>> {
        let disabled_set: std::collections::HashSet<&str> =
            disabled.iter().map(|s| s.as_str()).collect();
        // Build a fresh panel_tx (events go nowhere — we only need the tool list)
        let (panel_tx, _) = tokio::sync::mpsc::unbounded_channel();
        let all_tools = self.factory.build_tools_with_panel_tx(panel_tx, None);
        all_tools
            .into_iter()
            .filter(|t| !disabled_set.contains(t.definition().name.as_str()))
            .collect()
    }
}

/// Build plugin summaries from the shared plugin manager for protocol responses.
fn build_plugin_summaries(
    plugin_manager: Option<&std::sync::Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
) -> Vec<clankers_protocol::PluginSummary> {
    let Some(pm) = plugin_manager else {
        return Vec::new();
    };
    crate::plugin::build_protocol_plugin_summaries(pm)
}

/// Build a hook pipeline for a daemon session from settings.
///
/// Includes plugin hooks when a plugin manager is provided.
#[cfg_attr(dylint_lib = "tigerstyle", allow(unbounded_loop, reason = "event loop; bounded by channel close"))]
fn build_session_hook_pipeline(
    settings: &crate::config::settings::Settings,
    plugin_manager: Option<&std::sync::Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
) -> Option<std::sync::Arc<clankers_hooks::HookPipeline>> {
    if !settings.hooks.enabled {
        return None;
    }

    let cwd = std::env::current_dir().unwrap_or_default();
    let mut pipeline = clankers_hooks::HookPipeline::new();
    pipeline.set_disabled_hooks(settings.hooks.disabled_hooks.iter().cloned());

    // Script hooks
    let hooks_dir = settings.hooks.resolve_hooks_dir(&cwd);
    let timeout = std::time::Duration::from_secs(settings.hooks.script_timeout_secs);
    pipeline.register(std::sync::Arc::new(
        clankers_hooks::script::ScriptHookHandler::new(hooks_dir, timeout),
    ));

    // Git hooks
    if settings.hooks.manage_git_hooks {
        let mut current = cwd.as_path();
        loop {
            if current.join(".git").exists() {
                pipeline.register(std::sync::Arc::new(
                    clankers_hooks::git::GitHookHandler::new(current.to_path_buf()),
                ));
                break;
            }
            match current.parent() {
                Some(p) => current = p,
                None => break,
            }
        }
    }

    // Plugin hooks
    if let Some(pm) = plugin_manager {
        pipeline.register(std::sync::Arc::new(
            crate::plugin::hooks::PluginHookHandler::new(std::sync::Arc::clone(pm)),
        ));
    }

    Some(std::sync::Arc::new(pipeline))
}

/// Merge UCAN token capabilities with settings default_capabilities.
///
/// When both are present, the settings caps act as an outer boundary:
/// only UCAN capabilities that the settings also authorize are kept.
/// When only one source is present, use it. When neither, return None.
fn merge_capabilities(
    ucan_caps: Option<&[clankers_ucan::Capability]>,
    settings_caps: Option<&[clankers_ucan::Capability]>,
) -> Option<Vec<clankers_ucan::Capability>> {
    match (ucan_caps, settings_caps) {
        (None, None) => None,
        (Some(u), None) => Some(u.to_vec()),
        (None, Some(s)) => Some(s.to_vec()),
        (Some(u), Some(s)) => {
            // Both present — intersect. Keep only UCAN caps that the
            // settings also contain (settings is the outer boundary).
            let filtered: Vec<clankers_ucan::Capability> = u
                .iter()
                .filter(|cap| s.iter().any(|sc| sc.contains(cap)))
                .cloned()
                .collect();
            if filtered.is_empty() {
                // UCAN token has no capabilities that settings allow.
                // Return an empty set so the gate blocks everything.
                Some(Vec::new())
            } else {
                Some(filtered)
            }
        }
    }
}

#[cfg(test)]
mod factory_plugin_tests {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::Mutex;

    use super::SessionFactory;

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
    ) -> SessionFactory {
        SessionFactory {
            provider: Arc::new(StubProvider),
            tools: vec![],
            settings: crate::config::settings::Settings::default(),
            default_model: "test".to_string(),
            default_system_prompt: String::new(),
            registry: None,
            catalog: None,
            schedule_engine: None,
            plugin_manager,
        }
    }

    #[test]
    fn factory_with_plugins_returns_plugin_tools() {
        let plugins_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins");
        let pm = crate::modes::common::init_plugin_manager(&plugins_dir, None, &[]);
        let factory = make_factory(Some(pm));
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let tools = factory.build_tools_with_panel_tx(tx, None);
        let names: Vec<String> = tools.iter().map(|t| t.definition().name.clone()).collect();
        assert!(names.contains(&"test_echo".to_string()), "Should have test_echo, got: {names:?}");
        assert!(names.contains(&"test_reverse".to_string()), "Should have test_reverse, got: {names:?}");
    }

    #[test]
    fn factory_without_plugins_returns_only_builtins() {
        let factory = make_factory(None);
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let tools = factory.build_tools_with_panel_tx(tx, None);
        let names: Vec<String> = tools.iter().map(|t| t.definition().name.clone()).collect();
        assert!(!names.contains(&"test_echo".to_string()), "Should not have test_echo: {names:?}");
        assert!(!names.contains(&"hash_text".to_string()), "Should not have hash_text: {names:?}");
        // Should still have built-in tools
        assert!(names.contains(&"read".to_string()), "Should have read tool: {names:?}");
    }

    #[test]
    fn daemon_tool_rebuilder_filters_plugin_tools() {
        let plugins_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins");
        let pm = crate::modes::common::init_plugin_manager(&plugins_dir, None, &[]);
        let factory = Arc::new(make_factory(Some(pm)));
        let rebuilder = super::DaemonToolRebuilder { factory };
        use clankers_controller::ToolRebuilder;

        // All tools present when nothing disabled
        let all = rebuilder.rebuild_filtered(&[]);
        let all_names: Vec<String> = all.iter().map(|t| t.definition().name.clone()).collect();
        assert!(all_names.contains(&"test_echo".to_string()));

        // Disable test_echo — should be filtered out
        let filtered = rebuilder.rebuild_filtered(&["test_echo".to_string()]);
        let filtered_names: Vec<String> = filtered.iter().map(|t| t.definition().name.clone()).collect();
        assert!(!filtered_names.contains(&"test_echo".to_string()), "test_echo should be filtered out");
        assert!(filtered_names.contains(&"test_reverse".to_string()), "test_reverse should remain");
    }
}

#[cfg(test)]
mod merge_tests {
    use super::*;
    use clankers_ucan::Capability;

    #[test]
    fn neither_source_gives_none() {
        assert!(merge_capabilities(None, None).is_none());
    }

    #[test]
    fn ucan_only() {
        let ucan = vec![Capability::ToolUse {
            tool_pattern: "read".to_string(),
        }];
        let result = merge_capabilities(Some(&ucan), None).unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn settings_only() {
        let settings = vec![Capability::ToolUse {
            tool_pattern: "read,bash".to_string(),
        }];
        let result = merge_capabilities(None, Some(&settings)).unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn both_intersects() {
        // Settings allow read,bash,grep; UCAN token grants read,write
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
        let result = merge_capabilities(Some(&ucan), Some(&settings)).unwrap();
        // Only "read" survives — settings don't contain "write"
        assert_eq!(result.len(), 1);
        assert!(matches!(&result[0], Capability::ToolUse { tool_pattern } if tool_pattern == "read"));
    }

    #[test]
    fn both_no_overlap_gives_empty() {
        let settings = vec![Capability::ToolUse {
            tool_pattern: "read".to_string(),
        }];
        let ucan = vec![Capability::ToolUse {
            tool_pattern: "bash".to_string(),
        }];
        let result = merge_capabilities(Some(&ucan), Some(&settings)).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn settings_wildcard_passes_all_ucan() {
        let settings = vec![Capability::ToolUse {
            tool_pattern: "*".to_string(),
        }];
        let ucan = vec![
            Capability::ToolUse {
                tool_pattern: "read".to_string(),
            },
            Capability::ToolUse {
                tool_pattern: "bash".to_string(),
            },
        ];
        let result = merge_capabilities(Some(&ucan), Some(&settings)).unwrap();
        assert_eq!(result.len(), 2);
    }
}
