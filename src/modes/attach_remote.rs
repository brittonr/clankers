//! Remote attach mode — connect to a daemon session via iroh QUIC.
//!
//! Extracted from `attach.rs`. Contains the QUIC stream adapter, remote attach
//! entry point, event loop, reconnection logic, and framing helpers.

use std::io;
use std::time::Duration;

use clankers_controller::client::ClientAdapter;
use clankers_controller::transport_convert::client_handshake;
use clankers_protocol::DaemonEvent;
use clankers_protocol::SessionCommand;
use clankers_protocol::frame;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tracing::info;
use tracing::warn;

use super::attach::AttachParityTracker;
use super::attach::build_client_slash_registry;
use super::attach::drain_daemon_events;
use super::attach::handle_terminal_events;
use crate::config::keybindings::Keymap;
use crate::config::settings::Settings;
use crate::config::theme::load_theme;
use crate::error::Result;
use crate::slash_commands;
use crate::tui::app::App;
use crate::tui::render;

// ── QUIC stream adapter ─────────────────────────────────────────────────────

/// Combine iroh QUIC send + recv into a single `AsyncRead + AsyncWrite` stream.
///
/// This lets us pass QUIC bidirectional streams to `ClientAdapter::connect()`
/// which expects a unified stream type (same as `UnixStream`, `TcpStream`).
struct QuicBiStream {
    send: ::iroh::endpoint::SendStream,
    recv: ::iroh::endpoint::RecvStream,
}

impl tokio::io::AsyncRead for QuicBiStream {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        std::pin::Pin::new(&mut self.recv).poll_read(cx, buf)
    }
}

impl tokio::io::AsyncWrite for QuicBiStream {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<io::Result<usize>> {
        std::pin::Pin::new(&mut self.send).poll_write(cx, buf).map_err(|e| io::Error::other(e.to_string()))
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        std::pin::Pin::new(&mut self.send).poll_flush(cx).map_err(|e| io::Error::other(e.to_string()))
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        std::pin::Pin::new(&mut self.send).poll_shutdown(cx).map_err(|e| io::Error::other(e.to_string()))
    }
}

// ── Entry point ─────────────────────────────────────────────────────────────

