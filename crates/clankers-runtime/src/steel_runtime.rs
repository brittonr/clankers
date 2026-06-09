//! Constrained Steel Scheme runtime wrapper DTOs and deterministic fixture evaluator.
//!
//! This module is the only runtime-crate surface that owns Steel evaluation
//! request/response/receipt types. The current evaluator is a deterministic
//! wrapper fixture: it models the host-function, budget, and receipt contracts
//! before wiring an upstream Steel interpreter dependency. Shell crates should
//! call these DTOs/functions instead of constructing interpreter internals.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

pub use clanker_message::SteelHostCallOutcome;
pub use clanker_message::SteelRuntimeReasonCode;
pub use clanker_message::SteelRuntimeStatusCode;
use clankers_artifacts::ArtifactHash;
use serde::Deserialize;
use serde::Serialize;

pub const STEEL_RUNTIME_RECEIPT_SCHEMA: &str = "clankers.steel_runtime.receipt.v1";
pub const STEEL_RUNTIME_STATUS_SCHEMA: &str = "clankers.steel_runtime.status.v1";

const DEFAULT_PROFILE_NAME: &str = "default-deny";
const DEFAULT_MAX_SOURCE_BYTES: u64 = 4096;
const DEFAULT_MAX_OUTPUT_BYTES: u64 = 1024;
const DEFAULT_MAX_HOST_CALLS: u64 = 4;
const DEFAULT_MAX_STEPS: u64 = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelRuntimeProfile {
    pub name: String,
    pub max_source_bytes: u64,
    pub max_output_bytes: u64,
    pub max_host_calls: u64,
    pub max_steps: u64,
    pub ambient_authority: bool,
    pub agent_tool_enabled: bool,
}

