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
use tracing::debug;
use tracing::info;

use super::socket_bridge::SessionFactory;

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
            run_agent_actor(
                controller,
                cmd_rx,
                driver_event_tx,
                session_id,
                panel_rx,
                &mut signal_rx,
            )
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