/// Launch the TUI in remote attach mode over iroh QUIC.
///
/// Connects to a remote daemon's `clankers/daemon/1` ALPN, performs the
/// attach handshake, then reuses the same `ClientAdapter` + event loop as
/// local Unix socket attach.
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(function_length, reason = "sequential event handling logic")
)]
pub async fn run_remote_attach(
    remote_id: &str,
    session_id: Option<String>,
    should_create_new: bool,
    model: Option<String>,
    settings: &Settings,
    paths: &crate::config::ClankersPaths,
) -> Result<()> {
    use crate::modes::rpc::iroh;

    // Load or generate identity
    let identity_path = iroh::identity_path(paths);
    let identity = iroh::Identity::load_or_generate(&identity_path);

    // Resolve remote_id: try as peer name from peers.json first, then as raw node ID
    let resolved_id = {
        let registry_path = crate::modes::rpc::peers::registry_path(paths);
        let registry = crate::modes::rpc::peers::PeerRegistry::load(&registry_path);
        if let Some(peer) = registry.peers.values().find(|p| p.name == remote_id) {
            peer.node_id.clone()
        } else {
            remote_id.to_string()
        }
    };

    let remote_pk: ::iroh::PublicKey = resolved_id.parse().map_err(|e| crate::error::Error::Provider {
        message: format!("Invalid remote node ID '{resolved_id}' (from '{remote_id}'): {e}"),
    })?;

    // Start endpoint
    let endpoint = iroh::start_endpoint(&identity).await?;
    info!("local node: {}", endpoint.id().fmt_short());
    println!("Connecting to {}...", remote_pk.fmt_short());

    // Connect with daemon ALPN
    let conn = endpoint.connect(remote_pk, clankers_protocol::types::ALPN_DAEMON).await.map_err(|e| {
        crate::error::Error::Provider {
            message: format!("Failed to connect to remote daemon: {e}"),
        }
    })?;
    info!("connected to remote daemon {}", remote_pk.fmt_short());

    // If --new, create the session first via a control stream
    let target_session_id = if should_create_new {
        let sid = create_remote_session(&conn, model.clone()).await?;
        println!("Created remote session: {sid}");
        Some(sid)
    } else {
        session_id
    };

    // Open an attach stream
    let (mut send, mut recv) = conn.open_bi().await.map_err(|e| crate::error::Error::Provider {
        message: format!("Failed to open QUIC stream: {e}"),
    })?;

    // Send DaemonRequest::Attach as the first frame, then the normal
    // session protocol continues over the same stream.
    let handshake =
        client_handshake(&format!("clankers-tui/{}", env!("CARGO_PKG_VERSION")), None, target_session_id.clone());
    let request = clankers_protocol::DaemonRequest::Attach {
        handshake: handshake.clone(),
    };
    quic_write_frame(&mut send, &request).await?;

    // Read AttachResponse
    let response: clankers_protocol::AttachResponse = quic_read_frame(&mut recv).await?;
    let resolved_session_id = match response {
        clankers_protocol::AttachResponse::Ok { session_id } => session_id,
        clankers_protocol::AttachResponse::Error { message } => {
            return Err(crate::error::Error::Provider {
                message: format!("Remote attach failed: {message}"),
            });
        }
    };

    println!("Attached to remote session: {resolved_session_id}");

    // Now the QUIC stream carries the standard session protocol:
    // DaemonEvent frames (recv) and SessionCommand frames (send).
    // Wrap send+recv into a single stream and hand to ClientAdapter.
    //
    // Note: ClientAdapter performs its own handshake (Handshake frame),
    // but the daemon-side QUIC handler already consumed our DaemonRequest
    // and sent back SessionInfo. We need to skip the ClientAdapter handshake.
    //
    // Instead, we read the SessionInfo ourselves and feed events manually.
    let (model_name, _session_hash) = match quic_read_frame::<DaemonEvent>(&mut recv).await {
        Ok(DaemonEvent::SessionInfo {
            model,
            system_prompt_hash,
            ..
        }) => (model, system_prompt_hash),
        Ok(other) => {
            warn!("expected SessionInfo, got: {other:?}");
            (String::new(), String::new())
        }
        Err(e) => {
            return Err(crate::error::Error::Provider {
                message: format!("Session disconnected before sending SessionInfo: {e}"),
            });
        }
    };

    // Now wrap the remaining stream into a QuicBiStream and create a
    // ClientAdapter. The handshake is already done (we consumed Attach +
    // SessionInfo above), but ClientAdapter::connect() will try to send
    // another handshake and read SessionInfo. So instead, we write a
    // synthetic handshake frame that the daemon will ignore (the session
    // stream is already established), and the daemon's next frames will
    // be treated as events.
    //
    // Actually, the cleaner approach: build the ClientAdapter directly
    // from channels, bypassing the handshake. But ClientAdapter's ctor
    // requires a stream. Let's create it from the QUIC bi-stream — the
    // daemon side has already sent SessionInfo, and any subsequent frames
    // are events. ClientAdapter::connect would send a Handshake and expect
    // a SessionInfo — that won't work here since those already happened.
    //
    // Solution: build ClientAdapter manually from channels with a thin
    // adapter that reads/writes frames on the QUIC stream.
    let bi = QuicBiStream { send, recv };
    let client = build_quic_client_adapter(bi);

    // Replay history
    client.replay_history();

    // Set up TUI
    let mut term = super::common::init_terminal()?;

    let display_model = if model_name.is_empty() {
        "remote".to_string()
    } else {
        model_name
    };

    let cwd = std::env::current_dir().unwrap_or_default().to_string_lossy().into_owned();
    let paths = crate::config::ClankersPaths::get();
    let theme = load_theme(settings.theme.as_deref(), &paths.global_themes_dir);
    let keymap = settings.keymap.clone().into_keymap();

    let mut app = App::new(display_model.clone(), cwd, theme);
    app.auto_theme = crate::config::theme::is_auto_theme(settings.theme.as_deref());
    app.session_id = resolved_session_id.clone();
    app.highlighter = Box::new(crate::util::syntax::SyntectHighlighter);

    let slash_registry = build_client_slash_registry();
    app.set_completion_source(Box::new(clanker_tui_types::CompletionSnapshot::from_source(&slash_registry)));
    crate::modes::interactive::rebuild_leader_menu(&mut app, None, settings);
    app.connection_mode = clanker_tui_types::ConnectionMode::Remote {
        node_id_short: remote_pk.fmt_short().to_string(),
    };

    app.push_system(
        format!(
            "attached to remote session {} at {} (model: {})",
            resolved_session_id,
            remote_pk.fmt_short(),
            display_model,
        ),
        false,
    );
    app.push_system("Type /detach or Ctrl+Q to disconnect.".to_string(), false);

    let max_subagent_panes = settings.max_subagent_panes;

    // Run the event loop with QUIC-aware reconnection.
    // We hold the connection so we can open new streams on disconnect.
    let result = run_remote_attach_loop(
        &mut term,
        &mut app,
        client,
        conn,
        &endpoint,
        remote_pk,
        keymap,
        &slash_registry,
        max_subagent_panes,
        &resolved_session_id,
    )
    .await;

    super::common::restore_terminal(&mut term);
    endpoint.close().await;
    result
}