impl SteelRuntimeProfile {
    #[must_use]
    pub fn default_deny() -> Self {
        Self {
            name: DEFAULT_PROFILE_NAME.to_string(),
            max_source_bytes: DEFAULT_MAX_SOURCE_BYTES,
            max_output_bytes: DEFAULT_MAX_OUTPUT_BYTES,
            max_host_calls: DEFAULT_MAX_HOST_CALLS,
            max_steps: DEFAULT_MAX_STEPS,
            ambient_authority: false,
            agent_tool_enabled: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelRuntimeRequest {
    pub profile: SteelRuntimeProfile,
    pub source: String,
    pub session_capabilities: Vec<String>,
    pub disabled_tools: Vec<String>,
    pub host_functions: Vec<SteelHostFunctionRegistration>,
    pub receipt_destination: String,
}

impl SteelRuntimeRequest {
    #[must_use]
    pub fn pure(source: impl Into<String>) -> Self {
        Self {
            profile: SteelRuntimeProfile::default_deny(),
            source: source.into(),
            session_capabilities: Vec::new(),
            disabled_tools: Vec::new(),
            host_functions: Vec::new(),
            receipt_destination: "stdout".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelHostFunctionRegistration {
    pub name: String,
    pub required_capability: String,
    pub output: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelRuntimeStatus {
    pub schema: String,
    pub available: bool,
    pub implementation: String,
    pub profile: SteelRuntimeProfile,
    pub agent_tool_enabled: bool,
    pub ambient_authority: bool,
    pub sandbox_claim: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelRuntimeReceipt {
    pub schema: String,
    pub status: SteelRuntimeStatusCode,
    pub reason_code: SteelRuntimeReasonCode,
    pub safe_message: String,
    pub profile_name: String,
    pub source_hash: ArtifactHash,
    pub output_hash: Option<ArtifactHash>,
    pub output: Option<String>,
    pub host_calls: Vec<SteelHostCallReceipt>,
    pub redactions: Vec<String>,
    pub steps_used: u64,
    pub ambient_authority: bool,
    pub sandbox_claim: String,
}

impl SteelRuntimeReceipt {
    #[must_use]
    pub fn receipt_hash(&self) -> ArtifactHash {
        let bytes = serde_json::to_vec(self).expect("Steel runtime receipt serializes");
        ArtifactHash::digest(&bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelHostCallReceipt {
    pub name: String,
    pub outcome: SteelHostCallOutcome,
    pub safe_message: String,
}

#[must_use]
pub fn steel_runtime_status(profile: SteelRuntimeProfile) -> SteelRuntimeStatus {
    SteelRuntimeStatus {
        schema: STEEL_RUNTIME_STATUS_SCHEMA.to_string(),
        available: true,
        implementation: "clankers-wrapper-fixture".to_string(),
        agent_tool_enabled: profile.agent_tool_enabled,
        ambient_authority: profile.ambient_authority,
        profile,
        sandbox_claim: "constrained embedded interpreter; no OS/process sandbox claim".to_string(),
    }
}

#[must_use]
pub fn evaluate_steel_request(request: &SteelRuntimeRequest) -> SteelRuntimeReceipt {
    let source_hash = ArtifactHash::digest(request.source.as_bytes());
    let source_bytes = request.source.len() as u64;
    if source_bytes > request.profile.max_source_bytes {
        return runtime_receipt(
            request,
            source_hash,
            SteelRuntimeStatusCode::ResourceLimited,
            SteelRuntimeReasonCode::SourceTooLarge,
            "source exceeds the selected Steel runtime profile limit",
            None,
            Vec::new(),
            0,
        );
    }
    let steps = estimate_steps(&request.source);
    if steps > request.profile.max_steps {
        return runtime_receipt(
            request,
            source_hash,
            SteelRuntimeStatusCode::ResourceLimited,
            SteelRuntimeReasonCode::ExecutionBudgetExceeded,
            "execution budget exceeded before evaluation",
            None,
            Vec::new(),
            steps,
        );
    }
    if let Some(kind) = ambient_attempt(&request.source) {
        return runtime_receipt(
            request,
            source_hash,
            SteelRuntimeStatusCode::Denied,
            SteelRuntimeReasonCode::AmbientAuthorityDenied,
            format!("ambient {kind} authority is denied; use an approved host function"),
            None,
            Vec::new(),
            steps,
        );
    }
    match evaluate_fixture_expression(request) {
        FixtureEval::Output(output, host_calls) => {
            if output.len() as u64 > request.profile.max_output_bytes {
                let bounded = truncate_to_limit(&output, u64_to_usize_saturating(request.profile.max_output_bytes));
                return runtime_receipt(
                    request,
                    source_hash,
                    SteelRuntimeStatusCode::ResourceLimited,
                    SteelRuntimeReasonCode::OutputTooLarge,
                    "output exceeds the selected Steel runtime profile limit",
                    Some(bounded),
                    host_calls,
                    steps,
                );
            }
            runtime_receipt(
                request,
                source_hash,
                SteelRuntimeStatusCode::Succeeded,
                SteelRuntimeReasonCode::Ok,
                "Steel evaluation completed through the Clankers runtime wrapper",
                Some(output),
                host_calls,
                steps,
            )
        }
        FixtureEval::Denied(reason, message, host_calls) => runtime_receipt(
            request,
            source_hash,
            SteelRuntimeStatusCode::Denied,
            reason,
            message,
            None,
            host_calls,
            steps,
        ),
        FixtureEval::Unsupported => runtime_receipt(
            request,
            source_hash,
            SteelRuntimeStatusCode::EvaluationFailed,
            SteelRuntimeReasonCode::UnsupportedExpression,
            "fixture evaluator supports literals, addition, display, and host calls only",
            None,
            Vec::new(),
            steps,
        ),
    }
}

enum FixtureEval {
    Output(String, Vec<SteelHostCallReceipt>),
    Denied(SteelRuntimeReasonCode, String, Vec<SteelHostCallReceipt>),
    Unsupported,
}

fn evaluate_fixture_expression(request: &SteelRuntimeRequest) -> FixtureEval {
    let source = request.source.trim();
    if let Some(text) = string_literal(source) {
        return FixtureEval::Output(text.to_string(), Vec::new());
    }
    if let Some(inner) = source.strip_prefix("(+ ").and_then(|value| value.strip_suffix(')')) {
        return add_expression(inner)
            .map_or(FixtureEval::Unsupported, |sum| FixtureEval::Output(sum.to_string(), Vec::new()));
    }
    if let Some(inner) = source.strip_prefix("(display ").and_then(|value| value.strip_suffix(')')) {
        if let Some(text) = string_literal(inner.trim()) {
            return FixtureEval::Output(text.to_string(), Vec::new());
        }
        return FixtureEval::Unsupported;
    }
    if let Some(inner) = source.strip_prefix("(host ").and_then(|value| value.strip_suffix(')')) {
        return evaluate_host_call(request, inner);
    }
    FixtureEval::Unsupported
}

fn evaluate_host_call(request: &SteelRuntimeRequest, inner: &str) -> FixtureEval {
    let args = quoted_args(inner);
    if args.is_empty() {
        return FixtureEval::Unsupported;
    }
    if request.profile.max_host_calls == 0 {
        return FixtureEval::Denied(
            SteelRuntimeReasonCode::HostCallBudgetExceeded,
            "host-call budget is exhausted by the selected Steel runtime profile".to_string(),
            Vec::new(),
        );
    }
    let name = &args[0];
    let Some(registration) = request.host_functions.iter().find(|item| &item.name == name) else {
        return FixtureEval::Denied(
            SteelRuntimeReasonCode::UnknownHostFunction,
            "Steel host function is not registered for this evaluation".to_string(),
            vec![denied_host_call(DeniedHostCall {
                name,
                message: "unknown host function",
            })],
        );
    };
    if disabled_tools(request).contains(name) || disabled_tools(request).contains("steel.host.*") {
        return FixtureEval::Denied(
            SteelRuntimeReasonCode::DisabledHostFunction,
            "Steel host function is disabled for this session".to_string(),
            vec![denied_host_call(DeniedHostCall {
                name,
                message: "disabled host function",
            })],
        );
    }
    if !session_capabilities(request).contains(&registration.required_capability) {
        return FixtureEval::Denied(
            SteelRuntimeReasonCode::MissingHostCapability,
            "session lacks the host-function capability required by the Steel profile".to_string(),
            vec![denied_host_call(DeniedHostCall {
                name,
                message: "missing session capability",
            })],
        );
    }
    FixtureEval::Output(registration.output.clone(), vec![SteelHostCallReceipt {
        name: name.clone(),
        outcome: SteelHostCallOutcome::Approved,
        safe_message: "approved host function executed through typed fixture seam".to_string(),
    }])
}

fn add_expression(inner: &str) -> Option<i64> {
    inner
        .split_whitespace()
        .try_fold(0_i64, |sum, item| item.parse::<i64>().ok().map(|value| sum + value))
}

fn string_literal(source: &str) -> Option<&str> {
    source.strip_prefix('"').and_then(|value| value.strip_suffix('"'))
}

fn quoted_args(source: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut rest = source.trim();
    while let Some(stripped) = rest.strip_prefix('"') {
        let Some(end) = stripped.find('"') else { break };
        args.push(stripped[..end].to_string());
        rest = stripped[end + 1..].trim_start();
    }
    args
}

fn session_capabilities(request: &SteelRuntimeRequest) -> BTreeSet<String> {
    request.session_capabilities.iter().cloned().collect()
}

fn disabled_tools(request: &SteelRuntimeRequest) -> BTreeSet<String> {
    request.disabled_tools.iter().cloned().collect()
}

struct DeniedHostCall<'a> {
    name: &'a str,
    message: &'a str,
}

fn denied_host_call(call: DeniedHostCall<'_>) -> SteelHostCallReceipt {
    SteelHostCallReceipt {
        name: call.name.to_string(),
        outcome: SteelHostCallOutcome::Denied,
        safe_message: call.message.to_string(),
    }
}

fn u64_to_usize_saturating(value: u64) -> usize {
    match usize::try_from(value) {
        Ok(converted) => converted,
        Err(_) => usize::MAX,
    }
}

fn ambient_attempt(source: &str) -> Option<&'static str> {
    let mut denied = BTreeMap::new();
    denied.insert("filesystem", ["open-file", "write-file", "read-file"]);
    denied.insert("process", ["system", "process", "shell"]);
    denied.insert("network", ["tcp-connect", "http-get", "socket"]);
    denied.insert("credential", ["credential", "secret", "token"]);
    denied.insert("provider", ["provider", "model-request", "router"]);
    denied.insert("daemon", ["daemon", "session-mutate", "attach"]);
    denied.insert("tui", ["tui", "screen", "key-event"]);
    denied.insert("native-tool", ["native-tool", "tool-exec", "bash"]);
    denied
        .into_iter()
        .find_map(|(kind, markers)| markers.iter().any(|marker| source.contains(marker)).then_some(kind))
}

fn estimate_steps(source: &str) -> u64 {
    source.chars().filter(|ch| *ch == '(' || *ch == ')' || ch.is_whitespace()).count() as u64 + 1
}

fn truncate_to_limit(output: &str, limit: usize) -> String {
    if limit == 0 {
        return String::new();
    }
    output.chars().take(limit).collect()
}

fn runtime_receipt(
    request: &SteelRuntimeRequest,
    source_hash: ArtifactHash,
    status: SteelRuntimeStatusCode,
    reason_code: SteelRuntimeReasonCode,
    safe_message: impl Into<String>,
    output: Option<String>,
    host_calls: Vec<SteelHostCallReceipt>,
    steps_used: u64,
) -> SteelRuntimeReceipt {
    let output_hash = output.as_ref().map(|value| ArtifactHash::digest(value.as_bytes()));
    let mut redactions = vec!["source".to_string(), "paths".to_string(), "credentials".to_string()];
    if matches!(reason_code, SteelRuntimeReasonCode::OutputTooLarge) {
        redactions.push("oversized-output".to_string());
    }
    SteelRuntimeReceipt {
        schema: STEEL_RUNTIME_RECEIPT_SCHEMA.to_string(),
        status,
        reason_code,
        safe_message: safe_message.into(),
        profile_name: request.profile.name.clone(),
        source_hash,
        output_hash,
        output,
        host_calls,
        redactions,
        steps_used,
        ambient_authority: request.profile.ambient_authority,
        sandbox_claim: "constrained embedded interpreter; no OS/process sandbox claim".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approved_host_request() -> SteelRuntimeRequest {
        SteelRuntimeRequest {
            profile: SteelRuntimeProfile::default_deny(),
            source: "(host \"steel.host.echo\")".to_string(),
            session_capabilities: vec!["steel.host.echo".to_string()],
            disabled_tools: Vec::new(),
            host_functions: vec![SteelHostFunctionRegistration {
                name: "steel.host.echo".to_string(),
                required_capability: "steel.host.echo".to_string(),
                output: "host-ok".to_string(),
            }],
            receipt_destination: "target/steel-runtime/fixture.json".to_string(),
        }
    }

    #[test]
    fn pure_fixture_evaluation_is_deterministic() {
        let request = SteelRuntimeRequest::pure("(+ 1 2 3)");
        let first = evaluate_steel_request(&request);
        let second = evaluate_steel_request(&request);
        assert_eq!(first.status, SteelRuntimeStatusCode::Succeeded);
        assert_eq!(first.output.as_deref(), Some("6"));
        assert_eq!(first.receipt_hash(), second.receipt_hash());
    }

    #[test]
    fn approved_host_function_requires_registration_and_capability() {
        let request = approved_host_request();
        let receipt = evaluate_steel_request(&request);
        assert_eq!(receipt.status, SteelRuntimeStatusCode::Succeeded);
        assert_eq!(receipt.output.as_deref(), Some("host-ok"));
        assert_eq!(receipt.host_calls[0].outcome, SteelHostCallOutcome::Approved);
    }

    #[test]
    fn denied_host_function_performs_no_fallback_effect() {
        let mut request = approved_host_request();
        request.session_capabilities.clear();
        let receipt = evaluate_steel_request(&request);
        assert_eq!(receipt.status, SteelRuntimeStatusCode::Denied);
        assert_eq!(receipt.reason_code, SteelRuntimeReasonCode::MissingHostCapability);
        assert!(receipt.output.is_none());
        assert_eq!(receipt.host_calls[0].outcome, SteelHostCallOutcome::Denied);
    }

    #[test]
    fn ambient_authority_attempts_are_denied_with_stable_reason() {
        for source in [
            "(write-file \"/tmp/x\" \"secret\")",
            "(system \"git status\")",
            "(http-get \"https://example.com\")",
            "(credential \"api\")",
        ] {
            let receipt = evaluate_steel_request(&SteelRuntimeRequest::pure(source));
            assert_eq!(receipt.status, SteelRuntimeStatusCode::Denied);
            assert_eq!(receipt.reason_code, SteelRuntimeReasonCode::AmbientAuthorityDenied);
            assert!(receipt.output.is_none());
        }
    }

    #[test]
    fn profile_limits_are_typed_and_redacted() {
        let mut request = SteelRuntimeRequest::pure("(display \"abcdef\")");
        request.profile.max_output_bytes = 3;
        let receipt = evaluate_steel_request(&request);
        assert_eq!(receipt.status, SteelRuntimeStatusCode::ResourceLimited);
        assert_eq!(receipt.reason_code, SteelRuntimeReasonCode::OutputTooLarge);
        assert_eq!(receipt.output.as_deref(), Some("abc"));
        assert!(receipt.redactions.iter().any(|item| item == "oversized-output"));
    }

    #[test]
    fn status_does_not_claim_sandboxing() {
        let status = steel_runtime_status(SteelRuntimeProfile::default_deny());
        assert!(status.available);
        assert!(!status.ambient_authority);
        assert!(status.sandbox_claim.contains("no OS/process sandbox claim"));
    }
}
