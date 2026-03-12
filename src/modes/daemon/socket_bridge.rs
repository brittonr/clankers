//! Bridge between Unix domain sockets and SessionController instances.
//!
//! Wires `clankers-controller`'s transport layer into the daemon. The
//! control socket handles session creation; per-session sockets relay
//! commands and events between clients and their SessionController.

use std::sync::Arc;

use clankers_controller::SessionController;
use clankers_controller::config::ControllerConfig;
use clankers_controller::transport::DaemonState;
use clankers_controller::transport::SessionHandle;
use clankers_controller::transport::session_socket_path;
use clankers_protocol::DaemonEvent;
use clankers_protocol::SessionCommand;
use clankers_protocol::control::ControlCommand;
use clankers_protocol::control::ControlResponse;
use clankers_protocol::frame::{self};
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
}

/// Run the control socket with session creation support.
///
/// Unlike `transport::run_control_socket`, this version handles
/// `CreateSession` by constructing a `SessionController` and spawning
/// the session socket + driver tasks.
pub async fn run_control_socket_with_factory(
    state: Arc<Mutex<DaemonState>>,
    factory: Arc<SessionFactory>,
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
                        let shutdown = shutdown.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_control(stream, state, factory, shutdown).await {
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
            let model = model.unwrap_or_else(|| factory.default_model.clone());
            let system_prompt =
                system_prompt.unwrap_or_else(|| factory.default_system_prompt.clone());

            // Build the agent
            let agent = crate::agent::builder::AgentBuilder::new(
                Arc::clone(&factory.provider),
                factory.settings.clone(),
                model.clone(),
                system_prompt.clone(),
            )
            .with_tools(factory.tools.clone())
            .build();

            let config = ControllerConfig {
                session_id: session_id.clone(),
                model: model.clone(),
                system_prompt: Some(system_prompt),
                ..Default::default()
            };

            let controller = SessionController::new(agent, config);

            // Create channels
            let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<SessionCommand>();
            let (event_tx, _) = broadcast::channel::<DaemonEvent>(256);

            let socket_path = session_socket_path(&session_id);

            // Register in daemon state
            {
                let mut st = state.lock().await;
                st.sessions.insert(session_id.clone(), SessionHandle {
                    session_id: session_id.clone(),
                    model: model.clone(),
                    turn_count: 0,
                    last_active: chrono::Utc::now().to_rfc3339(),
                    client_count: 0,
                    cmd_tx: cmd_tx.clone(),
                    event_tx: event_tx.clone(),
                    socket_path: socket_path.clone(),
                });
            }

            // Spawn the session driver (controller loop)
            let driver_event_tx = event_tx.clone();
            let driver_session_id = session_id.clone();
            tokio::spawn(async move {
                run_session_driver(controller, cmd_rx, driver_event_tx, driver_session_id).await;
            });

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

            info!("created session {session_id} (model: {model})");
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

/// Session driver: reads commands from the channel, feeds them to the
/// controller, drains events and broadcasts them to connected clients.
async fn run_session_driver(
    mut controller: SessionController,
    mut cmd_rx: mpsc::UnboundedReceiver<SessionCommand>,
    event_tx: broadcast::Sender<DaemonEvent>,
    session_id: String,
) {
    info!("session driver started: {session_id}");

    loop {
        tokio::select! {
            cmd = cmd_rx.recv() => {
                let Some(cmd) = cmd else {
                    // All senders dropped
                    break;
                };

                let is_disconnect = matches!(cmd, SessionCommand::Disconnect);

                controller.handle_command(cmd).await;

                // Drain and broadcast events
                let events = controller.drain_events();
                for event in events {
                    // Ignore send errors (no receivers)
                    let _ = event_tx.send(event);
                }

                // After prompt completion, check for auto-test or loop continuation
                if !controller.is_busy() {
                    if let Some(auto_prompt) = controller.maybe_auto_test() {
                        controller
                            .handle_command(SessionCommand::Prompt {
                                text: auto_prompt,
                                images: vec![],
                            })
                            .await;
                        let events = controller.drain_events();
                        for event in events {
                            let _ = event_tx.send(event);
                        }
                    }
                    controller.clear_auto_test();
                }

                if is_disconnect {
                    break;
                }
            }

            // Periodic drain of background agent events (tool execution, etc.)
            () = tokio::time::sleep(tokio::time::Duration::from_millis(50)) => {
                let events = controller.drain_events();
                for event in events {
                    let _ = event_tx.send(event);
                }
            }
        }
    }

    controller.shutdown().await;
    info!("session driver stopped: {session_id}");
}

async fn shutdown_signal(shutdown: &tokio::sync::watch::Receiver<bool>) {
    let mut rx = shutdown.clone();
    while !*rx.borrow_and_update() {
        if rx.changed().await.is_err() {
            break;
        }
    }
}