// ── Event loop ──────────────────────────────────────────────────────────────

/// Event loop for remote QUIC attach with reconnection support.
///
/// Holds the QUIC connection so that on stream failure, we can open a
/// new bi-stream on the same multiplexed connection rather than needing
/// a full re-connect.
async fn run_remote_attach_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    mut client: ClientAdapter,
    conn: ::iroh::endpoint::Connection,
    endpoint: &::iroh::Endpoint,
    remote_pk: ::iroh::PublicKey,
    keymap: Keymap,
    slash_registry: &slash_commands::SlashRegistry,
    max_subagent_panes: usize,
    session_id: &str,
) -> Result<()> {
    let mut is_replaying_history = true;
    let mut parity_tracker = AttachParityTracker::default();

    loop {
        terminal.draw(|frame| render::render(frame, app)).map_err(|e| crate::error::Error::Tui {
            message: format!("Render failed: {e}"),
        })?;

        if app.should_quit {
            client.disconnect();
            break;
        }

        // Drain daemon events
        drain_daemon_events(app, &mut client, &mut is_replaying_history, max_subagent_panes, &mut parity_tracker);

        // Detect disconnect and attempt reconnection over the same QUIC connection
        if client.is_disconnected() && app.connection_mode != clanker_tui_types::ConnectionMode::Reconnecting {
            app.connection_mode = clanker_tui_types::ConnectionMode::Reconnecting;
            app.push_system("QUIC stream lost. Reconnecting on same connection...".to_string(), true);

            match try_quic_reconnect(&conn, endpoint, remote_pk, session_id).await {
                Some((new_client, _new_conn)) => {
                    // Note: if _new_conn is Some, the old conn is dead. We can't
                    // replace `conn` because it's borrowed immutably. But the new
                    // client has its own stream — future reconnects would need the
                    // new connection. For now, the client works and a second
                    // disconnect would fail reconnect on the old conn then succeed
                    // by re-establishing again.
                    finish_remote_reconnect(
                        app,
                        &mut client,
                        new_client,
                        &mut is_replaying_history,
                        &mut parity_tracker,
                    );
                }
                None => {
                    app.push_system("Failed to reconnect after 5 attempts. Use /quit to exit.".to_string(), true);
                }
            }
        }

        // Handle terminal events — same as regular attach
        handle_terminal_events(app, &mut client, terminal, &keymap, slash_registry, &mut parity_tracker)?;

        if app.open_editor_requested {
            app.open_editor_requested = false;
            crate::tui::clipboard::open_external_editor(terminal, app);
        }
    }

    Ok(())
}

fn finish_remote_reconnect(
    app: &mut App,
    client: &mut ClientAdapter,
    new_client: ClientAdapter,
    is_replaying_history: &mut bool,
    parity_tracker: &mut AttachParityTracker,
) {
    *client = new_client;
    client.replay_history();
    *is_replaying_history = true;
    *parity_tracker = AttachParityTracker::default();
    app.connection_mode = clanker_tui_types::ConnectionMode::Attached;
    app.push_system("Reconnected to remote session.".to_string(), false);
}

// ── Reconnection ────────────────────────────────────────────────────────────

/// Maximum reconnect attempts before giving up.
const QUIC_RECONNECT_MAX_ATTEMPTS: usize = 5;

