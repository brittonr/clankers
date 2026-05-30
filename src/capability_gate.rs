//! UCAN-backed capability gate for session and tool-call authorization.
//!
//! Converts verified UCAN capabilities into per-call enforcement checks.
//! The gate is attached to the Agent at session creation and consulted
//! before protected prompts, session operations, model switches, and tools.

use std::sync::Arc;
use std::time::SystemTime;

use clankers_agent::CapabilityGate;
use clankers_runtime::process_jobs::ProcessJobOperation;
use clankers_ucan::BasaltAdmissionRequest;
use clankers_ucan::BasaltUcanAuthority;
use clankers_ucan::Capability;
use clankers_ucan::EffectCapability;
use clankers_ucan::EffectKind;
use clankers_ucan::Operation;
use clankers_ucan::PublicCredentialEnvelope;
use clankers_ucan::RedbPublicCredentialStore;
use serde_json::Value;

/// File tools that perform read-only operations.
const FILE_READ_TOOLS: &[&str] = &["read", "rg", "grep", "find", "ls"];

/// File tools that perform write operations.
const FILE_WRITE_TOOLS: &[&str] = &["write", "edit"];

#[derive(Clone)]
pub struct PublicUcanToolAuthorization {
    envelope: PublicCredentialEnvelope,
    policy: Arc<basalt::Policy>,
    store: RedbPublicCredentialStore,
    session_resource_id: Option<String>,
}

impl PublicUcanToolAuthorization {
    #[must_use]
    pub fn new(
        envelope: PublicCredentialEnvelope,
        policy: Arc<basalt::Policy>,
        store: RedbPublicCredentialStore,
    ) -> Self {
        Self {
            envelope,
            policy,
            store,
            session_resource_id: None,
        }
    }

    #[must_use]
    pub fn with_session_resource_id(mut self, session_resource_id: impl Into<String>) -> Self {
        self.session_resource_id = Some(session_resource_id.into());
        self
    }
}

pub struct PublicUcanCapabilityGate {
    auth: PublicUcanToolAuthorization,
}

impl PublicUcanCapabilityGate {
    #[must_use]
    pub const fn new(auth: PublicUcanToolAuthorization) -> Self {
        Self { auth }
    }

    fn authorize_request(&self, request: &BasaltAdmissionRequest) -> Result<(), String> {
        let time = ucan::VerificationTime::try_from_system_time(SystemTime::now())
            .map_err(|error| format!("public UCAN clock error: {error}"))?;
        let authority = BasaltUcanAuthority::new(&self.auth.policy);
        let receipt = authority.authorize_with_revocations(&self.auth.envelope, time, &self.auth.store, request);
        if receipt.is_allowed() {
            return Ok(());
        }
        Err(format!(
            "public UCAN/Basalt denied {} {} on {}: {}",
            request.contract(),
            request.ability(),
            request.resource(),
            receipt.reason
        ))
    }

    fn authorize_all(&self, requests: &[BasaltAdmissionRequest]) -> Result<(), String> {
        for request in requests {
            self.authorize_request(request)?;
        }
        Ok(())
    }

    fn session_resource_id<'a>(&'a self, session_id: &'a str) -> &'a str {
        self.auth.session_resource_id.as_deref().unwrap_or(session_id)
    }
}

impl CapabilityGate for PublicUcanCapabilityGate {
    fn check_prompt(&self, session_id: &str, _text: &str) -> Result<(), String> {
        self.authorize_request(&public_prompt_request(self.session_resource_id(session_id)))
    }

    fn check_session_manage(&self, session_id: &str, _action: &str) -> Result<(), String> {
        self.authorize_request(&public_session_manage_request(self.session_resource_id(session_id)))
    }

    fn check_model_switch(&self, model: &str) -> Result<(), String> {
        self.authorize_request(&public_model_request(model))
    }

    fn check_tool_call(&self, tool_name: &str, input: &Value) -> Result<(), String> {
        let requests = public_tool_requests(tool_name, input)?;
        self.authorize_all(requests.as_slice())
    }
}

fn public_prompt_request(session_id: &str) -> BasaltAdmissionRequest {
    BasaltAdmissionRequest::new("session-prompt", format!("clankers:session/{session_id}"), "session/prompt")
}

