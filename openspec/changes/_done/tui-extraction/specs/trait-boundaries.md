# Trait Boundaries — TUI ↔ Application Interface

## Overview

The TUI crate defines traits for every piece of external data it needs. The
main crate implements these traits with concrete types. Traits are collected
under a single `AppBridge` supertrait that's stored as `Box<dyn AppBridge>`
on the `App` struct.

## Trait Definitions

All traits live in `crates/clankers-tui/src/traits.rs`.

### CostProvider

Replaces direct access to `CostTracker` (from `model_selection`) and
`BudgetStatus` (from `model_selection::cost_tracker`).

**Used by:** `cost_overlay.rs` (3 refs), `status_bar.rs` (1 ref)

```rust
pub trait CostProvider: Send + Sync {
    /// Get the current cost summary (totals, per-model breakdown)
    fn summary(&self) -> Option<CostSummary>;

    /// Get the current budget status
    fn budget_status(&self) -> Option<BudgetStatus>;

    /// Get per-model cost entries for the detail overlay
    fn model_entries(&self) -> Vec<CostModelEntry>;
}

/// A single model's cost entry for the detail overlay
#[derive(Debug, Clone)]
pub struct CostModelEntry {
    pub model: String,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub cost: f64,
}
```

**Main crate implementation:**

```rust
// src/bridge.rs
impl CostProvider for CostTrackerAdapter {
    fn summary(&self) -> Option<CostSummary> {
        let tracker = self.tracker.as_ref()?;
        let snap = tracker.snapshot();
        Some(CostSummary {
            total_cost: snap.total_cost,
            total_tokens: snap.total_tokens,
            input_tokens: snap.input_tokens,
            output_tokens: snap.output_tokens,
            model_breakdown: snap.by_model.iter()
                .map(|(m, c)| (m.clone(), *c))
                .collect(),
        })
    }

    fn budget_status(&self) -> Option<BudgetStatus> {
        self.tracker.as_ref()?.budget_status().map(|bs| match bs {
            model_selection::BudgetStatus::NoBudget => BudgetStatus::NoBudget,
            model_selection::BudgetStatus::Ok { remaining, total } =>
                BudgetStatus::Ok { remaining, total },
            // ... etc
        })
    }
}
```

### CompletionSource

Replaces direct access to `SlashRegistry` (from `slash_commands`),
`CompletionItem`, `SlashCommand`, `BuiltinSlashContributor`.

**Used by:** `slash_menu.rs` (1 ref), `leader_menu/builder.rs` (7 refs)

```rust
pub trait CompletionSource: Send + Sync {
    /// Get completions matching a prefix (for slash menu)
    fn completions(&self, prefix: &str) -> Vec<CompletionItem>;

    /// Get all registered slash commands (for leader menu building)
    fn commands(&self) -> Vec<SlashCommandInfo>;
}
```

**Main crate implementation** wraps `SlashRegistry::complete()` and
`SlashRegistry::commands()`, mapping to TUI-owned types.

### PluginWidgetHost

Replaces direct access to `PluginUIState`, `Widget`, `PluginNotification`,
`StatusSegment`, `Direction` (from `plugin::ui`).

**Used by:** `widget_host.rs` (5 refs), `status_bar.rs` (implicit via render)

```rust
pub trait PluginWidgetHost: Send + Sync {
    /// Get active plugin widgets to render
    fn widgets(&self) -> Vec<WidgetSpec>;

    /// Get pending plugin notifications
    fn notifications(&self) -> Vec<NotificationSpec>;

    /// Get plugin status bar segments
    fn status_segments(&self) -> Vec<StatusSegmentSpec>;

    /// Dismiss a notification
    fn dismiss_notification(&mut self, index: usize);
}

/// A plugin widget to render in the TUI
#[derive(Debug, Clone)]
pub struct WidgetSpec {
    pub plugin_name: String,
    pub content: String,
    pub width: u16,
    pub height: u16,
}

/// A plugin notification
#[derive(Debug, Clone)]
pub struct NotificationSpec {
    pub plugin_name: String,
    pub message: String,
    pub level: NotificationLevel,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationLevel {
    Info, Warning, Error,
}

/// A plugin's status bar segment
#[derive(Debug, Clone)]
pub struct StatusSegmentSpec {
    pub plugin_name: String,
    pub text: String,
}
```

### ProcessDataSource

Replaces direct access to `ProcessMonitorHandle` and `ProcessState`
(from `procmon`).

**Used by:** `process_panel.rs` (2 refs)

