//! Router RPC server
//!
//! Accepts iroh QUIC connections and handles completion requests,
//! model listing, and status queries.
//!
//! Provides two APIs:
//! - [`RpcServer`] — standalone server that owns its own iroh endpoint
//! - [`RpcHandler`] — `ProtocolHandler` impl for use with iroh's `protocol::Router`

use std::sync::Arc;

use iroh::Endpoint;
use iroh::endpoint::Connection;
use iroh::protocol::AcceptError;
use iroh::protocol::ProtocolHandler;
use serde_json::json;
use tokio::sync::mpsc;
use tracing::info;
use tracing::warn;

use super::ALPN;
use super::MDNS_SERVICE;
use super::protocol::Notification;
use super::protocol::Request;
use super::protocol::Response;
use super::protocol::{self};
use crate::Router;
use crate::error::Error;
use crate::error::Result;
use crate::provider::CompletionRequest;
use crate::streaming::StreamEvent;

/// RPC server wrapping a [`Router`].
pub struct RpcServer {
    router: Arc<Router>,
    endpoint: Endpoint,
}

impl RpcServer {
    /// Create a new RPC server with a fresh iroh endpoint.
    pub async fn new(router: Router) -> Result<Self> {
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
                Ok(ep) => {
                    info!("RPC server bound with mDNS discovery");
                    ep
                }
                Err(e) => {
                    warn!("mDNS unavailable ({}), binding without discovery", e);
                    Endpoint::builder().secret_key(secret_key).alpns(vec![ALPN.to_vec()]).bind().await.map_err(|e| {
                        Error::Provider {
                            message: format!("Failed to bind iroh endpoint: {}", e),
                            status: None,
                        }
                    })?
                }
            }
        };

        Ok(Self {
            router: Arc::new(router),
            endpoint,
        })
    }

    /// The server's iroh node ID (hex string).
    pub fn node_id(&self) -> String {
        self.endpoint.id().to_string()
    }

    /// Get the direct socket addresses the endpoint is bound to.
    ///
    /// Rewrites unspecified addresses (0.0.0.0, [::]) to localhost
    /// so clients can actually connect.
    pub fn bound_addrs(&self) -> Vec<String> {
        self.endpoint
            .bound_sockets()
            .into_iter()
            .map(|mut a| {
                if a.ip().is_unspecified() {
                    a.set_ip(if a.is_ipv4() {
                        std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
                    } else {
                        std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST)
                    });
                }
                a.to_string()
            })
            .collect()
    }

    /// Accept connections and handle RPC requests forever.
    pub async fn serve(self) -> Result<()> {
        info!("Router RPC server listening as {}", self.endpoint.id().fmt_short());

        loop {
            let incoming = match self.endpoint.accept().await {
                Some(inc) => inc,
                None => break,
            };

            let router = Arc::clone(&self.router);
            tokio::spawn(async move {
                if let Err(e) = handle_connection(incoming, router).await {
                    warn!("RPC connection error: {}", e);
                }
            });
        }

        Ok(())
    }
}

// ── RpcHandler (ProtocolHandler for iroh's protocol::Router) ────────────

/// RPC protocol handler for use with iroh's `protocol::Router`.
///
/// Unlike `RpcServer` which owns its own endpoint, `RpcHandler` implements
/// `ProtocolHandler` and can be registered alongside other protocols
/// (like the HTTP tunnel) on a shared iroh endpoint.
#[derive(Debug)]
pub struct RpcHandler {
    router: Arc<Router>,
}

impl RpcHandler {
    /// Create a new RPC handler wrapping a shared router.
    ///
    /// Accepts an `Arc<Router>` so the same instance can be shared with
    /// the HTTP proxy when both are co-hosted in `run_serve()`.
    pub fn new(router: Arc<Router>) -> Self {
        Self { router }
    }
}

impl ProtocolHandler for RpcHandler {
    async fn accept(&self, connection: Connection) -> std::result::Result<(), AcceptError> {
        let remote = connection.remote_id();
        info!("RPC: accepted connection from {}", remote.fmt_short());

        let router = Arc::clone(&self.router);
        serve_connection(connection, router)
            .await
            .map_err(|e| AcceptError::from_err(Box::new(std::io::Error::other(e.to_string()))))
    }
}

// ── Connection handling (shared between RpcServer and RpcHandler) ───────

async fn handle_connection(incoming: iroh::endpoint::Incoming, router: Arc<Router>) -> Result<()> {
    let conn = incoming.await.map_err(|e| Error::Provider {
        message: format!("Connection failed: {}", e),
        status: None,
    })?;

    serve_connection(conn, router).await
}

