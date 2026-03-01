//! Router RPC client
//!
//! Connects to a running clankers-router daemon and sends completion requests,
//! model queries, and status checks over iroh QUIC.

use iroh::Endpoint;
use iroh::EndpointAddr;
use iroh::PublicKey;
use serde_json::json;
use tokio::sync::mpsc;
use tracing::debug;
use tracing::info;

use super::ALPN;
use super::MDNS_SERVICE;
use super::protocol::Request;
use super::protocol::Response;
use super::protocol::{self};
use crate::error::Error;
use crate::error::Result;
use crate::model::Model;
use crate::provider::CompletionRequest;
use crate::streaming::StreamEvent;

/// Client for connecting to a clankers-router daemon.
pub struct RpcClient {
    endpoint: Endpoint,
    remote: EndpointAddr,
}

impl RpcClient {
    /// Connect to a router daemon by its node ID and optional direct addresses.
    ///
    /// Creates a lightweight iroh endpoint with mDNS discovery
    /// for finding the daemon on the local network.
    pub async fn connect(node_id: &str) -> Result<Self> {
        Self::connect_with_addrs(node_id, &[]).await
    }

    /// Connect with explicit direct addresses (from daemon.json).
    pub async fn connect_with_addrs(node_id: &str, addrs: &[String]) -> Result<Self> {
        let remote: PublicKey = node_id.parse().map_err(|e| Error::Config {
            message: format!("Invalid node ID '{}': {}", node_id, e),
        })?;

        let secret_key = iroh::SecretKey::generate(&mut rand::rng());

        // Try with mDNS first, fall back to no discovery
        let endpoint = {
            let mdns = iroh::address_lookup::MdnsAddressLookup::builder().service_name(MDNS_SERVICE);
            match Endpoint::builder()
                .secret_key(secret_key.clone())
                .alpns(vec![ALPN.to_vec()])
                .address_lookup(mdns)
                .bind()
                .await
            {
                Ok(ep) => ep,
                Err(_) => {
                    Endpoint::builder().secret_key(secret_key).alpns(vec![ALPN.to_vec()]).bind().await.map_err(|e| {
                        Error::Provider {
                            message: format!("Failed to bind client endpoint: {}", e),
                            status: None,
                        }
                    })?
                }
            }
        };

        // Build EndpointAddr with direct address hints from daemon.json
        let mut addr = iroh::EndpointAddr::new(remote);
        for a in addrs {
            if let Ok(sock) = a.parse::<std::net::SocketAddr>() {
                addr = addr.with_ip_addr(sock);
            }
        }
        if !addrs.is_empty() {
            debug!("Added {} direct address hints", addrs.len());
        }

        info!("RPC client connecting to {}", &node_id[..12.min(node_id.len())]);

        Ok(Self { endpoint, remote: addr })
    }

    /// Send a completion request and stream events back via the channel.
    pub async fn complete(&self, request: CompletionRequest, tx: mpsc::Sender<StreamEvent>) -> Result<()> {
        let params = serde_json::to_value(&request).map_err(|e| Error::Provider {
            message: format!("Failed to serialize request: {}", e),
            status: None,
        })?;

        let rpc_request = Request::new("complete", params);
        let (mut send, mut recv) = self.open_stream().await?;

        // Send request
        let req_bytes = serde_json::to_vec(&rpc_request)?;
        protocol::write_frame(&mut send, &req_bytes).await?;
        send.finish().map_err(|e| Error::Streaming {
            message: format!("Failed to finish send: {}", e),
        })?;

        // Read frames: notifications (stream events) then final response
        loop {
            let data = protocol::read_frame(&mut recv).await?;
            let value: serde_json::Value = serde_json::from_slice(&data)?;

            // Response has "id" field; notifications have "method"
            if value.get("id").is_some() {
                // Final response
                let response: Response = serde_json::from_value(value)?;
                if let Some(err) = response.error {
                    return Err(Error::Provider {
                        message: format!("Router error: {}", err.message),
                        status: None,
                    });
                }
                break;
            }

            // Notification — extract StreamEvent from params
            if let Some(params) = value.get("params") {
                match serde_json::from_value::<StreamEvent>(params.clone()) {
                    Ok(event) => {
                        if tx.send(event).await.is_err() {
                            break; // receiver dropped
                        }
                    }
                    Err(e) => {
                        debug!("Failed to parse stream event: {}", e);
                    }
                }
            }
        }

        Ok(())
    }

    /// List available models from the router.
    pub async fn list_models(&self) -> Result<Vec<Model>> {
        let response = self.simple_rpc("models.list", json!({})).await?;
        let models: Vec<Model> =
            serde_json::from_value(response.result.unwrap_or_default()).map_err(|e| Error::Provider {
                message: format!("Failed to parse models: {}", e),
                status: None,
            })?;
        Ok(models)
    }

    /// Get router status.
    pub async fn status(&self) -> Result<serde_json::Value> {
        let response = self.simple_rpc("status", json!({})).await?;
        Ok(response.result.unwrap_or_default())
    }

    /// Resolve a model name/alias.
    pub async fn resolve(&self, name: &str) -> Result<Option<Model>> {
        let response = self.simple_rpc("resolve", json!({"name": name})).await?;
        if response.is_error() {
            return Ok(None);
        }
        let model: Model =
            serde_json::from_value(response.result.unwrap_or_default()).map_err(|e| Error::Provider {
                message: format!("Failed to parse model: {}", e),
                status: None,
            })?;
        Ok(Some(model))
    }

    /// Check if the daemon is reachable (quick ping via status).
    pub async fn ping(&self) -> bool {
        matches!(tokio::time::timeout(std::time::Duration::from_secs(5), self.status()).await, Ok(Ok(_)))
    }

    // ── internal ────────────────────────────────────────────────────────

    async fn open_stream(&self) -> Result<(iroh::endpoint::SendStream, iroh::endpoint::RecvStream)> {
        let conn = self.endpoint.connect(self.remote.clone(), ALPN).await.map_err(|e| Error::Provider {
            message: format!("Failed to connect to router daemon: {}", e),
            status: None,
        })?;

        conn.open_bi().await.map_err(|e| Error::Provider {
            message: format!("Failed to open stream: {}", e),
            status: None,
        })
    }

    async fn simple_rpc(&self, method: &str, params: serde_json::Value) -> Result<Response> {
        let request = Request::new(method, params);
        let (mut send, mut recv) = self.open_stream().await?;

        let req_bytes = serde_json::to_vec(&request)?;
        protocol::write_frame(&mut send, &req_bytes).await?;
        send.finish().map_err(|e| Error::Streaming {
            message: format!("Failed to finish send: {}", e),
        })?;

        let data = protocol::read_frame(&mut recv).await?;
        let response: Response = serde_json::from_slice(&data)?;

        if let Some(ref err) = response.error {
            return Err(Error::Provider {
                message: format!("Router error: {}", err.message),
                status: None,
            });
        }

        Ok(response)
    }
}

impl Drop for RpcClient {
    fn drop(&mut self) {
        // Endpoint cleanup happens automatically
    }
}
