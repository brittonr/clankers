//! Integration tests for clankers-protocol.
//!
//! Tests frame I/O over real Unix domain sockets and duplex streams,
//! verifying correct behavior under realistic transport conditions.

use std::time::Duration;

use clankers_protocol::command::SessionCommand;
use clankers_protocol::control::ControlCommand;
use clankers_protocol::control::ControlResponse;
use clankers_protocol::control::DaemonStatus;
use clankers_protocol::control::SessionSummary;
use clankers_protocol::event::DaemonEvent;
use clankers_protocol::frame::FrameError;
use clankers_protocol::frame::read_frame;
use clankers_protocol::frame::read_raw_frame;
use clankers_protocol::frame::write_frame;
use clankers_protocol::frame::write_raw_frame;
use clankers_protocol::types::Handshake;
use clankers_protocol::types::ImageData;
use clankers_protocol::types::PROTOCOL_VERSION;
use tempfile;
use tokio::io::duplex;
use tokio::net::UnixListener;
use tokio::net::UnixStream;

// ── Duplex stream round-trips ───────────────────────────────

#[tokio::test]
async fn duplex_command_stream() {
    let (mut client, mut server) = duplex(64 * 1024);

    let commands = vec![
        SessionCommand::Prompt {
            text: "hello".into(),
            images: vec![],
        },
        SessionCommand::SetModel { model: "opus".into() },
        SessionCommand::Abort,
        SessionCommand::Disconnect,
    ];

    // Client writes
    for cmd in &commands {
        write_frame(&mut client, cmd).await.unwrap();
    }

    // Server reads
    for expected in &commands {
        let decoded: SessionCommand = read_frame(&mut server).await.unwrap();
        assert_eq!(&decoded, expected);
    }
}

#[tokio::test]
async fn duplex_event_stream() {
    let (mut client, mut server) = duplex(64 * 1024);

    let events = vec![
        DaemonEvent::AgentStart,
        DaemonEvent::TextDelta {
            text: "thinking...".into(),
        },
        DaemonEvent::ToolCall {
            tool_name: "bash".into(),
            call_id: "c1".into(),
            input: serde_json::json!({"command": "ls -la"}),
        },
        DaemonEvent::ToolDone {
            call_id: "c1".into(),
            text: "file.rs\nCargo.toml".into(),
            images: vec![ImageData {
                data: "iVBOR...".into(),
                media_type: "image/png".into(),
            }],
            is_error: false,
        },
        DaemonEvent::PromptDone { error: None },
        DaemonEvent::AgentEnd,
    ];

    // Server writes events
    for event in &events {
        write_frame(&mut server, event).await.unwrap();
    }

    // Client reads events
    for expected in &events {
        let decoded: DaemonEvent = read_frame(&mut client).await.unwrap();
        assert_eq!(&decoded, expected);
    }
}

// ── Unix domain socket transport ────────────────────────────

#[tokio::test]
async fn unix_socket_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("test.sock");

    let listener = UnixListener::bind(&sock_path).unwrap();

    let path = sock_path.clone();
    let server_handle = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let cmd: SessionCommand = read_frame(&mut stream).await.unwrap();
        assert_eq!(cmd, SessionCommand::Prompt {
            text: "hello from socket".into(),
            images: vec![],
        });

        let response = DaemonEvent::TextDelta {
            text: "response from daemon".into(),
        };
        write_frame(&mut stream, &response).await.unwrap();
    });

    let mut client = UnixStream::connect(&path).await.unwrap();
    let cmd = SessionCommand::Prompt {
        text: "hello from socket".into(),
        images: vec![],
    };
    write_frame(&mut client, &cmd).await.unwrap();

    let response: DaemonEvent = read_frame(&mut client).await.unwrap();
    assert_eq!(response, DaemonEvent::TextDelta {
        text: "response from daemon".into(),
    });

    server_handle.await.unwrap();
}

// ── Handshake flow ──────────────────────────────────────────

#[tokio::test]
async fn handshake_then_commands() {
    let (mut client, mut server) = duplex(64 * 1024);

    // Client sends handshake
    let hs = Handshake {
        protocol_version: PROTOCOL_VERSION,
        client_name: "clankers-tui/0.1.0".into(),
        token: Some("bearer-token".into()),
        session_id: None,
    };
    write_frame(&mut client, &hs).await.unwrap();

    // Server reads handshake
    let decoded_hs: Handshake = read_frame(&mut server).await.unwrap();
    assert_eq!(decoded_hs.protocol_version, PROTOCOL_VERSION);
    assert_eq!(decoded_hs.client_name, "clankers-tui/0.1.0");
    assert_eq!(decoded_hs.token, Some("bearer-token".into()));

    // Server responds with SessionInfo
    let info = DaemonEvent::SessionInfo {
        session_id: "sess-001".into(),
        model: "claude-sonnet-4-20250514".into(),
        system_prompt_hash: "abc123".into(),
    };
    write_frame(&mut server, &info).await.unwrap();

    let decoded_info: DaemonEvent = read_frame(&mut client).await.unwrap();
    assert_eq!(decoded_info, info);

    // Now client can send commands
    let cmd = SessionCommand::Prompt {
        text: "what is 2+2?".into(),
        images: vec![],
    };
    write_frame(&mut client, &cmd).await.unwrap();

    let decoded_cmd: SessionCommand = read_frame(&mut server).await.unwrap();
    assert_eq!(decoded_cmd, cmd);
}