```rust
pub trait ProcessDataSource: Send + Sync {
    /// Get all tracked processes and their current state
    fn processes(&self) -> Vec<ProcessInfo>;

    /// Get detailed info for a specific process
    fn process_detail(&self, pid: u32) -> Option<ProcessDetail>;
}

/// Process info for the overview list
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub command: String,
    pub state: ProcessState,
    pub cpu_percent: f32,
    pub rss_bytes: u64,
    pub wall_time: std::time::Duration,
    pub cpu_history: Vec<f32>,   // for sparkline
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    Running,
    Exited { code: Option<i32> },
}

/// Detailed process info for the detail view
#[derive(Debug, Clone)]
pub struct ProcessDetail {
    pub info: ProcessInfo,
    pub children: Vec<u32>,
    pub peak_rss: u64,
    pub output_lines: usize,
}
```

### MergeMessageProvider

Replaces direct access to `AgentMessage`, `Content`, `MessageId`,
`UserMessage`, `MessageEntry` (from `provider::message` and `session::entry`).

**Used by:** `merge_interactive.rs` (5 refs)

```rust
pub trait MergeMessageProvider: Send + Sync {
    /// Get message summaries for the interactive merge checkbox view
    fn message_summaries(&self, message_ids: &[String]) -> Vec<MergeMessageSummary>;
}

/// A message summary for the merge checkbox list
#[derive(Debug, Clone)]
pub struct MergeMessageSummary {
    pub id: String,
    pub role: MessageRole,
    pub preview: String,
    pub timestamp: Option<chrono::DateTime<chrono::Utc>>,
}
```

### AppBridge (supertrait)

Combines all provider traits plus direct-access state that was on `App`.

```rust
pub trait AppBridge: Send {
    // Data providers
    fn cost_provider(&self) -> Option<&dyn CostProvider>;
    fn completion_source(&self) -> &dyn CompletionSource;
    fn plugin_host(&self) -> &dyn PluginWidgetHost;
    fn plugin_host_mut(&mut self) -> &mut dyn PluginWidgetHost;
    fn process_source(&self) -> &dyn ProcessDataSource;
    fn merge_provider(&self) -> &dyn MergeMessageProvider;

    // Action registry (key → action mapping)
    fn resolve_key(&self, mode: InputMode, key: crossterm::event::KeyEvent)
        -> Option<Action>;
    fn action_name(&self, action: &Action) -> &str;

    // Plan mode (simple enum, but owned by main crate)
    fn plan_state(&self) -> PlanState;
    fn set_plan_state(&mut self, state: PlanState);

    // Clipboard
    fn poll_clipboard(&mut self) -> Option<ClipboardResult>;
}

/// Clipboard result (image or text)
#[derive(Debug, Clone)]
pub enum ClipboardResult {
    Image { data: Vec<u8>, media_type: String },
    Text(String),
    Error(String),
}

/// Plan mode state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanState {
    Inactive,
    Active,
    Executing,
}
```

## Fields Removed from App

These fields currently on `App` are replaced by `AppBridge`:

| Field | Type | Replacement |
|-------|------|-------------|
| `input_mode` | `InputMode` | `bridge.input_mode()` / `bridge.set_input_mode()` → **No**, `InputMode` stays on App since it's in tui-types and the TUI mutates it directly on every keypress. Moving it behind a trait adds overhead for the most frequent operation. |
| `thinking_level` | `ThinkingLevel` | Same reasoning — stays on App as `ThinkingLevel` is in tui-types |
| `cost_tracker` | `Option<Arc<CostTracker>>` | `bridge.cost_provider()` |
| `slash_registry` | `SlashRegistry` | `bridge.completion_source()` |
| `action_registry` | `ActionRegistry` | `bridge.resolve_key()` |
| `plugin_ui` | `PluginUIState` | `bridge.plugin_host()` |
| `clipboard_rx` | `Option<Receiver<ClipboardResult>>` | `bridge.poll_clipboard()` |
| `overlays.plan_state` | `PlanState` | `bridge.plan_state()` |

**Revised:** `InputMode` and `ThinkingLevel` stay as direct fields on `App`
since they're in `clankers-tui-types` (no external dep) and are mutated on
every keypress / toggle. Only 6 fields actually move behind the bridge.

## Interaction Patterns

### Read-only data (cost, processes, completions)

Components call through the bridge on each render frame:

```rust
fn draw(&self, f: &mut Frame, area: Rect, app: &App) {
    if let Some(cost) = app.bridge.cost_provider() {
        if let Some(summary) = cost.summary() {
            // render cost display
        }
    }
}
```

### Mutable operations (dismiss notification, set plan state)

Components return `PanelAction` to the event loop. The event loop calls
bridge methods:

```rust
// In event loop
match action {
    PanelAction::SlashCommand(cmd) => {
        // Main crate handles slash commands
        output_tx.send(TuiOutput::SlashCommand(cmd));
    }
}
```

### Key resolution

The event loop resolves keys before passing to TUI components:

```rust
// In EventLoopRunner
if let Some(action) = app.bridge.resolve_key(app.input_mode, key_event) {
    app.handle_action(action);
}
```

This replaces the current pattern where `ActionRegistry` is stored on `App`.