fn public_session_manage_request(session_id: &str) -> BasaltAdmissionRequest {
    BasaltAdmissionRequest::new("session-manage", format!("clankers:session/{session_id}"), "session/manage")
}

fn public_model_request(model: &str) -> BasaltAdmissionRequest {
    BasaltAdmissionRequest::new("model-use", format!("clankers:model/{}", encode_resource_segment(model)), "model/use")
}

fn public_tool_requests(tool_name: &str, input: &Value) -> Result<Vec<BasaltAdmissionRequest>, String> {
    let mut requests = vec![BasaltAdmissionRequest::new(
        "tool-use",
        format!("clankers:tool/{}", encode_resource_segment(tool_name)),
        "tool/use",
    )];

    if let Some(path) = input.get("path").and_then(|value| value.as_str()) {
        if FILE_READ_TOOLS.contains(&tool_name) {
            requests.push(file_request("file-read", EffectKind::FileRead, path)?);
        }
        if FILE_WRITE_TOOLS.contains(&tool_name) {
            requests.push(file_request("file-write", EffectKind::FileWrite, path)?);
        }
    }

    if tool_name == "bash" {
        let cwd = input.get("cwd").and_then(|value| value.as_str()).unwrap_or(".");
        requests.push(BasaltAdmissionRequest::new(
            "shell-execute",
            format!("clankers:shell:{}", encode_resource_segment(cwd)),
            "shell/execute",
        ));
    }

    if tool_name == "process"
        && let Some(action) = input.get("action").and_then(|value| value.as_str())
    {
        requests.extend(public_process_requests(action, input)?);
    }

    if tool_name == "switch_model" {
        let target = input
            .get("model")
            .or_else(|| input.get("role"))
            .and_then(|value| value.as_str())
            .unwrap_or("default");
        requests.push(public_model_request(target));
    }

    Ok(requests)
}

fn file_request(contract: &str, kind: EffectKind, path: &str) -> Result<BasaltAdmissionRequest, String> {
    let capability = EffectCapability::new(kind, path).map_err(|error| error.to_string())?;
    Ok(BasaltAdmissionRequest::new(contract, capability.resource(), capability.ability()))
}

fn public_process_requests(action: &str, input: &Value) -> Result<Vec<BasaltAdmissionRequest>, String> {
    let Some(requirement) = process_action_requirement(action) else {
        return Err(format!("Process action not authorized: {action}"));
    };
    let backend = input.get("backend").and_then(|value| value.as_str()).unwrap_or("native");
    let resource = format!("clankers:process/{}", encode_resource_segment(backend));
    let mut requests = requirement
        .public_abilities(input)
        .into_iter()
        .map(|ability| BasaltAdmissionRequest::new("process-action", resource.clone(), ability))
        .collect::<Vec<_>>();
    if requires_backend_selection(input) {
        requests.push(BasaltAdmissionRequest::new("tool-use", "clankers:tool/backend%2Fprocess", "tool/use"));
    }
    Ok(requests)
}

