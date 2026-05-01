//! Stateful browser automation tool adapter.
//!
//! This module defines the clankers-facing browser tool and policy checks. A
//! transport-specific CDP backend can implement `BrowserRuntime` without changing
//! the model-visible tool schema or safety boundaries.

use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use async_trait::async_trait;
use clankers_config::BrowserAutomationBackend;
use clankers_config::BrowserAutomationSettings;
use serde_json::Value;
use tokio::process::Child;
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::sync::OnceCell;
use url::Url;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserRequest {
    pub action: BrowserAction,
    pub url: Option<String>,
    pub selector: Option<String>,
    pub text: Option<String>,
    pub script: Option<String>,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserAction {
    Navigate,
    Click,
    Type,
    Snapshot,
    Evaluate,
    Screenshot,
    CurrentUrl,
    Close,
}

impl BrowserAction {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "navigate" => Some(Self::Navigate),
            "click" => Some(Self::Click),
            "type" | "fill" => Some(Self::Type),
            "snapshot" => Some(Self::Snapshot),
            "evaluate" => Some(Self::Evaluate),
            "screenshot" => Some(Self::Screenshot),
            "current_url" => Some(Self::CurrentUrl),
            "close" => Some(Self::Close),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Navigate => "navigate",
            Self::Click => "click",
            Self::Type => "fill",
            Self::Snapshot => "snapshot",
            Self::Evaluate => "evaluate",
            Self::Screenshot => "screenshot",
            Self::CurrentUrl => "current_url",
            Self::Close => "close",
        }
    }
}

#[async_trait]
pub trait BrowserRuntime: Send + Sync {
    async fn perform(&self, request: BrowserRequest) -> Result<Value, String>;
}

#[derive(Debug)]
pub struct CdpBrowserRuntime {
    client: reqwest::Client,
    endpoint: String,
    backend: &'static str,
    _owned_browser: Option<Mutex<Child>>,
}

impl CdpBrowserRuntime {
    pub async fn from_settings(settings: &BrowserAutomationSettings) -> Result<Self, String> {
        if settings.backend != BrowserAutomationBackend::Cdp {
            return Err("only the `cdp` browser automation backend is supported".to_string());
        }
        let timeout = Duration::from_millis(settings.timeout_ms.unwrap_or(30_000));
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|error| format!("failed to build browser HTTP client: {error}"))?;

        if let Some(cdp_url) = settings.cdp_url.as_deref().filter(|url| !url.trim().is_empty()) {
            let endpoint = cdp_url.trim().trim_end_matches('/').to_string();
            wait_for_cdp(&client, &endpoint, timeout).await?;
            return Ok(Self {
                client,
                endpoint,
                backend: "cdp",
                _owned_browser: None,
            });
        }