/// Serve RPC requests on an established connection.
///
/// Reads bidirectional streams, dispatches requests, and spawns handlers.
/// Shared by both `RpcServer` (from incoming) and `RpcHandler` (from ProtocolHandler).
async fn serve_connection(conn: Connection, router: Arc<Router>) -> Result<()> {
    let remote = conn.remote_id();
    info!("Serving RPC connection from {}", remote.fmt_short());

    loop {
        let (send, mut recv) = match conn.accept_bi().await {
            Ok(s) => s,
            Err(_) => break,
        };

        let data = match protocol::read_frame(&mut recv).await {
            Ok(d) => d,
            Err(_) => break,
        };

        let request: Request = match serde_json::from_slice(&data) {
            Ok(r) => r,
            Err(e) => {
                warn!("Bad request frame: {}", e);
                continue;
            }
        };

        let router = Arc::clone(&router);
        tokio::spawn(async move {
            handle_request(request, router, send).await;
        });
    }

    Ok(())
}

async fn handle_request(request: Request, router: Arc<Router>, mut send: iroh::endpoint::SendStream) {
    let response = match request.method.as_str() {
        "complete" => {
            handle_complete(&request, &router, &mut send).await;
            return; // complete sends its own response
        }
        "models.list" => handle_models_list(&request, &router),
        "status" => handle_status(&request, &router),
        "resolve" => handle_resolve(&request, &router),
        _ => Response::error(request.id, -32601, format!("Unknown method: {}", request.method)),
    };

    let _ = send_response(&mut send, &response).await;
    let _ = send.finish();
}

// ── complete (streaming) ────────────────────────────────────────────────

async fn handle_complete(request: &Request, router: &Router, send: &mut iroh::endpoint::SendStream) {
    let completion_request: CompletionRequest = match serde_json::from_value(request.params.clone()) {
        Ok(r) => r,
        Err(e) => {
            let resp = Response::error(request.id, -32602, format!("Invalid params: {}", e));
            let _ = send_response(send, &resp).await;
            let _ = send.finish();
            return;
        }
    };

    let req_id = request.id;
    let (tx, mut rx) = mpsc::channel::<StreamEvent>(64);

    // Drive completion and event forwarding concurrently using select.
    let complete_fut = router.complete(completion_request, tx);
    tokio::pin!(complete_fut);

    let mut complete_done = false;
    let mut complete_result: std::result::Result<(), Error> = Ok(());

    loop {
        tokio::select! {
            result = &mut complete_fut, if !complete_done => {
                complete_result = result;
                complete_done = true;
                // Don't break — drain remaining events from rx
            }
            event = rx.recv() => {
                match event {
                    Some(event) => {
                        let notification = Notification {
                            method: "stream.event".to_string(),
                            params: match serde_json::to_value(&event) {
                                Ok(v) => v,
                                Err(_) => continue,
                            },
                        };
                        let bytes = match serde_json::to_vec(&notification) {
                            Ok(b) => b,
                            Err(_) => continue,
                        };
                        if protocol::write_frame(send, &bytes).await.is_err() {
                            break;
                        }
                    }
                    None => break, // tx dropped, all events sent
                }
            }
        }
    }

    let response = match complete_result {
        Ok(()) => Response::success(req_id, json!({"status": "complete"})),
        Err(e) => Response::error(req_id, -32000, e.to_string()),
    };
    let _ = send_response(send, &response).await;
    let _ = send.finish();
}

// ── models.list ─────────────────────────────────────────────────────────

fn handle_models_list(request: &Request, router: &Router) -> Response {
    let models = router.list_models();
    let json_models = serde_json::to_value(models).unwrap_or_default();
    Response::success(request.id, json_models)
}

// ── status ──────────────────────────────────────────────────────────────

fn handle_status(request: &Request, router: &Router) -> Response {
    let providers = router.provider_names();
    let model_count = router.list_models().len();
    Response::success(
        request.id,
        json!({
            "status": "running",
            "providers": providers,
            "model_count": model_count,
            "default_model": router.default_model(),
        }),
    )
}

// ── resolve ─────────────────────────────────────────────────────────────

fn handle_resolve(request: &Request, router: &Router) -> Response {
    let name = request.params.get("name").and_then(|v| v.as_str()).unwrap_or("");

    match router.resolve_model(name) {
        Some(model) => Response::success(request.id, serde_json::to_value(model).unwrap_or_default()),
        None => Response::error(request.id, -32001, format!("Model not found: {}", name)),
    }
}

// ── helpers ─────────────────────────────────────────────────────────────

