//! Length-prefixed JSON frame I/O.
//!
//! All frames are: `[4-byte big-endian length][JSON payload]`.
//! Generic over `AsyncRead`/`AsyncWrite` — no transport dependency.

// u32 always fits in usize on 32-bit+ platforms (our minimum).
const _: () = assert!(u32::MAX as u128 <= usize::MAX as u128);

use std::fmt;

use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;

/// Maximum frame size (10 MB).
// r[impl protocol.frame.max-fits-u32]
const MAX_FRAME_SIZE: usize = 10_000_000;

/// Errors from frame read/write operations.
#[derive(Debug)]
pub enum FrameError {
    /// I/O error on the underlying transport.
    Io(std::io::Error),
    /// Frame exceeds the size limit.
    TooLarge { size: usize },
    /// JSON serialization/deserialization failed.
    Json(serde_json::Error),
    /// Connection closed cleanly (EOF).
    Eof,
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FrameError::Io(e) => write!(f, "frame IO error: {e}"),
            FrameError::TooLarge { size } => {
                write!(f, "frame too large: {size} bytes (max {MAX_FRAME_SIZE})")
            }
            FrameError::Json(e) => write!(f, "frame JSON error: {e}"),
            FrameError::Eof => write!(f, "connection closed"),
        }
    }
}

impl std::error::Error for FrameError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            FrameError::Io(e) => Some(e),
            FrameError::Json(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for FrameError {
    fn from(e: std::io::Error) -> Self {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            FrameError::Eof
        } else {
            FrameError::Io(e)
        }
    }
}

impl From<serde_json::Error> for FrameError {
    fn from(e: serde_json::Error) -> Self {
        FrameError::Json(e)
    }
}

/// Write a length-prefixed JSON frame.
///
/// Serializes `value` to JSON, writes `[4-byte length][JSON]` to `writer`.
// r[impl protocol.frame.roundtrip]
// r[impl protocol.frame.size-reject-write]
// r[impl protocol.frame.length-encoding]
pub async fn write_frame<W, T>(writer: &mut W, value: &T) -> Result<(), FrameError>
where
    W: AsyncWrite + Unpin,
    T: serde::Serialize,
{
    let data = serde_json::to_vec(value)?;
    if data.len() > MAX_FRAME_SIZE {
        return Err(FrameError::TooLarge { size: data.len() });
    }
    let len = u32::try_from(data.len()).map_err(|_| FrameError::TooLarge { size: data.len() })?.to_be_bytes();
    writer.write_all(&len).await?;
    writer.write_all(&data).await?;
    writer.flush().await?;
    Ok(())
}

/// Read a length-prefixed JSON frame.
///
/// Reads `[4-byte length][JSON]` from `reader`, deserializes to `T`.
// r[impl protocol.frame.roundtrip]
// r[impl protocol.frame.size-reject-read]
// r[impl protocol.frame.length-encoding]
pub async fn read_frame<R, T>(reader: &mut R) -> Result<T, FrameError>
where
    R: AsyncRead + Unpin,
    T: serde::de::DeserializeOwned,
{
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await?;
    let len = usize::try_from(u32::from_be_bytes(len_buf)).unwrap_or(0);
    if len > MAX_FRAME_SIZE {
        return Err(FrameError::TooLarge { size: len });
    }
    let mut data = vec![0u8; len];
    reader.read_exact(&mut data).await?;
    let value = serde_json::from_slice(&data)?;
    Ok(value)
}

/// Write raw bytes as a length-prefixed frame.
pub async fn write_raw_frame<W>(writer: &mut W, data: &[u8]) -> Result<(), FrameError>
where W: AsyncWrite + Unpin {
    if data.len() > MAX_FRAME_SIZE {
        return Err(FrameError::TooLarge { size: data.len() });
    }
    let len = u32::try_from(data.len()).map_err(|_| FrameError::TooLarge { size: data.len() })?.to_be_bytes();
    writer.write_all(&len).await?;
    writer.write_all(data).await?;
    writer.flush().await?;
    Ok(())
}