        let binary = settings
            .browser_binary
            .as_deref()
            .filter(|binary| !binary.trim().is_empty())
            .ok_or_else(|| "browserAutomation requires `cdpUrl` or `browserBinary`".to_string())?;
        let port = reserve_local_port()?;
        let endpoint = format!("http://127.0.0.1:{port}");
        let user_data_dir = settings
            .user_data_dir
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join(format!("clankers-browser-{}", uuid::Uuid::new_v4())));

        let mut command = Command::new(binary.trim());
        command
            .arg(format!("--remote-debugging-port={port}"))
            .arg(format!("--user-data-dir={}", user_data_dir.display()))
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg("--disable-background-networking")
            .arg("about:blank")
            .env_clear()
            .envs(crate::tools::sandbox::sanitized_env())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        if settings.headless {
            command.arg("--headless=new").arg("--disable-gpu");
        }
        let child =
            command.spawn().map_err(|error| format!("failed to launch browser `{}`: {error}", binary.trim()))?;
        wait_for_cdp(&client, &endpoint, timeout).await?;
        Ok(Self {
            client,
            endpoint,
            backend: "cdp-launched",
            _owned_browser: Some(Mutex::new(child)),
        })
    }

    async fn list_targets(&self) -> Result<Vec<CdpTarget>, String> {
        let url = format!("{}/json/list", self.endpoint);
        self.client
            .get(url)
            .send()
            .await
            .map_err(|error| format!("failed to list CDP targets: {error}"))?
            .error_for_status()
            .map_err(|error| format!("CDP target list failed: {error}"))?
            .json::<Vec<CdpTarget>>()
            .await
            .map_err(|error| format!("failed to decode CDP target list: {error}"))
    }

    async fn open_target(&self, url: &str) -> Result<CdpTarget, String> {
        let escaped = url_encode(url);
        let endpoint = format!("{}/json/new?{escaped}", self.endpoint);
        let response = match self.client.put(&endpoint).send().await {
            Ok(response) if response.status().is_success() => response,
            _ => self
                .client
                .get(&endpoint)
                .send()
                .await
                .map_err(|error| format!("failed to create CDP target: {error}"))?
                .error_for_status()
                .map_err(|error| format!("CDP target create failed: {error}"))?,
        };
        response
            .json::<CdpTarget>()
            .await
            .map_err(|error| format!("failed to decode created CDP target: {error}"))
    }

    async fn close_target(&self, id: &str) -> Result<Value, String> {
        let endpoint = format!("{}/json/close/{}", self.endpoint, url_encode(id));
        let text = self
            .client
            .get(endpoint)
            .send()
            .await
            .map_err(|error| format!("failed to close CDP target `{id}`: {error}"))?
            .error_for_status()
            .map_err(|error| format!("CDP target close failed for `{id}`: {error}"))?
            .text()
            .await
            .map_err(|error| format!("failed to read CDP close response: {error}"))?;
        Ok(serde_json::json!({"sessionId": id, "closed": true, "response": text}))
    }

    fn normalize_target(&self, target: CdpTarget, action: BrowserAction) -> Value {
        serde_json::json!({
            "backend": self.backend,
            "action": action.as_str(),
            "status": "ok",
            "sessionId": target.id,
            "url": target.url,
            "title": target.title.unwrap_or_default(),
            "targetType": target.kind.unwrap_or_default()
        })
    }
}

