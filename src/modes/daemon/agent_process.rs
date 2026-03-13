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

    // Build tools with panel_tx for subagent event routing
    let tools = factory.build_tools_with_panel_tx(panel_tx);

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
                &mut signal_rx,
            ))
            .await
        },
    );

    (pid, cmd_tx, event_tx)
}

/// The actor loop: multiplexes session commands, actor signals, and
/// periodic event draining.
async fn run_agent_actor(
    mut controller: SessionController,
    mut cmd_rx: mpsc::UnboundedReceiver<SessionCommand>,
    event_tx: broadcast::Sender<DaemonEvent>,
    session_id: String,
    mut panel_rx: mpsc::UnboundedReceiver<SubagentEvent>,
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
