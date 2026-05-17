//! UCAN-backed capability gate for tool call authorization.
//!
//! Converts verified UCAN capabilities into per-call enforcement checks.
//! The gate is attached to the Agent at session creation and consulted
//! before every tool execution.

use clankers_agent::CapabilityGate;
use clankers_runtime::process_jobs::ProcessJobOperation;
use clankers_ucan::Capability;
use clankers_ucan::Operation;
use serde_json::Value;

/// File tools that perform read-only operations.
const FILE_READ_TOOLS: &[&str] = &["read", "rg", "grep", "find", "ls"];

/// File tools that perform write operations.
const FILE_WRITE_TOOLS: &[&str] = &["write", "edit"];

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
        if requirement.allows(self) {
            return Ok(());
        }
        Err(format!(
            "Process action '{}' not authorized by capability token; requires {}",
            action,
            requirement.description()
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

    fn description(&self) -> String {
        format!("{} capability ({})", process_operation_label(self.operation), self.required_tools.join(" + "))
    }
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
        _ => return None,
    })
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
