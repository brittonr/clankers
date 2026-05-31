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

use std::collections::VecDeque;
use std::sync::Arc;

use clanker_actor::DeathReason;
use clanker_actor::ProcessId;
use clanker_actor::ProcessRegistry;
use clanker_actor::Signal;
use clanker_tui_types::SubagentEvent;
use clankers_controller::SessionController;
use clankers_controller::config::ControllerConfig;
use clankers_protocol::DaemonEvent;
use clankers_protocol::SessionCommand;
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
/// `capabilities` — if set, tool calls are checked against legacy Clankers
/// UCAN capabilities. `public_auth` installs the public UCAN + Basalt gate.
/// Both `None` means full access (local sessions).
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
    public_auth: Option<crate::capability_gate::PublicUcanToolAuthorization>,
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
    let effective_caps = merge_capabilities(capabilities.as_deref(), factory.settings.default_capabilities.as_deref());

    let mut builder = clankers_agent::builder::AgentBuilder::new(
        Arc::clone(&factory.provider),
        factory.settings.clone(),
        model.clone(),
        system_prompt.clone(),
    )
    .with_tools(tools);

    if let Some(public_auth) = public_auth {
        let gate = std::sync::Arc::new(crate::capability_gate::PublicUcanCapabilityGate::new(public_auth));
        builder = builder.with_capability_gate(gate);
    } else if let Some(caps) = &effective_caps {
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
        let paths = clankers_config::ClankersPaths::get();
        let cwd = std::env::current_dir().unwrap_or_default().to_string_lossy().into_owned();
        match clankers_session::SessionManager::create(&paths.global_sessions_dir, &cwd, &model, None, None, None) {
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
        initial_thinking_level: crate::modes::common::core_thinking_level(factory.settings.parsed_thinking_level()),
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

    let pid = registry.spawn(Some(process_name), parent, move |_pid, mut signal_rx| async move {
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
    });

    SpawnedSession {
        pid,
        cmd_tx,
        event_tx,
        automerge_path,
    }
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
    plugin_manager: Option<std::sync::Arc<std::sync::Mutex<clankers_plugin::PluginManager>>>,
) -> DeathReason {
    info!("agent process started: {session_id}");

    let mut pending_commands = VecDeque::new();

    // Fire plugin_init so plugins can set up initial UI state
    if let Some(ref pm) = plugin_manager {
        for action in crate::modes::common::fire_plugin_init(pm) {
            event_tx.send(crate::modes::plugin_dispatch::ui_action_to_daemon_event(action)).ok();
        }
        drain_plugin_runtime_events(&event_tx, Some(pm));
    }

    loop {
        if let Some(cmd) = pending_commands.pop_front() {
            if handle_actor_session_command(
                &mut controller,
                cmd,
                &mut cmd_rx,
                &event_tx,
                &mut panel_rx,
                plugin_manager.as_ref(),
                &mut pending_commands,
            )
            .await
            {
                break;
            }
            continue;
        }

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
                if handle_actor_session_command(
                    &mut controller,
                    cmd,
                    &mut cmd_rx,
                    &event_tx,
                    &mut panel_rx,
                    plugin_manager.as_ref(),
                    &mut pending_commands,
                )
                .await
                {
                    break;
                }
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
                sync_tool_inventory(&mut controller, &event_tx);
                super::socket_bridge::drain_and_broadcast(
                    &mut controller, &event_tx, &mut panel_rx,
                    plugin_manager.as_ref(),
                );
                drain_plugin_runtime_events(&event_tx, plugin_manager.as_ref());
            }
        }
    }

    controller.shutdown().await;
    info!("agent process stopped: {session_id}");
    DeathReason::Normal
}

async fn handle_actor_session_command(
    controller: &mut SessionController,
    cmd: SessionCommand,
    cmd_rx: &mut mpsc::UnboundedReceiver<SessionCommand>,
    event_tx: &broadcast::Sender<DaemonEvent>,
    panel_rx: &mut mpsc::UnboundedReceiver<SubagentEvent>,
    plugin_manager: Option<&Arc<std::sync::Mutex<clankers_plugin::PluginManager>>>,
    pending_commands: &mut VecDeque<SessionCommand>,
) -> bool {
    let is_disconnect = matches!(cmd, SessionCommand::Disconnect);

    // Handle plugin queries locally (controller doesn't know about plugins).
    if matches!(cmd, SessionCommand::GetPlugins) {
        let summaries = build_plugin_summaries(plugin_manager);
        event_tx.send(DaemonEvent::PluginList { plugins: summaries }).ok();
        return is_disconnect;
    }

    let tools_before = controller.current_tool_infos();
    handle_controller_command_with_interrupts(
        controller,
        cmd,
        cmd_rx,
        event_tx,
        panel_rx,
        plugin_manager,
        pending_commands,
    )
    .await;
    super::socket_bridge::drain_and_broadcast(controller, event_tx, panel_rx, plugin_manager);
    let tools_after = controller.current_tool_infos();
    if tools_after != tools_before {
        event_tx.send(DaemonEvent::ToolList { tools: tools_after }).ok();
    }
    drain_plugin_runtime_events(event_tx, plugin_manager);

    // Post-prompt actions (auto-test, loop continuation).
    if !controller.is_busy() {
        if let Some(auto_prompt) = controller.maybe_auto_test() {
            handle_controller_command_with_interrupts(
                controller,
                SessionCommand::Prompt {
                    text: auto_prompt,
                    images: vec![],
                },
                cmd_rx,
                event_tx,
                panel_rx,
                plugin_manager,
                pending_commands,
            )
            .await;
            super::socket_bridge::drain_and_broadcast(controller, event_tx, panel_rx, plugin_manager);
        }
        controller.clear_auto_test();
    }

    is_disconnect
}