async fn send_response(send: &mut iroh::endpoint::SendStream, response: &Response) -> Result<()> {
    let bytes = serde_json::to_vec(response).map_err(|e| Error::Streaming {
        message: format!("Failed to serialize response: {}", e),
    })?;
    protocol::write_frame(send, &bytes).await
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use serde_json::json;

    use super::*;
    use crate::model::Model;
    use crate::provider::CompletionRequest;
    use crate::provider::Provider;
    use crate::provider::Usage;
    use crate::streaming::ContentDelta;
    use crate::streaming::MessageMetadata;
    use crate::streaming::StreamEvent;

    struct MockProvider {
        name: String,
        models: Vec<Model>,
    }

    #[async_trait]
    impl Provider for MockProvider {
        async fn complete(&self, _request: CompletionRequest, tx: mpsc::Sender<StreamEvent>) -> Result<()> {
            let _ = tx
                .send(StreamEvent::MessageStart {
                    message: MessageMetadata {
                        id: "msg-1".into(),
                        model: "test-model".into(),
                        role: "assistant".into(),
                    },
                })
                .await;
            let _ = tx
                .send(StreamEvent::ContentBlockDelta {
                    index: 0,
                    delta: ContentDelta::TextDelta {
                        text: format!("Hello from {}", self.name),
                    },
                })
                .await;
            let _ = tx
                .send(StreamEvent::MessageDelta {
                    stop_reason: Some("end_turn".into()),
                    usage: Usage {
                        input_tokens: 10,
                        output_tokens: 5,
                        ..Default::default()
                    },
                })
                .await;
            let _ = tx.send(StreamEvent::MessageStop).await;
            Ok(())
        }

        fn models(&self) -> &[Model] {
            &self.models
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    fn make_model(id: &str, provider: &str) -> Model {
        Model {
            id: id.into(),
            name: id.into(),
            provider: provider.into(),
            max_input_tokens: 128_000,
            max_output_tokens: 8_192,
            supports_thinking: false,
            supports_images: false,
            supports_tools: true,
            input_cost_per_mtok: None,
            output_cost_per_mtok: None,
        }
    }

    fn make_router() -> Router {
        let mut router = Router::new("test-model");
        router.register_provider(Arc::new(MockProvider {
            name: "test".into(),
            models: vec![make_model("test-model", "test"), make_model("test-fast", "test")],
        }));
        router
    }

    #[test]
    fn test_handle_models_list() {
        let router = make_router();
        let req = Request::new("models.list", json!({}));
        let resp = handle_models_list(&req, &router);

        assert!(!resp.is_error());
        let models: Vec<Model> = serde_json::from_value(resp.result.unwrap()).unwrap();
        assert_eq!(models.len(), 2);
        assert!(models.iter().any(|m| m.id == "test-model"));
        assert!(models.iter().any(|m| m.id == "test-fast"));
    }

    #[test]
    fn test_handle_status() {
        let router = make_router();
        let req = Request::new("status", json!({}));
        let resp = handle_status(&req, &router);

        assert!(!resp.is_error());
        let result = resp.result.unwrap();
        assert_eq!(result["status"], "running");
        assert_eq!(result["model_count"], 2);
        assert_eq!(result["default_model"], "test-model");
        let providers = result["providers"].as_array().unwrap();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0], "test");
    }

    #[test]
    fn test_handle_resolve_found() {
        let router = make_router();
        let req = Request::new("resolve", json!({"name": "test-model"}));
        let resp = handle_resolve(&req, &router);

        assert!(!resp.is_error());
        let model: Model = serde_json::from_value(resp.result.unwrap()).unwrap();
        assert_eq!(model.id, "test-model");
        assert_eq!(model.provider, "test");
    }

    #[test]
    fn test_handle_resolve_not_found() {
        let router = make_router();
        let req = Request::new("resolve", json!({"name": "nonexistent"}));
        let resp = handle_resolve(&req, &router);

        assert!(resp.is_error());
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32001);
        assert!(err.message.contains("nonexistent"));
    }

    #[test]
    fn test_handle_resolve_missing_name_field() {
        let router = make_router();
        // No "name" field → defaults to empty string → substring matches something
        let req = Request::new("resolve", json!({}));
        let resp = handle_resolve(&req, &router);

        // Empty name matches via substring logic (all IDs contain "")
        assert!(!resp.is_error());
        let model: Model = serde_json::from_value(resp.result.unwrap()).unwrap();
        // Should be one of our registered models
        assert!(model.id == "test-model" || model.id == "test-fast", "unexpected model: {}", model.id,);
    }

    #[test]
    fn test_rpc_handler_new() {
        let router = Arc::new(make_router());
        let handler = RpcHandler::new(router);
        // Just verify it doesn't panic and is Debug
        let _ = format!("{:?}", handler);
    }
}