// ── Control socket flow ─────────────────────────────────────

#[tokio::test]
async fn control_create_session() {
    let (mut client, mut server) = duplex(64 * 1024);

    let cmd = ControlCommand::CreateSession {
        model: Some("opus".into()),
        system_prompt: Some("You are helpful.".into()),
        token: None,
    };
    write_frame(&mut client, &cmd).await.unwrap();

    let decoded: ControlCommand = read_frame(&mut server).await.unwrap();
    assert_eq!(decoded, cmd);

    let response = ControlResponse::Created {
        session_id: "sess-002".into(),
        socket_path: "/tmp/clankers/session-sess-002.sock".into(),
    };
    write_frame(&mut server, &response).await.unwrap();

    let decoded_resp: ControlResponse = read_frame(&mut client).await.unwrap();
    assert_eq!(decoded_resp, response);
}

#[tokio::test]
async fn control_list_sessions() {
    let (mut client, mut server) = duplex(64 * 1024);

    write_frame(&mut client, &ControlCommand::ListSessions).await.unwrap();
    let decoded: ControlCommand = read_frame(&mut server).await.unwrap();
    assert_eq!(decoded, ControlCommand::ListSessions);

    let response = ControlResponse::Sessions(vec![
        SessionSummary {
            session_id: "s1".into(),
            model: "sonnet".into(),
            turn_count: 5,
            last_active: "2026-03-12T09:00:00Z".into(),
            client_count: 1,
            socket_path: "/tmp/clankers/session-s1.sock".into(),
        },
        SessionSummary {
            session_id: "s2".into(),
            model: "opus".into(),
            turn_count: 20,
            last_active: "2026-03-12T08:00:00Z".into(),
            client_count: 0,
            socket_path: "/tmp/clankers/session-s2.sock".into(),
        },
    ]);
    write_frame(&mut server, &response).await.unwrap();

    let decoded_resp: ControlResponse = read_frame(&mut client).await.unwrap();
    assert_eq!(decoded_resp, response);
}

#[tokio::test]
async fn control_daemon_status() {
    let (mut client, mut server) = duplex(64 * 1024);

    write_frame(&mut client, &ControlCommand::Status).await.unwrap();
    let _: ControlCommand = read_frame(&mut server).await.unwrap();

    let status = DaemonStatus {
        uptime_secs: 7200.0,
        session_count: 3,
        total_clients: 5,
        pid: 12345,
    };
    let response = ControlResponse::Status(status);
    write_frame(&mut server, &response).await.unwrap();

    let decoded: ControlResponse = read_frame(&mut client).await.unwrap();
    if let ControlResponse::Status(s) = decoded {
        assert_eq!(s.session_count, 3);
        assert_eq!(s.total_clients, 5);
        assert_eq!(s.pid, 12345);
    } else {
        panic!("expected Status response");
    }
}

// ── Concurrent clients ──────────────────────────────────────

#[tokio::test]
async fn concurrent_clients_on_unix_socket() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("multi.sock");

    let listener = UnixListener::bind(&sock_path).unwrap();

    let path = sock_path.clone();
    let server_handle = tokio::spawn(async move {
        for _ in 0..3 {
            let (mut stream, _) = listener.accept().await.unwrap();
            tokio::spawn(async move {
                let cmd: SessionCommand = read_frame(&mut stream).await.unwrap();
                if let SessionCommand::Prompt { text, .. } = &cmd {
                    let resp = DaemonEvent::TextDelta {
                        text: format!("echo: {text}"),
                    };
                    write_frame(&mut stream, &resp).await.unwrap();
                }
            });
        }
    });

    let mut handles = Vec::new();
    for i in 0..3 {
        let p = sock_path.clone();
        handles.push(tokio::spawn(async move {
            let mut stream = UnixStream::connect(&p).await.unwrap();
            let cmd = SessionCommand::Prompt {
                text: format!("client-{i}"),
                images: vec![],
            };
            write_frame(&mut stream, &cmd).await.unwrap();

            let resp: DaemonEvent = read_frame(&mut stream).await.unwrap();
            if let DaemonEvent::TextDelta { text } = resp {
                assert_eq!(text, format!("echo: client-{i}"));
            } else {
                panic!("expected TextDelta");
            }
        }));
    }

    for h in handles {
        h.await.unwrap();
    }
    server_handle.await.unwrap();
}

