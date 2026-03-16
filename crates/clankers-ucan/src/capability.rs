//! Capability definitions for clankers agent authorization.
//!
//! Capabilities represent what operations a token holder can perform
//! with the clankers agent system.

use clanker_auth::Cap;
use serde::Deserialize;
use serde::Serialize;

/// Simple glob pattern matching for shell command and tool authorization.
///
/// Supports only `*` wildcards at the end of patterns (e.g., "pg_*").
/// Returns true if the pattern matches the input.
fn glob_match(pattern: &str, input: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    if let Some(prefix) = pattern.strip_suffix('*') {
        // Pattern like "pg_*" matches anything starting with "pg_"
        input.starts_with(prefix)
    } else if pattern.contains('*') {
        // More complex patterns: split by * and check each segment
        let parts: Vec<&str> = pattern.split('*').collect();
        if parts.is_empty() {
            return true;
        }

        let mut remaining = input;

        // First part must be at the start
        if !parts[0].is_empty() && !remaining.starts_with(parts[0]) {
            return false;
        }
        remaining = &remaining[parts[0].len()..];

        // Middle parts must exist somewhere in order
        for part in parts.iter().skip(1).take(parts.len().saturating_sub(2)) {
            if part.is_empty() {
                continue;
            }
            if let Some(pos) = remaining.find(part) {
                remaining = &remaining[pos + part.len()..];
            } else {
                return false;
            }
        }

        // Last part must be at the end (if non-empty)
        if let Some(last) = parts.last()
            && !last.is_empty()
            && !remaining.ends_with(last)
        {
            return false;
        }

        true
    } else {
        // No wildcards, exact match
        pattern == input
    }
}

/// What operations a token holder can perform with clankers agents.
///
/// # Tiger Style
///
/// - Explicit variants for each operation type
/// - Pattern matching for tools and commands (exact, comma-separated, or wildcard)
/// - Prefix-based file access scoping
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Capability {
    /// Can send prompts to the agent.
    Prompt,

    /// Can use specific tools. Pattern: exact "read", comma-separated "read,grep,find", or "*"
    /// wildcard.
    ToolUse { tool_pattern: String },

    /// Can execute shell commands via bash tool.
    ShellExecute {
        command_pattern: String,
        working_dir: Option<String>,
    },

    /// Can access files matching a path prefix. Scopes read/write/edit operations.
    FileAccess { prefix: String, read_only: bool },

    /// Can use bot commands. Pattern: comma-separated names or "*".
    BotCommand { command_pattern: String },

    /// Can manage sessions (restart, compact).
    SessionManage,

    /// Can switch the model.
    ModelSwitch,

    /// Can create child tokens with attenuated capabilities.
    Delegate,
}

impl Capability {
    /// Check if this capability authorizes the given operation.
    ///
    /// Returns true if this capability grants permission for the operation.
    // r[impl ucan.auth.wildcard-matches-all]
    // r[impl ucan.auth.read-only-blocks-write]
    pub fn authorizes(&self, op: &Operation) -> bool {
        match (self, op) {
            // Prompt operations
            (Capability::Prompt, Operation::Prompt { .. }) => true,

            // ToolUse operations
            (Capability::ToolUse { tool_pattern }, Operation::ToolUse { tool_name }) => {
                match_pattern(tool_pattern, tool_name)
            }

            // ShellExecute operations
            //
            // Tiger Style: decomposed compound condition into early returns
            (
                Capability::ShellExecute {
                    command_pattern,
                    working_dir: cap_wd,
                },
                Operation::ShellExecute {
                    command,
                    working_dir: op_wd,
                },
            ) => {
                if !glob_match(command_pattern, command) {
                    return false;
                }
                match (cap_wd, op_wd) {
                    (None, _) => true,
                    (Some(cap_dir), Some(req_dir)) => req_dir.starts_with(cap_dir),
                    (Some(_), None) => false,
                }
            }

            // FileAccess operations
            (
                Capability::FileAccess {
                    prefix,
                    read_only: false,
                },
                Operation::FileRead { path },
            ) => path.starts_with(prefix),
            (
                Capability::FileAccess {
                    prefix,
                    read_only: false,
                },
                Operation::FileWrite { path },
            ) => path.starts_with(prefix),
            (
                Capability::FileAccess {
                    prefix,
                    read_only: true,
                },
                Operation::FileRead { path },
            ) => path.starts_with(prefix),
            (
                Capability::FileAccess {
                    prefix: _,
                    read_only: true,
                },
                Operation::FileWrite { .. },
            ) => false, // Read-only doesn't allow writes

            // BotCommand operations
            (Capability::BotCommand { command_pattern }, Operation::BotCommand { command }) => {
                match_pattern(command_pattern, command)
            }

            // SessionManage operations
            (Capability::SessionManage, Operation::SessionManage { .. }) => true,

            // ModelSwitch operations
            (Capability::ModelSwitch, Operation::ModelSwitch { .. }) => true,

            // No match
            _ => false,
        }
    }

