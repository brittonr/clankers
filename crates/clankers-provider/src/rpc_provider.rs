//! Provider that delegates to a clanker-router daemon over iroh RPC
//!
//! Connects to a running clanker-router daemon and forwards completion
//! requests over QUIC. Falls back to in-process routing if the daemon
//! is unavailable.

use std::sync::Arc;

use async_trait::async_trait;
use clanker_message::streaming::StreamEvent;
use clanker_router::rpc::client::RpcClient;
use clanker_router::rpc::daemon::DaemonInfo;
use tokio::sync::mpsc;
use tracing::info;
use tracing::warn;

use crate::CompletionRequest;
use crate::Model;
use crate::Provider;
use crate::error::Result;

/// Provider that talks to a clanker-router daemon over iroh QUIC RPC.
pub struct RpcProvider {
    client: RpcClient,
    models: Vec<Model>,
}

impl RpcProvider {
    /// Connect to a running daemon and fetch its model list.
    pub async fn connect() -> Option<Arc<dyn Provider>> {
        let info_path = clanker_router::rpc::daemon::daemon_info_path();

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
        clanker_router::rpc::daemon::auto_start_daemon()?;

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
        let router_request = crate::router_request_bridge::build_router_request(request);

        // Send to daemon and translate streaming events
        let (router_tx, mut router_rx) = mpsc::channel(64);

        let tx_clone = tx.clone();
        let translate_handle = tokio::spawn(async move {
            while let Some(event) = router_rx.recv().await {
                if tx_clone.send(StreamEvent::from(event)).await.is_err() {
                    break;
                }
            }
        });

        let result = self.client.complete(router_request, router_tx).await.map_err(crate::error::ProviderError::from);

        translate_handle.await.ok();
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use clanker_message::Content;
    use clanker_message::transcript::AgentMessage;
    use clanker_message::transcript::MessageId;
    use clanker_message::transcript::UserMessage;
    use serde_json::json;

    use crate::CompletionRequest;
    use crate::router_request_bridge::build_router_request;

    fn make_request() -> CompletionRequest {
        CompletionRequest {
            model: "test-model".to_string(),
            messages: vec![AgentMessage::User(UserMessage {
                id: MessageId::new("test-user"),
                content: vec![Content::Text {
                    text: "hello".to_string(),
                }],
                timestamp: chrono::Utc::now(),
            })],
            system_prompt: Some("Be helpful".to_string()),
            max_tokens: Some(128),
            temperature: Some(0.2),
            tools: vec![],
            thinking: None,
            no_cache: false,
            cache_ttl: Some("1h".to_string()),
            extra_params: HashMap::from([("_session_id".to_string(), json!("session-rpc-1"))]),
        }
    }

    #[test]
    fn rpc_request_conversion_preserves_session_id_extra_param() {
        let router_request = build_router_request(make_request());
        assert_eq!(router_request.extra_params.get("_session_id"), Some(&json!("session-rpc-1")));
    }

    #[test]
    fn rpc_request_serialization_preserves_session_id_extra_param() {
        let router_request = build_router_request(make_request());
        let encoded = serde_json::to_string(&router_request).expect("router request should serialize");
        let decoded: clanker_router::CompletionRequest =
            serde_json::from_str(&encoded).expect("router request should deserialize");
        assert_eq!(decoded.extra_params.get("_session_id"), Some(&json!("session-rpc-1")));
    }
}
