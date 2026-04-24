//! UCAN-backed capability gate for tool call authorization.
//!
//! Converts verified UCAN capabilities into per-call enforcement checks.
//! The gate is attached to the Agent at session creation and consulted
//! before every tool execution.

use clankers_agent::CapabilityGate;
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
}

impl CapabilityGate for UcanCapabilityGate {
    // r[impl ucan.gate.tool-check]
    // r[impl ucan.gate.file-read-check]
    // r[impl ucan.gate.file-write-check]
    fn check_tool_call(&self, tool_name: &str, input: &Value) -> Result<(), String> {
        // 1. Check ToolUse capability
        let is_tool_allowed = self.capabilities.iter().any(|c| {
            c.authorizes(&Operation::ToolUse {
                tool_name: tool_name.to_string(),
            })
        });
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