fn encode_resource_segment(input: &str) -> String {
    use std::fmt::Write;

    let mut encoded = String::new();
    for byte in input.as_bytes() {
        if is_unreserved(*byte) {
            encoded.push(char::from(*byte));
        } else {
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

const fn is_unreserved(byte: u8) -> bool {
    matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_')
}

/// Capability gate backed by UCAN token capabilities.
///
/// Checks each tool call against the set of capabilities granted by the
/// session's verified UCAN token. Blocks calls that aren't authorized.
pub struct UcanCapabilityGate {
    capabilities: Vec<Capability>,
}

impl UcanCapabilityGate {
    /// Create a gate from a set of verified UCAN capabilities.
    pub fn new(capabilities: Vec<Capability>) -> Self {
        Self { capabilities }
    }

    fn authorizes_tool(&self, tool_name: &str) -> bool {
        self.capabilities.iter().any(|c| {
            c.authorizes(&Operation::ToolUse {
                tool_name: tool_name.to_string(),
            })
        })
    }

    fn check_process_tool_call(&self, input: &Value) -> Result<(), String> {
        if self.authorizes_tool("process") {
            return Ok(());
        }

        let Some(action) = input.get("action").and_then(|v| v.as_str()) else {
            return Err("Process action not authorized: missing action".to_string());
        };
        let Some(requirement) = process_action_requirement(action) else {
            return Err(format!("Process action not authorized: {action}"));
        };
        if requirement.allows_for_input(self, input) {
            return Ok(());
        }
        Err(format!(
            "Process action '{}' not authorized by capability token; requires {}",
            action,
            requirement.description(input)
        ))
    }
}

#[derive(Debug, Clone, Copy)]
struct ProcessActionRequirement {
    operation: ProcessJobOperation,
    required_tools: &'static [&'static str],
}

impl ProcessActionRequirement {
    fn allows(&self, gate: &UcanCapabilityGate) -> bool {
        self.required_tools.iter().all(|tool| gate.authorizes_tool(tool))
    }

    fn allows_for_input(&self, gate: &UcanCapabilityGate, input: &Value) -> bool {
        self.allows(gate) && (!requires_backend_selection(input) || gate.authorizes_tool("process:backend"))
    }

    fn description(&self, input: &Value) -> String {
        let mut tools = self.required_tools.to_vec();
        if requires_backend_selection(input) {
            tools.push("process:backend");
        }
        format!("{} capability ({})", process_operation_label(self.operation), tools.join(" + "))
    }

    fn public_abilities(&self, input: &Value) -> Vec<&'static str> {
        let mut abilities = vec![process_public_ability(self.operation)];
        if matches!(self.operation, ProcessJobOperation::Log) {
            abilities.push("process/logs");
        }
        if matches!(self.operation, ProcessJobOperation::WriteStdin | ProcessJobOperation::CloseStdin) {
            abilities.push("process/stdin");
        }
        if requires_backend_selection(input) {
            abilities.push("process/observe");
        }
        abilities.sort_unstable();
        abilities.dedup();
        abilities
    }
}

fn requires_backend_selection(input: &Value) -> bool {
    input.get("backend").and_then(|backend| backend.as_str()).is_some_and(|backend| backend != "native")
}

fn process_action_requirement(action: &str) -> Option<ProcessActionRequirement> {
    Some(match action {
        "list" => ProcessActionRequirement {
            operation: ProcessJobOperation::List,
            required_tools: &["process:observe"],
        },
        "poll" => ProcessActionRequirement {
            operation: ProcessJobOperation::Poll,
            required_tools: &["process:observe"],
        },
        "wait" => ProcessActionRequirement {
            operation: ProcessJobOperation::Wait,
            required_tools: &["process:observe"],
        },
        "log" => ProcessActionRequirement {
            operation: ProcessJobOperation::Log,
            required_tools: &["process:observe", "process:logs"],
        },
        "start" => ProcessActionRequirement {
            operation: ProcessJobOperation::Start,
            required_tools: &["process:start"],
        },
        "kill" => ProcessActionRequirement {
            operation: ProcessJobOperation::Kill,
            required_tools: &["process:mutate"],
        },
        "restart" => ProcessActionRequirement {
            operation: ProcessJobOperation::Restart,
            required_tools: &["process:mutate"],
        },
        "write" | "submit" => ProcessActionRequirement {
            operation: ProcessJobOperation::WriteStdin,
            required_tools: &["process:mutate", "process:stdin"],
        },
        "close" => ProcessActionRequirement {
            operation: ProcessJobOperation::CloseStdin,
            required_tools: &["process:mutate", "process:stdin"],
        },
        "adopt" => ProcessActionRequirement {
            operation: ProcessJobOperation::Adopt,
            required_tools: &["process:mutate"],
        },
        "gc" | "garbage_collect" => ProcessActionRequirement {
            operation: ProcessJobOperation::GarbageCollect,
            required_tools: &["process:mutate"],
        },
        _ => return None,
    })
}

fn process_public_ability(operation: ProcessJobOperation) -> &'static str {
    match operation {
        ProcessJobOperation::List | ProcessJobOperation::Poll | ProcessJobOperation::Wait => "process/observe",
        ProcessJobOperation::Start => "process/start",
        ProcessJobOperation::Log => "process/observe",
        ProcessJobOperation::Kill
        | ProcessJobOperation::Restart
        | ProcessJobOperation::Adopt
        | ProcessJobOperation::GarbageCollect => "process/mutate",
        ProcessJobOperation::WriteStdin | ProcessJobOperation::CloseStdin => "process/mutate",
    }
}

