//! Client adapter for TUI connections to a daemon session.
//!
//! Generic over the transport stream (`AsyncRead + AsyncWrite + Unpin + Send`),
//! so the same code works with Unix sockets and QUIC streams.

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use clankers_protocol::command::SessionCommand;
use clankers_protocol::event::DaemonEvent;
use clankers_protocol::frame::FrameError;
use clankers_protocol::frame::{self};
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tracing::debug;
use tracing::warn;

use crate::transport_convert::client_handshake;

/// Client-side adapter that converts between the protocol and local events.
///
/// Generic over transport — works with `UnixStream`, `IrohBiStream`, etc.
pub struct ClientAdapter {
    /// Send commands to the daemon.
    cmd_tx: mpsc::UnboundedSender<SessionCommand>,
    /// Receive events from the daemon.
    event_rx: mpsc::UnboundedReceiver<DaemonEvent>,
    /// True once the reader task exits (daemon disconnected).
    disconnected: bool,
}

impl ClientAdapter {
    /// Build a `ClientAdapter` from pre-wired channels.
    ///
    /// Use when the handshake has already been performed out-of-band
    /// (e.g., QUIC streams where DaemonRequest::Attach was sent before
    /// the session protocol starts). The caller is responsible for spawning
    /// background reader/writer tasks that feed these channels.
    pub fn from_channels(
        cmd_tx: mpsc::UnboundedSender<SessionCommand>,
        event_rx: mpsc::UnboundedReceiver<DaemonEvent>,
    ) -> Self {
        Self {
            cmd_tx,
            event_rx,
            disconnected: false,
        }
    }

    /// Connect to a daemon session over the given stream.
    ///
    /// Performs the handshake, then spawns background tasks for reading/writing.
    #[cfg_attr(
        dylint_lib = "tigerstyle",
        allow(unbounded_loop, reason = "event loop; bounded by channel close")
    )]
    pub async fn connect<S>(
        stream: S,
        client_name: &str,
        token: Option<String>,
        session_id: Option<String>,
    ) -> Result<Self, FrameError>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let (reader, mut writer) = tokio::io::split(stream);

        // Send handshake
        let handshake = client_handshake(client_name, token, session_id);
        frame::write_frame(&mut writer, &handshake).await?;

        // Read initial SessionInfo
        // (handled by the event loop below — first event will be SessionInfo)

        let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<SessionCommand>();
        let (event_tx, event_rx) = mpsc::unbounded_channel::<DaemonEvent>();

        // Spawn writer task (commands → daemon)
        tokio::spawn(async move {
            while let Some(cmd) = cmd_rx.recv().await {
                if frame::write_frame(&mut writer, &cmd).await.is_err() {
                    break;
                }
            }
            writer.shutdown().await.ok();
        });

        // Spawn reader task (daemon → events)
        tokio::spawn(async move {
            let mut reader = reader;
            loop {
                match frame::read_frame::<_, DaemonEvent>(&mut reader).await {
                    Ok(event) => {
                        if event_tx.send(event).is_err() {
                            break;
                        }
                    }
                    Err(FrameError::Eof) => {
                        debug!("daemon connection closed");
                        break;
                    }
                    Err(e) => {
                        warn!("daemon read error: {e}");
                        break;
                    }
                }
            }
        });

        Ok(Self {
            cmd_tx,
            event_rx,
            disconnected: false,
        })
    }

    /// Send a command to the daemon.
    pub fn send(&self, cmd: SessionCommand) -> bool {
        self.cmd_tx.send(cmd).is_ok()
    }

    /// Receive the next event from the daemon.
    pub async fn recv(&mut self) -> Option<DaemonEvent> {
        self.event_rx.recv().await
    }

    /// Try to receive an event without blocking.
    ///
    /// Returns `None` when no event is pending. Sets `disconnected` when
    /// the event channel closes (daemon reader task exited).
    pub fn try_recv(&mut self) -> Option<DaemonEvent> {
        match self.event_rx.try_recv() {
            Ok(event) => Some(event),
            Err(mpsc::error::TryRecvError::Empty) => None,
            Err(mpsc::error::TryRecvError::Disconnected) => {
                self.disconnected = true;
                None
            }
        }
    }

    /// Whether the daemon connection has been lost.
    pub fn is_disconnected(&self) -> bool {
        self.disconnected || self.cmd_tx.is_closed()
    }

    /// Send a prompt to the daemon.
    pub fn prompt(&self, text: String) {
        self.send(SessionCommand::Prompt { text, images: vec![] });
    }

    /// Request history replay.
    pub fn replay_history(&self) {
        self.send(SessionCommand::ReplayHistory);
    }

    /// Cancel the current operation.
    pub fn abort(&self) {
        self.send(SessionCommand::Abort);
    }

    /// Disconnect cleanly.
    pub fn disconnect(&self) {
        self.send(SessionCommand::Disconnect);
    }
}