// ── Error handling ──────────────────────────────────────────

#[tokio::test]
async fn eof_detection() {
    let (mut client, server) = duplex(1024);
    // Drop server immediately
    drop(server);

    let result: Result<SessionCommand, FrameError> = read_frame(&mut client).await;
    assert!(matches!(result, Err(FrameError::Eof)));
}

#[tokio::test]
async fn corrupted_json_frame() {
    let (mut client, mut server) = duplex(1024);

    // Write a raw frame with invalid JSON
    write_raw_frame(&mut server, b"not valid json").await.unwrap();

    let result: Result<SessionCommand, FrameError> = read_frame(&mut client).await;
    assert!(matches!(result, Err(FrameError::Json(_))));
}

#[tokio::test]
async fn oversized_frame_rejected() {
    let (mut _client, mut server) = duplex(16 * 1024 * 1024);

    // 10 MB + 1 byte = over limit
    let data = vec![b'x'; 10_000_001];
    let result = write_raw_frame(&mut server, &data).await;
    assert!(matches!(result, Err(FrameError::TooLarge { .. })));
}

// ── Bidirectional interleaved ───────────────────────────────

#[tokio::test]
async fn bidirectional_interleaved_messages() {
    let (mut client, mut server) = duplex(64 * 1024);

    // Simulate a realistic exchange
    // Client: prompt
    write_frame(&mut client, &SessionCommand::Prompt {
        text: "explain rust".into(),
        images: vec![],
    })
    .await
    .unwrap();

    // Server: acknowledge
    let _cmd: SessionCommand = read_frame(&mut server).await.unwrap();
    write_frame(&mut server, &DaemonEvent::AgentStart).await.unwrap();

    // Server: stream thinking
    write_frame(&mut server, &DaemonEvent::ContentBlockStart { is_thinking: true }).await.unwrap();
    write_frame(&mut server, &DaemonEvent::ThinkingDelta {
        text: "let me think...".into(),
    })
    .await
    .unwrap();
    write_frame(&mut server, &DaemonEvent::ContentBlockStop).await.unwrap();

    // Server: stream response
    write_frame(&mut server, &DaemonEvent::ContentBlockStart { is_thinking: false }).await.unwrap();
    write_frame(&mut server, &DaemonEvent::TextDelta {
        text: "Rust is a systems programming language".into(),
    })
    .await
    .unwrap();
    write_frame(&mut server, &DaemonEvent::ContentBlockStop).await.unwrap();
    write_frame(&mut server, &DaemonEvent::PromptDone { error: None }).await.unwrap();

    // Client reads all events
    let e1: DaemonEvent = read_frame(&mut client).await.unwrap();
    assert!(matches!(e1, DaemonEvent::AgentStart));

    let e2: DaemonEvent = read_frame(&mut client).await.unwrap();
    assert!(matches!(e2, DaemonEvent::ContentBlockStart { is_thinking: true }));

    let e3: DaemonEvent = read_frame(&mut client).await.unwrap();
    assert!(matches!(e3, DaemonEvent::ThinkingDelta { .. }));

    let e4: DaemonEvent = read_frame(&mut client).await.unwrap();
    assert!(matches!(e4, DaemonEvent::ContentBlockStop));

    let e5: DaemonEvent = read_frame(&mut client).await.unwrap();
    assert!(matches!(e5, DaemonEvent::ContentBlockStart { is_thinking: false }));

    let e6: DaemonEvent = read_frame(&mut client).await.unwrap();
    assert!(matches!(e6, DaemonEvent::TextDelta { .. }));

    let e7: DaemonEvent = read_frame(&mut client).await.unwrap();
    assert!(matches!(e7, DaemonEvent::ContentBlockStop));

    let e8: DaemonEvent = read_frame(&mut client).await.unwrap();
    assert!(matches!(e8, DaemonEvent::PromptDone { error: None }));

    // Client: abort (interleaved)
    write_frame(&mut client, &SessionCommand::Abort).await.unwrap();
    let abort_cmd: SessionCommand = read_frame(&mut server).await.unwrap();
    assert!(matches!(abort_cmd, SessionCommand::Abort));
}

// ── Large payload ───────────────────────────────────────────

#[tokio::test]
async fn large_payload_round_trip() {
    let (mut client, mut server) = duplex(1024 * 1024);

    // 500KB of text — within the 10MB limit
    let big_text = "x".repeat(500_000);
    let cmd = SessionCommand::Prompt {
        text: big_text.clone(),
        images: vec![],
    };
    write_frame(&mut client, &cmd).await.unwrap();

    let decoded: SessionCommand = read_frame(&mut server).await.unwrap();
    if let SessionCommand::Prompt { text, .. } = decoded {
        assert_eq!(text.len(), 500_000);
    } else {
        panic!("expected Prompt");
    }
}
