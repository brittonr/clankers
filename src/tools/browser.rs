//! Stateful browser automation tool adapter.
//!
//! This module defines the clankers-facing browser tool and policy checks. A
//! transport-specific CDP backend can implement `BrowserRuntime` without changing
//! the model-visible tool schema or safety boundaries.

use std::sync::Arc;

use async_trait::async_trait;
use clankers_config::BrowserAutomationSettings;
use serde_json::Value;
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserAction {
    Navigate,
    Click,
    Type,
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
            "type" => Some(Self::Type),
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
            Self::Type => "type",
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
        let action = request.action.as_str();
        match self.runtime.perform(request).await {
            Ok(value) => ToolResult::text(serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string()))
                .with_details(serde_json::json!({"source":"browser", "action": action})),
            Err(error) => ToolResult::error(format!("browser automation error: {error}"))
                .with_details(serde_json::json!({"source":"browser", "action": action})),
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
                    "enum": ["navigate", "click", "type", "evaluate", "screenshot", "current_url", "close"]
                },
                "url": {"type": "string", "description": "URL for navigate actions"},
                "selector": {"type": "string", "description": "CSS selector for click/type actions"},
                "text": {"type": "string", "description": "Text for type actions"},
                "script": {"type": "string", "description": "JavaScript for evaluate actions when allowEvaluate is enabled"}
            },
            "additionalProperties": false
        }),
    }
}

pub fn parse_browser_request(params: &Value) -> Result<BrowserRequest, String> {
    let object = params.as_object().ok_or_else(|| "browser params must be a JSON object".to_string())?;
    let action = object.get("action").and_then(Value::as_str).and_then(BrowserAction::parse).ok_or_else(|| {
        "browser `action` must be one of navigate, click, type, evaluate, screenshot, current_url, close".to_string()
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
        assert_eq!(
            result.details.as_ref().and_then(|details| details.get("source")).and_then(Value::as_str),
            Some("browser")
        );
    }
}