/// Reconnect to a session by opening a new bi-stream on the existing
/// QUIC connection. If the connection itself is dead (daemon restarted),
/// attempts to re-establish via the endpoint.
async fn try_quic_reconnect(
    conn: &::iroh::endpoint::Connection,
    endpoint: &::iroh::Endpoint,
    remote_pk: ::iroh::PublicKey,
    session_id: &str,
) -> Option<(ClientAdapter, Option<::iroh::endpoint::Connection>)> {
    // Delays: 1s, 2s, 4s, 8s, 16s
    let delays_ms = [1000, 2000, 4000, 8000, 16000];

    for attempt in 0..QUIC_RECONNECT_MAX_ATTEMPTS {
        if attempt > 0 {
            let delay = delays_ms.get(attempt).copied().unwrap_or(16000);
            info!("QUIC reconnect attempt {}/{QUIC_RECONNECT_MAX_ATTEMPTS} (delay {delay}ms)", attempt + 1);
            tokio::time::sleep(Duration::from_millis(delay as u64)).await;
        }

        // Try the existing connection first
        if let Some(client) = try_quic_attach_stream(conn, session_id).await {
            info!("QUIC reconnect succeeded on existing connection (attempt {})", attempt + 1);
            return Some((client, None));
        }

        // Existing connection dead — try re-establishing
        match endpoint.connect(remote_pk, clankers_protocol::types::ALPN_DAEMON).await {
            Ok(new_conn) => {
                if let Some(client) = try_quic_attach_stream(&new_conn, session_id).await {
                    info!("QUIC reconnect succeeded on new connection (attempt {})", attempt + 1);
                    return Some((client, Some(new_conn)));
                }
            }
            Err(e) => {
                warn!("QUIC reconnect attempt {}: connect failed: {e}", attempt + 1);
            }
        }
    }

    None
}

/// Open a new bi-stream on a connection and perform the attach handshake.
async fn try_quic_attach_stream(conn: &::iroh::endpoint::Connection, session_id: &str) -> Option<ClientAdapter> {
    let (mut send, mut recv) = conn.open_bi().await.ok()?;

    let request = clankers_protocol::DaemonRequest::Attach {
        handshake: clankers_protocol::Handshake {
            protocol_version: clankers_protocol::types::PROTOCOL_VERSION,
            client_name: format!("clankers-tui/{}", env!("CARGO_PKG_VERSION")),
            token: None,
            session_id: Some(session_id.to_string()),
        },
    };
    quic_write_frame(&mut send, &request).await.ok()?;

    let response: clankers_protocol::AttachResponse = quic_read_frame(&mut recv).await.ok()?;
    match response {
        clankers_protocol::AttachResponse::Ok { .. } => {}
        clankers_protocol::AttachResponse::Error { message } => {
            warn!("QUIC attach rejected: {message}");
            return None;
        }
    }

    // Read SessionInfo
    match quic_read_frame::<DaemonEvent>(&mut recv).await {
        Ok(DaemonEvent::SessionInfo { .. }) => {}
        _ => return None,
    }

    let bi = QuicBiStream { send, recv };
    Some(build_quic_client_adapter(bi))
}

// ── Client adapter construction ─────────────────────────────────────────────

/// Build a ClientAdapter from a QUIC stream, skipping the handshake.
///
/// The DaemonRequest::Attach + AttachResponse + SessionInfo exchange has
/// already completed. The stream now carries raw DaemonEvent/SessionCommand
/// frames, which is exactly what ClientAdapter's background tasks expect.
fn build_quic_client_adapter(stream: QuicBiStream) -> ClientAdapter {
    use tokio::sync::mpsc;

    let (reader, writer) = tokio::io::split(stream);

    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<SessionCommand>();
    let (event_tx, event_rx) = mpsc::unbounded_channel::<DaemonEvent>();

    // Spawn writer: SessionCommand → QUIC
    tokio::spawn(async move {
        let mut writer = writer;
        while let Some(cmd) = cmd_rx.recv().await {
            if frame::write_frame(&mut writer, &cmd).await.is_err() {
                break;
            }
        }
        tokio::io::AsyncWriteExt::shutdown(&mut writer).await.ok();
    });

    // Spawn reader: QUIC → DaemonEvent
    tokio::spawn(async move {
        let mut reader = reader;
        while let Ok(event) = frame::read_frame::<_, DaemonEvent>(&mut reader).await {
            if event_tx.send(event).is_err() {
                break;
            }
        }
    });

    ClientAdapter::from_channels(cmd_tx, event_rx)
}

// ── Remote session management ───────────────────────────────────────────────

