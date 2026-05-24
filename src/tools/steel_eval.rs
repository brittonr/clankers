//! Agent-visible Steel evaluation tool.
//!
//! This is a thin built-in tool shell over `clankers_runtime::steel_runtime`.
//! It owns request validation and presentation, while all evaluation remains in
//! the runtime wrapper DTO/function seam.

use async_trait::async_trait;
use clankers_runtime::steel_runtime::SteelHostFunctionRegistration;
use clankers_runtime::steel_runtime::SteelRuntimeProfile;
use clankers_runtime::steel_runtime::SteelRuntimeReasonCode;
use clankers_runtime::steel_runtime::SteelRuntimeReceipt;
use clankers_runtime::steel_runtime::SteelRuntimeRequest;
use clankers_runtime::steel_runtime::SteelRuntimeStatusCode;
use clankers_runtime::steel_runtime::evaluate_steel_request;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;

pub const STEEL_EVAL_TOOL_NAME: &str = "steel_eval";
const STEEL_EVAL_TOOL_RECEIPT_SCHEMA: &str = "clankers.steel_eval.tool_receipt.v1";
const DEFAULT_PROFILE_ID: &str = "default";
const DEFAULT_RECEIPT_DESTINATION: &str = "session";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SteelEvalToolConfig {
    pub default_profile: SteelEvalProfileConfig,
    pub profiles: Vec<SteelEvalProfileConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteelEvalProfileConfig {
    pub id: String,
    pub max_source_bytes: u64,
    pub max_output_bytes: u64,
    pub max_host_calls: u64,
    pub max_steps: u64,
    pub session_capabilities: Vec<String>,
    pub host_functions: Vec<SteelHostFunctionRegistration>,
}

impl SteelEvalToolConfig {
    #[must_use]
    pub fn new(default_profile: SteelEvalProfileConfig, profiles: Vec<SteelEvalProfileConfig>) -> Self {
        Self {
            default_profile,
            profiles,
        }
    }

    fn profile(&self, requested: Option<&str>) -> Result<&SteelEvalProfileConfig, SteelEvalIssue> {
        let id = requested.unwrap_or(DEFAULT_PROFILE_ID);
        if id == self.default_profile.id {
            return Ok(&self.default_profile);
        }
        self.profiles.iter().find(|profile| profile.id == id).ok_or(SteelEvalIssue::UnknownProfile)
    }
}

impl Default for SteelEvalProfileConfig {
    fn default() -> Self {
        Self {
            id: DEFAULT_PROFILE_ID.to_string(),
            max_source_bytes: 4096,
            max_output_bytes: 1024,
            max_host_calls: 0,
            max_steps: 256,
            session_capabilities: Vec::new(),
            host_functions: Vec::new(),
        }
    }
}

pub struct SteelEvalTool {
    definition: ToolDefinition,
    config: SteelEvalToolConfig,
}

impl SteelEvalTool {
    #[must_use]
    pub fn new(config: SteelEvalToolConfig) -> Self {
        Self {
            definition: tool_definition(),
            config,
        }
    }
}

impl Default for SteelEvalTool {
    fn default() -> Self {
        Self::new(SteelEvalToolConfig::default())
    }
}

#[async_trait]
impl Tool for SteelEvalTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, _ctx: &ToolContext, params: Value) -> ToolResult {
        let result = run_steel_eval(&self.config, &params);
        match serde_json::to_string_pretty(&result) {
            Ok(text) => {
                if result.status == "succeeded" {
                    ToolResult::text(text)
                } else {
                    ToolResult::error(text)
                }
            }
            Err(err) => ToolResult::error(format!("steel_eval receipt serialization failed: {err}")),
        }
    }
}

