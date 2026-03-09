//! Structured progress and result streaming for tools

use std::time::Instant;

use serde_json::Value;

use super::ToolResult;

// ProgressKind and ToolProgress re-exported from clankers-tui-types (canonical definitions).
pub use clankers_tui_types::ProgressKind;
pub use clankers_tui_types::ToolProgress;


/// Result chunk that tools emit as they produce output
#[derive(Debug, Clone)]
pub struct ResultChunk {
    /// The content of this chunk (text, base64, etc.)
    pub content: String,
    /// The type of content ("text", "base64", "json")
    pub content_type: String,
    /// Sequence number (for ordering, starts at 0)
    pub sequence: u64,
    /// Timestamp when this chunk was emitted
    pub timestamp: Instant,
}

impl ResultChunk {
    /// Create a text chunk
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            content_type: "text".to_string(),
            sequence: 0, // Caller should set this
            timestamp: Instant::now(),
        }
    }

    /// Create a base64-encoded chunk (e.g., for binary data)
    pub fn base64(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            content_type: "base64".to_string(),
            sequence: 0,
            timestamp: Instant::now(),
        }
    }

    /// Create a JSON chunk
    pub fn json(value: &Value) -> Self {
        Self {
            content: value.to_string(),
            content_type: "json".to_string(),
            sequence: 0,
            timestamp: Instant::now(),
        }
    }

    /// Set sequence number
    pub fn with_sequence(mut self, sequence: u64) -> Self {
        self.sequence = sequence;
        self
    }
}

/// Configuration for result truncation
#[derive(Debug, Clone)]
pub struct TruncationConfig {
    /// Maximum lines before truncation (default: 1000)
    pub max_lines: usize,
    /// Head window size (default: 500)
    pub head_lines: usize,
    /// Tail window size (default: 500)
    pub tail_lines: usize,
    /// Maximum bytes (default: 1MB)
    pub max_bytes: usize,
}

impl Default for TruncationConfig {
    fn default() -> Self {
        Self {
            max_lines: 1000,
            head_lines: 500,
            tail_lines: 500,
            max_bytes: 1024 * 1024, // 1MB
        }
    }
}

/// Helper struct to accumulate result chunks and apply truncation
pub struct ToolResultAccumulator {
    /// Accumulated chunks
    chunks: Vec<ResultChunk>,
    /// Sequence counter for chunks
    next_sequence: u64,
    /// Total bytes accumulated
    total_bytes: usize,
    /// Total lines accumulated (if content is text)
    total_lines: usize,
    /// Truncation configuration
    config: TruncationConfig,
}

impl ToolResultAccumulator {
    /// Create a new accumulator with default configuration
    pub fn new() -> Self {
        Self::with_config(TruncationConfig::default())
    }

    /// Create a new accumulator with custom configuration
    pub fn with_config(config: TruncationConfig) -> Self {
        Self {
            chunks: Vec::new(),
            next_sequence: 0,
            total_bytes: 0,
            total_lines: 0,
            config,
        }
    }

    /// Add a chunk (automatically assigns sequence number)
    pub fn push(&mut self, mut chunk: ResultChunk) {
        chunk.sequence = self.next_sequence;
        self.next_sequence += 1;

        self.total_bytes += chunk.content.len();
        if chunk.content_type == "text" {
            self.total_lines += chunk.content.lines().count();
        }

        self.chunks.push(chunk);
    }

    /// Create a text chunk and add it
    pub fn push_text(&mut self, text: impl Into<String>) {
        self.push(ResultChunk::text(text));
    }

    /// Get total lines accumulated
    pub fn total_lines(&self) -> usize {
        self.total_lines
    }