/// Read raw bytes from a length-prefixed frame.
pub async fn read_raw_frame<R>(reader: &mut R) -> Result<Vec<u8>, FrameError>
where R: AsyncRead + Unpin {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await?;
    let len = usize::try_from(u32::from_be_bytes(len_buf)).unwrap_or(0);
    if len > MAX_FRAME_SIZE {
        return Err(FrameError::TooLarge { size: len });
    }
    let mut data = vec![0u8; len];
    reader.read_exact(&mut data).await?;
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::SessionCommand;
    use crate::control::ControlCommand;
    use crate::control::ControlResponse;
    use crate::event::DaemonEvent;
    use crate::types::Handshake;
    use crate::types::ImageData;

    // r[verify protocol.frame.roundtrip]
    // r[verify protocol.frame.length-encoding]
    #[tokio::test]
    async fn test_round_trip_json() {
        let cmd = SessionCommand::Prompt {
            text: "hello".to_string(),
            images: vec![],
        };

        let mut buf = Vec::new();
        write_frame(&mut buf, &cmd).await.unwrap();

        let mut cursor = std::io::Cursor::new(buf);
        let decoded: SessionCommand = read_frame(&mut cursor).await.unwrap();

        assert_eq!(cmd, decoded);
    }

    #[tokio::test]
    async fn test_round_trip_daemon_event() {
        let event = DaemonEvent::TextDelta {
            text: "hello world".to_string(),
        };

        let mut buf = Vec::new();
        write_frame(&mut buf, &event).await.unwrap();

        let mut cursor = std::io::Cursor::new(buf);
        let decoded: DaemonEvent = read_frame(&mut cursor).await.unwrap();

        assert_eq!(event, decoded);
    }

    #[tokio::test]
    async fn test_round_trip_raw_frame() {
        let data = b"raw bytes here";

        let mut buf = Vec::new();
        write_raw_frame(&mut buf, data).await.unwrap();

        let mut cursor = std::io::Cursor::new(buf);
        let decoded = read_raw_frame(&mut cursor).await.unwrap();

        assert_eq!(data.as_slice(), decoded.as_slice());
    }

    // r[verify protocol.frame.size-reject-write]
    #[tokio::test]
    async fn test_frame_too_large() {
        let data = vec![0u8; 10_000_001];

        let mut buf = Vec::new();
        let result = write_raw_frame(&mut buf, &data).await;

        assert!(matches!(result, Err(FrameError::TooLarge { size: 10_000_001 })));
    }

    // r[verify protocol.frame.size-reject-read]
    #[tokio::test]
    async fn test_eof_on_empty_read() {
        let buf: &[u8] = &[];
        let mut cursor = std::io::Cursor::new(buf);
        let result: Result<Vec<u8>, _> = read_raw_frame(&mut cursor).await;

        assert!(matches!(result, Err(FrameError::Eof)));
    }

    #[tokio::test]
    async fn test_multiple_frames() {
        let cmds = vec![
            SessionCommand::Abort,
            SessionCommand::ResetCancel,
            SessionCommand::ClearHistory,
            SessionCommand::GetSystemPrompt,
            SessionCommand::ReplayHistory,
            SessionCommand::GetCapabilities,
            SessionCommand::GetPlugins,
            SessionCommand::Disconnect,
        ];

        let mut buf = Vec::new();
        for cmd in &cmds {
            write_frame(&mut buf, cmd).await.unwrap();
        }

        let mut cursor = std::io::Cursor::new(buf);
        for expected in &cmds {
            let decoded: SessionCommand = read_frame(&mut cursor).await.unwrap();
            assert_eq!(expected, &decoded);
        }
    }

    #[tokio::test]
    async fn test_round_trip_control_command() {
        let cmd = ControlCommand::CreateSession {
            model: Some("sonnet".to_string()),
            system_prompt: None,
            token: Some("base64token".to_string()),
            resume_id: None,
            continue_last: false,
            cwd: None,
        };

        let mut buf = Vec::new();
        write_frame(&mut buf, &cmd).await.unwrap();
        let mut cursor = std::io::Cursor::new(buf);
        let decoded: ControlCommand = read_frame(&mut cursor).await.unwrap();
        assert_eq!(cmd, decoded);

        let resp = ControlResponse::Created {
            session_id: "abc123".to_string(),
            socket_path: "/tmp/clankers/session-abc123.sock".to_string(),
        };
        let mut buf = Vec::new();
        write_frame(&mut buf, &resp).await.unwrap();
        let mut cursor = std::io::Cursor::new(buf);
        let decoded: ControlResponse = read_frame(&mut cursor).await.unwrap();
        assert_eq!(resp, decoded);
    }

    #[tokio::test]
    async fn test_round_trip_handshake() {
        let hs = Handshake {
            protocol_version: 1,
            client_name: "clankers-tui/0.1.0".to_string(),
            token: None,
            session_id: Some("session-123".to_string()),
        };

        let mut buf = Vec::new();
        write_frame(&mut buf, &hs).await.unwrap();
        let mut cursor = std::io::Cursor::new(buf);
        let decoded: Handshake = read_frame(&mut cursor).await.unwrap();
        assert_eq!(hs, decoded);
    }

    #[tokio::test]
    async fn test_round_trip_all_session_commands() {
        let commands = vec![
            SessionCommand::Prompt {
                text: "hello".to_string(),
                images: vec![ImageData {
                    data: "base64data".to_string(),
                    media_type: "image/png".to_string(),
                }],
            },
            SessionCommand::Abort,
            SessionCommand::ResetCancel,
            SessionCommand::SetModel {
                model: "opus".to_string(),
            },
            SessionCommand::ClearHistory,
            SessionCommand::TruncateMessages { count: 10 },
            SessionCommand::SetThinkingLevel {
                level: "high".to_string(),
            },
            SessionCommand::CycleThinkingLevel,
            SessionCommand::SeedMessages { messages: vec![] },
            SessionCommand::SetSystemPrompt {
                prompt: "You are a helpful assistant.".to_string(),
            },
            SessionCommand::GetSystemPrompt,
            SessionCommand::SwitchAccount {
                account: "work".to_string(),
            },
            SessionCommand::SetDisabledTools {
                tools: vec!["bash".to_string()],
            },
            SessionCommand::ConfirmBash {
                request_id: "req-1".to_string(),
                approved: true,
            },
            SessionCommand::TodoResponse {
                request_id: "req-2".to_string(),
                response: serde_json::json!({"action": "add", "text": "fix tests"}),
            },
            SessionCommand::SlashCommand {
                command: "model".to_string(),
                args: "sonnet".to_string(),
            },
            SessionCommand::ReplayHistory,
            SessionCommand::GetCapabilities,
            SessionCommand::GetPlugins,
            SessionCommand::Disconnect,
        ];

        for cmd in &commands {
            let mut buf = Vec::new();
            write_frame(&mut buf, cmd).await.unwrap();
            let mut cursor = std::io::Cursor::new(buf);
            let decoded: SessionCommand = read_frame(&mut cursor).await.unwrap();
            assert_eq!(cmd, &decoded, "round-trip failed for {cmd:?}");
        }
    }

    #[tokio::test]
    async fn test_round_trip_all_daemon_events() {
        let events = vec![
            DaemonEvent::AgentStart,
            DaemonEvent::AgentEnd,
            DaemonEvent::ContentBlockStart { is_thinking: true },
            DaemonEvent::TextDelta {
                text: "hello".to_string(),
            },
            DaemonEvent::ThinkingDelta {
                text: "hmm".to_string(),
            },
            DaemonEvent::ContentBlockStop,
            DaemonEvent::ToolCall {
                tool_name: "bash".to_string(),
                call_id: "c1".to_string(),
                input: serde_json::json!({"command": "ls"}),
            },
            DaemonEvent::ToolStart {
                call_id: "c1".to_string(),
                tool_name: "bash".to_string(),
            },
            DaemonEvent::ToolOutput {
                call_id: "c1".to_string(),
                text: "file.rs".to_string(),
                images: vec![],
            },
            DaemonEvent::ToolProgressUpdate {
                call_id: "c1".to_string(),
                progress: serde_json::json!({"percent": 50}),
            },
            DaemonEvent::ToolChunk {
                call_id: "c1".to_string(),
                content: "chunk data".to_string(),
                content_type: "text/plain".to_string(),
            },
            DaemonEvent::ToolDone {
                call_id: "c1".to_string(),
                text: "done".to_string(),
                images: vec![ImageData {
                    data: "img".to_string(),
                    media_type: "image/png".to_string(),
                }],
                is_error: false,
            },
            DaemonEvent::UserInput {
                text: "hello agent".to_string(),
                agent_msg_count: 5,
            },
            DaemonEvent::SessionCompaction {
                compacted_count: 10,
                tokens_saved: 5000,
            },
            DaemonEvent::UsageUpdate {
                input_tokens: 1000,
                output_tokens: 500,
                cache_read: 300,
                model: "sonnet".to_string(),
            },
            DaemonEvent::ModelChanged {
                from: "sonnet".to_string(),
                to: "opus".to_string(),
                reason: "user request".to_string(),
            },
            DaemonEvent::ConfirmRequest {
                request_id: "r1".to_string(),
                command: "rm -rf /".to_string(),
                working_dir: "/home/user".to_string(),
            },
            DaemonEvent::TodoRequest {
                request_id: "r2".to_string(),
                action: serde_json::json!({"action": "add"}),
            },
            DaemonEvent::SessionInfo {
                session_id: "s1".to_string(),
                model: "sonnet".to_string(),
                system_prompt_hash: "abc".to_string(),
                available_models: Vec::new(),
                active_account: String::new(),
                disabled_tools: Vec::new(),
                auto_test_command: None,
            },
            DaemonEvent::SystemPromptResponse {
                prompt: "You are helpful.".to_string(),
            },
            DaemonEvent::SubagentStarted {
                id: "sa1".to_string(),
                name: "worker".to_string(),
                task: "research".to_string(),
                pid: Some(12345),
            },
            DaemonEvent::SubagentOutput {
                id: "sa1".to_string(),
                line: "found 3 files".to_string(),
            },
            DaemonEvent::SubagentDone { id: "sa1".to_string() },
            DaemonEvent::SubagentError {
                id: "sa1".to_string(),
                message: "timeout".to_string(),
            },
            DaemonEvent::Capabilities {
                capabilities: Some(vec!["read".to_string(), "grep".to_string()]),
            },
            DaemonEvent::ToolBlocked {
                call_id: "c2".to_string(),
                tool_name: "bash".to_string(),
                reason: "not allowed".to_string(),
            },
            DaemonEvent::SystemMessage {
                text: "session started".to_string(),
                is_error: false,
            },
            DaemonEvent::PromptDone { error: None },
            DaemonEvent::PromptDone {
                error: Some("cancelled".to_string()),
            },
            DaemonEvent::HistoryBlock {
                block: serde_json::json!({"role": "user", "content": "hi"}),
            },
            DaemonEvent::HistoryEnd,
            // Plugin events
            DaemonEvent::PluginWidget {
                plugin: "calendar".to_string(),
                widget: Some(serde_json::json!({"type": "Text", "content": "hello", "bold": true})),
            },
            DaemonEvent::PluginWidget {
                plugin: "calendar".to_string(),
                widget: None,
            },
            DaemonEvent::PluginStatus {
                plugin: "github".to_string(),
                text: Some("3 PRs open".to_string()),
                color: Some("green".to_string()),
            },
            DaemonEvent::PluginStatus {
                plugin: "github".to_string(),
                text: None,
                color: None,
            },
            DaemonEvent::PluginNotify {
                plugin: "hash".to_string(),
                message: "Done!".to_string(),
                level: "info".to_string(),
            },
            DaemonEvent::PluginList {
                plugins: vec![crate::event::PluginSummary {
                    name: "test-plugin".to_string(),
                    version: "0.1.0".to_string(),
                    state: "Active".to_string(),
                    tools: vec!["test_echo".to_string()],
                    permissions: vec!["fs:read".to_string()],
                }],
            },
        ];

        for event in &events {
            let mut buf = Vec::new();
            write_frame(&mut buf, event).await.unwrap();
            let mut cursor = std::io::Cursor::new(buf);
            let decoded: DaemonEvent = read_frame(&mut cursor).await.unwrap();
            assert_eq!(event, &decoded, "round-trip failed for {event:?}");
        }
    }

    // ── Missing SessionCommand variants ─────────────────────────────

    #[tokio::test]
    async fn test_round_trip_remaining_session_commands() {
        let commands = vec![
            SessionCommand::RewriteAndPrompt {
                text: "rewritten prompt".to_string(),
            },
            SessionCommand::CompactHistory,
            SessionCommand::StartLoop {
                iterations: 5,
                prompt: "fix all tests".to_string(),
                break_condition: Some("all tests pass".to_string()),
            },
            SessionCommand::StartLoop {
                iterations: 10,
                prompt: "keep going".to_string(),
                break_condition: None,
            },
            SessionCommand::StopLoop,
            SessionCommand::SetAutoTest {
                enabled: true,
                command: Some("cargo nextest run".to_string()),
            },
            SessionCommand::SetAutoTest {
                enabled: false,
                command: None,
            },
            SessionCommand::GetToolList,
            SessionCommand::SetCapabilities {
                capabilities: Some(vec!["read".to_string(), "bash".to_string()]),
            },
            SessionCommand::SetCapabilities {
                capabilities: None,
            },
        ];

        for cmd in &commands {
            let mut buf = Vec::new();
            write_frame(&mut buf, cmd).await.unwrap();
            let mut cursor = std::io::Cursor::new(buf);
            let decoded: SessionCommand = read_frame(&mut cursor).await.unwrap();
            assert_eq!(cmd, &decoded, "round-trip failed for {cmd:?}");
        }
    }

    // ── Missing DaemonEvent variants ────────────────────────────────

    #[tokio::test]
    async fn test_round_trip_remaining_daemon_events() {
        use crate::event::ToolInfo;

        let events = vec![
            DaemonEvent::ToolList {
                tools: vec![
                    ToolInfo {
                        name: "bash".to_string(),
                        description: "Run shell commands".to_string(),
                        source: "built-in".to_string(),
                    },
                    ToolInfo {
                        name: "read".to_string(),
                        description: "Read files".to_string(),
                        source: "built-in".to_string(),
                    },
                ],
            },
            DaemonEvent::ToolList { tools: vec![] },
            DaemonEvent::DisabledToolsChanged {
                tools: vec!["bash".to_string(), "write".to_string()],
            },
            DaemonEvent::ThinkingLevelChanged {
                from: "off".to_string(),
                to: "high".to_string(),
            },
            DaemonEvent::LoopStatus {
                active: true,
                iteration: Some(3),
                max_iterations: Some(10),
                break_condition: Some("tests pass".to_string()),
            },
            DaemonEvent::LoopStatus {
                active: false,
                iteration: None,
                max_iterations: None,
                break_condition: None,
            },
            DaemonEvent::AutoTestChanged {
                enabled: true,
                command: Some("cargo test".to_string()),
            },
            DaemonEvent::AutoTestChanged {
                enabled: false,
                command: None,
            },
            DaemonEvent::CostUpdate {
                total_cost_usd: 1.234,
                total_input_tokens: 50000,
                total_output_tokens: 10000,
            },
        ];

        for event in &events {
            let mut buf = Vec::new();
            write_frame(&mut buf, event).await.unwrap();
            let mut cursor = std::io::Cursor::new(buf);
            let decoded: DaemonEvent = read_frame(&mut cursor).await.unwrap();
            assert_eq!(event, &decoded, "round-trip failed for {event:?}");
        }
    }

    // ── All ControlCommand variants ─────────────────────────────────

    #[tokio::test]
    async fn test_round_trip_all_control_commands() {
        let commands = vec![
            ControlCommand::ListSessions,
            ControlCommand::CreateSession {
                model: Some("sonnet".to_string()),
                system_prompt: Some("be helpful".to_string()),
                token: None,
                resume_id: Some("sess-123".to_string()),
                continue_last: false,
                cwd: Some("/home/user/project".to_string()),
            },
            ControlCommand::CreateSession {
                model: None,
                system_prompt: None,
                token: None,
                resume_id: None,
                continue_last: true,
                cwd: None,
            },
            ControlCommand::AttachSession {
                session_id: "sess-456".to_string(),
            },
            ControlCommand::ProcessTree,
            ControlCommand::KillSession {
                session_id: "sess-789".to_string(),
            },
            ControlCommand::Shutdown,
            ControlCommand::Status,
            ControlCommand::RestartDaemon,
            ControlCommand::ListPlugins,
        ];

        for cmd in &commands {
            let mut buf = Vec::new();
            write_frame(&mut buf, cmd).await.unwrap();
            let mut cursor = std::io::Cursor::new(buf);
            let decoded: ControlCommand = read_frame(&mut cursor).await.unwrap();
            assert_eq!(cmd, &decoded, "round-trip failed for {cmd:?}");
        }
    }

    // ── All ControlResponse variants ────────────────────────────────

    #[tokio::test]
    async fn test_round_trip_all_control_responses() {
        use crate::control::DaemonStatus;
        use crate::control::SessionSummary;

        let responses = vec![
            ControlResponse::Sessions(vec![
                SessionSummary {
                    session_id: "s1".to_string(),
                    model: "sonnet".to_string(),
                    turn_count: 5,
                    last_active: "2026-03-21T12:00:00Z".to_string(),
                    client_count: 1,
                    socket_path: "/tmp/s1.sock".to_string(),
                    state: "active".to_string(),
                },
            ]),
            ControlResponse::Sessions(vec![]),
            ControlResponse::Created {
                session_id: "s2".to_string(),
                socket_path: "/tmp/s2.sock".to_string(),
            },
            ControlResponse::Attached {
                socket_path: "/tmp/s1.sock".to_string(),
            },
            ControlResponse::Tree(vec![]),
            ControlResponse::Killed,
            ControlResponse::ShuttingDown,
            ControlResponse::Status(DaemonStatus {
                uptime_secs: 3600.5,
                session_count: 3,
                total_clients: 7,
                pid: 12345,
            }),
            ControlResponse::Restarting,
            ControlResponse::Plugins(vec![crate::event::PluginSummary {
                name: "test-plugin".to_string(),
                version: "0.1.0".to_string(),
                state: "Active".to_string(),
                tools: vec!["echo".to_string()],
                permissions: vec!["net".to_string()],
            }]),
            ControlResponse::Plugins(vec![]),
            ControlResponse::Error {
                message: "session not found".to_string(),
            },
        ];

        for resp in &responses {
            let mut buf = Vec::new();
            write_frame(&mut buf, resp).await.unwrap();
            let mut cursor = std::io::Cursor::new(buf);
            let decoded: ControlResponse = read_frame(&mut cursor).await.unwrap();
            assert_eq!(resp, &decoded, "round-trip failed for {resp:?}");
        }
    }

    // ── DaemonRequest / AttachResponse ──────────────────────────────

    #[tokio::test]
    async fn test_round_trip_daemon_request() {
        use crate::types::AttachResponse;
        use crate::types::DaemonRequest;

        let requests = vec![
            DaemonRequest::Control {
                command: ControlCommand::ListSessions,
            },
            DaemonRequest::Control {
                command: ControlCommand::Status,
            },
            DaemonRequest::Attach {
                handshake: Handshake {
                    protocol_version: 1,
                    client_name: "test-client".to_string(),
                    token: Some("ucan-token".to_string()),
                    session_id: Some("sess-123".to_string()),
                },
            },
            DaemonRequest::Attach {
                handshake: Handshake {
                    protocol_version: 1,
                    client_name: "anon".to_string(),
                    token: None,
                    session_id: None,
                },
            },
        ];

        for req in &requests {
            let mut buf = Vec::new();
            write_frame(&mut buf, req).await.unwrap();
            let mut cursor = std::io::Cursor::new(buf);
            let decoded: DaemonRequest = read_frame(&mut cursor).await.unwrap();
            assert_eq!(req, &decoded, "round-trip failed for {req:?}");
        }

        // AttachResponse
        let responses = vec![
            AttachResponse::Ok {
                session_id: "sess-789".to_string(),
            },
            AttachResponse::Error {
                message: "no such session".to_string(),
            },
        ];

        for resp in &responses {
            let mut buf = Vec::new();
            write_frame(&mut buf, resp).await.unwrap();
            let mut cursor = std::io::Cursor::new(buf);
            let decoded: AttachResponse = read_frame(&mut cursor).await.unwrap();
            assert_eq!(resp, &decoded, "round-trip failed for {resp:?}");
        }
    }

    // ── Frame edge cases ────────────────────────────────────────────

    #[tokio::test]
    async fn test_read_oversized_length_prefix() {
        // Craft a frame with a length prefix exceeding MAX_FRAME_SIZE
        let len = (MAX_FRAME_SIZE as u32 + 1).to_be_bytes();
        let buf = len.to_vec();
        let mut cursor = std::io::Cursor::new(buf);
        let result: Result<Vec<u8>, _> = read_raw_frame(&mut cursor).await;

        assert!(matches!(result, Err(FrameError::TooLarge { .. })));
    }

    #[tokio::test]
    async fn test_read_truncated_payload() {
        // Length says 100 bytes but only 10 are present
        let mut buf = Vec::new();
        buf.extend_from_slice(&100u32.to_be_bytes());
        buf.extend_from_slice(&[0u8; 10]);
        let mut cursor = std::io::Cursor::new(buf);
        let result: Result<Vec<u8>, _> = read_raw_frame(&mut cursor).await;

        assert!(matches!(result, Err(FrameError::Eof)));
    }

    #[tokio::test]
    async fn test_read_truncated_length_prefix() {
        // Only 2 bytes of the 4-byte length prefix
        let buf = vec![0u8, 5];
        let mut cursor = std::io::Cursor::new(buf);
        let result: Result<Vec<u8>, _> = read_raw_frame(&mut cursor).await;

        assert!(matches!(result, Err(FrameError::Eof)));
    }

    #[tokio::test]
    async fn test_zero_length_frame() {
        let mut buf = Vec::new();
        write_raw_frame(&mut buf, &[]).await.unwrap();

        let mut cursor = std::io::Cursor::new(buf);
        let decoded = read_raw_frame(&mut cursor).await.unwrap();
        assert!(decoded.is_empty());
    }

    #[tokio::test]
    async fn test_invalid_json_frame() {
        // Valid length prefix but garbage JSON
        let garbage = b"not valid json {{{";
        let mut buf = Vec::new();
        buf.extend_from_slice(&(garbage.len() as u32).to_be_bytes());
        buf.extend_from_slice(garbage);
        let mut cursor = std::io::Cursor::new(buf);
        let result: Result<SessionCommand, _> = read_frame(&mut cursor).await;

        assert!(matches!(result, Err(FrameError::Json(_))));
    }

    #[tokio::test]
    async fn test_wrong_type_json_frame() {
        // Valid JSON but wrong type — a DaemonEvent where SessionCommand expected
        let event = DaemonEvent::AgentStart;
        let mut buf = Vec::new();
        write_frame(&mut buf, &event).await.unwrap();

        let mut cursor = std::io::Cursor::new(buf);
        let result: Result<SessionCommand, _> = read_frame(&mut cursor).await;

        // serde should reject this since the discriminant won't match
        assert!(matches!(result, Err(FrameError::Json(_))));
    }

    // ── FrameError Display ──────────────────────────────────────────

    #[test]
    fn test_frame_error_display() {
        let io_err = FrameError::Io(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            "pipe broken",
        ));
        assert!(io_err.to_string().contains("frame IO error"));

        let too_large = FrameError::TooLarge { size: 999 };
        let msg = too_large.to_string();
        assert!(msg.contains("999"), "got: {msg}");
        assert!(msg.contains("too large"), "got: {msg}");

        let eof = FrameError::Eof;
        assert!(eof.to_string().contains("closed"));

        let json_err = FrameError::Json(serde_json::from_str::<String>("{{").unwrap_err());
        assert!(json_err.to_string().contains("JSON"));
    }

    #[test]
    fn test_frame_error_source() {
        let io_err = FrameError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            "test",
        ));
        assert!(std::error::Error::source(&io_err).is_some());

        let json_err = FrameError::Json(serde_json::from_str::<String>("{{").unwrap_err());
        assert!(std::error::Error::source(&json_err).is_some());

        assert!(std::error::Error::source(&FrameError::Eof).is_none());
        assert!(std::error::Error::source(&FrameError::TooLarge { size: 1 }).is_none());
    }

    #[test]
    fn test_unexpected_eof_becomes_frame_eof() {
        let io_err = std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "eof");
        let frame_err: FrameError = io_err.into();
        assert!(matches!(frame_err, FrameError::Eof));
    }

    #[test]
    fn test_other_io_error_stays_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "broken");
        let frame_err: FrameError = io_err.into();
        assert!(matches!(frame_err, FrameError::Io(_)));
    }

    // ── Backward compatibility: missing optional fields ─────────────

    #[test]
    fn test_session_info_missing_optional_fields() {
        // Old daemon sends SessionInfo without newer optional fields
        let json = r#"{"SessionInfo":{"session_id":"s1","model":"sonnet","system_prompt_hash":"abc"}}"#;
        let event: DaemonEvent = serde_json::from_str(json).unwrap();
        match event {
            DaemonEvent::SessionInfo {
                session_id,
                available_models,
                active_account,
                disabled_tools,
                auto_test_command,
                ..
            } => {
                assert_eq!(session_id, "s1");
                assert!(available_models.is_empty());
                assert!(active_account.is_empty());
                assert!(disabled_tools.is_empty());
                assert!(auto_test_command.is_none());
            }
            _ => panic!("expected SessionInfo"),
        }
    }

    #[test]
    fn test_session_summary_missing_state_field() {
        // Old daemon sends SessionSummary without "state" field
        let json = r#"{"session_id":"s1","model":"sonnet","turn_count":3,"last_active":"now","client_count":1,"socket_path":"/tmp/s.sock"}"#;
        let summary: crate::control::SessionSummary = serde_json::from_str(json).unwrap();
        assert_eq!(summary.state, "active"); // default
    }

    #[test]
    fn test_create_session_missing_optional_fields() {
        // Minimal CreateSession without optional fields
        let json = r#"{"CreateSession":{"model":null,"system_prompt":null,"token":null}}"#;
        let cmd: ControlCommand = serde_json::from_str(json).unwrap();
        match cmd {
            ControlCommand::CreateSession {
                resume_id,
                continue_last,
                cwd,
                ..
            } => {
                assert!(resume_id.is_none());
                assert!(!continue_last);
                assert!(cwd.is_none());
            }
            _ => panic!("expected CreateSession"),
        }
    }
}
