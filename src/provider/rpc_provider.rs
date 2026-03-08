//! Provider that delegates to a clankers-router daemon over iroh RPC
//!
//! Connects to a running clankers-router daemon and forwards completion
//! requests over QUIC. Falls back to in-process routing if the daemon
//! is unavailable.

use std::sync::Arc;

use async_trait::async_trait;
use clankers_router::rpc::client::RpcClient;
use clankers_router::rpc::daemon::DaemonInfo;
use tokio::sync::mpsc;
use tracing::info;
use tracing::warn;

use crate::error::Result;
use crate::provider::CompletionRequest;
use crate::provider::Model;
use crate::provider::Provider;
use crate::provider::streaming::StreamEvent;

/// Provider that talks to a clankers-router daemon over iroh QUIC RPC.
pub struct RpcProvider {
    client: RpcClient,
    models: Vec<Model>,
}

impl RpcProvider {
    /// Connect to a running daemon and fetch its model list.
    pub async fn connect() -> Option<Arc<dyn Provider>> {
        let info_path = clankers_router::rpc::daemon::daemon_info_path();

        // Try loading daemon info
        let info = DaemonInfo::load(&info_path)?;
        if !info.is_alive() {
            info!("Router daemon pid {} is not alive, cleaning up", info.pid);
            DaemonInfo::remove(&info_path);
            return None;
        }

        // Connect with direct address hints
        let client = match RpcClient::connect_with_addrs(&info.node_id, &info.addrs).await {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed to connect to router daemon: {}", e);
                return None;
            }
        };

        // Verify connectivity
        if !client.ping().await {
            warn!("Router daemon not responding to ping");
            return None;
        }

        // Fetch models
        let router_models = match client.list_models().await {
            Ok(m) => m,
            Err(e) => {
                warn!("Failed to list models from daemon: {}", e);
                return None;
            }
        };

        let models: Vec<Model> = router_models;

        info!("Connected to router daemon (pid {}, {} models)", info.pid, models.len());

        Some(Arc::new(Self { client, models }))
    }

    /// Try to auto-start the daemon, then connect.
    pub async fn auto_start_and_connect() -> Option<Arc<dyn Provider>> {
        // First try connecting to existing daemon
        if let Some(provider) = Self::connect().await {
            return Some(provider);
        }

        // Try auto-starting
        info!("Attempting to auto-start router daemon...");
        clankers_router::rpc::daemon::auto_start_daemon()?;

        // Try connecting again
        Self::connect().await
    }
}

#[async_trait]
impl Provider for RpcProvider {
    async fn complete(&self, request: CompletionRequest, tx: mpsc::Sender<StreamEvent>) -> Result<()> {
        // Convert clankers CompletionRequest → router CompletionRequest
        // Messages must be in Anthropic API format (role + content), not clankers
        // internal AgentMessage enum format.
        let router_request = clankers_router::CompletionRequest {
            model: request.model,
            messages: convert_messages_to_api(&request.messages),
            system_prompt: request.system_prompt,
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            tools: request
                .tools
                .into_iter()
                .map(|t| clankers_router::provider::ToolDefinition {
                    name: t.name,
                    description: t.description,
                    input_schema: t.input_schema,
                })
                .collect(),
            thinking: request.thinking,
            extra_params: Default::default(),
        };

        // Send to daemon and translate streaming events
        let (router_tx, mut router_rx) = mpsc::channel(64);

        let tx_clone = tx.clone();
        let translate_handle = tokio::spawn(async move {
            while let Some(event) = router_rx.recv().await {
                let clankers_event = translate_stream_event(event);
                if tx_clone.send(clankers_event).await.is_err() {
                    break;
                }
            }
        });

        let result = self
            .client
            .complete(router_request, router_tx)
            .await
            .map_err(|e| crate::error::Error::Provider { message: e.to_string() });

        let _ = translate_handle.await;
        result
    }

    fn models(&self) -> &[Model] {
        &self.models
    }

    fn name(&self) -> &str {
        "rpc-router"
    }

    async fn reload_credentials(&self) {
        // Daemon manages its own credentials
    }
}

