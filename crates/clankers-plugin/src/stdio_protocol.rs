use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;

pub const STDIO_PLUGIN_PROTOCOL_VERSION: u32 = 1;
pub const MAX_STDIO_PLUGIN_FRAME_BYTES: usize = 10_000_000;

#[derive(Debug)]
pub enum StdioProtocolError {
    Io(std::io::Error),
    Json(serde_json::Error),
    TooLarge { size: usize },
    UnsupportedProtocol { version: u32 },
}

impl std::fmt::Display for StdioProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "stdio protocol IO error: {error}"),
            Self::Json(error) => write!(f, "stdio protocol JSON error: {error}"),
            Self::TooLarge { size } => write!(f, "stdio protocol frame too large: {size} bytes"),
            Self::UnsupportedProtocol { version } => {
                write!(f, "unsupported stdio plugin protocol version: {version}")
            }
        }
    }
}

impl std::error::Error for StdioProtocolError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::TooLarge { .. } | Self::UnsupportedProtocol { .. } => None,
        }
    }
}

impl From<std::io::Error> for StdioProtocolError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for StdioProtocolError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PluginRuntimeMode {
    Standalone,
    Daemon,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginEventEnvelope {
    pub name: String,
    pub data: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegisteredTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HostToPluginFrame {
    Hello {
        plugin_protocol: u32,
        plugin: String,
        cwd: String,
        mode: PluginRuntimeMode,
    },
    Event {
        plugin_protocol: u32,
        event: PluginEventEnvelope,
    },
    ToolInvoke {
        plugin_protocol: u32,
        call_id: String,
        tool: String,
        args: Value,
    },
    ToolCancel {
        plugin_protocol: u32,
        call_id: String,
        reason: String,
    },
    Shutdown {
        plugin_protocol: u32,
        reason: String,
    },
}

impl HostToPluginFrame {
    pub const fn plugin_protocol(&self) -> u32 {
        match self {
            Self::Hello { plugin_protocol, .. }
            | Self::Event { plugin_protocol, .. }
            | Self::ToolInvoke { plugin_protocol, .. }
            | Self::ToolCancel { plugin_protocol, .. }
            | Self::Shutdown { plugin_protocol, .. } => *plugin_protocol,
        }
    }

    pub fn ensure_supported_protocol(&self) -> Result<(), StdioProtocolError> {
        ensure_protocol(self.plugin_protocol())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PluginToHostFrame {
    Hello {
        plugin_protocol: u32,
        plugin: String,
        version: String,
    },
    Ready {
        plugin_protocol: u32,
    },
    RegisterTools {
        plugin_protocol: u32,
        tools: Vec<RegisteredTool>,
    },
    UnregisterTools {
        plugin_protocol: u32,
        tools: Vec<String>,
    },
    SubscribeEvents {
        plugin_protocol: u32,
        events: Vec<String>,
    },
    ToolProgress {
        plugin_protocol: u32,
        call_id: String,
        message: String,
    },
    ToolResult {
        plugin_protocol: u32,
        call_id: String,
        content: Value,
    },
    ToolError {
        plugin_protocol: u32,
        call_id: String,
        message: String,
    },
    ToolCancelled {
        plugin_protocol: u32,
        call_id: String,
    },
    Ui {
        plugin_protocol: u32,
        actions: Vec<Value>,
    },
    Display {
        plugin_protocol: u32,
        message: String,
    },
}

impl PluginToHostFrame {
    pub const fn plugin_protocol(&self) -> u32 {
        match self {
            Self::Hello { plugin_protocol, .. }
            | Self::Ready { plugin_protocol }
            | Self::RegisterTools { plugin_protocol, .. }
            | Self::UnregisterTools { plugin_protocol, .. }
            | Self::SubscribeEvents { plugin_protocol, .. }
            | Self::ToolProgress { plugin_protocol, .. }
            | Self::ToolResult { plugin_protocol, .. }
            | Self::ToolError { plugin_protocol, .. }
            | Self::ToolCancelled { plugin_protocol, .. }
            | Self::Ui { plugin_protocol, .. }
            | Self::Display { plugin_protocol, .. } => *plugin_protocol,
        }
    }

    pub fn ensure_supported_protocol(&self) -> Result<(), StdioProtocolError> {
        ensure_protocol(self.plugin_protocol())
    }
}

pub async fn write_stdio_plugin_frame<W: AsyncWrite + Unpin>(
    writer: &mut W,
    frame: &impl Serialize,
) -> Result<(), StdioProtocolError> {
    let payload = serde_json::to_vec(frame)?;
    if payload.len() > MAX_STDIO_PLUGIN_FRAME_BYTES {
        return Err(StdioProtocolError::TooLarge { size: payload.len() });
    }

    writer.write_all(&(payload.len() as u32).to_be_bytes()).await?;
    writer.write_all(&payload).await?;
    Ok(())
}

pub async fn read_host_to_plugin_frame<R: AsyncRead + Unpin>(
    reader: &mut R,
) -> Result<HostToPluginFrame, StdioProtocolError> {
    let frame: HostToPluginFrame = read_stdio_plugin_frame(reader).await?;
    frame.ensure_supported_protocol()?;
    Ok(frame)
}

pub async fn read_plugin_to_host_frame<R: AsyncRead + Unpin>(
    reader: &mut R,
) -> Result<PluginToHostFrame, StdioProtocolError> {
    let frame: PluginToHostFrame = read_stdio_plugin_frame(reader).await?;
    frame.ensure_supported_protocol()?;
    Ok(frame)
}

async fn read_stdio_plugin_frame<R: AsyncRead + Unpin, T: serde::de::DeserializeOwned>(
    reader: &mut R,
) -> Result<T, StdioProtocolError> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await?;
    let len = usize::try_from(u32::from_be_bytes(len_buf)).unwrap_or(0);
    if len > MAX_STDIO_PLUGIN_FRAME_BYTES {
        return Err(StdioProtocolError::TooLarge { size: len });
    }

    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload).await?;
    Ok(serde_json::from_slice(&payload)?)
}

fn ensure_protocol(version: u32) -> Result<(), StdioProtocolError> {
    if version == STDIO_PLUGIN_PROTOCOL_VERSION {
        Ok(())
    } else {
        Err(StdioProtocolError::UnsupportedProtocol { version })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tokio::io::AsyncWriteExt;

    use super::*;

    #[tokio::test]
    async fn host_frames_round_trip() {
        let frames = vec![
            HostToPluginFrame::Hello {
                plugin_protocol: STDIO_PLUGIN_PROTOCOL_VERSION,
                plugin: "github".to_string(),
                cwd: "/tmp/worktree".to_string(),
                mode: PluginRuntimeMode::Daemon,
            },
            HostToPluginFrame::Event {
                plugin_protocol: STDIO_PLUGIN_PROTOCOL_VERSION,
                event: PluginEventEnvelope {
                    name: "tool_call".to_string(),
                    data: json!({"tool": "read"}),
                },
            },
            HostToPluginFrame::ToolInvoke {
                plugin_protocol: STDIO_PLUGIN_PROTOCOL_VERSION,
                call_id: "call-1".to_string(),
                tool: "github_pr_list".to_string(),
                args: json!({"owner": "foo", "repo": "bar"}),
            },
            HostToPluginFrame::ToolCancel {
                plugin_protocol: STDIO_PLUGIN_PROTOCOL_VERSION,
                call_id: "call-1".to_string(),
                reason: "user interrupt".to_string(),
            },
            HostToPluginFrame::Shutdown {
                plugin_protocol: STDIO_PLUGIN_PROTOCOL_VERSION,
                reason: "normal exit".to_string(),
            },
        ];

        for frame in frames {
            let (mut writer, mut reader) = tokio::io::duplex(4096);
            write_stdio_plugin_frame(&mut writer, &frame).await.unwrap();
            writer.shutdown().await.unwrap();
            let decoded = read_host_to_plugin_frame(&mut reader).await.unwrap();
            assert_eq!(decoded, frame);
        }
    }

    #[tokio::test]
    async fn plugin_frames_round_trip() {
        let frames = vec![
            PluginToHostFrame::Hello {
                plugin_protocol: STDIO_PLUGIN_PROTOCOL_VERSION,
                plugin: "github".to_string(),
                version: "1.2.3".to_string(),
            },
            PluginToHostFrame::Ready {
                plugin_protocol: STDIO_PLUGIN_PROTOCOL_VERSION,
            },
            PluginToHostFrame::RegisterTools {
                plugin_protocol: STDIO_PLUGIN_PROTOCOL_VERSION,
                tools: vec![RegisteredTool {
                    name: "github_pr_list".to_string(),
                    description: "List pull requests".to_string(),
                    input_schema: json!({"type": "object"}),
                }],
            },
            PluginToHostFrame::UnregisterTools {
                plugin_protocol: STDIO_PLUGIN_PROTOCOL_VERSION,
                tools: vec!["github_pr_list".to_string()],
            },
            PluginToHostFrame::SubscribeEvents {
                plugin_protocol: STDIO_PLUGIN_PROTOCOL_VERSION,
                events: vec!["tool_call".to_string(), "agent_start".to_string()],
            },
            PluginToHostFrame::ToolProgress {
                plugin_protocol: STDIO_PLUGIN_PROTOCOL_VERSION,
                call_id: "call-1".to_string(),
                message: "working".to_string(),
            },
            PluginToHostFrame::ToolResult {
                plugin_protocol: STDIO_PLUGIN_PROTOCOL_VERSION,
                call_id: "call-1".to_string(),
                content: json!({"items": []}),
            },
            PluginToHostFrame::ToolError {
                plugin_protocol: STDIO_PLUGIN_PROTOCOL_VERSION,
                call_id: "call-1".to_string(),
                message: "boom".to_string(),
            },
            PluginToHostFrame::ToolCancelled {
                plugin_protocol: STDIO_PLUGIN_PROTOCOL_VERSION,
                call_id: "call-1".to_string(),
            },
            PluginToHostFrame::Ui {
                plugin_protocol: STDIO_PLUGIN_PROTOCOL_VERSION,
                actions: vec![json!({"action": "notify", "message": "Done", "level": "info"})],
            },
            PluginToHostFrame::Display {
                plugin_protocol: STDIO_PLUGIN_PROTOCOL_VERSION,
                message: "hello".to_string(),
            },
        ];

        for frame in frames {
            let (mut writer, mut reader) = tokio::io::duplex(4096);
            write_stdio_plugin_frame(&mut writer, &frame).await.unwrap();
            writer.shutdown().await.unwrap();
            let decoded = read_plugin_to_host_frame(&mut reader).await.unwrap();
            assert_eq!(decoded, frame);
        }
    }

    #[tokio::test]
    async fn rejects_host_frames_with_wrong_protocol_version() {
        let frame = HostToPluginFrame::Hello {
            plugin_protocol: 99,
            plugin: "github".to_string(),
            cwd: "/tmp/worktree".to_string(),
            mode: PluginRuntimeMode::Standalone,
        };
        let (mut writer, mut reader) = tokio::io::duplex(4096);
        write_stdio_plugin_frame(&mut writer, &frame).await.unwrap();
        writer.shutdown().await.unwrap();

        let error = read_host_to_plugin_frame(&mut reader).await.unwrap_err();
        assert!(matches!(error, StdioProtocolError::UnsupportedProtocol { version: 99 }));
    }

    #[tokio::test]
    async fn rejects_plugin_frames_with_wrong_protocol_version() {
        let frame = PluginToHostFrame::Ready { plugin_protocol: 42 };
        let (mut writer, mut reader) = tokio::io::duplex(4096);
        write_stdio_plugin_frame(&mut writer, &frame).await.unwrap();
        writer.shutdown().await.unwrap();

        let error = read_plugin_to_host_frame(&mut reader).await.unwrap_err();
        assert!(matches!(error, StdioProtocolError::UnsupportedProtocol { version: 42 }));
    }

    #[tokio::test]
    async fn rejects_malformed_json_frames() {
        let (mut writer, mut reader) = tokio::io::duplex(4096);
        let payload = b"{not-json";
        writer.write_all(&(payload.len() as u32).to_be_bytes()).await.unwrap();
        writer.write_all(payload).await.unwrap();
        writer.shutdown().await.unwrap();

        let error = read_plugin_to_host_frame(&mut reader).await.unwrap_err();
        assert!(matches!(error, StdioProtocolError::Json(_)));
    }
}
