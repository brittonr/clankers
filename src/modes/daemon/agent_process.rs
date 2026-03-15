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

use clankers_actor::DeathReason;
use clankers_actor::ProcessId;
use clankers_actor::ProcessRegistry;
use clankers_actor::Signal;
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
pub fn spawn_agent_process(
    registry: &ProcessRegistry,
    factory: &SessionFactory,
    session_id: String,
    model: Option<String>,
    system_prompt: Option<String>,
    parent: Option<ProcessId>,
) -> (ProcessId, mpsc::UnboundedSender<SessionCommand>, broadcast::Sender<DaemonEvent>) {
    let model = model.unwrap_or_else(|| factory.default_model.clone());
    let system_prompt = system_prompt.unwrap_or_else(|| factory.default_system_prompt.clone());

    // Create subagent event channel
    let (panel_tx, panel_rx) = mpsc::unbounded_channel::<SubagentEvent>();

    // Create bash confirm channel so dangerous commands can be approved
    // by attached clients via the ConfirmRequest/ConfirmBash protocol.
    let (bash_confirm_tx, bash_confirm_rx) = crate::tools::bash::confirm_channel();

    // Build tools with panel_tx for subagent event routing
    let tools = factory.build_tools_with_panel_tx(panel_tx, Some(bash_confirm_tx));

    // Build the agent
    let agent = crate::agent::builder::AgentBuilder::new(
        Arc::clone(&factory.provider),
        factory.settings.clone(),
        model.clone(),
        system_prompt.clone(),
    )
    .with_tools(tools)
    .build();

    let config = ControllerConfig {
        session_id: session_id.clone(),
        model: model.clone(),
        system_prompt: Some(system_prompt),
        ..Default::default()
    };

    let controller = SessionController::new(agent, config);

    // Create channels for external command/event access
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<SessionCommand>();
    let (event_tx, _) = broadcast::channel::<DaemonEvent>(256);

    let driver_event_tx = event_tx.clone();
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
            ))
            .await
        },
    );

    (pid, cmd_tx, event_tx)
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
) -> DeathReason {
    info!("agent process started: {session_id}");

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

                controller.handle_command(cmd).await;
                super::socket_bridge::drain_and_broadcast(
                    &mut controller, &event_tx, &mut panel_rx,
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
                let _ = event_tx.send(DaemonEvent::ConfirmRequest {
                    request_id,
                    command: req.command,
                    working_dir: cwd,
                });
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
                            let _ = req.resp_tx.send(approved);
                        }
                        _ => {
                            // Timeout or channel dropped — block the command
                            let _ = req.resp_tx.send(false);
                        }
                    }
                });
            }

            // Periodic drain of background agent events
            () = tokio::time::sleep(tokio::time::Duration::from_millis(50)) => {
                super::socket_bridge::drain_and_broadcast(
                    &mut controller, &event_tx, &mut panel_rx,
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

    let (pid, cmd_tx, event_tx) = spawn_agent_process(
        registry,
        factory,
        session_id.clone(),
        model,
        system_prompt,
        parent_pid,
    );

    let mut event_rx = event_tx.subscribe();

    // Send the prompt
    let _ = cmd_tx.send(SessionCommand::Prompt {
        text: task.to_string(),
        images: vec![],
    });

    // Collect text from DaemonEvent stream
    let mut collected = String::new();

    loop {
        tokio::select! {
            event = event_rx.recv() => {
                match event {
                    Ok(DaemonEvent::TextDelta { text, .. }) => {
                        if let Some(tx) = panel_tx {
                            let _ = tx.send(SubagentEvent::Output {
                                id: sub_id.to_string(),
                                line: text.clone(),
                            });
                        }
                        if collected.len() < MAX_COLLECTED_BYTES {
                            collected.push_str(&text);
                        }
                    }
                    Ok(DaemonEvent::AgentEnd) => {
                        if let Some(tx) = panel_tx {
                            let _ = tx.send(SubagentEvent::Done {
                                id: sub_id.to_string(),
                            });
                        }
                        break;
                    }
                    Ok(DaemonEvent::PromptDone { error: Some(msg) }) => {
                        if let Some(tx) = panel_tx {
                            let _ = tx.send(SubagentEvent::Error {
                                id: sub_id.to_string(),
                                message: msg.clone(),
                            });
                        }
                        let _ = cmd_tx.send(SessionCommand::Disconnect);
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
                    let _ = tx.send(SubagentEvent::Error {
                        id: sub_id.to_string(),
                        message: "Cancelled".into(),
                    });
                }
                return Err("Cancelled".to_string());
            }
        }
    }

    // Disconnect cleanly
    let _ = cmd_tx.send(SessionCommand::Disconnect);

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

/// Get or create an actor session for a transport key.
///
/// Looks up the session in `DaemonState` by key. If not found, spawns a
/// new actor process via `spawn_agent_process`, registers it in the state
/// with the key index, and returns the session's command/event channels.
///
/// Returns `(session_id, cmd_tx, event_tx)`.
pub async fn get_or_create_keyed_session(
    state: &tokio::sync::Mutex<clankers_controller::transport::DaemonState>,
    registry: &clankers_actor::ProcessRegistry,
    factory: &super::socket_bridge::SessionFactory,
    key: &clankers_protocol::SessionKey,
) -> (
    String,
    tokio::sync::mpsc::UnboundedSender<SessionCommand>,
    tokio::sync::broadcast::Sender<DaemonEvent>,
) {
    // Fast path: session already exists
    {
        let st = state.lock().await;
        if let Some(handle) = st.session_by_key(key) {
            return (
                handle.session_id.clone(),
                handle.cmd_tx.clone(),
                handle.event_tx.clone(),
            );
        }
    }

    // Slow path: create a new session
    let session_id = clankers_message::generate_id();
    let (_pid, cmd_tx, event_tx) = spawn_agent_process(
        registry,
        factory,
        session_id.clone(),
        None,
        None,
        None,
    );

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
                cmd_tx: cmd_tx.clone(),
                event_tx: event_tx.clone(),
                socket_path,
            },
        );
        st.register_key(key.clone(), session_id.clone());
    }

    info!("created keyed session {} for {}", session_id, key);
    (session_id, cmd_tx, event_tx)
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
