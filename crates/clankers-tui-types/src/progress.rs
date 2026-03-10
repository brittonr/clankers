//! Tool progress display types.

use std::time::Instant;

/// Structured progress information emitted by tools during execution.
#[derive(Debug, Clone)]
pub struct ToolProgressData {
    /// The kind of progress (bytes, lines, items, percentage, phase).
    pub kind: ProgressKind,
    /// Optional human-readable message (e.g., "Searching /usr/lib...").
    pub message: Option<String>,
    /// Timestamp when this progress was emitted.
    pub timestamp: Instant,
}

/// Different types of progress a tool can report.
#[derive(Debug, Clone, PartialEq)]
pub enum ProgressKind {
    /// Bytes processed (e.g., downloaded, uploaded, read from disk).
    Bytes { current: u64, total: Option<u64> },

    /// Lines processed (e.g., grep matches, file lines scanned).
    Lines { current: u64, total: Option<u64> },

    /// Generic countable items (e.g., files processed, tests run).
    Items { current: u64, total: Option<u64> },

    /// Percentage complete (0.0 to 100.0).
    /// Use when the tool can calculate percentage but not absolute progress.
    Percentage { percent: f32 },

    /// Phase-based progress (e.g., "Fetching", "Parsing", "Cancelling").
    /// Use for multi-stage operations where each phase is distinct.
    Phase {
        name: String,
        step: u32,
        total_steps: Option<u32>,
    },
}

impl ProgressKind {
    /// Calculate percentage if total is known.
    pub fn as_percentage(&self) -> Option<f32> {
        match self {
            ProgressKind::Bytes {
                current,
                total: Some(total),
            } if *total > 0 => Some((*current as f32 / *total as f32) * 100.0),
            ProgressKind::Lines {
                current,
                total: Some(total),
            } if *total > 0 => Some((*current as f32 / *total as f32) * 100.0),
            ProgressKind::Items {
                current,
                total: Some(total),
            } if *total > 0 => Some((*current as f32 / *total as f32) * 100.0),
            ProgressKind::Percentage { percent } => Some(*percent),
            ProgressKind::Phase {
                step,
                total_steps: Some(total),
                ..
            } if *total > 0 => Some((*step as f32 / *total as f32) * 100.0),
            _ => None,
        }
    }

    /// Check if progress is complete (100%).
    pub fn is_complete(&self) -> bool {
        match self {
            ProgressKind::Bytes {
                current,
                total: Some(total),
            } => current >= total,
            ProgressKind::Lines {
                current,
                total: Some(total),
            } => current >= total,
            ProgressKind::Items {
                current,
                total: Some(total),
            } => current >= total,
            ProgressKind::Percentage { percent } => *percent >= 100.0,
            ProgressKind::Phase {
                step,
                total_steps: Some(total),
                ..
            } => step >= total,
            _ => false,
        }
    }

    /// Human-readable string for display.
    pub fn display_string(&self) -> String {
        match self {
            ProgressKind::Bytes {
                current,
                total: Some(total),
            } => format!("{}/{} bytes", current, total),
            ProgressKind::Bytes { current, total: None } => format!("{} bytes", current),
            ProgressKind::Lines {
                current,
                total: Some(total),
            } => format!("{}/{} lines", current, total),
            ProgressKind::Lines { current, total: None } => format!("{} lines", current),
            ProgressKind::Items {
                current,
                total: Some(total),
            } => format!("{}/{} items", current, total),
            ProgressKind::Items { current, total: None } => format!("{} items", current),
            ProgressKind::Percentage { percent } => format!("{:.1}%", percent),
            ProgressKind::Phase {
                name,
                step,
                total_steps: Some(total),
            } => format!("Phase {}/{}: {}", step, total, name),
            ProgressKind::Phase {
                name,
                step,
                total_steps: None,
            } => format!("Phase {}: {}", step, name),
        }
    }
}

/// Structured progress information emitted by tools during execution.
#[derive(Debug, Clone)]
pub struct ToolProgress {
    /// The kind of progress (bytes, lines, items, percentage, phase).
    pub kind: ProgressKind,
    /// Optional human-readable message (e.g., "Searching /usr/lib...").
    pub message: Option<String>,
    /// Timestamp when this progress was emitted.
    pub timestamp: Instant,
}

impl ToolProgress {
    /// Create progress from bytes processed.
    pub fn bytes(current: u64, total: Option<u64>) -> Self {
        Self {
            kind: ProgressKind::Bytes { current, total },
            message: None,
            timestamp: Instant::now(),
        }
    }

    /// Create progress from lines processed.
    pub fn lines(current: u64, total: Option<u64>) -> Self {
        Self {
            kind: ProgressKind::Lines { current, total },
            message: None,
            timestamp: Instant::now(),
        }
    }

    /// Create progress from items processed (generic countable units).
    pub fn items(current: u64, total: Option<u64>) -> Self {
        Self {
            kind: ProgressKind::Items { current, total },
            message: None,
            timestamp: Instant::now(),
        }
    }

    /// Create progress from percentage (0.0 to 100.0).
    pub fn percentage(percent: f32) -> Self {
        Self {
            kind: ProgressKind::Percentage { percent },
            message: None,
            timestamp: Instant::now(),
        }
    }

    /// Create phase progress (e.g., "Fetching", "Parsing", "Cancelling").
    pub fn phase(name: impl Into<String>, step: u32, total_steps: Option<u32>) -> Self {
        Self {
            kind: ProgressKind::Phase {
                name: name.into(),
                step,
                total_steps,
            },
            message: None,
            timestamp: Instant::now(),
        }
    }

    /// Add a message to this progress.
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
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
        assert_eq!(
            ProgressKind::Bytes {
                current: 50,
                total: None
            }
            .as_percentage(),
            None
        );
        assert_eq!(
            ProgressKind::Lines {
                current: 75,
                total: None
            }
            .as_percentage(),
            None
        );
    }

    #[test]
    fn progress_kind_as_percentage_with_zero_total() {
        assert_eq!(
            ProgressKind::Bytes {
                current: 0,
                total: Some(0)
            }
            .as_percentage(),
            None
        );
    }

    #[test]
    fn progress_kind_percentage_variant() {
        assert_eq!(
            ProgressKind::Percentage { percent: 42.5 }.as_percentage(),
            Some(42.5)
        );
    }

    #[test]
    fn progress_kind_phase_with_total_steps() {
        let phase = ProgressKind::Phase {
            name: "Building".to_string(),
            step: 2,
            total_steps: Some(3),
        };
        assert!(
            (phase
                .as_percentage()
                .expect("phase with total should have percentage")
                - 66.666)
                .abs()
                < 0.01
        );
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
        assert_eq!(
            ProgressKind::Percentage { percent: 42.5 }.display_string(),
            "42.5%"
        );
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

        assert!(matches!(
            progress.kind,
            ProgressKind::Bytes {
                current: 50,
                total: Some(100)
            }
        ));
        assert_eq!(progress.message, Some("Downloading".to_string()));

        let phase = ToolProgress::phase("Building", 1, Some(3));
        assert!(matches!(phase.kind, ProgressKind::Phase { .. }));
    }
}