#[derive(Debug, Serialize)]
struct SteelEvalToolReceipt {
    schema: &'static str,
    status: &'static str,
    issue_code: &'static str,
    safe_message: String,
    profile_id: Option<String>,
    output: Option<String>,
    output_len: Option<usize>,
    receipt_hash: Option<String>,
    runtime: Option<SteelRuntimeReceipt>,
    redaction: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SteelEvalIssue {
    InvalidRequest,
    SourceTooLarge,
    UnknownProfile,
}

impl SteelEvalIssue {
    fn code(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid-request",
            Self::SourceTooLarge => "source-too-large",
            Self::UnknownProfile => "unknown-profile",
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::InvalidRequest => "steel_eval requires a non-empty string source field",
            Self::SourceTooLarge => "source exceeds the selected Steel eval profile limit",
            Self::UnknownProfile => "requested Steel eval profile is not enabled by reviewed settings",
        }
    }
}

fn run_steel_eval(config: &SteelEvalToolConfig, params: &Value) -> SteelEvalToolReceipt {
    let Some(source) = params.get("source").and_then(Value::as_str) else {
        return denied_receipt(SteelEvalIssue::InvalidRequest, None);
    };
    if source.trim().is_empty() {
        return denied_receipt(SteelEvalIssue::InvalidRequest, None);
    }
    let requested_profile = params.get("profile").and_then(Value::as_str);
    let profile = match config.profile(requested_profile) {
        Ok(profile) => profile,
        Err(issue) => return denied_receipt(issue, requested_profile.map(ToString::to_string)),
    };
    if source.len() as u64 > profile.max_source_bytes {
        return denied_receipt(SteelEvalIssue::SourceTooLarge, Some(profile.id.clone()));
    }

    let runtime_request = SteelRuntimeRequest {
        profile: runtime_profile(profile),
        source: source.to_string(),
        session_capabilities: profile.session_capabilities.clone(),
        disabled_tools: Vec::new(),
        host_functions: profile.host_functions.clone(),
        receipt_destination: DEFAULT_RECEIPT_DESTINATION.to_string(),
    };
    let runtime = evaluate_steel_request(&runtime_request);
    runtime_receipt(profile.id.clone(), runtime)
}

fn runtime_profile(profile: &SteelEvalProfileConfig) -> SteelRuntimeProfile {
    SteelRuntimeProfile {
        name: profile.id.clone(),
        max_source_bytes: profile.max_source_bytes,
        max_output_bytes: profile.max_output_bytes,
        max_host_calls: profile.max_host_calls,
        max_steps: profile.max_steps,
        ambient_authority: false,
        agent_tool_enabled: true,
    }
}

fn denied_receipt(issue: SteelEvalIssue, profile_id: Option<String>) -> SteelEvalToolReceipt {
    SteelEvalToolReceipt {
        schema: STEEL_EVAL_TOOL_RECEIPT_SCHEMA,
        status: "denied",
        issue_code: issue.code(),
        safe_message: issue.message().to_string(),
        profile_id,
        output: None,
        output_len: None,
        receipt_hash: None,
        runtime: None,
        redaction: "source_and_sensitive_material_omitted",
    }
}

fn runtime_receipt(profile_id: String, runtime: SteelRuntimeReceipt) -> SteelEvalToolReceipt {
    let status = match runtime.status {
        SteelRuntimeStatusCode::Succeeded => "succeeded",
        SteelRuntimeStatusCode::Denied => "denied",
        SteelRuntimeStatusCode::ResourceLimited => "resource_limited",
        SteelRuntimeStatusCode::EvaluationFailed => "evaluation_failed",
    };
    let issue_code = reason_code(runtime.reason_code.clone());
    let output_len = runtime.output.as_ref().map(String::len);
    let receipt_hash = runtime.receipt_hash().to_string();
    SteelEvalToolReceipt {
        schema: STEEL_EVAL_TOOL_RECEIPT_SCHEMA,
        status,
        issue_code,
        safe_message: runtime.safe_message.clone(),
        profile_id: Some(profile_id),
        output: runtime.output.clone(),
        output_len,
        receipt_hash: Some(receipt_hash),
        runtime: Some(runtime),
        redaction: "runtime_receipt_redacts_source_paths_credentials",
    }
}