async fn handle_controller_command_with_interrupts(
    controller: &mut SessionController,
    cmd: SessionCommand,
    cmd_rx: &mut mpsc::UnboundedReceiver<SessionCommand>,
    event_tx: &broadcast::Sender<DaemonEvent>,
    panel_rx: &mut mpsc::UnboundedReceiver<SubagentEvent>,
    plugin_manager: Option<&Arc<std::sync::Mutex<clankers_plugin::PluginManager>>>,
    pending_commands: &mut VecDeque<SessionCommand>,
) {
    let cancel_token = if is_prompt_command(&cmd) {
        controller.current_cancel_token()
    } else {
        None
    };

    let mut broadcast_stream_events = |events| {
        super::socket_bridge::broadcast_events(events, event_tx, panel_rx, plugin_manager);
    };
    let command_future = controller.handle_command_with_streaming_events(cmd, &mut broadcast_stream_events);
    tokio::pin!(command_future);

    loop {
        tokio::select! {
            () = &mut command_future => break,
            maybe_cmd = cmd_rx.recv(), if cancel_token.is_some() => {
                let Some(next_cmd) = maybe_cmd else {
                    if let Some(cancel) = &cancel_token {
                        cancel.cancel();
                    }
                    break;
                };
                if matches!(next_cmd, SessionCommand::Abort) {
                    if let Some(cancel) = &cancel_token {
                        cancel.cancel();
                    }
                } else {
                    pending_commands.push_back(next_cmd);
                }
            }
        }
    }
}

fn is_prompt_command(cmd: &SessionCommand) -> bool {
    matches!(cmd, SessionCommand::Prompt { .. } | SessionCommand::RewriteAndPrompt { .. })
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
    let session_id = clanker_message::generate_id();

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
        None,
    );
    let pid = spawned.pid;
    let cmd_tx = spawned.cmd_tx;
    let event_tx = spawned.event_tx;

    let mut event_rx = event_tx.subscribe();

    // Send the prompt
    cmd_tx
        .send(SessionCommand::Prompt {
            text: task.to_string(),
            images: vec![],
        })
        .ok();

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
/// `cmd_tx`, and collects `DaemonEvent::TextDelta` until `PromptDone`.
/// Waiting for controller-level completion avoids returning a reply while the
/// session is still busy between `AgentEnd` and `PromptDone`. Used by chat/1 and Matrix bridge
/// instead of the old clone-seed-prompt pattern.
///
/// If `update_last_active` is false (proactive prompts like heartbeats),
/// the session handle's timestamp is not updated, so idle reaping still
/// works correctly.
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(unbounded_loop, reason = "event loop; bounded by channel close")
)]
pub async fn prompt_and_collect(
    cmd_tx: &tokio::sync::mpsc::UnboundedSender<SessionCommand>,
    event_tx: &tokio::sync::broadcast::Sender<DaemonEvent>,
    text: String,
    images: Vec<clankers_protocol::ImageData>,
) -> String {
    let mut event_rx = event_tx.subscribe();

    // Send the prompt
    if cmd_tx.send(SessionCommand::Prompt { text, images }).is_err() {
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
            Ok(DaemonEvent::AgentEnd) => {}
            Ok(DaemonEvent::PromptDone { error: Some(message) }) => {
                if collected.is_empty() {
                    collected.push_str(&message);
                }
                break;
            }
            Ok(DaemonEvent::PromptDone { error: None }) => break,
            Ok(DaemonEvent::SystemMessage { text, is_error: true }) if text == "A prompt is already in progress" => {
                if collected.is_empty() {
                    collected.push_str(&text);
                }
                break;
            }
            Err(broadcast::error::RecvError::Closed) => break,
            Err(broadcast::error::RecvError::Lagged(n)) => {
                warn!("prompt_and_collect: skipped {n} events");
            }
            _ => {}
        }
    }

    collected
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
    public_auth: Option<crate::capability_gate::PublicUcanToolAuthorization>,
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
                return (handle.session_id.clone(), cmd_tx.clone(), event_tx.clone());
            }
            suspended_session_id = Some(handle.session_id.clone());
        }
    }

    let builder = super::session_builder::SessionBuilder::from_global_paths(factory.default_model.clone());

    if let Some(session_id) = suspended_session_id
        && let Some(catalog) = factory.catalog.as_ref()
        && let Some(entry) = catalog.get_session(&session_id)
    {
        let mut plan = builder.plan_recovered_keyed_session(key, &entry, public_auth.clone());
        let spawned = spawn_agent_process(
            registry,
            factory,
            plan.spawn.session_id.clone(),
            plan.spawn.model.clone(),
            plan.spawn.system_prompt.clone(),
            None,
            plan.spawn.capabilities.take(),
            plan.spawn.public_auth.take(),
        );
        let cmd_tx = spawned.cmd_tx;
        let event_tx = spawned.event_tx;

        if let Some(command) = plan.seed_command() {
            cmd_tx.send(command).ok();
        }

        {
            let mut st = state.lock().await;
            if let Some(handle) = st.sessions.get_mut(&plan.session_id) {
                handle.model.clone_from(&plan.resolved_model);
                handle.cmd_tx = Some(cmd_tx.clone());
                handle.event_tx = Some(event_tx.clone());
                handle.state = "active".to_string();
            }
        }

        catalog.set_state(&plan.session_id, super::session_store::SessionLifecycle::Active);
        info!("recovered keyed session {} for {}", plan.session_id, key);
        return (plan.session_id, cmd_tx, event_tx);
    }

    // Slow path: create a new session.
    let mut plan = builder.plan_new_keyed_session(key, capabilities, public_auth);
    let spawned = spawn_agent_process(
        registry,
        factory,
        plan.spawn.session_id.clone(),
        plan.spawn.model.clone(),
        plan.spawn.system_prompt.clone(),
        None,
        plan.spawn.capabilities.take(),
        plan.spawn.public_auth.take(),
    );
    let cmd_tx = spawned.cmd_tx;
    let event_tx = spawned.event_tx;

    {
        let mut st = state.lock().await;
        st.sessions.insert(plan.session_id.clone(), plan.session_handle(cmd_tx.clone(), event_tx.clone()));
        st.register_key(key.clone(), plan.session_id.clone());
    }

    // Write catalog entry + key mapping.
    if let Some(ref catalog) = factory.catalog {
        let now = chrono::Utc::now().to_rfc3339();
        catalog.insert_session(&plan.catalog_entry(spawned.automerge_path.clone().unwrap_or_default(), now));
        catalog.insert_key(key, &plan.session_id);
    }

    info!("created keyed session {} for {}", plan.session_id, key);
    (plan.session_id, cmd_tx, event_tx)
}

