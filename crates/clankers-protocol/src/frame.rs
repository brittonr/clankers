//! Length-prefixed JSON frame I/O.
//!
//! All frames are: `[4-byte big-endian length][JSON payload]`.
//! Generic over `AsyncRead`/`AsyncWrite` — no transport dependency.

use std::fmt;

use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;

/// Maximum frame size (10 MB).
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
pub async fn write_frame<W, T>(writer: &mut W, value: &T) -> Result<(), FrameError>
where
    W: AsyncWrite + Unpin,
    T: serde::Serialize,
{
    let data = serde_json::to_vec(value)?;
    if data.len() > MAX_FRAME_SIZE {
        return Err(FrameError::TooLarge { size: data.len() });
    }
    let len = (data.len() as u32).to_be_bytes();
    writer.write_all(&len).await?;
    writer.write_all(&data).await?;
    writer.flush().await?;
    Ok(())
}

/// Read a length-prefixed JSON frame.
///
/// Reads `[4-byte length][JSON]` from `reader`, deserializes to `T`.
pub async fn read_frame<R, T>(reader: &mut R) -> Result<T, FrameError>
where
    R: AsyncRead + Unpin,
    T: serde::de::DeserializeOwned,
{
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;
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
    let len = (data.len() as u32).to_be_bytes();
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
    let len = u32::from_be_bytes(len_buf) as usize;
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

    #[tokio::test]
    async fn test_frame_too_large() {
        let data = vec![0u8; 10_000_001];

        let mut buf = Vec::new();
        let result = write_raw_frame(&mut buf, &data).await;

        assert!(matches!(result, Err(FrameError::TooLarge { size: 10_000_001 })));
    }

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
        ];

        for event in &events {
            let mut buf = Vec::new();
            write_frame(&mut buf, event).await.unwrap();
            let mut cursor = std::io::Cursor::new(buf);
            let decoded: DaemonEvent = read_frame(&mut cursor).await.unwrap();
            assert_eq!(event, &decoded, "round-trip failed for {event:?}");
        }
    }
}
