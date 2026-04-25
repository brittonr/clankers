//! Reusable tool execution contracts for engine-host runners.
//!
//! This crate owns plain tool-host outcomes and result accumulation. It does
//! not supervise Clankers plugins, discover built-in tools, or interpret engine
//! reducer policy.

use clanker_message::Content;
use clankers_engine::EngineCorrelationId;
use clankers_engine::EngineToolCall;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

pub const DEFAULT_TOOL_MAX_BYTES: usize = 200_000;
pub const DEFAULT_TOOL_MAX_LINES: usize = 10_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolDescriptor {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityDecision {
    Allowed,
    Denied { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolTruncationLimits {
    pub max_bytes: usize,
    pub max_lines: usize,
}

impl Default for ToolTruncationLimits {
    fn default() -> Self {
        Self {
            max_bytes: DEFAULT_TOOL_MAX_BYTES,
            max_lines: DEFAULT_TOOL_MAX_LINES,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolTruncationMetadata {
    pub original_bytes: usize,
    pub original_lines: usize,
    pub truncated_bytes: usize,
    pub truncated_lines: usize,
}

#[derive(Debug, Clone)]
pub enum ToolHostOutcome {
    Succeeded {
        content: Vec<Content>,
        details: Value,
    },
    ToolError {
        content: Vec<Content>,
        details: Value,
        message: String,
    },
    MissingTool {
        name: String,
    },
    CapabilityDenied {
        name: String,
        reason: String,
    },
    Cancelled {
        name: String,
    },
    Truncated {
        content: Vec<Content>,
        metadata: ToolTruncationMetadata,
    },
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ToolHostError {
    #[error("tool host failed: {message}")]
    HostFailed { message: String },
}

pub trait ToolCatalog {
    fn describe_tools(&self) -> Vec<ToolDescriptor>;
    fn contains_tool(&self, name: &str) -> bool;
}

pub trait CapabilityChecker {
    fn check_capability(&mut self, call: &EngineToolCall) -> CapabilityDecision;
}

pub trait ToolHook {
    fn before_tool(&mut self, call: &EngineToolCall) -> Result<(), ToolHostError>;
    fn after_tool(&mut self, call: &EngineToolCall, outcome: &ToolHostOutcome) -> Result<(), ToolHostError>;
}

pub trait ToolExecutor {
    fn execute_tool(&mut self, call: EngineToolCall) -> impl core::future::Future<Output = ToolHostOutcome> + Send;
}

#[derive(Debug, Clone)]
pub struct ToolOutputAccumulator {
    limits: ToolTruncationLimits,
    chunks: Vec<String>,
}

impl ToolOutputAccumulator {
    #[must_use]
    pub fn new(limits: ToolTruncationLimits) -> Self {
        assert!(limits.max_bytes > 0, "tool truncation max_bytes must be positive");
        assert!(limits.max_lines > 0, "tool truncation max_lines must be positive");
        Self {
            limits,
            chunks: Vec::new(),
        }
    }

    pub fn push(&mut self, chunk: impl Into<String>) {
        self.chunks.push(chunk.into());
    }

    #[must_use]
    pub fn finish(self) -> ToolHostOutcome {
        let combined = self.chunks.concat();
        let original_bytes = combined.len();
        let original_lines = count_lines(&combined);
        let truncated = truncate_utf8_by_bytes_and_lines(&combined, &self.limits);
        let truncated_bytes = truncated.len();
        let truncated_lines = count_lines(&truncated);
        let content = vec![Content::Text { text: truncated }];
        if original_bytes > truncated_bytes || original_lines > truncated_lines {
            return ToolHostOutcome::Truncated {
                content,
                metadata: ToolTruncationMetadata {
                    original_bytes,
                    original_lines,
                    truncated_bytes,
                    truncated_lines,
                },
            };
        }
        ToolHostOutcome::Succeeded {
            content,
            details: serde_json::json!({ "truncated": false }),
        }
    }
}

#[must_use]
pub fn tool_call_id(call: &EngineToolCall) -> &EngineCorrelationId {
    &call.call_id
}

#[must_use]
fn count_lines(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    text.split_inclusive('\n').count() + usize::from(!text.ends_with('\n'))
}

#[must_use]
fn truncate_utf8_by_bytes_and_lines(text: &str, limits: &ToolTruncationLimits) -> String {
    let mut kept = String::new();
    let mut line_count = 0usize;
    for piece in text.split_inclusive('\n') {
        if line_count >= limits.max_lines {
            break;
        }
        let remaining = limits.max_bytes.saturating_sub(kept.len());
        if remaining == 0 {
            break;
        }
        let prefix = utf8_prefix(piece, remaining);
        kept.push_str(prefix);
        line_count += usize::from(prefix.ends_with('\n'));
        if prefix.len() < piece.len() {
            break;
        }
        if !piece.ends_with('\n') {
            line_count += 1;
        }
    }
    kept
}

#[must_use]
fn utf8_prefix(text: &str, max_bytes: usize) -> &str {
    if text.len() <= max_bytes {
        return text;
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    const SMALL_BYTES: usize = 6;
    const TWO_LINES: usize = 2;

    struct FakeCatalog {
        tools: Vec<ToolDescriptor>,
    }

    impl ToolCatalog for FakeCatalog {
        fn describe_tools(&self) -> Vec<ToolDescriptor> {
            self.tools.clone()
        }

        fn contains_tool(&self, name: &str) -> bool {
            self.tools.iter().any(|tool| tool.name == name)
        }
    }

    struct FakeCapabilityChecker {
        decision: CapabilityDecision,
    }

    impl CapabilityChecker for FakeCapabilityChecker {
        fn check_capability(&mut self, _call: &EngineToolCall) -> CapabilityDecision {
            self.decision.clone()
        }
    }

    #[derive(Default)]
    struct RecordingHook {
        events: Vec<&'static str>,
    }

    impl ToolHook for RecordingHook {
        fn before_tool(&mut self, _call: &EngineToolCall) -> Result<(), ToolHostError> {
            self.events.push("before");
            Ok(())
        }

        fn after_tool(&mut self, _call: &EngineToolCall, _outcome: &ToolHostOutcome) -> Result<(), ToolHostError> {
            self.events.push("after");
            Ok(())
        }
    }

    fn engine_tool_call(name: &str) -> EngineToolCall {
        EngineToolCall {
            call_id: EngineCorrelationId("call-1".to_string()),
            tool_name: name.to_string(),
            input: serde_json::json!({}),
        }
    }

    fn first_text(content: &[Content]) -> &str {
        let Some(Content::Text { text }) = content.first() else {
            panic!("expected first text content block");
        };
        text
    }

    #[test]
    fn accumulator_keeps_short_output() {
        let mut accumulator = ToolOutputAccumulator::new(ToolTruncationLimits {
            max_bytes: DEFAULT_TOOL_MAX_BYTES,
            max_lines: DEFAULT_TOOL_MAX_LINES,
        });
        accumulator.push("hello");
        let outcome = accumulator.finish();
        assert!(matches!(outcome, ToolHostOutcome::Succeeded { .. }));
    }

    #[test]
    fn accumulator_truncates_by_utf8_boundary() {
        let mut accumulator = ToolOutputAccumulator::new(ToolTruncationLimits {
            max_bytes: SMALL_BYTES,
            max_lines: DEFAULT_TOOL_MAX_LINES,
        });
        accumulator.push("éééé");
        let outcome = accumulator.finish();
        let ToolHostOutcome::Truncated { content, metadata } = outcome else {
            panic!("expected truncated output");
        };
        assert_eq!(first_text(&content), "ééé");
        assert_eq!(metadata.original_bytes, "éééé".len());
        assert_eq!(metadata.truncated_bytes, SMALL_BYTES);
    }

    #[test]
    fn accumulator_truncates_by_line_boundary() {
        let mut accumulator = ToolOutputAccumulator::new(ToolTruncationLimits {
            max_bytes: DEFAULT_TOOL_MAX_BYTES,
            max_lines: TWO_LINES,
        });
        accumulator.push("one\ntwo\nthree\n");
        let outcome = accumulator.finish();
        let ToolHostOutcome::Truncated { content, metadata } = outcome else {
            panic!("expected line truncation");
        };
        assert_eq!(first_text(&content), "one\ntwo\n");
        assert_eq!(metadata.original_lines, 3);
        assert_eq!(metadata.truncated_lines, TWO_LINES);
    }

    #[test]
    #[should_panic(expected = "tool truncation max_bytes must be positive")]
    fn accumulator_rejects_zero_byte_limit() {
        let _ = ToolOutputAccumulator::new(ToolTruncationLimits {
            max_bytes: 0,
            max_lines: DEFAULT_TOOL_MAX_LINES,
        });
    }

    #[test]
    fn catalog_lists_metadata_and_checks_lookup() {
        let catalog = FakeCatalog {
            tools: vec![ToolDescriptor {
                name: "read".to_string(),
                description: "read file".to_string(),
            }],
        };

        let tools = catalog.describe_tools();

        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "read");
        assert!(catalog.contains_tool("read"));
        assert!(!catalog.contains_tool("write"));
    }

    #[test]
    fn capability_checker_allows_and_denies() {
        let call = engine_tool_call("read");
        let mut allowed = FakeCapabilityChecker {
            decision: CapabilityDecision::Allowed,
        };
        let mut denied = FakeCapabilityChecker {
            decision: CapabilityDecision::Denied {
                reason: "blocked".to_string(),
            },
        };

        assert_eq!(allowed.check_capability(&call), CapabilityDecision::Allowed);
        assert_eq!(denied.check_capability(&call), CapabilityDecision::Denied {
            reason: "blocked".to_string()
        });
    }

    #[test]
    fn hook_ordering_is_explicit() {
        let call = engine_tool_call("read");
        let outcome = ToolHostOutcome::Succeeded {
            content: vec![Content::Text { text: "ok".to_string() }],
            details: serde_json::json!({}),
        };
        let mut hook = RecordingHook::default();

        hook.before_tool(&call).expect("before hook should pass");
        hook.after_tool(&call, &outcome).expect("after hook should pass");

        assert_eq!(hook.events, vec!["before", "after"]);
    }

    #[test]
    fn outcome_variants_are_explicit() {
        let outcomes = vec![
            ToolHostOutcome::Succeeded {
                content: Vec::new(),
                details: serde_json::json!({}),
            },
            ToolHostOutcome::ToolError {
                content: Vec::new(),
                details: serde_json::json!({}),
                message: "bad".to_string(),
            },
            ToolHostOutcome::MissingTool {
                name: "missing".to_string(),
            },
            ToolHostOutcome::CapabilityDenied {
                name: "read".to_string(),
                reason: "blocked".to_string(),
            },
            ToolHostOutcome::Cancelled {
                name: "read".to_string(),
            },
            ToolHostOutcome::Truncated {
                content: Vec::new(),
                metadata: ToolTruncationMetadata {
                    original_bytes: 2,
                    original_lines: 1,
                    truncated_bytes: 1,
                    truncated_lines: 1,
                },
            },
        ];

        assert_eq!(outcomes.len(), 6);
    }

    #[test]
    #[should_panic(expected = "tool truncation max_lines must be positive")]
    fn accumulator_rejects_zero_line_limit() {
        let _ = ToolOutputAccumulator::new(ToolTruncationLimits {
            max_bytes: DEFAULT_TOOL_MAX_BYTES,
            max_lines: 0,
        });
    }

    #[test]
    fn capability_decision_can_deny() {
        let denied = CapabilityDecision::Denied {
            reason: "blocked".to_string(),
        };
        assert_eq!(denied, CapabilityDecision::Denied {
            reason: "blocked".to_string()
        });
    }
}
