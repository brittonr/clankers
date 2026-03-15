# Progress Protocol — Structured Progress Events

## Overview

The progress protocol defines structured progress metadata that tools emit
during execution. Unlike text-only `emit_progress`, structured progress gives
the TUI semantic information to render appropriate widgets (progress bars,
percentages, phase names, ETA estimates).

## Data Structures

### ToolProgress

The main progress event struct emitted by tools.

```rust
pub struct ToolProgress {
    /// The kind of progress (bytes, lines, items, percentage, phase)
    pub kind: ProgressKind,
    /// Optional human-readable message (e.g., "Searching /usr/lib...")
    pub message: Option<String>,
    /// Timestamp when this progress was emitted
    pub timestamp: std::time::Instant,
}

impl ToolProgress {
    /// Create progress from bytes processed
    pub fn bytes(current: u64, total: Option<u64>) -> Self {
        Self {
            kind: ProgressKind::Bytes { current, total },
            message: None,
            timestamp: Instant::now(),
        }
    }

    /// Create progress from lines processed
    pub fn lines(current: u64, total: Option<u64>) -> Self {
        Self {
            kind: ProgressKind::Lines { current, total },
            message: None,
            timestamp: Instant::now(),
        }
    }

    /// Create progress from items processed (generic countable units)
    pub fn items(current: u64, total: Option<u64>) -> Self {
        Self {
            kind: ProgressKind::Items { current, total },
            message: None,
            timestamp: Instant::now(),
        }
    }

    /// Create progress from percentage (0.0 to 100.0)
    pub fn percentage(percent: f32) -> Self {
        Self {
            kind: ProgressKind::Percentage { percent },
            message: None,
            timestamp: Instant::now(),
        }
    }

    /// Create phase progress (e.g., "Fetching", "Parsing", "Cancelling")
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

    /// Add a message to this progress
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }
}
```

### ProgressKind

Enum defining the different types of progress a tool can report.

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum ProgressKind {
    /// Bytes processed (e.g., downloaded, uploaded, read from disk)
    Bytes {
        current: u64,
        total: Option<u64>,
    },

    /// Lines processed (e.g., grep matches, file lines scanned)
    Lines {
        current: u64,
        total: Option<u64>,
    },

    /// Generic countable items (e.g., files processed, tests run)
    Items {
        current: u64,
        total: Option<u64>,
    },

    /// Percentage complete (0.0 to 100.0)
    /// Use when the tool can calculate percentage but not absolute progress
    Percentage {
        percent: f32,
    },

    /// Phase-based progress (e.g., "Fetching", "Parsing", "Cancelling")
    /// Use for multi-stage operations where each phase is distinct
    Phase {
        name: String,
        step: u32,
        total_steps: Option<u32>,
    },
}

impl ProgressKind {
    /// Calculate percentage if total is known
    pub fn as_percentage(&self) -> Option<f32> {
        match self {
            ProgressKind::Bytes { current, total: Some(total) } if *total > 0 => {
                Some((*current as f32 / *total as f32) * 100.0)
            }
            ProgressKind::Lines { current, total: Some(total) } if *total > 0 => {
                Some((*current as f32 / *total as f32) * 100.0)
            }
            ProgressKind::Items { current, total: Some(total) } if *total > 0 => {
                Some((*current as f32 / *total as f32) * 100.0)
            }
            ProgressKind::Percentage { percent } => Some(*percent),
            ProgressKind::Phase { step, total_steps: Some(total), .. } if *total > 0 => {
                Some((*step as f32 / *total as f32) * 100.0)
            }
            _ => None,
        }
    }

    /// Check if progress is complete (100%)
    pub fn is_complete(&self) -> bool {
        match self {
            ProgressKind::Bytes { current, total: Some(total) } => current >= total,
            ProgressKind::Lines { current, total: Some(total) } => current >= total,
            ProgressKind::Items { current, total: Some(total) } => current >= total,
            ProgressKind::Percentage { percent } => *percent >= 100.0,
            ProgressKind::Phase { step, total_steps: Some(total), .. } => step >= total,
            _ => false,
        }
    }