#[async_trait]
impl BrowserRuntime for CdpBrowserRuntime {
    async fn perform(&self, request: BrowserRequest) -> Result<Value, String> {
        match request.action {
            BrowserAction::Navigate => {
                let url = request.url.as_deref().ok_or_else(|| "browser navigate requires `url`".to_string())?;
                let target = self.open_target(url).await?;
                Ok(self.normalize_target(target, request.action))
            }
            BrowserAction::Snapshot | BrowserAction::CurrentUrl => {
                let targets = self.list_targets().await?;
                let target = select_target(targets, request.session_id.as_deref())?;
                Ok(self.normalize_target(target, request.action))
            }
            BrowserAction::Close => {
                let id = request
                    .session_id
                    .as_deref()
                    .ok_or_else(|| "browser close requires `sessionId` for the CDP HTTP backend".to_string())?;
                self.close_target(id).await
            }
            BrowserAction::Click | BrowserAction::Type | BrowserAction::Evaluate | BrowserAction::Screenshot => {
                Err(format!(
                    "browser action `{}` requires the CDP WebSocket command backend, which is not implemented in this slice; use navigate/snapshot/current_url/close or configure a follow-up backend",
                    request.action.as_str()
                ))
            }
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct CdpTarget {
    id: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    url: String,
    #[serde(default, rename = "type")]
    kind: Option<String>,
}

fn select_target(targets: Vec<CdpTarget>, session_id: Option<&str>) -> Result<CdpTarget, String> {
    if let Some(id) = session_id {
        return targets
            .into_iter()
            .find(|target| target.id == id)
            .ok_or_else(|| format!("browser session `{id}` was not found"));
    }
    targets
        .into_iter()
        .find(|target| target.kind.as_deref() == Some("page"))
        .ok_or_else(|| "CDP backend has no open page targets; call browser navigate first".to_string())
}

async fn wait_for_cdp(client: &reqwest::Client, endpoint: &str, timeout: Duration) -> Result<(), String> {
    let start = Instant::now();
    let version_url = format!("{}/json/version", endpoint);
    loop {
        match client.get(&version_url).send().await {
            Ok(response) if response.status().is_success() => return Ok(()),
            _ if start.elapsed() >= timeout => {
                return Err(format!("CDP endpoint `{endpoint}` did not become ready within {}ms", timeout.as_millis()));
            }
            _ => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }
}

fn reserve_local_port() -> Result<u16, String> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|error| format!("failed to reserve browser debug port: {error}"))?;
    let port = listener.local_addr().map_err(|error| format!("failed to read browser debug port: {error}"))?.port();
    drop(listener);
    Ok(port)
}

fn url_encode(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

pub async fn browser_runtime_from_settings(
    settings: &BrowserAutomationSettings,
) -> Result<Arc<dyn BrowserRuntime>, String> {
    CdpBrowserRuntime::from_settings(settings)
        .await
        .map(|runtime| Arc::new(runtime) as Arc<dyn BrowserRuntime>)
}

pub struct LazyBrowserRuntime {
    settings: BrowserAutomationSettings,
    runtime: OnceCell<Arc<dyn BrowserRuntime>>,
}

impl LazyBrowserRuntime {
    pub fn new(settings: BrowserAutomationSettings) -> Self {
        Self {
            settings,
            runtime: OnceCell::new(),
        }
    }

    async fn runtime(&self) -> Result<&Arc<dyn BrowserRuntime>, String> {
        self.runtime.get_or_try_init(|| async { browser_runtime_from_settings(&self.settings).await }).await
    }
}

#[async_trait]
impl BrowserRuntime for LazyBrowserRuntime {
    async fn perform(&self, request: BrowserRequest) -> Result<Value, String> {
        self.runtime().await?.perform(request).await
    }
}

pub struct BrowserTool {
    definition: ToolDefinition,
    settings: BrowserAutomationSettings,
    runtime: Arc<dyn BrowserRuntime>,
}

impl BrowserTool {
    pub fn new(settings: BrowserAutomationSettings, runtime: Arc<dyn BrowserRuntime>) -> Self {
        Self {
            definition: browser_tool_definition(),
            settings,
            runtime,
        }
    }
}

#[async_trait]
impl Tool for BrowserTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    fn source(&self) -> &str {
        "browser"
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        ctx.emit_progress("browser: validating request");
        let request = match parse_browser_request(&params) {
            Ok(request) => request,
            Err(error) => return ToolResult::error(error),
        };
        if let Err(error) = enforce_browser_policy(&self.settings, &request) {
            return ToolResult::error(error);
        }

        ctx.emit_progress(&format!("browser: {}", request.action.as_str()));
        let started = Instant::now();
        match self.runtime.perform(request.clone()).await {
            Ok(value) => {
                let details = browser_result_details(&request, Some(&value), "ok", started.elapsed(), None);
                ToolResult::text(serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string()))
                    .with_details(details)
            }
            Err(error) => {
                let details = browser_result_details(&request, None, "error", started.elapsed(), Some(&error));
                ToolResult::error(format!("browser automation error: {error}")).with_details(details)
            }
        }
    }
}

pub fn build_browser_tool(
    settings: &BrowserAutomationSettings,
    runtime: Option<Arc<dyn BrowserRuntime>>,
) -> Option<Arc<dyn Tool>> {
    if !settings.enabled {
        return None;
    }
    if let Err(error) = settings.validate() {
        tracing::warn!(error = %error, "skipping invalid browser automation configuration");
        return None;
    }
    let runtime = runtime?;
    Some(Arc::new(BrowserTool::new(settings.clone(), runtime)))
}

pub fn build_browser_tool_from_settings(settings: &BrowserAutomationSettings) -> Option<Arc<dyn Tool>> {
    if !settings.enabled {
        return None;
    }
    if let Err(error) = settings.validate() {
        tracing::warn!(error = %error, "skipping invalid browser automation configuration");
        return None;
    }
    let runtime = Arc::new(LazyBrowserRuntime::new(settings.clone()));
    Some(Arc::new(BrowserTool::new(settings.clone(), runtime)))
}

pub fn browser_tool_definition() -> ToolDefinition {
    ToolDefinition {
        name: "browser".to_string(),
        description: "Control a stateful browser session using local CDP automation. Supports navigate, click, type, evaluate, screenshot, current_url, and close actions subject to configuration policy.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "required": ["action"],
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["navigate", "snapshot", "click", "fill", "type", "evaluate", "screenshot", "current_url", "close"]
                },
                "url": {"type": "string", "description": "URL for navigate actions"},
                "selector": {"type": "string", "description": "CSS selector for click/fill actions"},
                "text": {"type": "string", "description": "Text for fill/type actions"},
                "script": {"type": "string", "description": "JavaScript for evaluate actions when allowEvaluate is enabled"},
                "sessionId": {"type": "string", "description": "Browser session/target id returned by navigate or snapshot"}
            },
            "additionalProperties": false
        }),
    }
}

