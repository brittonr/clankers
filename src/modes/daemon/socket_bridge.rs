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
}

impl SessionFactory {
    /// Rebuild tools with a panel_tx for subagent event routing.
    ///
    /// Clones all tools, injecting the panel sender into SubagentTool,
    /// DelegateTool, and ValidatorTool. Other tools are passed through.
    pub fn build_tools_with_panel_tx(
        &self,
        panel_tx: mpsc::UnboundedSender<SubagentEvent>,
        bash_confirm_tx: Option<crate::tools::bash::ConfirmTx>,
    ) -> Vec<Arc<dyn Tool>> {
        let actor_ctx = self.registry.as_ref().map(|reg| {
            crate::tools::subagent::ActorContext {
                registry: reg.clone(),
                factory: std::sync::Arc::new(Self {
                    provider: Arc::clone(&self.provider),
                    tools: self.tools.clone(),
                    settings: self.settings.clone(),
                    default_model: self.default_model.clone(),
                    default_system_prompt: self.default_system_prompt.clone(),
                    // Don't recurse — child agents use subprocess fallback
                    registry: None,
                }),
            }
        });
        let env = crate::modes::common::ToolEnv {
            panel_tx: Some(panel_tx),
            bash_confirm_tx,
            actor_ctx,
            ..Default::default()
        };
        let tiered = crate::modes::common::build_tiered_tools(&env);
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
    let _ = std::fs::remove_file(&path);

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
        } => {
            let session_id = clankers_message::generate_id();
            let resolved_model = model.clone().unwrap_or_else(|| factory.default_model.clone());

            // Spawn as an actor process in the registry
            let (_pid, cmd_tx, event_tx) = super::agent_process::spawn_agent_process(
                &registry,
                &factory,
                session_id.clone(),
                model,
                system_prompt,
                None,
                None, // local sessions get full access
            );

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
                    cmd_tx: cmd_tx.clone(),
                    event_tx: event_tx.clone(),
                    socket_path: socket_path.clone(),
                });
            }

            // Spawn the session socket listener
            let sock_shutdown = shutdown.clone();
            let sock_cmd_tx = cmd_tx.clone();
            let sock_event_tx = event_tx.clone();
            let sock_session_id = session_id.clone();
            tokio::spawn(async move {
                clankers_controller::transport::run_session_socket(
                    sock_session_id,
                    sock_cmd_tx,
                    sock_event_tx,
                    sock_shutdown,
                )
                .await;
            });

            info!("created session {session_id} (model: {resolved_model})");
            ControlResponse::Created {
                session_id,
                socket_path: socket_path.to_string_lossy().into_owned(),
            }
        }

        // Delegate non-creation commands to the standard handler
        other => dispatch_control_command(other, &state).await,
    };

    frame::write_frame(&mut writer, &response).await?;
    Ok(())
}

/// Dispatch non-creation control commands.
async fn dispatch_control_command(
    cmd: ControlCommand,
    state: &Arc<Mutex<DaemonState>>,
) -> ControlResponse {
    let st = state.lock().await;
    match cmd {
        ControlCommand::ListSessions => ControlResponse::Sessions(st.session_summaries()),
        ControlCommand::Status => ControlResponse::Status(st.status()),
        ControlCommand::ProcessTree => ControlResponse::Tree(vec![]),
        ControlCommand::KillSession { session_id } => {
            if let Some(handle) = st.sessions.get(&session_id) {
                let _ = handle.cmd_tx.send(SessionCommand::Disconnect);
                ControlResponse::Killed
            } else {
                ControlResponse::Error {
                    message: format!("session '{session_id}' not found"),
                }
            }
        }
        ControlCommand::AttachSession { session_id } => {
            if let Some(handle) = st.sessions.get(&session_id) {
                ControlResponse::Attached {
                    socket_path: handle.socket_path.to_string_lossy().into_owned(),
                }
            } else {
                ControlResponse::Error {
                    message: format!("session '{session_id}' not found"),
                }
            }
        }
        ControlCommand::CreateSession { .. } => {
            // Should not reach here — handled in the caller
            ControlResponse::Error {
                message: "internal error: CreateSession routed to dispatch".to_string(),
            }
        }
        ControlCommand::Shutdown => ControlResponse::ShuttingDown,
    }
}

/// Drain controller events and subagent panel events, broadcasting all as DaemonEvents.
pub fn drain_and_broadcast(
    controller: &mut SessionController,
    event_tx: &broadcast::Sender<DaemonEvent>,
    panel_rx: &mut mpsc::UnboundedReceiver<SubagentEvent>,
) {
    // Drain controller events
    let events = controller.drain_events();
    for event in events {
        let _ = event_tx.send(event);
    }

    // Drain subagent panel events → DaemonEvent
    while let Ok(panel_event) = panel_rx.try_recv() {
        let daemon_event = match panel_event {
            SubagentEvent::Started { id, name, task, pid } => {
                DaemonEvent::SubagentStarted { id, name, task, pid }
            }
            SubagentEvent::Output { id, line } => DaemonEvent::SubagentOutput { id, line },
            SubagentEvent::Done { id } => DaemonEvent::SubagentDone { id },
            SubagentEvent::Error { id, message } => DaemonEvent::SubagentError { id, message },
            // KillRequest and InputRequest are TUI→tool direction, not relevant here
            SubagentEvent::KillRequest { .. } | SubagentEvent::InputRequest { .. } => continue,
        };
        let _ = event_tx.send(daemon_event);
    }
}

async fn shutdown_signal(shutdown: &tokio::sync::watch::Receiver<bool>) {
    let mut rx = shutdown.clone();
    while !*rx.borrow_and_update() {
        if rx.changed().await.is_err() {
            break;
        }
    }
}
