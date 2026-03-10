//! Display types for the chat view.

use std::time::Instant;

/// Application state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    Idle,
    Streaming,
    Command,
    Dialog,
}

/// Connection status to the clankers-router daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouterStatus {
    /// Connected to the router daemon via RPC.
    Connected,
    /// Using in-process provider (no daemon).
    Local,
    /// Disconnected / unreachable.
    Disconnected,
}

/// A message for display in the chat view.
#[derive(Debug, Clone)]
pub struct DisplayMessage {
    pub role: MessageRole,
    pub content: String,
    pub tool_name: Option<String>,
    pub is_error: bool,
    /// Optional inline images (base64 data + media type) for terminal rendering.
    pub images: Vec<DisplayImage>,
}

/// An image attached to a display message for inline terminal rendering.
#[derive(Debug, Clone)]
pub struct DisplayImage {
    /// Base64-encoded image data.
    pub data: String,
    /// MIME type (e.g. "image/png").
    pub media_type: String,
}

/// Role of a message in the chat view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
    ToolCall,
    ToolResult,
    Thinking,
    System,
}

/// An image attached to the editor, waiting to be sent with the next prompt.
#[derive(Debug, Clone)]
pub struct PendingImage {
    /// Base64-encoded image data.
    pub data: String,
    /// MIME type (e.g. "image/png").
    pub media_type: String,
    /// Approximate size in bytes (of the raw image data).
    pub size: usize,
}

/// State for a currently-executing tool (used for live output rendering).
#[derive(Debug, Clone)]
pub struct ActiveToolExecution {
    /// Name of the tool (e.g. "bash").
    pub tool_name: String,
    /// When execution started.
    pub started_at: Instant,
    /// Number of output lines received so far.
    pub line_count: usize,
}

/// Modal editing mode.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Default,
    serde::Serialize,
    serde::Deserialize
)]
#[serde(rename_all = "lowercase")]
pub enum InputMode {
    /// Navigation mode — bare keys trigger actions, no text insertion.
    #[default]
    Normal,
    /// Typing mode — bare keys insert characters, modified keys trigger actions.
    Insert,
}

impl std::fmt::Display for InputMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Normal => write!(f, "NORMAL"),
            Self::Insert => write!(f, "INSERT"),
        }
    }
}

/// Named thinking budget levels.
///
/// Provides quick presets for thinking token budgets that can be cycled
/// through with a keybinding or set via `/think <level>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThinkingLevel {
    /// Thinking disabled.
    Off,
    /// Quick reasoning (~5k tokens).
    Low,
    /// Moderate reasoning (~10k tokens, default).
    Medium,
    /// Deep reasoning (~32k tokens).
    High,
    /// Maximum reasoning (~128k tokens).
    Max,
}

impl ThinkingLevel {
    /// Token budget for this level (None for Off).
    pub fn budget_tokens(self) -> Option<usize> {
        match self {
            Self::Off => None,
            Self::Low => Some(5_000),
            Self::Medium => Some(10_000),
            Self::High => Some(32_000),
            Self::Max => Some(128_000),
        }
    }

    /// Whether thinking is enabled at this level.
    pub fn is_enabled(self) -> bool {
        self != Self::Off
    }

    /// Cycle to the next level.
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::Low,
            Self::Low => Self::Medium,
            Self::Medium => Self::High,
            Self::High => Self::Max,
            Self::Max => Self::Off,
        }
    }

    /// Display name.
    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Max => "max",
        }
    }

    /// Parse from a string (name or raw number).
    pub fn from_str_or_budget(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "off" | "none" | "disable" | "disabled" => Some(Self::Off),
            "low" | "lo" | "l" => Some(Self::Low),
            "medium" | "med" | "m" | "default" => Some(Self::Medium),
            "high" | "hi" | "h" => Some(Self::High),
            "max" | "maximum" | "full" => Some(Self::Max),
            _ => None,
        }
    }

    /// Find the closest level for a raw token budget.
    pub fn from_budget(tokens: usize) -> Self {
        if tokens == 0 {
            Self::Off
        } else if tokens <= 5_000 {
            Self::Low
        } else if tokens <= 10_000 {
            Self::Medium
        } else if tokens <= 32_000 {
            Self::High
        } else {
            Self::Max
        }
    }

    /// All levels in order.
    pub fn all() -> &'static [Self] {
        &[Self::Off, Self::Low, Self::Medium, Self::High, Self::Max]
    }
}

/// Plan mode state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanState {
    /// Plan mode is inactive — normal operation.
    Inactive,
    /// Planning phase — agent reads/analyzes but doesn't edit.
    Planning,
    /// Plan has been proposed — waiting for user approval.
    AwaitingApproval,
    /// Plan approved — executing edits.
    Executing,
}

impl PlanState {
    pub fn is_active(&self) -> bool {
        !matches!(self, PlanState::Inactive)
    }

    pub fn label(&self) -> &'static str {
        match self {
            PlanState::Inactive => "off",
            PlanState::Planning => "planning",
            PlanState::AwaitingApproval => "awaiting approval",
            PlanState::Executing => "executing",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            PlanState::Inactive => "",
            PlanState::Planning => "📐",
            PlanState::AwaitingApproval => "⏳",
            PlanState::Executing => "🔨",
        }
    }
}

/// Loop mode display state — what the status bar and TUI need to render.
#[derive(Debug, Clone)]
pub struct LoopDisplayState {
    /// Current iteration (1-based for display).
    pub iteration: u32,
    /// Maximum iterations (from loop definition).
    pub max_iterations: u32,
    /// Loop name.
    pub name: String,
    /// Whether the loop is actively running (vs paused).
    pub active: bool,
    /// Break condition text (for "until" mode). None for fixed-count loops.
    pub break_text: Option<String>,
    /// The prompt to re-send each iteration. Stored here so pause/resume
    /// can access it from slash command handlers without reaching into
    /// the event loop runner.
    pub prompt: Option<String>,
}

impl LoopDisplayState {
    /// Status bar label, e.g. "⟳ loop 4/10" or "⏸ loop 4/10".
    pub fn label(&self) -> String {
        if self.active {
            format!("loop {}/{}", self.iteration, self.max_iterations)
        } else {
            format!("loop {}/{} paused", self.iteration, self.max_iterations)
        }
    }
}

/// Result of a clipboard read operation.
#[derive(Debug, Clone)]
pub enum ClipboardResult {
    /// Text was found in the clipboard.
    Text(String),
    /// An image was found: base64 PNG, mime type, raw size, width, height.
    Image {
        encoded: String,
        mime: String,
        raw_size: usize,
        width: u32,
        height: u32,
    },
    /// Nothing useful in clipboard.
    Empty(String),
    /// Error accessing the clipboard.
    Error(String),
}
