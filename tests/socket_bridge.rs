//! Integration tests for the daemon socket bridge.
//!
//! Tests the full stack: control socket → session creation → session socket
//! → SessionCommand/DaemonEvent round-trip.

use std::sync::Arc;

use clankers_controller::transport::DaemonState;
use clankers_protocol::control::{ControlCommand, ControlResponse};
use clankers_protocol::frame;
use clankers_protocol::types::Handshake;
use clankers_protocol::{DaemonEvent, SessionCommand};
use tokio::net::UnixStream;
use tokio::sync::Mutex;

/// Override socket dir to a temp directory so tests don't conflict with a real
/// daemon or each other.
fn set_test_socket_dir(dir: &std::path::Path) {
    // SAFETY: tests are single-threaded per process (nextest), and we
    // set this before spawning any tasks that read XDG_RUNTIME_DIR.
    unsafe {
        std::env::set_var("XDG_RUNTIME_DIR", dir);
    }
}

/// Minimal mock provider.
struct MockProvider;

#[async_trait::async_trait]
impl clankers::provider::Provider for MockProvider {
    async fn complete(
        &self,
        _request: clankers_provider::CompletionRequest,
        _tx: tokio::sync::mpsc::Sender<clankers_provider::streaming::StreamEvent>,
    ) -> clankers_provider::error::Result<()> {
        Ok(())
    }
    fn models(&self) -> &[clankers_provider::Model] {
        &[]
    }
    fn name(&self) -> &str {
        "mock"
    }
}

#[tokio::test]
async fn test_control_socket_list_empty() {
    let tmp = tempfile::tempdir().unwrap();
    set_test_socket_dir(tmp.path());
    std::fs::create_dir_all(clankers_controller::transport::socket_dir()).unwrap();

    let state = Arc::new(Mutex::new(DaemonState::new()));
    let factory = Arc::new(clankers::modes::daemon::socket_bridge::SessionFactory {
        provider: Arc::new(MockProvider),
        tools: vec![],
        settings: clankers_config::settings::Settings::default(),
        default_model: "test-model".to_string(),
        default_system_prompt: "You are a test.".to_string(),
        registry: None,
    });

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    // Start control socket
    let registry = clanker_actor::ProcessRegistry::new();
    let server = tokio::spawn({
        let state = Arc::clone(&state);
        async move {
            clankers::modes::daemon::socket_bridge::run_control_socket_with_factory(
                state,
                factory,
                registry,
                shutdown_rx,
            )
            .await;
        }
    });

    // Wait for socket to be ready
    let sock_path = clankers_controller::transport::control_socket_path();
    for _ in 0..50 {
        if sock_path.exists() {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    }

    // Connect and list sessions
    let stream = UnixStream::connect(&sock_path).await.unwrap();
    let (mut reader, mut writer) = stream.into_split();

    frame::write_frame(&mut writer, &ControlCommand::ListSessions)
        .await
        .unwrap();
    let resp: ControlResponse = frame::read_frame(&mut reader).await.unwrap();

    assert!(matches!(resp, ControlResponse::Sessions(ref s) if s.is_empty()));

    // Shutdown
    let _ = shutdown_tx.send(true);
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), server).await;
}

#[tokio::test]
async fn test_control_socket_create_session() {
    let tmp = tempfile::tempdir().unwrap();
    set_test_socket_dir(tmp.path());
    std::fs::create_dir_all(clankers_controller::transport::socket_dir()).unwrap();

    let state = Arc::new(Mutex::new(DaemonState::new()));
    let factory = Arc::new(clankers::modes::daemon::socket_bridge::SessionFactory {
        provider: Arc::new(MockProvider),
        tools: vec![],
        settings: clankers_config::settings::Settings::default(),
        default_model: "test-model".to_string(),
        default_system_prompt: "You are a test.".to_string(),
        registry: None,
    });

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    let registry = clanker_actor::ProcessRegistry::new();
    let server = tokio::spawn({
        let state = Arc::clone(&state);
        let factory = Arc::clone(&factory);
        async move {
            clankers::modes::daemon::socket_bridge::run_control_socket_with_factory(
                state,
                factory,
                registry,
                shutdown_rx,
            )
            .await;
        }
    });

    let sock_path = clankers_controller::transport::control_socket_path();
    for _ in 0..50 {
        if sock_path.exists() {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    }

    // Create a session
    let stream = UnixStream::connect(&sock_path).await.unwrap();
    let (mut reader, mut writer) = stream.into_split();

    frame::write_frame(
        &mut writer,
        &ControlCommand::CreateSession {
            model: Some("opus".to_string()),
            system_prompt: None,
            token: None,
        },
    )
    .await
    .unwrap();

    let resp: ControlResponse = frame::read_frame(&mut reader).await.unwrap();
    let (session_id, session_socket) = match resp {
        ControlResponse::Created {
            session_id,
            socket_path,
        } => (session_id, socket_path),
        other => panic!("expected Created, got {other:?}"),
    };

    assert!(!session_id.is_empty());
    assert!(session_socket.contains(&session_id));

    // Verify session appears in the state
    {
        let st = state.lock().await;
        assert_eq!(st.sessions.len(), 1);
        let handle = st.sessions.get(&session_id).unwrap();
        assert_eq!(handle.model, "opus");
    }

    // Wait for session socket to be ready
    let session_sock_path = std::path::PathBuf::from(&session_socket);
    for _ in 0..50 {
        if session_sock_path.exists() {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    }

    // Connect to the session socket
    let session_stream = UnixStream::connect(&session_sock_path).await.unwrap();
    let (mut sr, mut sw) = session_stream.into_split();

    // Send handshake
    frame::write_frame(
        &mut sw,
        &Handshake {
            protocol_version: 1,
            client_name: "test-client".to_string(),
            token: None,
            session_id: Some(session_id.clone()),
        },
    )
    .await
    .unwrap();

    // Should receive SessionInfo
    let event: DaemonEvent = frame::read_frame(&mut sr).await.unwrap();
    assert!(
        matches!(event, DaemonEvent::SessionInfo { ref session_id, .. } if !session_id.is_empty()),
        "expected SessionInfo, got {event:?}"
    );

    // Send a command and check that we get a response
    frame::write_frame(
        &mut sw,
        &SessionCommand::GetSystemPrompt,
    )
    .await
    .unwrap();

    // Give the driver time to process
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let event: DaemonEvent = frame::read_frame(&mut sr).await.unwrap();
    assert!(
        matches!(event, DaemonEvent::SystemPromptResponse { ref prompt } if prompt == "You are a test."),
        "expected SystemPromptResponse, got {event:?}"
    );

    // Disconnect
    frame::write_frame(&mut sw, &SessionCommand::Disconnect)
        .await
        .unwrap();

    let _ = shutdown_tx.send(true);
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), server).await;
}