fn process_operation_label(operation: ProcessJobOperation) -> &'static str {
    match operation {
        ProcessJobOperation::Start => "process start",
        ProcessJobOperation::List | ProcessJobOperation::Poll | ProcessJobOperation::Wait => "process observe",
        ProcessJobOperation::Log => "process log",
        ProcessJobOperation::Kill | ProcessJobOperation::Restart => "process mutation",
        ProcessJobOperation::WriteStdin | ProcessJobOperation::CloseStdin => "process stdin mutation",
        ProcessJobOperation::Adopt => "process adoption",
        ProcessJobOperation::GarbageCollect => "process garbage collection",
    }
}

impl CapabilityGate for UcanCapabilityGate {
    // r[impl ucan.gate.tool-check]
    // r[impl ucan.gate.file-read-check]
    // r[impl ucan.gate.file-write-check]
    fn check_tool_call(&self, tool_name: &str, input: &Value) -> Result<(), String> {
        if tool_name == "process" {
            return self.check_process_tool_call(input);
        }

        // 1. Check ToolUse capability
        let is_tool_allowed = self.authorizes_tool(tool_name);
        if !is_tool_allowed {
            return Err(format!("Tool '{}' not authorized by capability token", tool_name));
        }

        // 2. For bash/shell tools, check ShellExecute capability
        if tool_name == "bash"
            && let Some(cmd) = input.get("command").and_then(|v| v.as_str())
        {
            let wd = input.get("cwd").and_then(|v| v.as_str()).map(String::from);
            let is_shell_allowed = self.capabilities.iter().any(|c| {
                c.authorizes(&Operation::ShellExecute {
                    command: cmd.to_string(),
                    working_dir: wd.clone(),
                })
            });
            if !is_shell_allowed {
                let preview = &cmd[..80.min(cmd.len())];
                return Err(format!("Shell command not authorized: {preview}"));
            }
        }

        // 3. For file tools, check FileAccess capability
        if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
            if FILE_READ_TOOLS.contains(&tool_name) {
                let is_read_allowed =
                    self.capabilities.iter().any(|c| c.authorizes(&Operation::FileRead { path: path.to_string() }));
                if !is_read_allowed {
                    return Err(format!("File read not authorized: {path}"));
                }
            }
            if FILE_WRITE_TOOLS.contains(&tool_name) {
                let is_write_allowed =
                    self.capabilities.iter().any(|c| c.authorizes(&Operation::FileWrite { path: path.to_string() }));
                if !is_write_allowed {
                    return Err(format!("File write not authorized: {path}"));
                }
            }
        }

        Ok(())
    }
}