    /// Check if this capability is a superset of another (for delegation).
    ///
    /// During delegation, a child token can only have capabilities that are
    /// subsets of the parent's capabilities. This prevents privilege escalation.
    ///
    /// Returns true if `self` contains `other`.
    // r[impl ucan.auth.no-escalation]
    // r[impl ucan.auth.pattern-set-containment]
    pub fn contains(&self, other: &Capability) -> bool {
        match (self, other) {
            // Simple capabilities only contain themselves
            (Capability::Prompt, Capability::Prompt) => true,
            (Capability::SessionManage, Capability::SessionManage) => true,
            (Capability::ModelSwitch, Capability::ModelSwitch) => true,
            (Capability::Delegate, Capability::Delegate) => true,

            // ToolUse pattern containment
            (Capability::ToolUse { tool_pattern: p1 }, Capability::ToolUse { tool_pattern: p2 }) => {
                pattern_contains(p1, p2)
            }

            // FileAccess containment
            //
            // Tiger Style: decomposed into sequential checks
            (
                Capability::FileAccess {
                    prefix: p1,
                    read_only: r1,
                },
                Capability::FileAccess {
                    prefix: p2,
                    read_only: r2,
                },
            ) => {
                // Child must have narrower or equal prefix
                if !p2.starts_with(p1) {
                    return false;
                }
                // Child cannot escalate from read-only to read-write
                if *r1 && !*r2 {
                    return false;
                }
                true
            }

            // BotCommand pattern containment
            (Capability::BotCommand { command_pattern: p1 }, Capability::BotCommand { command_pattern: p2 }) => {
                pattern_contains(p1, p2)
            }

            // ShellExecute containment
            (
                Capability::ShellExecute {
                    command_pattern: p1,
                    working_dir: wd1,
                },
                Capability::ShellExecute {
                    command_pattern: p2,
                    working_dir: wd2,
                },
            ) => {
                // Child pattern must be subset of parent
                let pattern_ok = if p1 == "*" {
                    true // Parent allows everything
                } else if p2 == "*" {
                    false // Child wants everything but parent doesn't allow it
                } else if p1.ends_with('*') && p2.ends_with('*') {
                    // Both are prefix patterns, child must be more specific
                    p2.starts_with(p1.trim_end_matches('*'))
                } else if p1.ends_with('*') {
                    // Parent is prefix pattern, child is exact
                    p2.starts_with(p1.trim_end_matches('*'))
                } else {
                    // Parent is exact, child must be same
                    p1 == p2
                };

                // Tiger Style: decomposed — check pattern first, then wd
                if !pattern_ok {
                    return false;
                }
                match (wd1, wd2) {
                    (None, _) => true,
                    (Some(_), None) => false,
                    (Some(parent_wd), Some(child_wd)) => child_wd.starts_with(parent_wd),
                }
            }

            _ => false,
        }
    }
}

/// Match a pattern (exact, comma-separated list, or wildcard).
///
/// Examples:
/// - `match_pattern("read", "read")` => true
/// - `match_pattern("read,grep,find", "grep")` => true
/// - `match_pattern("*", "anything")` => true
/// - `match_pattern("read,grep", "bash")` => false
// r[impl ucan.auth.wildcard-matches-all]
fn match_pattern(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    pattern.split(',').any(|item| item.trim() == value)
}

/// Check if pattern1 contains pattern2 (for delegation).
///
/// Rules:
/// - "*" contains anything
/// - "a,b,c" contains "a,b" (all items in p2 must be in p1)
/// - Exact match contains itself
// r[impl ucan.auth.pattern-set-containment]
fn pattern_contains(p1: &str, p2: &str) -> bool {
    if p1 == "*" {
        return true;
    }

    if p2 == "*" {
        return false; // Child wants everything but parent doesn't allow it
    }

    // Both are comma-separated lists - check if all p2 items are in p1
    let p1_items: std::collections::HashSet<&str> = p1.split(',').map(|s| s.trim()).collect();
    let p2_items: Vec<&str> = p2.split(',').map(|s| s.trim()).collect();

    p2_items.iter().all(|item| p1_items.contains(item))
}

/// Operations that require authorization.
///
/// These map to clankers agent operations that need capability checks.
#[derive(Debug, Clone)]
pub enum Operation {
    /// Send a prompt to the agent.
    Prompt { text: String },

    /// Use a specific tool.
    ToolUse { tool_name: String },

    /// Execute a shell command.
    ShellExecute {
        command: String,
        working_dir: Option<String>,
    },

    /// Read a file.
    FileRead { path: String },

    /// Write or edit a file.
    FileWrite { path: String },

    /// Use a bot command.
    BotCommand { command: String },

    /// Manage session (restart, compact, etc.).
    SessionManage { action: String },

    /// Switch the model.
    ModelSwitch { model: String },
}

impl std::fmt::Display for Operation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Operation::Prompt { text } => write!(f, "Prompt({:.40}...)", text),
            Operation::ToolUse { tool_name } => write!(f, "ToolUse({})", tool_name),
            Operation::ShellExecute { command, working_dir } => {
                write!(f, "ShellExecute({}, wd={:?})", command, working_dir.as_deref().unwrap_or("<default>"))
            }
            Operation::FileRead { path } => write!(f, "FileRead({})", path),
            Operation::FileWrite { path } => write!(f, "FileWrite({})", path),
            Operation::BotCommand { command } => write!(f, "BotCommand({})", command),
            Operation::SessionManage { action } => write!(f, "SessionManage({})", action),
            Operation::ModelSwitch { model } => write!(f, "ModelSwitch({})", model),
        }
    }
}

impl Cap for Capability {
    type Operation = Operation;

    fn authorizes(&self, op: &Operation) -> bool {
        self.authorizes(op)
    }

    fn contains(&self, child: &Capability) -> bool {
        self.contains(child)
    }

    fn is_delegate(&self) -> bool {
        matches!(self, Capability::Delegate)
    }
}