    /// Human-readable string for display (e.g., "50/100 bytes", "75%", "Phase 2/3: Parsing")
    pub fn display_string(&self) -> String {
        match self {
            ProgressKind::Bytes { current, total: Some(total) } => {
                format!("{}/{} bytes", current, total)
            }
            ProgressKind::Bytes { current, total: None } => {
                format!("{} bytes", current)
            }
            ProgressKind::Lines { current, total: Some(total) } => {
                format!("{}/{} lines", current, total)
            }
            ProgressKind::Lines { current, total: None } => {
                format!("{} lines", current)
            }
            ProgressKind::Items { current, total: Some(total) } => {
                format!("{}/{} items", current, total)
            }
            ProgressKind::Items { current, total: None } => {
                format!("{} items", current)
            }
            ProgressKind::Percentage { percent } => {
                format!("{:.1}%", percent)
            }
            ProgressKind::Phase { name, step, total_steps: Some(total) } => {
                format!("Phase {}/{}: {}", step, total, name)
            }
            ProgressKind::Phase { name, step, total_steps: None } => {
                format!("Phase {}: {}", step, name)
            }
        }
    }
}
```

### ToolResultChunk

Result chunks that tools emit as they produce output.

```rust
pub struct ToolResultChunk {
    /// The content of this chunk (text, base64, etc.)
    pub content: String,
    /// The type of content ("text", "base64", "json")
    pub content_type: String,
    /// Sequence number (for ordering, starts at 0)
    pub sequence: u64,
    /// Timestamp when this chunk was emitted
    pub timestamp: std::time::Instant,
}

impl ToolResultChunk {
    /// Create a text chunk
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            content_type: "text".to_string(),
            sequence: 0,  // Caller should set this
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
    pub fn json(value: &serde_json::Value) -> Self {
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
```

## AgentEvent Extensions

New event variants added to `AgentEvent` enum:

```rust
pub enum AgentEvent {
    // ... existing variants ...

    /// Structured progress update from a tool
    ToolProgressUpdate {
        call_id: String,
        progress: ToolProgress,
    },

    /// Result chunk streamed from a tool
    ToolResultChunk {
        call_id: String,
        chunk: ToolResultChunk,
    },

    // ToolExecutionUpdate remains for backward compatibility
}
```

## Usage Examples

### Grep tool reporting lines found

```rust
async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
    let pattern = /* ... */;
    let mut lines_found = 0;
    let mut chunks = Vec::new();

    for entry in walk_dir(path) {
        if ctx.signal.is_cancelled() {
            ctx.emit_structured_progress(
                ToolProgress::phase("Cancelling", 1, 1)
            );
            break;
        }

        if let Some(line) = grep_line(&entry, &pattern) {
            lines_found += 1;

            // Progress update (throttled to 10/sec by ToolContext)
            ctx.emit_structured_progress(
                ToolProgress::lines(lines_found, None)
                    .with_message(format!("Searching {}", entry.path().display()))
            );

            // Result chunk (not throttled)
            ctx.emit_result_chunk(
                ToolResultChunk::text(line).with_sequence(lines_found)
            );
        }
    }

    Ok(ToolResult::text(format!("Found {} matches", lines_found)))
}
```

### Web fetch reporting download progress

```rust
async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
    let response = reqwest::get(url).await?;
    let total_bytes = response.content_length();
    let mut downloaded = 0u64;

    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        downloaded += chunk.len() as u64;

        ctx.emit_structured_progress(
            ToolProgress::bytes(downloaded, total_bytes)
                .with_message("Downloading...")
        );

        ctx.emit_result_chunk(
            ToolResultChunk::base64(base64::encode(&chunk))
                .with_sequence(downloaded)
        );
    }

    Ok(ToolResult::success("Download complete"))
}
```

### Multi-phase nix build

```rust
async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
    ctx.emit_structured_progress(
        ToolProgress::phase("Fetching dependencies", 1, 3)
    );
    fetch_deps()?;

    ctx.emit_structured_progress(
        ToolProgress::phase("Building", 2, 3)
    );
    build()?;

    ctx.emit_structured_progress(
        ToolProgress::phase("Installing", 3, 3)
    );
    install()?;

    Ok(ToolResult::success("Build complete"))
}
```

## File Location

New types go in `src/tools/progress.rs` (new file).

`AgentEvent` extensions go in `src/agent/events.rs` (existing file).
