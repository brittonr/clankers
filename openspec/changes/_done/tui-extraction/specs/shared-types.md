# Shared Types — `clankers-tui-types` Crate

## Overview

The types crate contains every type that crosses the TUI↔application boundary.
These types have zero rendering logic and no dependency on ratatui or crossterm.
They're plain data — enums, structs, and one trait (`MenuContributor`).

## Crate Layout

```
crates/clankers-tui-types/
├── Cargo.toml
└── src/
    ├── lib.rs          # Module declarations + prelude re-exports
    ├── actions.rs      # Action, CoreAction, ExtendedAction
    ├── block.rs        # BlockEntry, ConversationBlock
    ├── completion.rs   # CompletionItem, SlashCommandInfo, CompletionKind
    ├── cost.rs         # CostSummary, BudgetStatus
    ├── display.rs      # DisplayMessage, DisplayImage, MessageRole, AppState,
    │                   # RouterStatus, PendingImage, ActiveToolExecution,
    │                   # InputMode, ThinkingLevel
    ├── menu.rs         # MenuPlacement, MenuContribution, MenuContributor,
    │                   # LeaderAction, LeaderBinding
    ├── panel.rs        # PanelId, PanelAction, HitRegion, TodoStatus
    ├── progress.rs     # ToolProgressData (TUI-owned mirror)
    ├── registry.rs     # Conflict, PRIORITY_BUILTIN/PLUGIN/USER
    └── subagent.rs     # SubagentEvent
```

## Cargo.toml

```toml
[package]
name = "clankers-tui-types"
version = "0.1.0"
edition = "2024"

[dependencies]
serde = { version = "1", features = ["derive"] }
chrono = { version = "0.4", features = ["serde"] }
```

No ratatui, no crossterm, no tokio, no async runtime.

## Key Types

### SubagentEvent (from `src/tui/components/subagent_event.rs`)

Moves verbatim. Used by 8 files in tools/modes, 7 files in TUI.

```rust
#[derive(Debug, Clone)]
pub enum SubagentEvent {
    Started { id: String, name: String, task: String, pid: Option<u32> },
    Output { id: String, line: String },
    Done { id: String },
    Error { id: String, message: String },
    KillRequest { id: String },
    InputRequest { id: String, text: String },
}
```

### Display types (from `src/tui/app/mod.rs`)

```rust
#[derive(Debug, Clone)]
pub struct DisplayMessage {
    pub role: MessageRole,
    pub content: String,
    pub tool_name: Option<String>,
    pub is_error: bool,
    pub images: Vec<DisplayImage>,
}

#[derive(Debug, Clone)]
pub struct DisplayImage {
    pub data: String,
    pub media_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageRole {
    User, Assistant, ToolCall, ToolResult, Thinking, System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    Idle, Streaming, Command, Dialog,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouterStatus {
    Connected, Local, Disconnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal, Insert, Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThinkingLevel {
    Off, Brief, Full,
}

#[derive(Debug, Clone)]
pub struct PendingImage {
    pub data: String,
    pub media_type: String,
    pub size: usize,
}

#[derive(Debug, Clone)]
pub struct ActiveToolExecution {
    pub tool_name: String,
    pub started_at: std::time::Instant,
    pub line_count: usize,
}
```

### Block types (from `src/tui/components/block.rs`)

```rust
#[derive(Debug, Clone)]
pub enum BlockEntry {
    System(DisplayMessage),
    Conversation(ConversationBlock),
}

#[derive(Debug, Clone)]
pub struct ConversationBlock {
    pub id: usize,
    pub parent_block_id: Option<usize>,
    pub prompt: String,
    pub responses: Vec<DisplayMessage>,
    pub is_collapsed: bool,
    pub timestamp: Option<chrono::DateTime<chrono::Utc>>,
}
```

### Panel types (from `src/tui/panel.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PanelId {
    Todo, Files, Subagents, Peers, Processes, Branches,
}

#[derive(Debug, Clone)]
pub enum PanelAction {
    Consumed,
    Unfocus,
    SlashCommand(String),
    FocusPanel(PanelId),
    FocusSubagent(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HitRegion {
    Chat,
    Editor,
    StatusBar,
    Header,
    Panel(PanelId),
    Subagent(String),
    PanelBorder(PanelId),
    Empty,
}
```

### Cost types (new, TUI-owned)

```rust
#[derive(Debug, Clone, Default)]
pub struct CostSummary {
    pub total_cost: f64,
    pub total_tokens: usize,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub model_breakdown: Vec<(String, f64)>,
}

#[derive(Debug, Clone)]
pub enum BudgetStatus {
    NoBudget,
    Ok { remaining: f64, total: f64 },
    Warning { remaining: f64, total: f64 },
    Exceeded { overage: f64, total: f64 },
}
```

### Action types (from `src/config/keybindings/actions.rs`)

`Action`, `CoreAction`, `ExtendedAction` move verbatim including the name
mapping table (`EXTENDED_ACTION_NAMES`). These are TUI-semantic operations.

### Menu types (from `src/tui/components/leader_menu/`)

```rust
pub trait MenuContributor {
    fn menu_items(&self) -> Vec<MenuContribution>;
}

#[derive(Debug, Clone)]
pub struct MenuContribution {
    pub key: char,
    pub label: String,
    pub action: LeaderAction,
    pub priority: u8,
    pub placement: MenuPlacement,
}

#[derive(Debug, Clone)]
pub enum MenuPlacement {
    TopLevel,
    Submenu(char),
}

#[derive(Debug, Clone)]
pub enum LeaderAction {
    SlashCommand(String),
    Submenu { key: char, label: String },
    KeymapAction(Action),
}
```

### Completion types (new)

```rust
#[derive(Debug, Clone)]
pub struct CompletionItem {
    pub name: String,
    pub description: String,
    pub kind: CompletionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    SlashCommand,
    Subcommand,
    Argument,
}

#[derive(Debug, Clone)]
pub struct SlashCommandInfo {
    pub name: String,
    pub description: String,
    pub leader_key: Option<LeaderBinding>,
}

#[derive(Debug, Clone)]
pub struct LeaderBinding {
    pub key: char,
    pub placement: MenuPlacement,
}
```

## Migration Path

### Phase 1: Types crate created, re-exports added

Original locations re-export from the types crate:

```rust
// src/tui/components/subagent_event.rs
pub use clankers_tui_types::SubagentEvent;

// src/tui/app/mod.rs
pub use clankers_tui_types::{DisplayMessage, DisplayImage, MessageRole, ...};
```

External code keeps working with `use crate::tui::app::DisplayMessage`.

### Phase 2: External code updated to use types crate directly

```rust
// Before (in src/tools/delegate/mod.rs)
use crate::tui::components::subagent_event::SubagentEvent;

// After
use clankers_tui_types::SubagentEvent;
```

### Phase 5: Re-exports removed when TUI module moves to crate

The `src/tui/` directory is deleted. All imports go through
`clankers_tui` or `clankers_tui_types`.
