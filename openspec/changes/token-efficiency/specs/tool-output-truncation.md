# Spec: Tool Output Truncation

## Overview

Cap tool result size before inserting into the conversation. When output
exceeds the limit, save the full output to a temp file and return a truncated
version with a reference path. Prevents catastrophic context blowups from
verbose tool output.

## Current State

Tool results are added to conversation history verbatim. No size limit.

Clankers' gitignore-aware grep/find tools prevent the worst cases (pi's 138K
blowup on prompt #3), but nothing stops a `bash: cat large_file.log` from
injecting megabytes into context. The `read` tool has its own offset/limit
mechanism, but `bash`, `grep`, and `find` do not.

Each turn re-sends the full conversation. A 200KB tool result at turn 1 is
re-transmitted at turns 2, 3, 4, etc. A single oversized result compounds
into tens of thousands of wasted tokens.

## Truncation Config

```rust
// crates/clankers-loop/src/truncation.rs (new file)

/// Truncation limits for tool output.
#[derive(Debug, Clone)]
pub struct TruncationConfig {
    /// Maximum bytes before truncation (default: 50KB).
    pub max_bytes: usize,
    /// Maximum lines before truncation (default: 2000).
    pub max_lines: usize,
}

impl Default for TruncationConfig {
    fn default() -> Self {
        Self {
            max_bytes: 50 * 1024,
            max_lines: 2000,
        }
    }
}
```

Defaults match pi's behavior (50KB / 2000 lines, whichever is hit first).

## Truncation Function

```rust
/// Result of attempting truncation.
pub struct TruncationResult {
    /// The (possibly truncated) content to use in conversation.
    pub content: String,
    /// Whether truncation was applied.
    pub truncated: bool,
    /// Path to full output if truncated.
    pub full_output_path: Option<PathBuf>,
    /// Original size stats.
    pub original_lines: usize,
    pub original_bytes: usize,
}

/// Truncate tool output if it exceeds configured limits.
///
/// Returns the original content unchanged if within limits.
/// Otherwise saves full output to a temp file and returns
/// truncated content with a reference footer.
pub fn truncate_tool_output(
    content: &str,
    config: &TruncationConfig,
) -> TruncationResult {
    let original_bytes = content.len();
    let lines: Vec<&str> = content.lines().collect();
    let original_lines = lines.len();

    // Within limits — return as-is
    if original_lines <= config.max_lines && original_bytes <= config.max_bytes {
        return TruncationResult {
            content: content.to_string(),
            truncated: false,
            full_output_path: None,
            original_lines,
            original_bytes,
        };
    }

    // Determine how many lines to keep
    let mut kept_bytes = 0;
    let mut kept_lines = 0;
    for line in &lines {
        let line_bytes = line.len() + 1; // +1 for newline
        if kept_lines >= config.max_lines || kept_bytes + line_bytes > config.max_bytes {
            break;
        }
        kept_bytes += line_bytes;
        kept_lines += 1;
    }

    // Save full output to temp file
    let tmp_path = save_full_output(content);

    // Build truncated result with footer
    let truncated_content = format!(
        "{}\n\n[Output truncated: {} lines / {} total. Full output: {}]\n[Use `read {} --offset {} --limit 200` to continue]",
        lines[..kept_lines].join("\n"),
        original_lines,
        format_size(original_bytes),
        tmp_path.display(),
        tmp_path.display(),
        kept_lines + 1,
    );

    TruncationResult {
        content: truncated_content,
        truncated: true,
        full_output_path: Some(tmp_path),
        original_lines,
        original_bytes,
    }
}

fn save_full_output(content: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("clankers-tool-output");
    std::fs::create_dir_all(&dir).ok();
    let path = dir.join(format!("{}.txt", uuid::Uuid::new_v4().simple()));
    std::fs::write(&path, content).ok();
    path
}

fn format_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{}B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
```

## Integration Point

The truncation layer sits in the agent loop, after tool execution and before
the tool result is appended to the conversation messages.

```rust
// In the agent loop, where tool results are processed:

let raw_result = tool.execute(args).await;

// Apply truncation before adding to conversation
let truncated = truncate_tool_output(&raw_result.content, &truncation_config);
let result_for_conversation = raw_result.with_content(truncated.content);

if truncated.truncated {
    tracing::info!(
        tool = tool.name(),
        original_lines = truncated.original_lines,
        original_bytes = truncated.original_bytes,
        saved_to = %truncated.full_output_path.unwrap().display(),
        "Tool output truncated"
    );
}

messages.push(tool_result_message(result_for_conversation));
```

## Tools That Already Truncate

The `read` tool has built-in offset/limit support and returns partial content
with instructions to continue. The truncation layer should not double-truncate
these results.

Two approaches (pick one during implementation):

**Option A: Check content length only.** If the read tool already returned
content within limits, the truncation layer is a no-op. If read returned
content exceeding limits (shouldn't happen with its own limits), truncation
catches it. No coordination needed.

**Option B: Opt-out flag.** Tools mark their result with `pre_truncated: true`
and the truncation layer skips them. More explicit but requires touching the
`ToolResult` type.

Recommend Option A for simplicity. The truncation layer is idempotent — applying
it to already-short content is free.

## Settings Integration

Users can override truncation limits in settings:

```toml
[tools]
max_output_bytes = 51200    # 50KB default
max_output_lines = 2000     # 2000 default
```

Or disable truncation entirely:

```toml
[tools]
truncate_output = false
```

## Temp File Cleanup

Temp files are written to `/tmp/clankers-tool-output/`. They are not
automatically cleaned up during the session (the model may need to read
them). Cleanup happens:

1. At session end (if the session manager is active).
2. On next startup, files older than 24 hours are purged.
3. The OS `/tmp` cleanup handles the rest.

## Invariants

- Truncation is applied uniformly to all tool results. No tool is exempt
  by default (Option A means tools that return short output are unaffected).
- The footer text must include the temp file path AND a usable `read` command
  with offset so the model can continue reading without guessing.
- The kept portion always ends at a line boundary. No mid-line truncation.
- Truncation config is per-session, not per-tool. A future enhancement could
  allow per-tool overrides but that's not in scope.

## Tests

- `within_limits_unchanged`: content under both limits returns as-is.
- `exceeds_line_limit`: 3000-line output truncated to 2000 lines, temp file created.
- `exceeds_byte_limit`: 100KB output truncated to 50KB, temp file created.
- `line_limit_hit_first`: 5000 short lines (total 20KB) truncated by line count.
- `byte_limit_hit_first`: 100 very long lines (total 200KB) truncated by byte count.
- `footer_contains_path`: truncated output includes temp file path.
- `footer_contains_read_command`: footer includes `read` command with correct offset.
- `temp_file_contains_full_output`: saved file matches original content exactly.
- `empty_content_unchanged`: empty string passes through.
- `single_line_over_byte_limit`: one extremely long line is truncated by bytes.
