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
            ProgressKind::Bytes {
                current,
                total: None,
            } => format!("{} bytes", current),
            ProgressKind::Lines {
                current,
                total: Some(total),
            } => format!("{}/{} lines", current, total),
            ProgressKind::Lines {
                current,
                total: None,
            } => format!("{} lines", current),
            ProgressKind::Items {
                current,
                total: Some(total),
            } => format!("{}/{} items", current, total),
            ProgressKind::Items {
                current,
                total: None,
            } => format!("{} items", current),
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
        Self { kind: ProgressKind::Bytes { current, total }, message: None, timestamp: Instant::now() }
    }

    /// Create progress from lines processed.
    pub fn lines(current: u64, total: Option<u64>) -> Self {
        Self { kind: ProgressKind::Lines { current, total }, message: None, timestamp: Instant::now() }
    }

    /// Create progress from items processed (generic countable units).
    pub fn items(current: u64, total: Option<u64>) -> Self {
        Self { kind: ProgressKind::Items { current, total }, message: None, timestamp: Instant::now() }
    }

    /// Create progress from percentage (0.0 to 100.0).
    pub fn percentage(percent: f32) -> Self {
        Self { kind: ProgressKind::Percentage { percent }, message: None, timestamp: Instant::now() }
    }

    /// Create phase progress (e.g., "Fetching", "Parsing", "Cancelling").
    pub fn phase(name: impl Into<String>, step: u32, total_steps: Option<u32>) -> Self {
        Self {
            kind: ProgressKind::Phase { name: name.into(), step, total_steps },
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