fn browser_result_details(
    request: &BrowserRequest,
    value: Option<&Value>,
    status: &str,
    elapsed: Duration,
    error: Option<&str>,
) -> Value {
    let mut details = serde_json::Map::new();
    details.insert("source".to_string(), serde_json::json!("browser_automation"));
    details.insert("action".to_string(), serde_json::json!(request.action.as_str()));
    details.insert("status".to_string(), serde_json::json!(status));
    details.insert("elapsedMs".to_string(), serde_json::json!(elapsed.as_millis()));
    if let Some(session_id) = request.session_id.as_deref().filter(|value| !value.is_empty()) {
        details.insert("sessionId".to_string(), serde_json::json!(session_id));
    }
    if let Some(url) = request.url.as_deref() {
        details.insert("url".to_string(), serde_json::json!(url));
        if let Ok(origin) = origin_from_url(url) {
            details.insert("origin".to_string(), serde_json::json!(origin));
        }
    }
    if let Some(value) = value.and_then(Value::as_object) {
        copy_detail(value, &mut details, "backend");
        copy_detail(value, &mut details, "sessionId");
        copy_detail(value, &mut details, "url");
        copy_detail(value, &mut details, "title");
        copy_detail(value, &mut details, "targetType");
    }
    if let Some(error) = error {
        details.insert("error".to_string(), serde_json::json!(redact_browser_error(error)));
    }
    Value::Object(details)
}

fn copy_detail(source: &serde_json::Map<String, Value>, details: &mut serde_json::Map<String, Value>, key: &str) {
    if let Some(value) = source.get(key) {
        details.insert(key.to_string(), value.clone());
    }
}

fn redact_browser_error(error: &str) -> String {
    let mut redacted = String::with_capacity(error.len());
    for part in error.split_whitespace() {
        let lower = part.to_ascii_lowercase();
        if lower.contains("token=") || lower.contains("access_token") || lower.contains("authorization") {
            redacted.push_str("[redacted]");
        } else {
            redacted.push_str(part);
        }
        redacted.push(' ');
    }
    redacted.trim_end().to_string()
}

pub fn parse_browser_request(params: &Value) -> Result<BrowserRequest, String> {
    let object = params.as_object().ok_or_else(|| "browser params must be a JSON object".to_string())?;
    let action = object.get("action").and_then(Value::as_str).and_then(BrowserAction::parse).ok_or_else(|| {
        "browser `action` must be one of navigate, snapshot, click, fill, type, evaluate, screenshot, current_url, close".to_string()
    })?;

    let get_string = |key: &str| -> Result<Option<String>, String> {
        match object.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(value)) if !value.trim().is_empty() => Ok(Some(value.clone())),
            Some(Value::String(_)) => Err(format!("browser `{key}` cannot be blank")),
            Some(_) => Err(format!("browser `{key}` must be a string")),
        }
    };

    let request = BrowserRequest {
        action,
        url: get_string("url")?,
        selector: get_string("selector")?,
        text: get_string("text")?,
        script: get_string("script")?,
        session_id: get_string("sessionId")?,
    };

    match request.action {
        BrowserAction::Navigate if request.url.is_none() => Err("browser navigate requires `url`".to_string()),
        BrowserAction::Click if request.selector.is_none() => Err("browser click requires `selector`".to_string()),
        BrowserAction::Type if request.selector.is_none() || request.text.is_none() => {
            Err("browser type requires `selector` and `text`".to_string())
        }
        BrowserAction::Evaluate if request.script.is_none() => Err("browser evaluate requires `script`".to_string()),
        _ => Ok(request),
    }
}