    /// Get total bytes accumulated
    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    /// Finalize: merge chunks, apply truncation, return ToolResult
    pub fn finalize(self) -> ToolResult {
        if self.chunks.is_empty() {
            return ToolResult::text("");
        }

        // Merge all text chunks
        let mut lines: Vec<String> = Vec::new();
        for chunk in &self.chunks {
            if chunk.content_type == "text" {
                for line in chunk.content.lines() {
                    lines.push(line.to_string());
                }
            }
        }

        // Apply truncation if needed
        let result_text = if lines.len() > self.config.max_lines {
            let head: Vec<_> = lines.iter().take(self.config.head_lines).map(|s| s.as_str()).collect();
            let tail: Vec<_> = lines.iter().skip(lines.len() - self.config.tail_lines).map(|s| s.as_str()).collect();

            let omitted = lines.len() - self.config.head_lines - self.config.tail_lines;
            let marker = format!("\n... [{} lines omitted] ...\n", omitted);

            let mut result = head.join("\n");
            result.push_str(&marker);
            result.push_str(&tail.join("\n"));
            result
        } else {
            lines.join("\n")
        };

        // Check if truncated
        let is_truncated = lines.len() > self.config.max_lines || self.total_bytes > self.config.max_bytes;

        ToolResult::text(result_text).with_details(serde_json::json!({
            "total_lines": self.total_lines,
            "total_bytes": self.total_bytes,
            "truncated": is_truncated,
            "chunks": self.chunks.len(),
        }))
    }
}

impl Default for ToolResultAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_kind_as_percentage_with_known_total() {
        let bytes = ProgressKind::Bytes {
            current: 50,
            total: Some(100),
        };
        assert_eq!(bytes.as_percentage(), Some(50.0));

        let lines = ProgressKind::Lines {
            current: 75,
            total: Some(100),
        };
        assert_eq!(lines.as_percentage(), Some(75.0));