/// Extract tool patterns from UCAN capabilities as `Vec<String>` for the
/// controller's simple capability field (used for `GetCapabilities` responses).
pub fn extract_tool_patterns(caps: &[Capability]) -> Option<Vec<String>> {
    let patterns: Vec<String> = caps
        .iter()
        .filter_map(|c| match c {
            Capability::ToolUse { tool_pattern } => Some(tool_pattern.clone()),
            _ => None,
        })
        .collect();

    if patterns.is_empty() || patterns.iter().any(|p| p == "*") {
        None // Full access or no tool restrictions
    } else {
        Some(patterns)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    // r[verify ucan.gate.tool-check]
    #[test]
    fn wildcard_tool_allows_all() {
        let gate = UcanCapabilityGate::new(vec![Capability::ToolUse {
            tool_pattern: "*".into(),
        }]);
        assert!(gate.check_tool_call("bash", &json!({})).is_ok());
        assert!(gate.check_tool_call("read", &json!({})).is_ok());
        assert!(gate.check_tool_call("anything", &json!({})).is_ok());
    }

    // r[verify ucan.gate.tool-check]
    #[test]
    fn specific_tools_block_unlisted() {
        let gate = UcanCapabilityGate::new(vec![Capability::ToolUse {
            tool_pattern: "read,grep,find".into(),
        }]);
        assert!(gate.check_tool_call("read", &json!({})).is_ok());
        assert!(gate.check_tool_call("grep", &json!({})).is_ok());
        assert!(gate.check_tool_call("bash", &json!({})).is_err());
        assert!(gate.check_tool_call("write", &json!({})).is_err());
    }

    #[test]
    fn process_observe_and_log_caps_do_not_allow_mutation() {
        let gate = UcanCapabilityGate::new(vec![Capability::ToolUse {
            tool_pattern: "process:observe,process:logs".into(),
        }]);

        for action in ["list", "poll", "wait", "log"] {
            assert!(gate.check_tool_call("process", &json!({"action": action})).is_ok(), "{action} should be allowed");
        }
        for action in ["start", "kill", "restart", "write", "submit", "close"] {
            assert!(gate.check_tool_call("process", &json!({"action": action})).is_err(), "{action} should be denied");
        }
    }

    #[test]
    fn process_log_requires_observe_and_log_caps() {
        let observe_only = UcanCapabilityGate::new(vec![Capability::ToolUse {
            tool_pattern: "process:observe".into(),
        }]);
        let logs_only = UcanCapabilityGate::new(vec![Capability::ToolUse {
            tool_pattern: "process:logs".into(),
        }]);
        assert!(observe_only.check_tool_call("process", &json!({"action": "list"})).is_ok());
        assert!(observe_only.check_tool_call("process", &json!({"action": "log"})).is_err());
        assert!(logs_only.check_tool_call("process", &json!({"action": "log"})).is_err());
    }

    #[test]
    fn process_stdin_requires_mutate_and_stdin_caps() {
        let mutate_only = UcanCapabilityGate::new(vec![Capability::ToolUse {
            tool_pattern: "process:mutate".into(),
        }]);
        let stdin_only = UcanCapabilityGate::new(vec![Capability::ToolUse {
            tool_pattern: "process:stdin".into(),
        }]);
        let both = UcanCapabilityGate::new(vec![Capability::ToolUse {
            tool_pattern: "process:mutate,process:stdin".into(),
        }]);

        for action in ["write", "submit", "close"] {
            assert!(mutate_only.check_tool_call("process", &json!({"action": action})).is_err());
            assert!(stdin_only.check_tool_call("process", &json!({"action": action})).is_err());
            assert!(both.check_tool_call("process", &json!({"action": action})).is_ok());
        }
    }

    #[test]
    fn non_native_backend_selection_requires_backend_cap() {
        let start_only = UcanCapabilityGate::new(vec![Capability::ToolUse {
            tool_pattern: "process:start".into(),
        }]);
        let start_with_backend = UcanCapabilityGate::new(vec![Capability::ToolUse {
            tool_pattern: "process:start,process:backend".into(),
        }]);

        assert!(start_only.check_tool_call("process", &json!({"action": "start", "backend": "native"})).is_ok());
        assert!(start_only.check_tool_call("process", &json!({"action": "start", "backend": "pueue"})).is_err());
        assert!(
            start_with_backend
                .check_tool_call("process", &json!({"action": "start", "backend": "pueue"}))
                .is_ok()
        );
    }

    #[test]
    fn legacy_process_tool_cap_allows_process_actions() {
        let gate = UcanCapabilityGate::new(vec![Capability::ToolUse {
            tool_pattern: "process".into(),
        }]);

        for action in [
            "start", "list", "poll", "log", "wait", "kill", "restart", "write", "submit", "close",
        ] {
            assert!(gate.check_tool_call("process", &json!({"action": action})).is_ok(), "{action} should be allowed");
        }
    }

    #[test]
    fn shell_execute_checked_for_bash() {
        let gate = UcanCapabilityGate::new(vec![
            Capability::ToolUse {
                tool_pattern: "*".into(),
            },
            Capability::ShellExecute {
                command_pattern: "ls*".into(),
                working_dir: None,
            },
        ]);

        // Allowed: matches ls* pattern
        assert!(gate.check_tool_call("bash", &json!({"command": "ls -la"})).is_ok());

        // Blocked: rm doesn't match ls* pattern
        assert!(gate.check_tool_call("bash", &json!({"command": "rm -rf /"})).is_err());
    }

    // r[verify ucan.gate.file-read-check]
    #[test]
    fn file_read_enforced() {
        let gate = UcanCapabilityGate::new(vec![
            Capability::ToolUse {
                tool_pattern: "*".into(),
            },
            Capability::FileAccess {
                prefix: "/home/alice/project/".into(),
                read_only: true,
            },
        ]);

        assert!(gate.check_tool_call("read", &json!({"path": "/home/alice/project/src/main.rs"})).is_ok());
        assert!(gate.check_tool_call("read", &json!({"path": "/etc/shadow"})).is_err());
    }

    // r[verify ucan.gate.file-write-check]
    // r[verify ucan.auth.read-only-blocks-write]
    #[test]
    fn file_write_blocked_by_read_only() {
        let gate = UcanCapabilityGate::new(vec![
            Capability::ToolUse {
                tool_pattern: "*".into(),
            },
            Capability::FileAccess {
                prefix: "/home/alice/".into(),
                read_only: true,
            },
        ]);

        assert!(gate.check_tool_call("read", &json!({"path": "/home/alice/file.txt"})).is_ok());
        assert!(gate.check_tool_call("write", &json!({"path": "/home/alice/file.txt"})).is_err());
    }

    // r[verify ucan.gate.file-read-check]
    // r[verify ucan.gate.file-write-check]
    #[test]
    fn no_file_capability_blocks_file_tools() {
        let gate = UcanCapabilityGate::new(vec![
            Capability::ToolUse {
                tool_pattern: "read,write".into(),
            },
            // No FileAccess capability
        ]);

        // Tool name check passes, but file path check fails
        assert!(gate.check_tool_call("read", &json!({"path": "/etc/passwd"})).is_err());
    }

    #[test]
    fn tool_without_path_skips_file_check() {
        let gate = UcanCapabilityGate::new(vec![Capability::ToolUse {
            tool_pattern: "read".into(),
        }]);

        // No path param — file check doesn't apply (tool name check passes)
        assert!(gate.check_tool_call("read", &json!({})).is_ok());
    }

    #[test]
    fn legacy_ucan_gate_preserves_tool_only_default_capabilities_behavior() {
        let gate = UcanCapabilityGate::new(vec![Capability::ToolUse {
            tool_pattern: "switch_model".into(),
        }]);

        // The legacy gate is still used for local settings.defaultCapabilities,
        // whose documented examples restrict tools without requiring Prompt,
        // SessionManage, or ModelSwitch entries. Public UCAN + Basalt gates
        // enforce those broader session/model operations.
        assert!(gate.check_prompt("session-1", "hello").is_ok());
        assert!(gate.check_session_manage("session-1", "compact_history").is_ok());
        assert!(gate.check_model_switch("slow").is_ok());
        assert!(gate.check_tool_call("switch_model", &json!({"role": "slow"})).is_ok());
    }

    fn public_gate(capabilities: Vec<ucan::CapabilityDocument>) -> (tempfile::TempDir, PublicUcanCapabilityGate) {
        public_gate_with_session_resource(capabilities, None)
    }

    fn public_gate_with_session_resource(
        capabilities: Vec<ucan::CapabilityDocument>,
        session_resource_id: Option<&str>,
    ) -> (tempfile::TempDir, PublicUcanCapabilityGate) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let db = Arc::new(redb::Database::create(tmp.path().join("auth.db")).expect("db"));
        let store = RedbPublicCredentialStore::new(db).expect("store");
        let root = clankers_ucan::PublicUcanIssuer::from_signer(ucan::Ed25519InMemorySigner::from_seed_bytes(
            [71; ucan::ED25519_SECRET_KEY_BYTES],
        ));
        let session = clankers_ucan::PublicUcanIssuer::from_signer(ucan::Ed25519InMemorySigner::from_seed_bytes(
            [73; ucan::ED25519_SECRET_KEY_BYTES],
        ));
        let envelope = root
            .issue_root_credential(
                session.audience().expect("audience"),
                ucan::CapabilitySet::new(capabilities).expect("capabilities"),
                std::time::Duration::from_secs(60),
            )
            .expect("credential");
        let policy = Arc::new(clankers_ucan::clankers_daemon_auth_policy().expect("policy"));
        let mut auth = PublicUcanToolAuthorization::new(envelope, policy, store);
        if let Some(session_resource_id) = session_resource_id {
            auth = auth.with_session_resource_id(session_resource_id);
        }
        (tmp, PublicUcanCapabilityGate::new(auth))
    }

    fn public_cap(resource: &str, ability: &str) -> ucan::CapabilityDocument {
        ucan::CapabilityDocument::new(resource.to_owned(), ability.to_owned()).expect("capability")
    }

    #[test]
    fn public_ucan_gate_requires_tool_and_file_capabilities() {
        let (_tmp, gate) = public_gate(vec![
            public_cap("clankers:tool/read", "tool/use"),
            public_cap("clankers:file:/tmp/project/", "file/read"),
        ]);

        assert!(gate.check_tool_call("read", &json!({"path": "/tmp/project/lib.rs"})).is_ok());
        assert!(gate.check_tool_call("read", &json!({"path": "/etc/passwd"})).is_err());
        assert!(gate.check_tool_call("write", &json!({"path": "/tmp/project/lib.rs"})).is_err());
    }

    #[test]
    fn public_ucan_gate_requires_shell_execute_in_addition_to_bash_tool() {
        let (_tmp, gate) = public_gate(vec![
            public_cap("clankers:tool/bash", "tool/use"),
            public_cap("clankers:shell:%2Fworkspace", "shell/execute"),
        ]);

        assert!(gate.check_tool_call("bash", &json!({"command": "ls", "cwd": "/workspace"})).is_ok());
        assert!(gate.check_tool_call("bash", &json!({"command": "ls", "cwd": "/other"})).is_err());
    }

    #[test]
    fn public_ucan_gate_maps_process_actions_to_process_abilities() {
        let (_tmp, gate) = public_gate(vec![
            public_cap("clankers:tool/process", "tool/use"),
            public_cap("clankers:process/native", "process/observe"),
            public_cap("clankers:process/native", "process/logs"),
        ]);

        assert!(gate.check_tool_call("process", &json!({"action": "list"})).is_ok());
        assert!(gate.check_tool_call("process", &json!({"action": "log"})).is_ok());
        assert!(gate.check_tool_call("process", &json!({"action": "kill"})).is_err());
    }

    #[test]
    fn public_ucan_gate_requires_backend_capability_for_non_native_processes() {
        let (_tmp, gate) = public_gate(vec![
            public_cap("clankers:tool/process", "tool/use"),
            public_cap("clankers:process/pueue", "process/start"),
            public_cap("clankers:process/pueue", "process/observe"),
        ]);

        let denied = gate.check_tool_call("process", &json!({"action": "start", "backend": "pueue"}));

        assert!(denied.is_err());
        assert!(denied.expect_err("backend cap missing").contains("backend%2Fprocess"));
    }

    #[test]
    fn public_ucan_gate_maps_model_switch_to_model_use() {
        let (_tmp, gate) = public_gate(vec![
            public_cap("clankers:tool/switch_model", "tool/use"),
            public_cap("clankers:model/slow", "model/use"),
        ]);

        assert!(gate.check_tool_call("switch_model", &json!({"role": "slow", "reason": "hard"})).is_ok());
        assert!(gate.check_tool_call("switch_model", &json!({"role": "smol", "reason": "easy"})).is_err());
    }

    #[test]
    fn public_ucan_gate_requires_session_prompt_capability() {
        let (_tmp, gate) = public_gate(vec![public_cap("clankers:session/alice", "session/prompt")]);

        assert!(gate.check_prompt("alice", "hello").is_ok());
        let denied = gate.check_prompt("bob", "hello");
        let reason = denied.expect_err("bob prompt should require its own session grant");
        assert!(reason.contains("session/prompt"));
        assert!(reason.contains("clankers:session/bob"));
    }

    #[test]
    fn public_ucan_gate_can_bind_prompt_checks_to_transport_identity() {
        let (_tmp, gate) = public_gate_with_session_resource(
            vec![public_cap("clankers:session/@alice:example.org", "session/prompt")],
            Some("@alice:example.org"),
        );

        assert!(gate.check_prompt("generated-session-id", "hello").is_ok());
    }

    #[test]
    fn public_ucan_gate_requires_session_manage_capability() {
        let (_tmp, prompt_only) = public_gate(vec![public_cap("clankers:session/alice", "session/prompt")]);
        let denied = prompt_only.check_session_manage("alice", "clear_history");
        let reason = denied.expect_err("prompt grant must not manage session");
        assert!(reason.contains("session/manage"));
        assert!(reason.contains("session-manage"));

        let (_tmp, manage) = public_gate(vec![public_cap("clankers:session/alice", "session/manage")]);
        assert!(manage.check_session_manage("alice", "clear_history").is_ok());
        assert!(manage.check_session_manage("bob", "clear_history").is_err());
    }

    #[test]
    fn public_ucan_gate_requires_model_use_for_protocol_model_switch() {
        let (_tmp, gate) = public_gate(vec![public_cap("clankers:model/claude-opus", "model/use")]);

        assert!(gate.check_model_switch("claude-opus").is_ok());
        let denied = gate.check_model_switch("claude-haiku");
        let reason = denied.expect_err("ungranted model should be denied");
        assert!(reason.contains("model/use"));
        assert!(reason.contains("clankers:model/claude-haiku"));
    }

    #[test]
    fn public_session_and_model_requests_are_concrete() {
        let prompt = public_prompt_request("alice");
        assert_eq!(prompt.contract(), "session-prompt");
        assert_eq!(prompt.resource(), "clankers:session/alice");
        assert_eq!(prompt.ability(), "session/prompt");

        let manage = public_session_manage_request("alice");
        assert_eq!(manage.contract(), "session-manage");
        assert_eq!(manage.resource(), "clankers:session/alice");
        assert_eq!(manage.ability(), "session/manage");

        let model = public_model_request("openai/gpt-5.3 codex");
        assert_eq!(model.contract(), "model-use");
        assert_eq!(model.resource(), "clankers:model/openai%2Fgpt-5.3%20codex");
        assert_eq!(model.ability(), "model/use");
    }

    #[test]
    fn public_tool_requests_are_concrete_and_receipts_identify_denial() {
        let requests = public_tool_requests("write", &json!({"path": "/tmp/project/out.txt"})).expect("requests");
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].contract(), "tool-use");
        assert_eq!(requests[0].resource(), "clankers:tool/write");
        assert_eq!(requests[0].ability(), "tool/use");
        assert_eq!(requests[1].contract(), "file-write");
        assert_eq!(requests[1].resource(), "clankers:file:/tmp/project/out.txt");
        assert_eq!(requests[1].ability(), "file/write");

        let (_tmp, gate) = public_gate(vec![
            public_cap("clankers:tool/write", "tool/use"),
            public_cap("clankers:file:/tmp/project/", "file/read"),
        ]);
        let denied = gate.check_tool_call("write", &json!({"path": "/tmp/project/out.txt"}));
        let reason = denied.expect_err("write grant missing");
        assert!(reason.contains("file/write"));
        assert!(reason.contains("Basalt"));
    }

    #[test]
    fn public_ucan_denial_happens_before_bash_confirmation_can_bypass_it() {
        let (_tmp, gate) = public_gate(vec![
            public_cap("clankers:tool/bash", "tool/use"),
            public_cap("clankers:shell:%2Fsafe", "shell/execute"),
        ]);

        let denied = gate.check_tool_call("bash", &json!({"command": "rm -rf /tmp/project", "cwd": "/unsafe"}));

        let reason = denied.expect_err("shell grant missing");
        assert!(reason.contains("shell/execute"));
        assert!(reason.contains("clankers:shell:%2Funsafe"));
    }

    #[test]
    fn extract_tool_patterns_wildcard() {
        let caps = vec![Capability::ToolUse {
            tool_pattern: "*".into(),
        }];
        assert!(extract_tool_patterns(&caps).is_none());
    }

    #[test]
    fn extract_tool_patterns_specific() {
        let caps = vec![
            Capability::Prompt,
            Capability::ToolUse {
                tool_pattern: "read,grep".into(),
            },
            Capability::FileAccess {
                prefix: "/".into(),
                read_only: false,
            },
        ];
        let patterns = extract_tool_patterns(&caps).unwrap();
        assert_eq!(patterns, vec!["read,grep"]);
    }
}