fn reason_code(reason: SteelRuntimeReasonCode) -> &'static str {
    match reason {
        SteelRuntimeReasonCode::Ok => "ok",
        SteelRuntimeReasonCode::SourceTooLarge => "source-too-large",
        SteelRuntimeReasonCode::OutputTooLarge => "output-too-large",
        SteelRuntimeReasonCode::ExecutionBudgetExceeded => "execution-budget-exceeded",
        SteelRuntimeReasonCode::HostCallBudgetExceeded => "host-call-budget-exceeded",
        SteelRuntimeReasonCode::UnknownHostFunction => "unknown-host-function",
        SteelRuntimeReasonCode::DisabledHostFunction => "disabled-host-function",
        SteelRuntimeReasonCode::MissingHostCapability => "missing-host-capability",
        SteelRuntimeReasonCode::AmbientAuthorityDenied => "ambient-authority-denied",
        SteelRuntimeReasonCode::UnsupportedExpression => "unsupported-expression",
    }
}

fn tool_definition() -> ToolDefinition {
    ToolDefinition {
        name: STEEL_EVAL_TOOL_NAME.to_string(),
        description: "Evaluate bounded Steel Scheme through the Clankers runtime wrapper. Constrained embedded interpreter; not an OS/process/VM sandbox. No ambient filesystem, process, network, provider, credential, daemon, TUI, git, mutation, or native tool authority."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "source": {
                    "type": "string",
                    "description": "Bounded Steel source to evaluate. Source is hashed/redacted in receipts."
                },
                "profile": {
                    "type": "string",
                    "description": "Optional reviewed profile id. Defaults to 'default'."
                }
            },
            "required": ["source"],
            "additionalProperties": false
        }),
    }
}

#[cfg(test)]
mod tests {
    use tokio_util::sync::CancellationToken;

    use super::*;
    use crate::tools::ToolResultContent;

    fn ctx() -> ToolContext {
        ToolContext::new("steel-call".to_string(), CancellationToken::new(), None)
    }

    fn result_text(result: &ToolResult) -> &str {
        match &result.content[0] {
            ToolResultContent::Text { text } => text,
            ToolResultContent::Image { .. } => panic!("expected text"),
        }
    }

    #[tokio::test]
    async fn pure_eval_is_deterministic_and_uses_runtime_receipt() {
        let tool = SteelEvalTool::default();
        let params = json!({"source":"(+ 1 2 3)"});
        let first = tool.execute(&ctx(), params.clone()).await;
        let second = tool.execute(&ctx(), params).await;
        assert!(!first.is_error);
        assert_eq!(result_text(&first), result_text(&second));
        let text = result_text(&first);
        assert!(text.contains("clankers.steel_runtime.receipt.v1"));
        assert!(text.contains("\"output\": \"6\""));
    }

    #[tokio::test]
    async fn invalid_and_unknown_profile_fail_closed() {
        let tool = SteelEvalTool::default();
        let invalid = tool.execute(&ctx(), json!({"source":""})).await;
        let unknown = tool.execute(&ctx(), json!({"source":"(+ 1 1)","profile":"missing"})).await;
        assert!(invalid.is_error);
        assert!(unknown.is_error);
        assert!(result_text(&invalid).contains("invalid-request"));
        assert!(result_text(&unknown).contains("unknown-profile"));
    }

    #[tokio::test]
    async fn ambient_and_host_authority_are_denied() {
        let tool = SteelEvalTool::default();
        let ambient = tool.execute(&ctx(), json!({"source":"(system \"git status\")"})).await;
        let host = tool.execute(&ctx(), json!({"source":"(host \"steel.host.echo\")"})).await;
        assert!(ambient.is_error);
        assert!(host.is_error);
        assert!(result_text(&ambient).contains("ambient-authority-denied"));
        assert!(result_text(&host).contains("host-call-budget-exceeded"));
    }
}