/// Convert clankers AgentMessage list → Anthropic API format JSON values.
///
/// The router's CompletionRequest expects messages in provider-native format
/// (e.g. `{"role": "user", "content": "..."}`) not clankers's internal enum format.
fn convert_messages_to_api(messages: &[crate::provider::message::AgentMessage]) -> Vec<serde_json::Value> {
    use serde_json::json;

    use crate::provider::message::AgentMessage;

    let mut out = Vec::new();
    for msg in messages {
        match msg {
            AgentMessage::User(user) => {
                let content: Vec<serde_json::Value> = user.content.iter().map(content_to_json).collect();
                out.push(json!({"role": "user", "content": content}));
            }
            AgentMessage::Assistant(assistant) => {
                let content: Vec<serde_json::Value> = assistant.content.iter().map(content_to_json).collect();
                out.push(json!({"role": "assistant", "content": content}));
            }
            AgentMessage::ToolResult(result) => {
                let content_blocks: Vec<serde_json::Value> = result.content.iter().map(content_to_json).collect();
                let mut block = json!({
                    "type": "tool_result",
                    "tool_use_id": result.call_id,
                    "content": content_blocks,
                });
                if result.is_error {
                    block["is_error"] = json!(true);
                }
                out.push(json!({"role": "user", "content": [block]}));
            }
            // Skip metadata messages — not sent to the LLM
            _ => {}
        }
    }
    out
}

/// Convert a single Content block to Anthropic API JSON.
fn content_to_json(content: &crate::provider::message::Content) -> serde_json::Value {
    use serde_json::json;

    use crate::provider::message::Content;
    use crate::provider::message::ImageSource;

    match content {
        Content::Text { text } => json!({"type": "text", "text": text}),
        Content::Image { source } => match source {
            ImageSource::Base64 { media_type, data } => json!({
                "type": "image",
                "source": {"type": "base64", "media_type": media_type, "data": data}
            }),
            ImageSource::Url { url } => json!({"type": "text", "text": format!("[Image URL: {}]", url)}),
        },
        Content::Thinking { thinking } => json!({"type": "thinking", "thinking": thinking, "signature": ""}),
        Content::ToolUse { id, name, input } => json!({
            "type": "tool_use", "id": id, "name": name, "input": input
        }),
        Content::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => {
            let blocks: Vec<serde_json::Value> = content.iter().map(content_to_json).collect();
            let mut v = json!({"type": "tool_result", "tool_use_id": tool_use_id, "content": blocks});
            if let Some(true) = is_error {
                v["is_error"] = json!(true);
            }
            v
        }
    }
}

/// Translate a clankers_router StreamEvent → clankers StreamEvent.
///
/// Same translation as in `src/provider/router.rs::RouterCompatAdapter`.
fn translate_stream_event(event: clankers_router::streaming::StreamEvent) -> StreamEvent {
    use clankers_router::streaming as router_stream;

    use crate::provider::message::Content;
    use crate::provider::streaming as clankers_stream;

    match event {
        router_stream::StreamEvent::MessageStart { message } => StreamEvent::MessageStart {
            message: clankers_stream::MessageMetadata {
                id: message.id,
                model: message.model,
                role: message.role,
            },
        },
        router_stream::StreamEvent::ContentBlockStart { index, content_block } => {
            let block = match content_block {
                router_stream::ContentBlock::Text { text } => Content::Text { text },
                router_stream::ContentBlock::Thinking { thinking } => Content::Thinking { thinking },
                router_stream::ContentBlock::ToolUse { id, name, input } => Content::ToolUse { id, name, input },
            };
            StreamEvent::ContentBlockStart {
                index,
                content_block: block,
            }
        }
        router_stream::StreamEvent::ContentBlockDelta { index, delta } => {
            let d = match delta {
                router_stream::ContentDelta::TextDelta { text } => clankers_stream::ContentDelta::TextDelta { text },
                router_stream::ContentDelta::ThinkingDelta { thinking } => {
                    clankers_stream::ContentDelta::ThinkingDelta { thinking }
                }
                router_stream::ContentDelta::InputJsonDelta { partial_json } => {
                    clankers_stream::ContentDelta::InputJsonDelta { partial_json }
                }
            };
            StreamEvent::ContentBlockDelta { index, delta: d }
        }
        router_stream::StreamEvent::ContentBlockStop { index } => StreamEvent::ContentBlockStop { index },
        router_stream::StreamEvent::MessageDelta { stop_reason, usage } => StreamEvent::MessageDelta {
            stop_reason,
            usage: crate::provider::Usage {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                cache_creation_input_tokens: usage.cache_creation_input_tokens,
                cache_read_input_tokens: usage.cache_read_input_tokens,
            },
        },
        router_stream::StreamEvent::MessageStop => StreamEvent::MessageStop,
        router_stream::StreamEvent::Error { error } => StreamEvent::Error { error },
    }
}