/// Lazily recover a suspended session: open the automerge file, spawn
/// an actor, seed its messages, and upgrade the placeholder handle.
///
/// Returns `(cmd_tx, event_tx)` on success, or an error message.
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(
        nested_conditionals,
        reason = "complex control flow — extracting helpers would obscure logic"
    )
)]
pub fn recover_session(
    session_id: &str,
    registry: &ProcessRegistry,
    factory: &super::socket_bridge::SessionFactory,
    state: &mut clankers_controller::transport::DaemonState,
    shutdown: &tokio::sync::watch::Receiver<bool>,
) -> Result<(mpsc::UnboundedSender<SessionCommand>, broadcast::Sender<DaemonEvent>), String> {
    // Look up catalog entry
    let catalog = factory.catalog.as_ref().ok_or("no session catalog")?;
    let entry = catalog.get_session(session_id).ok_or_else(|| format!("session '{session_id}' not in catalog"))?;

    let builder = super::session_builder::SessionBuilder::from_global_paths(factory.default_model.clone());
    let mut plan = builder.plan_recovered_catalog_session(&entry, None);

    // Spawn the actor.
    let spawned = spawn_agent_process(
        registry,
        factory,
        plan.spawn.session_id.clone(),
        plan.spawn.model.clone(),
        plan.spawn.system_prompt.clone(),
        None,
        plan.spawn.capabilities.take(),
        plan.spawn.public_auth.take(),
    );
    let cmd_tx = spawned.cmd_tx;
    let event_tx = spawned.event_tx;

    if let Some(command) = plan.seed_command() {
        cmd_tx.send(command).ok();
    }

    // Bind session socket before publishing recovered session.
    let listener = clankers_controller::transport::bind_session_socket(session_id)
        .map_err(|e| format!("failed to bind session socket for {session_id}: {e}"))?;
    let sock_shutdown = shutdown.clone();
    let sock_cmd_tx = cmd_tx.clone();
    let sock_event_tx = event_tx.clone();
    let sock_session_id = session_id.to_string();
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
fn resolve_agent_def(agent_def: Option<&str>, _factory: &SessionFactory) -> (Option<String>, Option<String>) {
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
        let disabled_set: std::collections::HashSet<String> = disabled.iter().cloned().collect();
        // Build a fresh panel_tx (events go nowhere — we only need the tool list)
        let (panel_tx, _) = tokio::sync::mpsc::unbounded_channel();
        let child_factory = self.factory.child_actor_factory();
        let actor_ctx = self.factory.registry.as_ref().zip(child_factory).map(|(registry, factory)| {
            crate::tools::subagent::ActorContext {
                registry: registry.clone(),
                factory,
            }
        });
        let env = crate::modes::common::ToolEnv {
            settings: Some(self.factory.settings.clone()),
            panel_tx: Some(panel_tx),
            actor_ctx,
            schedule_engine: self.factory.schedule_engine.clone(),
            ..Default::default()
        };
        let tiered = crate::modes::common::build_all_tiered_tools(&env, self.factory.plugin_manager.as_ref());
        crate::tool_gateway::allowed_tools_for_policy(&tiered, &crate::tool_gateway::daemon_toolsets(), &disabled_set)
    }
}

/// Build plugin summaries from the shared plugin manager for protocol responses.
fn build_plugin_summaries(
    plugin_manager: Option<&std::sync::Arc<std::sync::Mutex<clankers_plugin::PluginManager>>>,
) -> Vec<clankers_protocol::PluginSummary> {
    let Some(pm) = plugin_manager else {
        return Vec::new();
    };
    crate::plugin::build_protocol_plugin_summaries(pm)
}

fn sync_tool_inventory(controller: &mut SessionController, event_tx: &broadcast::Sender<DaemonEvent>) {
    if controller.refresh_tools() {
        event_tx
            .send(DaemonEvent::ToolList {
                tools: controller.current_tool_infos(),
            })
            .ok();
    }
}

fn drain_plugin_runtime_events(
    event_tx: &broadcast::Sender<DaemonEvent>,
    plugin_manager: Option<&std::sync::Arc<std::sync::Mutex<clankers_plugin::PluginManager>>>,
) {
    let Some(pm) = plugin_manager else {
        return;
    };

    let result = crate::modes::plugin_dispatch::drain_stdio_runtime_outputs(pm);
    for (plugin_name, message) in result.messages {
        event_tx
            .send(DaemonEvent::SystemMessage {
                text: format!("🔌 {}: {}", plugin_name, message),
                is_error: false,
            })
            .ok();
    }
    for action in result.ui_actions {
        event_tx.send(crate::modes::plugin_dispatch::ui_action_to_daemon_event(action)).ok();
    }
}

