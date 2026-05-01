use clankers::modes::common::ToolEnv;
use clankers::modes::common::ToolSet;
use clankers::modes::common::ToolTier;
use clankers::tools::ToolResultContent;
use serde_json::Value;
use serde_json::json;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

async fn spawn_fake_cdp_server(requests: usize) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        for _ in 0..requests {
            let (mut stream, _) = listener.accept().await.unwrap();
            tokio::spawn(async move {
                let mut buffer = [0_u8; 2048];
                let size = stream.read(&mut buffer).await.unwrap();
                let request = String::from_utf8_lossy(&buffer[..size]);
                let body = if request.starts_with("GET /json/version") {
                    json!({"Browser":"fake"}).to_string()
                } else if request.starts_with("PUT /json/new") || request.starts_with("GET /json/new") {
                    json!({"id":"target-1","type":"page","url":"https://example.test/","title":"Example"}).to_string()
                } else {
                    json!({"error":"unexpected request", "request": request.lines().next().unwrap_or("")}).to_string()
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream.write_all(response.as_bytes()).await.unwrap();
            });
        }
    });
    endpoint
}

fn browser_env(cdp_url: String, allowed_origins: Vec<String>) -> ToolEnv {
    let mut settings = clankers::config::settings::Settings::default();
    settings.browser_automation.enabled = true;
    settings.browser_automation.cdp_url = Some(cdp_url);
    settings.browser_automation.allowed_origins = allowed_origins;
    ToolEnv {
        settings: Some(settings),
        ..Default::default()
    }
}

#[tokio::test]
async fn configured_browser_tool_navigates_with_fake_cdp() {
    let endpoint = spawn_fake_cdp_server(2).await;
    let env = browser_env(endpoint, vec!["https://example.test".to_string()]);
    let tiered = clankers::modes::common::build_all_tiered_tools(&env, None);
    let tool_set = ToolSet::new(tiered, [ToolTier::Specialty]);
    let tools = tool_set.active_tools();
    let tool = tools
        .iter()
        .find(|tool| tool.definition().name == "browser")
        .expect("browser tool should be published");
    let ctx =
        clankers::tools::ToolContext::new("browser-call".to_string(), tokio_util::sync::CancellationToken::new(), None);

    let result = tool.execute(&ctx, json!({"action":"navigate", "url":"https://example.test/"})).await;

    assert!(!result.is_error);
    assert_eq!(
        result.details.as_ref().and_then(|details| details.get("source")).and_then(Value::as_str),
        Some("browser_automation")
    );
    assert_eq!(
        result.details.as_ref().and_then(|details| details.get("sessionId")).and_then(Value::as_str),
        Some("target-1")
    );
    match &result.content[0] {
        ToolResultContent::Text { text } => assert!(text.contains("target-1")),
        ToolResultContent::Image { .. } => panic!("expected text result"),
    }
}

#[tokio::test]
async fn configured_browser_tool_rejects_disallowed_origin_before_backend() {
    let env = browser_env("http://127.0.0.1:9".to_string(), vec!["https://allowed.test".to_string()]);
    let tiered = clankers::modes::common::build_all_tiered_tools(&env, None);
    let tool_set = ToolSet::new(tiered, [ToolTier::Specialty]);
    let tools = tool_set.active_tools();
    let tool = tools
        .iter()
        .find(|tool| tool.definition().name == "browser")
        .expect("browser tool should be published");
    let ctx =
        clankers::tools::ToolContext::new("browser-call".to_string(), tokio_util::sync::CancellationToken::new(), None);

    let result = tool.execute(&ctx, json!({"action":"navigate", "url":"https://blocked.test/"})).await;

    assert!(result.is_error);
    match &result.content[0] {
        ToolResultContent::Text { text } => assert!(text.contains("not allowed")),
        ToolResultContent::Image { .. } => panic!("expected text error"),
    }
}