/// Create a new session on the remote daemon via a control stream.
async fn create_remote_session(conn: &::iroh::endpoint::Connection, model: Option<String>) -> Result<String> {
    let (mut send, mut recv) = conn.open_bi().await.map_err(|e| crate::error::Error::Provider {
        message: format!("Failed to open control stream: {e}"),
    })?;

    let request = clankers_protocol::DaemonRequest::Control {
        command: clankers_protocol::ControlCommand::CreateSession {
            model,
            system_prompt: None,
            token: None,
            resume_id: None,
            continue_last: false,
            cwd: None,
        },
    };
    quic_write_frame(&mut send, &request).await?;
    send.finish().ok();

    let response: clankers_protocol::ControlResponse = quic_read_frame(&mut recv).await?;
    match response {
        clankers_protocol::ControlResponse::Created { session_id, .. } => Ok(session_id),
        clankers_protocol::ControlResponse::Error { message } => Err(crate::error::Error::Provider {
            message: format!("Failed to create remote session: {message}"),
        }),
        other => Err(crate::error::Error::Provider {
            message: format!("Unexpected response: {other:?}"),
        }),
    }
}

// ── QUIC frame helpers ──────────────────────────────────────────────────────

async fn quic_write_frame<T: serde::Serialize>(send: &mut ::iroh::endpoint::SendStream, value: &T) -> Result<()> {
    let data = serde_json::to_vec(value).map_err(|e| crate::error::Error::Provider {
        message: format!("Serialize error: {e}"),
    })?;
    let len = (data.len() as u32).to_be_bytes();
    send.write_all(&len).await.map_err(|e| crate::error::Error::Provider {
        message: format!("QUIC write error: {e}"),
    })?;
    send.write_all(&data).await.map_err(|e| crate::error::Error::Provider {
        message: format!("QUIC write error: {e}"),
    })?;
    Ok(())
}

async fn quic_read_frame<T: serde::de::DeserializeOwned>(recv: &mut ::iroh::endpoint::RecvStream) -> Result<T> {
    let mut len_buf = [0u8; 4];
    recv.read_exact(&mut len_buf).await.map_err(|e| crate::error::Error::Provider {
        message: format!("QUIC read error: {e}"),
    })?;
    let len = usize::try_from(u32::from_be_bytes(len_buf)).unwrap_or(0);
    if len > 10_000_000 {
        return Err(crate::error::Error::Provider {
            message: format!("Frame too large: {len}"),
        });
    }
    let mut data = vec![0u8; len];
    recv.read_exact(&mut data).await.map_err(|e| crate::error::Error::Provider {
        message: format!("QUIC read error: {e}"),
    })?;
    serde_json::from_slice(&data).map_err(|e| crate::error::Error::Provider {
        message: format!("Deserialize error: {e}"),
    })
}

#[cfg(test)]
mod tests {
    use clankers_controller::client::ClientAdapter;
    use clankers_protocol::DaemonEvent;
    use clankers_tui::app::App;
    use clanker_tui_types::BlockEntry;
    use clanker_tui_types::ConnectionMode;

    use super::AttachParityTracker;
    use super::drain_daemon_events;

    fn test_app() -> App {
        App::new("test-model".to_string(), ".".to_string(), crate::config::theme::detect_theme())
    }

    fn client_with_events(events: Vec<DaemonEvent>) -> ClientAdapter {
        let (cmd_tx, _cmd_rx) = tokio::sync::mpsc::unbounded_channel();
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        for event in events {
            event_tx.send(event).expect("event queued");
        }
        ClientAdapter::from_channels(cmd_tx, event_rx)
    }

    fn system_texts(app: &App) -> Vec<String> {
        app.conversation
            .blocks
            .iter()
            .filter_map(|entry| match entry {
                BlockEntry::System(message) => Some(message.content.clone()),
                BlockEntry::Conversation(_) => None,
            })
            .collect()
    }

    #[test]
    fn remote_reconnect_resets_parity_tracker_before_new_events_arrive() {
        let mut app = test_app();
        app.connection_mode = ConnectionMode::Reconnecting;
        let mut client = client_with_events(vec![]);
        let reconnect_client = client_with_events(vec![DaemonEvent::SystemMessage {
            text: "Disabled tools updated: bash".to_string(),
            is_error: false,
        }]);
        let mut is_replaying_history = false;
        let mut parity_tracker = AttachParityTracker::default();
        parity_tracker.expect_disabled_tools_message();

        super::finish_remote_reconnect(
            &mut app,
            &mut client,
            reconnect_client,
            &mut is_replaying_history,
            &mut parity_tracker,
        );
        drain_daemon_events(&mut app, &mut client, &mut is_replaying_history, 0, &mut parity_tracker);

        assert!(is_replaying_history);
        assert_eq!(app.connection_mode, ConnectionMode::Attached);
        let messages = system_texts(&app);
        assert!(messages.iter().any(|message| message == "Reconnected to remote session."));
        assert!(messages.iter().any(|message| message == "Disabled tools updated: bash"));
    }
}