pub fn enforce_browser_policy(settings: &BrowserAutomationSettings, request: &BrowserRequest) -> Result<(), String> {
    match request.action {
        BrowserAction::Evaluate if !settings.allow_evaluate => {
            return Err("browser evaluate is disabled by browserAutomation.allowEvaluate".to_string());
        }
        BrowserAction::Screenshot if !settings.allow_screenshots => {
            return Err("browser screenshots are disabled by browserAutomation.allowScreenshots".to_string());
        }
        _ => {}
    }

    if let Some(url) = &request.url {
        let origin = origin_from_url(url)?;
        if !settings.permits_origin(&origin) {
            return Err(format!("browser navigation to origin `{origin}` is not allowed"));
        }
    }

    Ok(())
}

fn origin_from_url(url: &str) -> Result<String, String> {
    let parsed = Url::parse(url).map_err(|error| format!("browser `url` is invalid: {error}"))?;
    let scheme = parsed.scheme();
    let host = parsed.host_str().ok_or_else(|| "browser `url` must include a host".to_string())?;
    let origin = match parsed.port() {
        Some(port) => format!("{scheme}://{host}:{port}"),
        None => format!("{scheme}://{host}"),
    };
    Ok(origin)
}

#[cfg(test)]
mod tests {
    use tokio::sync::Mutex;

    use super::*;

    struct FakeBrowserRuntime {
        calls: Mutex<Vec<BrowserRequest>>,
        result: Value,
    }

    #[async_trait]
    impl BrowserRuntime for FakeBrowserRuntime {
        async fn perform(&self, request: BrowserRequest) -> Result<Value, String> {
            self.calls.lock().await.push(request);
            Ok(self.result.clone())
        }
    }

    fn enabled_settings() -> BrowserAutomationSettings {
        BrowserAutomationSettings {
            enabled: true,
            cdp_url: Some("http://127.0.0.1:9222".to_string()),
            allowed_origins: vec!["https://example.test".to_string()],
            ..Default::default()
        }
    }

    #[test]
    fn parses_navigate_request() {
        let request = parse_browser_request(&serde_json::json!({
            "action": "navigate",
            "url": "https://example.test/page"
        }))
        .unwrap();
        assert_eq!(request.action, BrowserAction::Navigate);
        assert_eq!(request.url.as_deref(), Some("https://example.test/page"));
    }

    #[test]
    fn rejects_missing_required_action_fields() {
        assert!(parse_browser_request(&serde_json::json!({"action":"navigate"})).is_err());
        assert!(parse_browser_request(&serde_json::json!({"action":"click"})).is_err());
        assert!(parse_browser_request(&serde_json::json!({"action":"type", "selector":"#q"})).is_err());
        assert!(parse_browser_request(&serde_json::json!({"action":"evaluate"})).is_err());
    }

    #[test]
    fn policy_rejects_disallowed_origin_and_evaluate() {
        let settings = enabled_settings();
        let navigate = parse_browser_request(&serde_json::json!({
            "action": "navigate",
            "url": "https://evil.test/"
        }))
        .unwrap();
        assert!(enforce_browser_policy(&settings, &navigate).unwrap_err().contains("not allowed"));

        let evaluate = parse_browser_request(&serde_json::json!({
            "action": "evaluate",
            "script": "document.title"
        }))
        .unwrap();
        assert!(enforce_browser_policy(&settings, &evaluate).unwrap_err().contains("allowEvaluate"));
    }

