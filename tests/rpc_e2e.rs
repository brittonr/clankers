//! E2E tests for iroh P2P RPC
//!
//! These tests spin up an in-process server + client using iroh endpoints
//! and exercise the full RPC protocol including the streaming prompt flow.
//!
//! The prompt test uses a mock provider — no real API key is needed.

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use clankers::modes::rpc::iroh::*;
    use clankers::modes::rpc::protocol::Request;
    use clankers::provider::CompletionRequest;
    use clankers::provider::Model;
    use clankers::provider::Provider;
    use clankers::provider::Usage;
    use clankers::provider::message::AgentMessage;
    use clankers::provider::message::Content;
    use clankers::provider::streaming::ContentDelta;
    use clankers::provider::streaming::MessageMetadata;
    use clankers::provider::streaming::StreamEvent;
    use serde_json::json;
    use tokio::sync::mpsc;

    // ── Mock provider ───────────────────────────────────────────────────

    /// A deterministic mock provider that echoes the user's message back.
    struct EchoProvider;

    #[async_trait]
    impl Provider for EchoProvider {
        async fn complete(
            &self,
            request: CompletionRequest,
            tx: mpsc::Sender<StreamEvent>,
        ) -> clankers::provider::error::Result<()> {
            // Extract the last user message
            let user_text = request
                .messages
                .iter()
                .rev()
                .find_map(|m| {
                    if let AgentMessage::User(content) = m {
                        content.content.iter().find_map(|c| {
                            if let Content::Text { text } = c {
                                Some(text.clone())
                            } else {
                                None
                            }
                        })
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| "echo".to_string());

            let reply = format!("Echo: {}", user_text);

            let _ = tx
                .send(StreamEvent::MessageStart {
                    message: MessageMetadata {
                        id: "mock-1".to_string(),
                        model: "mock".to_string(),
                        role: "assistant".to_string(),
                    },
                })
                .await;

            let _ = tx
                .send(StreamEvent::ContentBlockStart {
                    index: 0,
                    content_block: Content::Text { text: String::new() },
                })
                .await;

            // Stream word by word
            for (i, word) in reply.split_whitespace().enumerate() {
                let chunk = if i == 0 { word.to_string() } else { format!(" {}", word) };
                let _ = tx
                    .send(StreamEvent::ContentBlockDelta {
                        index: 0,
                        delta: ContentDelta::TextDelta { text: chunk },
                    })
                    .await;
            }

            let _ = tx.send(StreamEvent::ContentBlockStop { index: 0 }).await;

            let _ = tx
                .send(StreamEvent::MessageDelta {
                    stop_reason: Some("end_turn".to_string()),
                    usage: Usage {
                        input_tokens: 10,
                        output_tokens: 5,
                        cache_creation_input_tokens: 0,
                        cache_read_input_tokens: 0,
                    },
                })
                .await;

            let _ = tx.send(StreamEvent::MessageStop).await;

            Ok(())
        }

        fn models(&self) -> &[Model] {
            &[]
        }

        fn name(&self) -> &str {
            "mock"
        }
    }

    // ── Helpers ─────────────────────────────────────────────────────────

    /// Create a fresh identity (ephemeral, not persisted).
    fn ephemeral_identity() -> Identity {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        Identity::load_or_generate(tmp.path())
    }

    /// Start a server with the given state and return the server's full
    /// `EndpointAddr` (including direct addresses) so clients can connect.
    async fn start_test_server(state: Arc<ServerState>) -> iroh::EndpointAddr {
        let id = ephemeral_identity();
        let endpoint = start_endpoint_no_mdns(&id).await.unwrap();
        let addr = endpoint.addr();
        tokio::spawn(async move {
            serve_rpc(endpoint, state).await.unwrap();
        });
        addr
    }

    fn metadata_only_state() -> Arc<ServerState> {
        Arc::new(ServerState {
            meta: NodeMeta {
                tags: vec!["test".to_string()],
                agent_names: vec![],
            },
            agent: None,
            acl: AccessControl::open(),
            receive_dir: None,
        })
    }

    fn agent_state() -> Arc<ServerState> {
        let provider: Arc<dyn Provider> = Arc::new(EchoProvider);
        let settings = clankers::config::settings::Settings::default();
        Arc::new(ServerState {
            meta: NodeMeta {
                tags: vec!["test".to_string(), "echo".to_string()],
                agent_names: vec!["echo-agent".to_string()],
            },
            agent: Some(RpcContext {
                provider,
                tools: vec![], // no tools for the echo test
                settings,
                model: "mock-model".to_string(),
                system_prompt: "You are an echo bot.".to_string(),
            }),
            acl: AccessControl::open(),
            receive_dir: None,
        })
    }

    // ── Tests ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_rpc_ping() {
        let state = metadata_only_state();
        let server_addr = start_test_server(state).await;

        let client_id = ephemeral_identity();
        let client_ep = start_endpoint_no_mdns(&client_id).await.unwrap();
        let request = Request::new("ping", json!({}));
        let response = send_rpc(&client_ep, server_addr.clone(), &request).await.unwrap();
        assert_eq!(response.ok.unwrap(), json!("pong"));
    }

    #[tokio::test]
    async fn test_rpc_version() {
        let state = metadata_only_state();
        let server_addr = start_test_server(state).await;

        let client_id = ephemeral_identity();
        let client_ep = start_endpoint_no_mdns(&client_id).await.unwrap();
        let request = Request::new("version", json!({}));
        let response = send_rpc(&client_ep, server_addr.clone(), &request).await.unwrap();
        let result = response.ok.unwrap();
        assert_eq!(result["name"], "clankers");
        assert!(result["version"].is_string());
    }

    #[tokio::test]
    async fn test_rpc_status_with_agent() {
        let state = agent_state();
        let server_addr = start_test_server(state).await;

        let client_id = ephemeral_identity();
        let client_ep = start_endpoint_no_mdns(&client_id).await.unwrap();
        let request = Request::new("status", json!({}));
        let response = send_rpc(&client_ep, server_addr.clone(), &request).await.unwrap();
        let result = response.ok.unwrap();
        assert_eq!(result["accepts_prompts"], true);
        assert_eq!(result["model"], "mock-model");
        assert_eq!(result["tags"], json!(["test", "echo"]));
        assert_eq!(result["agents"], json!(["echo-agent"]));
    }

    #[tokio::test]
    async fn test_rpc_status_without_agent() {
        let state = metadata_only_state();
        let server_addr = start_test_server(state).await;

        let client_id = ephemeral_identity();
        let client_ep = start_endpoint_no_mdns(&client_id).await.unwrap();
        let request = Request::new("status", json!({}));
        let response = send_rpc(&client_ep, server_addr.clone(), &request).await.unwrap();
        let result = response.ok.unwrap();
        assert_eq!(result["accepts_prompts"], false);
    }

    #[tokio::test]
    async fn test_rpc_method_not_found() {
        let state = metadata_only_state();
        let server_addr = start_test_server(state).await;

        let client_id = ephemeral_identity();
        let client_ep = start_endpoint_no_mdns(&client_id).await.unwrap();
        let request = Request::new("nonexistent", json!({}));
        let response = send_rpc(&client_ep, server_addr.clone(), &request).await.unwrap();

        let err = response.error.unwrap();
        assert!(err.contains("not found") || err.contains("unknown"), "expected method-not-found error, got: {err}");
    }

    #[tokio::test]
    async fn test_rpc_prompt_streaming() {
        let state = agent_state();
        let server_addr = start_test_server(state).await;

        let client_id = ephemeral_identity();
        let client_ep = start_endpoint_no_mdns(&client_id).await.unwrap();
        let request = Request::new("prompt", json!({ "text": "hello world" }));

        let mut deltas = Vec::new();
        let (notifications, response) = send_rpc_streaming(&client_ep, server_addr.clone(), &request, |notification| {
            if let Some(method) = notification.get("method").and_then(|v| v.as_str())
                && method == "agent.text_delta"
                && let Some(text) = notification.get("params").and_then(|p| p.get("text")).and_then(|v| v.as_str())
            {
                deltas.push(text.to_string());
            }
        })
        .await
        .unwrap();

        // Should have received text delta notifications
        assert!(!notifications.is_empty(), "Expected streaming notifications");

        // Final response should be successful
        let result = response.ok.unwrap();
        assert_eq!(result["status"], "complete");

        // The collected text should contain our echo
        let text = result["text"].as_str().unwrap();
        assert!(
            text.contains("Echo:") && text.contains("hello world"),
            "Expected echo of 'hello world', got: {}",
            text
        );
    }

    #[tokio::test]
    async fn test_rpc_prompt_without_agent() {
        let state = metadata_only_state(); // no agent context
        let server_addr = start_test_server(state).await;

        let client_id = ephemeral_identity();
        let client_ep = start_endpoint_no_mdns(&client_id).await.unwrap();
        let request = Request::new("prompt", json!({ "text": "hello" }));
        let response = send_rpc(&client_ep, server_addr.clone(), &request).await.unwrap();

        let err = response.error.unwrap();
        assert!(err.contains("agent capabilities"));
    }

    #[tokio::test]
    async fn test_rpc_prompt_missing_text() {
        let state = agent_state();
        let server_addr = start_test_server(state).await;

        let client_id = ephemeral_identity();
        let client_ep = start_endpoint_no_mdns(&client_id).await.unwrap();
        let request = Request::new("prompt", json!({})); // no "text" param
        let response = send_rpc(&client_ep, server_addr.clone(), &request).await.unwrap();

        let err = response.error.unwrap();
        assert!(err.contains("text"));
    }

    #[tokio::test]
    async fn test_rpc_acl_rejection() {
        // Server with a restrictive ACL
        let state = Arc::new(ServerState {
            meta: NodeMeta {
                tags: vec![],
                agent_names: vec![],
            },
            agent: None,
            acl: AccessControl::from_allowlist(["not-the-client".to_string()].into_iter().collect()),
            receive_dir: None,
        });
        let server_addr = start_test_server(state).await;

        let client_id = ephemeral_identity();
        let client_ep = start_endpoint_no_mdns(&client_id).await.unwrap();
        let request = Request::new("ping", json!({}));
        // The server should reject the connection — this should error
        let result = send_rpc(&client_ep, server_addr.clone(), &request).await;
        assert!(result.is_err(), "Expected ACL rejection");
    }

    #[tokio::test]
    async fn test_rpc_acl_allowed() {
        let client_id = ephemeral_identity();
        let client_pk = client_id.public_key().to_string();

        // Server that allows only this specific client
        let state = Arc::new(ServerState {
            meta: NodeMeta {
                tags: vec![],
                agent_names: vec![],
            },
            agent: None,
            acl: AccessControl::from_allowlist([client_pk].into_iter().collect()),
            receive_dir: None,
        });
        let server_addr = start_test_server(state).await;

        let client_ep = start_endpoint_no_mdns(&client_id).await.unwrap();
        let request = Request::new("ping", json!({}));
        let response = send_rpc(&client_ep, server_addr.clone(), &request).await.unwrap();
        assert_eq!(response.ok.unwrap(), json!("pong"));
    }

    // ── File transfer tests ─────────────────────────────────────────

    #[tokio::test]
    async fn test_rpc_send_file() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let receive_dir = tmp_dir.path().join("received");

        let state = Arc::new(ServerState {
            meta: NodeMeta {
                tags: vec![],
                agent_names: vec![],
            },
            agent: None,
            acl: AccessControl::open(),
            receive_dir: Some(receive_dir.clone()),
        });
        let server_addr = start_test_server(state).await;

        // Create a test file
        let src_file = tmp_dir.path().join("test.txt");
        std::fs::write(&src_file, "Hello from file transfer!").unwrap();

        let client_id = ephemeral_identity();
        let client_ep = start_endpoint_no_mdns(&client_id).await.unwrap();

        let response = send_file(&client_ep, server_addr.clone(), &src_file).await.unwrap();
        let result = response.ok.unwrap();
        assert_eq!(result["size"], 25); // "Hello from file transfer!".len()

        // Verify the file was saved on the server side
        let saved_path = result["path"].as_str().unwrap();
        let saved_content = std::fs::read_to_string(saved_path).unwrap();
        assert_eq!(saved_content, "Hello from file transfer!");
    }

    #[tokio::test]
    async fn test_rpc_recv_file() {
        let tmp_dir = tempfile::TempDir::new().unwrap();

        // Create a file that the server can serve
        let server_file = tmp_dir.path().join("serve-me.txt");
        std::fs::write(&server_file, "Content from server").unwrap();

        let state = Arc::new(ServerState {
            meta: NodeMeta {
                tags: vec![],
                agent_names: vec![],
            },
            agent: None,
            acl: AccessControl::open(),
            receive_dir: None,
        });
        let server_addr = start_test_server(state).await;

        let client_id = ephemeral_identity();
        let client_ep = start_endpoint_no_mdns(&client_id).await.unwrap();

        let download_path = tmp_dir.path().join("downloaded.txt");
        let total = recv_file(&client_ep, server_addr.clone(), server_file.to_str().unwrap(), &download_path)
            .await
            .unwrap();

        assert_eq!(total, 19); // "Content from server".len()
        let content = std::fs::read_to_string(&download_path).unwrap();
        assert_eq!(content, "Content from server");
    }

    #[tokio::test]
    async fn test_rpc_recv_file_not_found() {
        let state = Arc::new(ServerState {
            meta: NodeMeta {
                tags: vec![],
                agent_names: vec![],
            },
            agent: None,
            acl: AccessControl::open(),
            receive_dir: None,
        });
        let server_addr = start_test_server(state).await;

        let client_id = ephemeral_identity();
        let client_ep = start_endpoint_no_mdns(&client_id).await.unwrap();

        let tmp_dir = tempfile::TempDir::new().unwrap();
        let download_path = tmp_dir.path().join("nope.txt");
        let result = recv_file(&client_ep, server_addr.clone(), "/nonexistent/file.txt", &download_path).await;

        assert!(result.is_err());
    }
}