        let items = ProgressKind::Items {
            current: 25,
            total: Some(100),
        };
        assert_eq!(items.as_percentage(), Some(25.0));
    }

    #[test]
    fn progress_kind_as_percentage_with_unknown_total() {
        let bytes = ProgressKind::Bytes {
            current: 50,
            total: None,
        };
        assert_eq!(bytes.as_percentage(), None);

        let lines = ProgressKind::Lines {
            current: 75,
            total: None,
        };
        assert_eq!(lines.as_percentage(), None);
    }

    #[test]
    fn progress_kind_as_percentage_with_zero_total() {
        let bytes = ProgressKind::Bytes {
            current: 0,
            total: Some(0),
        };
        assert_eq!(bytes.as_percentage(), None);
    }

    #[test]
    fn progress_kind_percentage_variant() {
        let pct = ProgressKind::Percentage { percent: 42.5 };
        assert_eq!(pct.as_percentage(), Some(42.5));
    }

    #[test]
    fn progress_kind_phase_with_total_steps() {
        let phase = ProgressKind::Phase {
            name: "Building".to_string(),
            step: 2,
            total_steps: Some(3),
        };
        assert!((phase.as_percentage().expect("phase with total should have percentage") - 66.666).abs() < 0.01);
    }

    #[test]
    fn progress_kind_is_complete() {
        assert!(
            ProgressKind::Bytes {
                current: 100,
                total: Some(100)
            }
            .is_complete()
        );
        assert!(
            ProgressKind::Lines {
                current: 50,
                total: Some(50)
            }
            .is_complete()
        );
        assert!(ProgressKind::Percentage { percent: 100.0 }.is_complete());
        assert!(ProgressKind::Percentage { percent: 101.0 }.is_complete());

        assert!(
            !ProgressKind::Bytes {
                current: 50,
                total: Some(100)
            }
            .is_complete()
        );
        assert!(
            !ProgressKind::Bytes {
                current: 50,
                total: None
            }
            .is_complete()
        );
    }

    #[test]
    fn progress_kind_display_string() {
        assert_eq!(
            ProgressKind::Bytes {
                current: 50,
                total: Some(100)
            }
            .display_string(),
            "50/100 bytes"
        );
        assert_eq!(
            ProgressKind::Bytes {
                current: 50,
                total: None
            }
            .display_string(),
            "50 bytes"
        );
        assert_eq!(ProgressKind::Percentage { percent: 42.5 }.display_string(), "42.5%");
        assert_eq!(
            ProgressKind::Phase {
                name: "Building".to_string(),
                step: 2,
                total_steps: Some(3)
            }
            .display_string(),
            "Phase 2/3: Building"
        );
    }

    #[test]
    fn tool_progress_builders() {
        let progress = ToolProgress::bytes(50, Some(100)).with_message("Downloading");

        assert!(matches!(progress.kind, ProgressKind::Bytes {
            current: 50,
            total: Some(100)
        }));
        assert_eq!(progress.message, Some("Downloading".to_string()));

        let phase = ToolProgress::phase("Building", 1, Some(3));
        assert!(matches!(phase.kind, ProgressKind::Phase { .. }));
    }

    #[test]
    fn result_chunk_builders() {
        let text = ResultChunk::text("hello");
        assert_eq!(text.content, "hello");
        assert_eq!(text.content_type, "text");
        assert_eq!(text.sequence, 0);

        let with_seq = ResultChunk::text("world").with_sequence(42);
        assert_eq!(with_seq.sequence, 42);

        let json_val = serde_json::json!({"key": "value"});
        let json_chunk = ResultChunk::json(&json_val);
        assert_eq!(json_chunk.content_type, "json");
    }

    #[test]
    fn accumulator_no_truncation() {
        let mut acc = ToolResultAccumulator::new();

        acc.push_text("line 1");
        acc.push_text("line 2");
        acc.push_text("line 3");

        assert_eq!(acc.total_lines(), 3);

        let result = acc.finalize();
        let text = match &result.content[0] {
            crate::tools::ToolResultContent::Text { text } => text,
            _ => panic!("expected text"),
        };

        assert_eq!(text, "line 1\nline 2\nline 3");

        // Check details
        let details = result.details.expect("result should have details");
        assert_eq!(details["total_lines"], 3);
        assert_eq!(details["truncated"], false);
    }

    #[test]
    fn accumulator_with_truncation() {
        let config = TruncationConfig {
            max_lines: 10,
            head_lines: 3,
            tail_lines: 3,
            max_bytes: 1024 * 1024,
        };

        let mut acc = ToolResultAccumulator::with_config(config);

        // Add 20 lines
        for i in 0..20 {
            acc.push_text(format!("line {}", i));
        }

        assert_eq!(acc.total_lines(), 20);

        let result = acc.finalize();
        let text = match &result.content[0] {
            crate::tools::ToolResultContent::Text { text } => text,
            _ => panic!("expected text"),
        };

        // Should have first 3 lines + omission marker + last 3 lines
        assert!(text.contains("line 0"));
        assert!(text.contains("line 1"));
        assert!(text.contains("line 2"));
        assert!(text.contains("[14 lines omitted]"));
        assert!(text.contains("line 17"));
        assert!(text.contains("line 18"));
        assert!(text.contains("line 19"));

        // Check details
        let details = result.details.expect("result should have details");
        assert_eq!(details["total_lines"], 20);
        assert_eq!(details["truncated"], true);
    }

    #[test]
    fn accumulator_respects_max_bytes() {
        let config = TruncationConfig {
            max_lines: 1000,
            head_lines: 500,
            tail_lines: 500,
            max_bytes: 100, // Very small
        };

        let mut acc = ToolResultAccumulator::with_config(config);

        // Add large content
        acc.push_text("x".repeat(200));

        assert!(acc.total_bytes() > 100);

        let result = acc.finalize();
        let details = result.details.expect("result should have details");
        assert_eq!(details["truncated"], true);
    }

    #[test]
    fn accumulator_sequence_numbering() {
        let mut acc = ToolResultAccumulator::new();

        let chunk1 = ResultChunk::text("first");
        let chunk2 = ResultChunk::text("second");
        let chunk3 = ResultChunk::text("third");

        acc.push(chunk1);
        acc.push(chunk2);
        acc.push(chunk3);

        assert_eq!(acc.chunks[0].sequence, 0);
        assert_eq!(acc.chunks[1].sequence, 1);
        assert_eq!(acc.chunks[2].sequence, 2);
    }
}