    #[test]
    fn build_browser_tool_requires_enabled_valid_settings_and_runtime() {
        assert!(build_browser_tool(&BrowserAutomationSettings::default(), None).is_none());
        assert!(build_browser_tool(&enabled_settings(), None).is_none());
        let runtime = Arc::new(FakeBrowserRuntime {
            calls: Mutex::new(Vec::new()),
            result: serde_json::json!({"ok": true}),
        });
        assert!(build_browser_tool(&enabled_settings(), Some(runtime)).is_some());
        assert!(build_browser_tool_from_settings(&BrowserAutomationSettings::default()).is_none());
        assert!(build_browser_tool_from_settings(&enabled_settings()).is_some());
    }

    #[tokio::test]
    async fn browser_tool_executes_runtime_after_policy() {
        let runtime = Arc::new(FakeBrowserRuntime {
            calls: Mutex::new(Vec::new()),
            result: serde_json::json!({"url": "https://example.test/"}),
        });
        let tool = BrowserTool::new(enabled_settings(), runtime.clone());
        let ctx = ToolContext::new("browser-call".to_string(), tokio_util::sync::CancellationToken::new(), None);

        let result = tool.execute(&ctx, serde_json::json!({"action":"navigate", "url":"https://example.test/"})).await;

        assert!(!result.is_error);
        assert_eq!(runtime.calls.lock().await.len(), 1);
        let details = result.details.as_ref().unwrap();
        assert_eq!(details.get("source").and_then(Value::as_str), Some("browser_automation"));
        assert_eq!(details.get("status").and_then(Value::as_str), Some("ok"));
        assert!(details.get("elapsedMs").and_then(Value::as_u64).is_some());
        assert_eq!(details.get("url").and_then(Value::as_str), Some("https://example.test/"));
    }

    #[tokio::test]
    async fn cdp_runtime_uses_http_endpoint_for_navigate_and_snapshot() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            for _ in 0..3 {
                let (mut stream, _) = listener.accept().await.unwrap();
                tokio::spawn(async move {
                    use tokio::io::AsyncReadExt;
                    use tokio::io::AsyncWriteExt;

                    let mut buffer = [0_u8; 2048];
                    let size = stream.read(&mut buffer).await.unwrap();
                    let request = String::from_utf8_lossy(&buffer[..size]);
                    let body = if request.starts_with("GET /json/version") {
                        serde_json::json!({"Browser":"fake"}).to_string()
                    } else if request.starts_with("PUT /json/new") || request.starts_with("GET /json/new") {
                        serde_json::json!({"id":"target-1","type":"page","url":"https://example.test/","title":"Example"}).to_string()
                    } else if request.starts_with("GET /json/list") {
                        serde_json::json!([{ "id":"target-1", "type":"page", "url":"https://example.test/", "title":"Example" }]).to_string()
                    } else {
                        serde_json::json!({"error":"unexpected request", "request": request.lines().next().unwrap_or("")}).to_string()
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

        let mut settings = enabled_settings();
        settings.cdp_url = Some(endpoint);
        settings.allowed_origins.clear();
        let runtime = CdpBrowserRuntime::from_settings(&settings).await.unwrap();

        let navigate = runtime
            .perform(BrowserRequest {
                action: BrowserAction::Navigate,
                url: Some("https://example.test/".to_string()),
                selector: None,
                text: None,
                script: None,
                session_id: None,
            })
            .await
            .unwrap();
        assert_eq!(navigate.get("sessionId").and_then(Value::as_str), Some("target-1"));

        let snapshot = runtime
            .perform(BrowserRequest {
                action: BrowserAction::Snapshot,
                url: None,
                selector: None,
                text: None,
                script: None,
                session_id: Some("target-1".to_string()),
            })
            .await
            .unwrap();
        assert_eq!(snapshot.get("title").and_then(Value::as_str), Some("Example"));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn cdp_runtime_reports_unsupported_websocket_actions() {
        let runtime = CdpBrowserRuntime {
            client: reqwest::Client::new(),
            endpoint: "http://127.0.0.1:9".to_string(),
            backend: "cdp",
            _owned_browser: None,
        };
        let error = runtime
            .perform(BrowserRequest {
                action: BrowserAction::Click,
                url: None,
                selector: Some("button".to_string()),
                text: None,
                script: None,
                session_id: None,
            })
            .await
            .unwrap_err();
        assert!(error.contains("CDP WebSocket command backend"));
    }
}