/// Classify a slash command as client-side or agent-side.
///
/// Client-side commands are handled locally by the TUI without sending
/// to the daemon. Agent-side commands are forwarded as `SessionCommand`.
pub fn is_client_side_command(command: &str) -> bool {
    matches!(
        command,
        "zoom"
            | "layout"
            | "panel"
            | "theme"
            | "copy"
            | "yank"
            | "branch"
            | "switch"
            | "merge"
            | "cherry-pick"
            | "help"
            | "keys"
            | "quit"
            | "q"
            | "detach"
    )
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use clankers_protocol::types::Handshake;
    use clankers_protocol::types::PROTOCOL_VERSION;

    use super::*;

    #[test]
    fn test_client_side_commands() {
        assert!(is_client_side_command("zoom"));
        assert!(is_client_side_command("layout"));
        assert!(is_client_side_command("panel"));
        assert!(is_client_side_command("theme"));
        assert!(is_client_side_command("copy"));
        assert!(is_client_side_command("help"));
        assert!(is_client_side_command("quit"));
        assert!(is_client_side_command("detach"));
    }

    #[test]
    fn test_agent_side_commands() {
        assert!(!is_client_side_command("model"));
        assert!(!is_client_side_command("thinking"));
        assert!(!is_client_side_command("session"));
        assert!(!is_client_side_command("resume"));
        assert!(!is_client_side_command("compact"));
        assert!(!is_client_side_command("autotest"));
        assert!(!is_client_side_command("loop"));
        assert!(!is_client_side_command("hooks"));
        assert!(!is_client_side_command("auth"));
        assert!(!is_client_side_command("clear"));
    }

    #[tokio::test]
    async fn test_client_adapter_round_trip() {
        // Create a pair of connected streams
        let (client_stream, server_stream) = tokio::io::duplex(4096);

        // Spawn a mock server that reads handshake and sends an event
        tokio::spawn(async move {
            let (mut reader, mut writer) = tokio::io::split(server_stream);

            // Read handshake
            let hs: Handshake = frame::read_frame(&mut reader).await.unwrap();
            assert_eq!(hs.protocol_version, PROTOCOL_VERSION);

            // Send SessionInfo
            frame::write_frame(&mut writer, &DaemonEvent::SessionInfo {
                session_id: "test".to_string(),
                model: "model".to_string(),
                system_prompt_hash: "hash".to_string(),
                available_models: Vec::new(),
                active_account: String::new(),
                disabled_tools: Vec::new(),
                auto_test_command: None,
            })
            .await
            .unwrap();

            // Send a text delta
            frame::write_frame(&mut writer, &DaemonEvent::TextDelta {
                text: "hello from daemon".to_string(),
            })
            .await
            .unwrap();

            // Read a command
            let cmd: SessionCommand = frame::read_frame(&mut reader).await.unwrap();
            assert!(matches!(cmd, SessionCommand::Abort));
        });

        // Connect the client adapter
        let mut adapter = ClientAdapter::connect(client_stream, "test-client", None, None).await.unwrap();

        // Receive SessionInfo
        let event = adapter.recv().await.unwrap();
        assert!(matches!(event, DaemonEvent::SessionInfo { .. }));

        // Receive text delta
        let event = adapter.recv().await.unwrap();
        assert!(matches!(event, DaemonEvent::TextDelta { text } if text == "hello from daemon"));

        // Send a command
        adapter.abort();

        // Give the server time to process
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    #[test]
    fn test_from_channels() {
        let (cmd_tx, _cmd_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        let adapter = ClientAdapter::from_channels(cmd_tx, event_rx);
        assert!(!adapter.is_disconnected());

        // Send a test event
        event_tx
            .send(DaemonEvent::TextDelta {
                text: "test".to_string(),
            })
            .unwrap();

        // Should be able to send commands
        assert!(adapter.send(SessionCommand::Abort));
    }

    #[test]
    fn test_is_disconnected_when_sender_dropped() {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (_event_tx, event_rx) = mpsc::unbounded_channel();

        let adapter = ClientAdapter::from_channels(cmd_tx, event_rx);
        assert!(!adapter.is_disconnected());

        // Drop the command receiver (simulates daemon disconnect)
        drop(cmd_rx);

        assert!(adapter.is_disconnected());
    }

    #[test]
    fn test_try_recv_when_empty() {
        let (cmd_tx, _cmd_rx) = mpsc::unbounded_channel();
        let (_event_tx, event_rx) = mpsc::unbounded_channel();

        let mut adapter = ClientAdapter::from_channels(cmd_tx, event_rx);

        // try_recv should return None when no events pending
        assert!(adapter.try_recv().is_none());
        assert!(!adapter.is_disconnected());
    }

    #[test]
    fn test_try_recv_when_channel_closed() {
        let (cmd_tx, _cmd_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        let mut adapter = ClientAdapter::from_channels(cmd_tx, event_rx);

        // Drop the event sender (simulates server shutdown)
        drop(event_tx);

        // try_recv should set disconnected flag
        assert!(adapter.try_recv().is_none());
        assert!(adapter.is_disconnected());
    }

    #[test]
    fn test_convenience_methods() {
        let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel();
        let (_event_tx, event_rx) = mpsc::unbounded_channel();

        let adapter = ClientAdapter::from_channels(cmd_tx, event_rx);

        // Test prompt method
        adapter.prompt("test prompt".to_string());
        let cmd = cmd_rx.try_recv().unwrap();
        assert!(matches!(cmd, SessionCommand::Prompt { text, .. } if text == "test prompt"));

        // Test abort method
        adapter.abort();
        let cmd = cmd_rx.try_recv().unwrap();
        assert!(matches!(cmd, SessionCommand::Abort));

        // Test replay_history method
        adapter.replay_history();
        let cmd = cmd_rx.try_recv().unwrap();
        assert!(matches!(cmd, SessionCommand::ReplayHistory));

        // Test disconnect method
        adapter.disconnect();
        let cmd = cmd_rx.try_recv().unwrap();
        assert!(matches!(cmd, SessionCommand::Disconnect));
    }

    #[tokio::test]
    async fn test_try_recv_with_pending_events() {
        let (cmd_tx, _cmd_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        let mut adapter = ClientAdapter::from_channels(cmd_tx, event_rx);

        // Send an event
        let test_event = DaemonEvent::TextDelta {
            text: "test message".to_string(),
        };
        event_tx.send(test_event.clone()).unwrap();

        // try_recv should return the event
        let received = adapter.try_recv().unwrap();
        assert!(matches!(received, DaemonEvent::TextDelta { text } if text == "test message"));

        // Second try_recv should return None
        assert!(adapter.try_recv().is_none());
    }
}