/// Build a hook pipeline for a daemon session from settings.
///
/// Includes plugin hooks when a plugin manager is provided.
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(unbounded_loop, reason = "event loop; bounded by channel close")
)]
fn build_session_hook_pipeline(
    settings: &clankers_config::settings::Settings,
    plugin_manager: Option<&std::sync::Arc<std::sync::Mutex<clankers_plugin::PluginManager>>>,
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
    pipeline.register(std::sync::Arc::new(clankers_hooks::script::ScriptHookHandler::new(hooks_dir, timeout)));

    // Git hooks
    if settings.hooks.manage_git_hooks {
        let mut current = cwd.as_path();
        loop {
            if current.join(".git").exists() {
                pipeline.register(std::sync::Arc::new(clankers_hooks::git::GitHookHandler::new(current.to_path_buf())));
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
        pipeline.register(std::sync::Arc::new(clankers_plugin::hooks::PluginHookHandler::new(std::sync::Arc::clone(pm))));
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
            let filtered: Vec<clankers_ucan::Capability> =
                u.iter().filter(|cap| s.iter().any(|sc| sc.contains(cap))).cloned().collect();
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
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    use clanker_actor::ProcessRegistry;
    use clanker_actor::Signal;
    use tempfile::tempdir;
    use tokio::sync::broadcast;
    use tokio::sync::mpsc;

    use super::SessionFactory;

    struct StubProvider;

    struct DelayedStreamingProvider {
        streamed: Arc<tokio::sync::Notify>,
        release: Arc<tokio::sync::Notify>,
        returned: Arc<AtomicBool>,
    }

    #[async_trait::async_trait]
    impl clankers_provider::Provider for StubProvider {
        async fn complete(
            &self,
            _req: clankers_provider::CompletionRequest,
            _tx: tokio::sync::mpsc::Sender<clankers_provider::streaming::StreamEvent>,
        ) -> std::result::Result<(), clankers_provider::error::ProviderError> {
            Ok(())
        }
        fn models(&self) -> &[clankers_provider::Model] {
            &[]
        }
        fn name(&self) -> &str {
            "stub"
        }
    }

    #[async_trait::async_trait]
    impl clankers_provider::Provider for DelayedStreamingProvider {
        async fn complete(
            &self,
            _req: clankers_provider::CompletionRequest,
            tx: tokio::sync::mpsc::Sender<clankers_provider::streaming::StreamEvent>,
        ) -> std::result::Result<(), clankers_provider::error::ProviderError> {
            tx.send(clankers_provider::streaming::StreamEvent::MessageStart {
                message: clankers_provider::streaming::MessageMetadata {
                    id: "delayed-message".to_string(),
                    model: "test".to_string(),
                    role: "assistant".to_string(),
                },
            })
            .await
            .ok();
            tx.send(clankers_provider::streaming::StreamEvent::ContentBlockStart {
                index: 0,
                content_block: clanker_message::Content::Thinking {
                    thinking: String::new(),
                    signature: String::new(),
                },
            })
            .await
            .ok();
            tx.send(clankers_provider::streaming::StreamEvent::ContentBlockDelta {
                index: 0,
                delta: clankers_provider::streaming::ContentDelta::ThinkingDelta {
                    thinking: "actor thought".to_string(),
                },
            })
            .await
            .ok();
            tx.send(clankers_provider::streaming::StreamEvent::ContentBlockStop { index: 0 }).await.ok();
            tx.send(clankers_provider::streaming::StreamEvent::ContentBlockStart {
                index: 1,
                content_block: clanker_message::Content::Text { text: String::new() },
            })
            .await
            .ok();
            tx.send(clankers_provider::streaming::StreamEvent::ContentBlockDelta {
                index: 1,
                delta: clankers_provider::streaming::ContentDelta::TextDelta {
                    text: "actor stream".to_string(),
                },
            })
            .await
            .ok();
            self.streamed.notify_waiters();
            self.release.notified().await;
            self.returned.store(true, Ordering::SeqCst);
            tx.send(clankers_provider::streaming::StreamEvent::ContentBlockStop { index: 1 }).await.ok();
            tx.send(clankers_provider::streaming::StreamEvent::MessageDelta {
                stop_reason: Some("end_turn".to_string()),
                usage: clanker_message::Usage::default(),
            })
            .await
            .ok();
            tx.send(clankers_provider::streaming::StreamEvent::MessageStop).await.ok();
            Ok(())
        }

        fn models(&self) -> &[clankers_provider::Model] {
            &[]
        }

        fn name(&self) -> &str {
            "delayed-streaming"
        }
    }

    fn make_factory(plugin_manager: Option<Arc<Mutex<clankers_plugin::PluginManager>>>) -> SessionFactory {
        make_factory_with_provider(Arc::new(StubProvider), plugin_manager)
    }

    fn make_factory_with_provider(
        provider: Arc<dyn clankers_provider::Provider>,
        plugin_manager: Option<Arc<Mutex<clankers_plugin::PluginManager>>>,
    ) -> SessionFactory {
        SessionFactory {
            provider,
            tools: vec![],
            settings: clankers_config::settings::Settings::default(),
            default_model: "test".to_string(),
            default_system_prompt: String::new(),
            registry: None,
            catalog: None,
            schedule_engine: None,
            plugin_manager,
        }
    }

    async fn recv_plugin_list(
        event_rx: &mut broadcast::Receiver<clankers_protocol::DaemonEvent>,
        timeout: Duration,
    ) -> Vec<clankers_protocol::PluginSummary> {
        tokio::time::timeout(timeout, async {
            loop {
                match event_rx.recv().await {
                    Ok(clankers_protocol::DaemonEvent::PluginList { plugins }) => break plugins,
                    Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(error) => panic!("failed waiting for PluginList: {error}"),
                }
            }
        })
        .await
        .expect("timed out waiting for PluginList")
    }

    async fn recv_tool_list(
        event_rx: &mut broadcast::Receiver<clankers_protocol::DaemonEvent>,
        timeout: Duration,
    ) -> Vec<clankers_protocol::ToolInfo> {
        tokio::time::timeout(timeout, async {
            loop {
                match event_rx.recv().await {
                    Ok(clankers_protocol::DaemonEvent::ToolList { tools }) => break tools,
                    Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(error) => panic!("failed waiting for ToolList: {error}"),
                }
            }
        })
        .await
        .expect("timed out waiting for ToolList")
    }

    async fn wait_for_tool_visibility(
        event_rx: &mut broadcast::Receiver<clankers_protocol::DaemonEvent>,
        tool_name: &str,
        expected_present: bool,
        timeout: Duration,
    ) -> Vec<clankers_protocol::ToolInfo> {
        tokio::time::timeout(timeout, async {
            loop {
                let tools = recv_tool_list(event_rx, timeout).await;
                let present = tools.iter().any(|tool| tool.name == tool_name);
                if present == expected_present {
                    break tools;
                }
            }
        })
        .await
        .expect("timed out waiting for matching ToolList")
    }

    #[tokio::test]
    async fn daemon_actor_broadcasts_thinking_and_text_delta_before_provider_returns() {
        let streamed = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let returned = Arc::new(AtomicBool::new(false));
        let provider = Arc::new(DelayedStreamingProvider {
            streamed: Arc::clone(&streamed),
            release: Arc::clone(&release),
            returned: Arc::clone(&returned),
        });
        let registry = ProcessRegistry::new();
        let factory = make_factory_with_provider(provider, None);
        let spawned = super::spawn_agent_process(
            &registry,
            &factory,
            "daemon-streaming-test".to_string(),
            None,
            None,
            None,
            None,
            None,
        );
        let mut event_rx = spawned.event_tx.subscribe();

        spawned
            .cmd_tx
            .send(clankers_protocol::SessionCommand::Prompt {
                text: "stream before done".to_string(),
                images: vec![],
            })
            .expect("session command should be accepted");
        tokio::time::timeout(Duration::from_secs(1), streamed.notified())
            .await
            .expect("provider should stream before waiting for release");

        let (thinking_before_release, text_before_release) = tokio::time::timeout(Duration::from_secs(1), async {
            let mut saw_thinking = false;
            let mut saw_text = false;
            loop {
                match event_rx.recv().await {
                    Ok(clankers_protocol::DaemonEvent::ThinkingDelta { text }) if text == "actor thought" => {
                        saw_thinking = true;
                    }
                    Ok(clankers_protocol::DaemonEvent::TextDelta { text }) if text == "actor stream" => {
                        saw_text = true;
                    }
                    Ok(clankers_protocol::DaemonEvent::PromptDone { .. }) => break (saw_thinking, saw_text),
                    Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(error) => panic!("event stream closed before delta: {error}"),
                }

                if saw_thinking && saw_text {
                    break (true, true);
                }
            }
        })
        .await
        .unwrap_or((false, false));
        let returned_before_release = returned.load(Ordering::SeqCst);

        release.notify_waiters();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                match event_rx.recv().await {
                    Ok(clankers_protocol::DaemonEvent::PromptDone { error: None }) => break,
                    Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(error) => panic!("event stream closed before prompt completion: {error}"),
                }
            }
        })
        .await
        .expect("prompt should complete after provider release");
        shutdown_spawned_session(&registry, &spawned);

        assert!(
            thinking_before_release,
            "daemon actor should broadcast ThinkingDelta before PromptDone/provider return"
        );
        assert!(text_before_release, "daemon actor should broadcast TextDelta before PromptDone/provider return");
        assert!(!returned_before_release, "provider should still be blocked when deltas are broadcast");
    }

    #[tokio::test]
    async fn daemon_actor_processes_abort_while_prompt_is_streaming() {
        let streamed = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let returned = Arc::new(AtomicBool::new(false));
        let provider = Arc::new(DelayedStreamingProvider {
            streamed: Arc::clone(&streamed),
            release,
            returned: Arc::clone(&returned),
        });
        let registry = ProcessRegistry::new();
        let factory = make_factory_with_provider(provider, None);
        let spawned = super::spawn_agent_process(
            &registry,
            &factory,
            "daemon-abort-streaming-test".to_string(),
            None,
            None,
            None,
            None,
            None,
        );
        let mut event_rx = spawned.event_tx.subscribe();

        spawned
            .cmd_tx
            .send(clankers_protocol::SessionCommand::Prompt {
                text: "stream until aborted".to_string(),
                images: vec![],
            })
            .expect("prompt command should be accepted");
        tokio::time::timeout(Duration::from_secs(1), streamed.notified())
            .await
            .expect("provider should reach streaming wait point");

        spawned
            .cmd_tx
            .send(clankers_protocol::SessionCommand::Abort)
            .expect("abort command should be accepted while prompt runs");

        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                match event_rx.recv().await {
                    Ok(clankers_protocol::DaemonEvent::PromptDone { error: Some(error) }) if error == "cancelled" => {
                        break;
                    }
                    Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(error) => panic!("event stream closed before prompt cancellation: {error}"),
                }
            }
        })
        .await
        .expect("abort should finish the prompt before provider release");
        shutdown_spawned_session(&registry, &spawned);

        assert!(
            !returned.load(Ordering::SeqCst),
            "abort should cancel the running provider future rather than waiting for normal provider return"
        );
    }

    #[tokio::test]
    async fn prompt_and_collect_waits_for_prompt_done_after_agent_end() {
        let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel();
        let (event_tx, _) = broadcast::channel(8);
        let cmd_tx_for_task = cmd_tx.clone();
        let event_tx_for_task = event_tx.clone();
        let mut task = tokio::spawn(async move {
            super::prompt_and_collect(&cmd_tx_for_task, &event_tx_for_task, "followup".to_string(), vec![]).await
        });

        let cmd = tokio::time::timeout(Duration::from_secs(1), cmd_rx.recv())
            .await
            .expect("prompt command should be sent")
            .expect("command channel should stay open");
        assert!(matches!(cmd, clankers_protocol::SessionCommand::Prompt { text, .. } if text == "followup"));
        event_tx
            .send(clankers_protocol::DaemonEvent::TextDelta {
                text: "first reply".to_string(),
            })
            .expect("text delta should broadcast");
        event_tx.send(clankers_protocol::DaemonEvent::AgentEnd).expect("agent end should broadcast");

        assert!(
            tokio::time::timeout(Duration::from_millis(50), &mut task).await.is_err(),
            "collector returned at AgentEnd before the session was ready for another prompt"
        );

        event_tx
            .send(clankers_protocol::DaemonEvent::PromptDone { error: None })
            .expect("prompt completion should broadcast");
        let response = tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect("collector should finish after PromptDone")
            .expect("collector task should not panic");
        assert_eq!(response, "first reply");
    }

    #[tokio::test]
    async fn prompt_and_collect_returns_busy_rejection_as_message() {
        let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel();
        let (event_tx, _) = broadcast::channel(8);
        let cmd_tx_for_task = cmd_tx.clone();
        let event_tx_for_task = event_tx.clone();
        let task = tokio::spawn(async move {
            super::prompt_and_collect(&cmd_tx_for_task, &event_tx_for_task, "next".to_string(), vec![]).await
        });

        let cmd = tokio::time::timeout(Duration::from_secs(1), cmd_rx.recv())
            .await
            .expect("prompt command should be sent")
            .expect("command channel should stay open");
        assert!(matches!(cmd, clankers_protocol::SessionCommand::Prompt { text, .. } if text == "next"));
        event_tx
            .send(clankers_protocol::DaemonEvent::SystemMessage {
                text: "A prompt is already in progress".to_string(),
                is_error: true,
            })
            .expect("busy rejection should broadcast");

        let response = tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect("collector should not hang on busy rejection")
            .expect("collector task should not panic");
        assert_eq!(response, "A prompt is already in progress");
    }

    fn shutdown_spawned_session(registry: &ProcessRegistry, spawned: &super::SpawnedSession) {
        spawned.cmd_tx.send(clankers_protocol::SessionCommand::Disconnect).ok();
        registry.send(spawned.pid, Signal::Kill);
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

    #[tokio::test]
    async fn factory_with_live_stdio_plugins_returns_stdio_tools() {
        let dir = tempdir().unwrap();
        crate::plugin::tests::stdio_runtime::write_stdio_plugin_manifest(
            dir.path(),
            "stdio-daemon-factory",
            "ready_register",
            "daemon",
            None,
            None,
        );
        let pm = crate::plugin::tests::stdio_runtime::init_manager_with_restart_delays(
            dir.path(),
            clankers_plugin::PluginRuntimeMode::Daemon,
            "5,10,15,20,25",
        );
        crate::plugin::tests::stdio_runtime::wait_for_plugin_state(
            &pm,
            "stdio-daemon-factory",
            Duration::from_secs(2),
            |state| matches!(state, clankers_plugin::PluginState::Active),
        )
        .await;
        crate::plugin::tests::stdio_runtime::wait_for_live_tool(
            &pm,
            "stdio-daemon-factory",
            "stdio_daemon_factory_tool",
            Duration::from_secs(2),
        )
        .await;

        let factory = make_factory(Some(Arc::clone(&pm)));
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let tools = factory.build_tools_with_panel_tx(tx, None);
        let names: Vec<String> = tools.iter().map(|t| t.definition().name.clone()).collect();
        assert!(names.contains(&"stdio_daemon_factory_tool".to_string()), "Should have stdio tool, got: {names:?}");

        clankers_plugin::shutdown_plugin_runtime(&pm, "test shutdown").await;
    }

    #[tokio::test]
    async fn daemon_bridge_forwards_stdio_plugin_ui_and_display_events() {
        let dir = tempdir().unwrap();
        crate::plugin::tests::stdio_runtime::write_stdio_plugin_manifest(
            dir.path(),
            "stdio-daemon-events",
            "event_tool_call_ui_display",
            "daemon",
            None,
            None,
        );
        let pm = crate::plugin::tests::stdio_runtime::init_manager_with_restart_delays(
            dir.path(),
            clankers_plugin::PluginRuntimeMode::Daemon,
            "5,10,15,20,25",
        );
        crate::plugin::tests::stdio_runtime::wait_for_plugin_state(
            &pm,
            "stdio-daemon-events",
            Duration::from_secs(2),
            |state| matches!(state, clankers_plugin::PluginState::Active),
        )
        .await;

        let mut controller =
            clankers_controller::SessionController::new_embedded(clankers_controller::config::ControllerConfig {
                session_id: "daemon-stdio-events".to_string(),
                model: "test".to_string(),
                ..Default::default()
            });
        controller.feed_event(&clankers_agent::events::AgentEvent::ToolCall {
            tool_name: "bash".to_string(),
            call_id: "call-daemon-stdio-events".to_string(),
            input: serde_json::json!({"command": "echo hi"}),
        });

        let (event_tx, mut event_rx) = broadcast::channel(32);
        let (_panel_tx, mut panel_rx) = mpsc::unbounded_channel();
        crate::modes::daemon::socket_bridge::drain_and_broadcast(&mut controller, &event_tx, &mut panel_rx, Some(&pm));
        tokio::time::sleep(Duration::from_millis(150)).await;
        super::drain_plugin_runtime_events(&event_tx, Some(&pm));

        let mut saw_system = false;
        let mut saw_status = false;
        let mut saw_notify = false;
        let mut saw_widget = false;
        while let Ok(event) = event_rx.try_recv() {
            match event {
                clankers_protocol::DaemonEvent::SystemMessage { text, .. } => {
                    saw_system |= text.contains("stdio-daemon-events") && text.contains("tool_call for bash");
                }
                clankers_protocol::DaemonEvent::PluginStatus { plugin, text, color } => {
                    saw_status |= plugin == "stdio-daemon-events"
                        && text.as_deref() == Some("tool bash")
                        && color.as_deref() == Some("green");
                }
                clankers_protocol::DaemonEvent::PluginNotify { plugin, message, level } => {
                    saw_notify |= plugin == "stdio-daemon-events" && message == "note bash" && level == "info";
                }
                clankers_protocol::DaemonEvent::PluginWidget { plugin, widget } => {
                    saw_widget |= plugin == "stdio-daemon-events" && widget.is_some();
                }
                _ => {}
            }
        }

        assert!(saw_system, "expected plugin display message in daemon event stream");
        assert!(saw_status, "expected plugin status event in daemon event stream");
        assert!(saw_notify, "expected plugin notify event in daemon event stream");
        assert!(saw_widget, "expected plugin widget event in daemon event stream");

        clankers_plugin::shutdown_plugin_runtime(&pm, "test shutdown").await;
    }

    #[tokio::test]
    #[allow(
        clippy::await_holding_lock,
        reason = "test asserts plugin manager state then explicitly drops the sync guard before async shutdown"
    )]
    async fn shared_plugin_host_keeps_disabled_tools_session_local() {
        let dir = tempdir().unwrap();
        crate::plugin::tests::stdio_runtime::write_stdio_plugin_manifest(
            dir.path(),
            "stdio-shared-host",
            "ready_register",
            "daemon",
            None,
            None,
        );
        let pm = crate::plugin::tests::stdio_runtime::init_manager_with_restart_delays(
            dir.path(),
            clankers_plugin::PluginRuntimeMode::Daemon,
            "5,10,15,20,25",
        );
        crate::plugin::tests::stdio_runtime::wait_for_plugin_state(
            &pm,
            "stdio-shared-host",
            Duration::from_secs(2),
            |state| matches!(state, clankers_plugin::PluginState::Active),
        )
        .await;
        crate::plugin::tests::stdio_runtime::wait_for_live_tool(
            &pm,
            "stdio-shared-host",
            "stdio_shared_host_tool",
            Duration::from_secs(2),
        )
        .await;

        let registry = ProcessRegistry::new();
        let factory = make_factory(Some(Arc::clone(&pm)));
        let session_a =
            super::spawn_agent_process(&registry, &factory, "shared-a".to_string(), None, None, None, None, None);
        let session_b =
            super::spawn_agent_process(&registry, &factory, "shared-b".to_string(), None, None, None, None, None);
        let mut event_rx_a = session_a.event_tx.subscribe();
        let mut event_rx_b = session_b.event_tx.subscribe();

        session_a.cmd_tx.send(clankers_protocol::SessionCommand::GetToolList).unwrap();
        session_b.cmd_tx.send(clankers_protocol::SessionCommand::GetToolList).unwrap();
        let initial_a = recv_tool_list(&mut event_rx_a, Duration::from_secs(2)).await;
        let initial_b = recv_tool_list(&mut event_rx_b, Duration::from_secs(2)).await;
        assert!(initial_a.iter().any(|tool| tool.name == "stdio_shared_host_tool"));
        assert!(initial_b.iter().any(|tool| tool.name == "stdio_shared_host_tool"));

        session_a
            .cmd_tx
            .send(clankers_protocol::SessionCommand::SetDisabledTools {
                tools: vec!["stdio_shared_host_tool".to_string()],
            })
            .unwrap();
        let disabled_a =
            wait_for_tool_visibility(&mut event_rx_a, "stdio_shared_host_tool", false, Duration::from_secs(2)).await;
        assert!(!disabled_a.iter().any(|tool| tool.name == "stdio_shared_host_tool"));

        session_b.cmd_tx.send(clankers_protocol::SessionCommand::GetToolList).unwrap();
        let unchanged_b = recv_tool_list(&mut event_rx_b, Duration::from_secs(2)).await;
        assert!(unchanged_b.iter().any(|tool| tool.name == "stdio_shared_host_tool"));

        let guard = pm.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        assert_eq!(guard.get("stdio-shared-host").unwrap().state, clankers_plugin::PluginState::Active);
        drop(guard);

        shutdown_spawned_session(&registry, &session_a);
        shutdown_spawned_session(&registry, &session_b);
        clankers_plugin::shutdown_plugin_runtime(&pm, "test shutdown").await;
    }

    #[tokio::test]
    async fn shared_plugin_disconnect_and_reconnect_updates_all_sessions() {
        let dir = tempdir().unwrap();
        crate::plugin::tests::stdio_runtime::write_stdio_plugin_manifest(
            dir.path(),
            "stdio-shared-restart",
            "ready_register",
            "daemon",
            None,
            None,
        );
        let pm = crate::plugin::tests::stdio_runtime::init_manager_with_restart_delays(
            dir.path(),
            clankers_plugin::PluginRuntimeMode::Daemon,
            "5,10,15,20,25",
        );
        crate::plugin::tests::stdio_runtime::wait_for_plugin_state(
            &pm,
            "stdio-shared-restart",
            Duration::from_secs(2),
            |state| matches!(state, clankers_plugin::PluginState::Active),
        )
        .await;
        crate::plugin::tests::stdio_runtime::wait_for_live_tool(
            &pm,
            "stdio-shared-restart",
            "stdio_shared_restart_tool",
            Duration::from_secs(2),
        )
        .await;

        let registry = ProcessRegistry::new();
        let factory = make_factory(Some(Arc::clone(&pm)));
        let session_a = super::spawn_agent_process(
            &registry,
            &factory,
            "shared-restart-a".to_string(),
            None,
            None,
            None,
            None,
            None,
        );
        let session_b = super::spawn_agent_process(
            &registry,
            &factory,
            "shared-restart-b".to_string(),
            None,
            None,
            None,
            None,
            None,
        );
        let mut event_rx_a = session_a.event_tx.subscribe();
        let mut event_rx_b = session_b.event_tx.subscribe();

        session_a.cmd_tx.send(clankers_protocol::SessionCommand::GetToolList).unwrap();
        session_b.cmd_tx.send(clankers_protocol::SessionCommand::GetToolList).unwrap();
        assert!(
            recv_tool_list(&mut event_rx_a, Duration::from_secs(2))
                .await
                .iter()
                .any(|tool| tool.name == "stdio_shared_restart_tool")
        );
        assert!(
            recv_tool_list(&mut event_rx_b, Duration::from_secs(2))
                .await
                .iter()
                .any(|tool| tool.name == "stdio_shared_restart_tool")
        );

        {
            let mut guard = pm.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.disable("stdio-shared-restart").unwrap();
        }
        wait_for_tool_visibility(&mut event_rx_a, "stdio_shared_restart_tool", false, Duration::from_secs(2)).await;
        wait_for_tool_visibility(&mut event_rx_b, "stdio_shared_restart_tool", false, Duration::from_secs(2)).await;

        clankers_plugin::enable_plugin(&pm, "stdio-shared-restart").unwrap();
        wait_for_tool_visibility(&mut event_rx_a, "stdio_shared_restart_tool", true, Duration::from_secs(2)).await;
        wait_for_tool_visibility(&mut event_rx_b, "stdio_shared_restart_tool", true, Duration::from_secs(2)).await;

        shutdown_spawned_session(&registry, &session_a);
        shutdown_spawned_session(&registry, &session_b);
        clankers_plugin::shutdown_plugin_runtime(&pm, "test shutdown").await;
    }

    #[tokio::test]
    async fn spawned_session_get_plugins_reports_live_stdio_status() {
        let dir = tempdir().unwrap();
        crate::plugin::tests::stdio_runtime::write_stdio_plugin_manifest(
            dir.path(),
            "stdio-daemon-plugin-list",
            "ready_register",
            "daemon",
            None,
            None,
        );
        let pm = crate::plugin::tests::stdio_runtime::init_manager_with_restart_delays(
            dir.path(),
            clankers_plugin::PluginRuntimeMode::Daemon,
            "5,10,15,20,25",
        );
        crate::plugin::tests::stdio_runtime::wait_for_plugin_state(
            &pm,
            "stdio-daemon-plugin-list",
            Duration::from_secs(2),
            |state| matches!(state, clankers_plugin::PluginState::Active),
        )
        .await;
        crate::plugin::tests::stdio_runtime::wait_for_live_tool(
            &pm,
            "stdio-daemon-plugin-list",
            "stdio_daemon_plugin_list_tool",
            Duration::from_secs(2),
        )
        .await;

        let registry = ProcessRegistry::new();
        let factory = make_factory(Some(Arc::clone(&pm)));
        let spawned = super::spawn_agent_process(
            &registry,
            &factory,
            "daemon-plugin-list-session".to_string(),
            None,
            None,
            None,
            None,
            None,
        );
        let mut event_rx = spawned.event_tx.subscribe();
        spawned.cmd_tx.send(clankers_protocol::SessionCommand::GetPlugins).unwrap();

        let plugins = recv_plugin_list(&mut event_rx, Duration::from_secs(2)).await;

        let plugin = plugins
            .iter()
            .find(|plugin| plugin.name == "stdio-daemon-plugin-list")
            .expect("stdio plugin present in plugin list");
        assert_eq!(plugin.kind.as_deref(), Some("stdio"));
        assert_eq!(plugin.state, "Active");
        assert!(plugin.tools.iter().any(|tool| tool == "stdio_daemon_plugin_list_tool"));
        assert!(plugin.last_error.is_none());

        shutdown_spawned_session(&registry, &spawned);
        clankers_plugin::shutdown_plugin_runtime(&pm, "test shutdown").await;
    }
}

#[cfg(test)]
mod merge_tests {
    use clankers_ucan::Capability;

    use super::*;

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
