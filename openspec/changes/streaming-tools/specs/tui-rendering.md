# TUI Rendering — Progress and Streaming Output

## Overview

The TUI needs to render two kinds of streaming data:
1. **Progress indicators** — progress bars, percentages, ETA, phase names
2. **Streaming output** — live result chunks with smart truncation

These are rendered in the tool execution panel, which already shows
`ToolExecutionUpdate` events. This change extends the panel to handle
`ToolProgressUpdate` and `ToolResultChunk` events.

## Components

### ProgressRenderer

A new component responsible for rendering progress indicators.

```rust
pub struct ProgressRenderer {
    /// Current progress state per call_id
    states: HashMap<String, ProgressState>,
}

struct ProgressState {
    /// Latest progress update
    progress: ToolProgress,
    /// When we received the first progress update
    started_at: Instant,
    /// When we received the latest progress update
    updated_at: Instant,
    /// History for ETA calculation (last N samples)
    history: VecDeque<ProgressSample>,
}

struct ProgressSample {
    timestamp: Instant,
    value: f32,  // Percentage or absolute value
}

impl ProgressRenderer {
    /// Update progress for a call_id
    pub fn update(&mut self, call_id: &str, progress: ToolProgress) {
        let state = self.states.entry(call_id.to_string())
            .or_insert_with(|| ProgressState {
                progress: progress.clone(),
                started_at: Instant::now(),
                updated_at: Instant::now(),
                history: VecDeque::with_capacity(10),
            });

        state.progress = progress;
        state.updated_at = Instant::now();

        // Add sample for ETA calculation
        if let Some(percent) = progress.kind.as_percentage() {
            state.history.push_back(ProgressSample {
                timestamp: Instant::now(),
                value: percent,
            });
            if state.history.len() > 10 {
                state.history.pop_front();
            }
        }
    }

    /// Render progress for a call_id
    pub fn render(&self, call_id: &str, area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer) {
        let Some(state) = self.states.get(call_id) else {
            return;
        };

        match &state.progress.kind {
            ProgressKind::Bytes { current, total } |
            ProgressKind::Lines { current, total } |
            ProgressKind::Items { current, total } => {
                self.render_countable(state, area, buf, *current, *total);
            }
            ProgressKind::Percentage { percent } => {
                self.render_percentage(state, area, buf, *percent);
            }
            ProgressKind::Phase { name, step, total_steps } => {
                self.render_phase(state, area, buf, name, *step, *total_steps);
            }
        }
    }

    /// Render a countable progress (bytes/lines/items with known/unknown total)
    fn render_countable(
        &self,
        state: &ProgressState,
        area: Rect,
        buf: &mut Buffer,
        current: u64,
        total: Option<u64>,
    ) {
        let percent = if let Some(total) = total {
            if total > 0 {
                Some((current as f32 / total as f32) * 100.0)
            } else {
                None
            }
        } else {
            None
        };

        if let Some(percent) = percent {
            // Known total: show progress bar
            let gauge = Gauge::default()
                .gauge_style(Style::default().fg(Color::Cyan))
                .percent(percent as u16)
                .label(format!("{}/{} ({})", current, total.unwrap(), 
                    state.progress.kind.display_string()));
            gauge.render(area, buf);

            // Show ETA if we have enough history
            if let Some(eta) = self.calculate_eta(state, percent) {
                let eta_text = format!("ETA: {}", format_duration(eta));
                let eta_x = area.x + area.width.saturating_sub(eta_text.len() as u16 + 1);
                let eta_y = area.y;
                buf.set_string(eta_x, eta_y, eta_text, Style::default().fg(Color::DarkGray));
            }
        } else {
            // Unknown total: show spinner + count
            let spinner = self.spinner_char(state.started_at);
            let text = format!("{} {} {}", 
                spinner,
                current,
                state.progress.kind.display_string()
            );
            buf.set_string(area.x, area.y, text, Style::default().fg(Color::Yellow));
        }

        // Show message below if present
        if let Some(ref message) = state.progress.message {
            let message_y = area.y + 1;
            if message_y < area.y + area.height {
                buf.set_string(area.x + 2, message_y, message, Style::default().fg(Color::DarkGray));
            }
        }
    }

    /// Render percentage progress
    fn render_percentage(&self, state: &ProgressState, area: Rect, buf: &mut Buffer, percent: f32) {
        let gauge = Gauge::default()
            .gauge_style(Style::default().fg(Color::Cyan))
            .percent(percent as u16)
            .label(format!("{:.1}%", percent));
        gauge.render(area, buf);

        if let Some(eta) = self.calculate_eta(state, percent) {
            let eta_text = format!("ETA: {}", format_duration(eta));
            let eta_x = area.x + area.width.saturating_sub(eta_text.len() as u16 + 1);
            buf.set_string(eta_x, area.y, eta_text, Style::default().fg(Color::DarkGray));
        }
    }

    /// Render phase progress
    fn render_phase(
        &self,
        state: &ProgressState,
        area: Rect,
        buf: &mut Buffer,
        name: &str,
        step: u32,
        total_steps: Option<u32>,
    ) {
        let text = if let Some(total) = total_steps {
            format!("Phase {}/{}: {}", step, total, name)
        } else {
            format!("Phase {}: {}", step, name)
        };

        buf.set_string(area.x, area.y, text, Style::default().fg(Color::Magenta));

        // Show progress bar if total_steps is known
        if let Some(total) = total_steps {
            if total > 0 && area.height > 1 {
                let percent = (step as f32 / total as f32) * 100.0;
                let gauge_area = Rect {
                    x: area.x,
                    y: area.y + 1,
                    width: area.width,
                    height: 1,
                };
                let gauge = Gauge::default()
                    .gauge_style(Style::default().fg(Color::Magenta))
                    .percent(percent as u16);
                gauge.render(gauge_area, buf);
            }
        }
    }

    /// Calculate ETA based on progress history
    fn calculate_eta(&self, state: &ProgressState, current_percent: f32) -> Option<Duration> {
        if state.history.len() < 2 {
            return None;
        }

        let oldest = state.history.front()?;
        let newest = state.history.back()?;

        let time_delta = newest.timestamp.duration_since(oldest.timestamp).as_secs_f32();
        let percent_delta = newest.value - oldest.value;

        if percent_delta <= 0.0 || time_delta <= 0.0 {
            return None;
        }

        let percent_remaining = 100.0 - current_percent;
        let rate = percent_delta / time_delta;  // percent per second
        let eta_seconds = percent_remaining / rate;

        if eta_seconds > 0.0 && eta_seconds < 3600.0 * 24.0 {  // Cap at 24 hours
            Some(Duration::from_secs_f32(eta_seconds))
        } else {
            None
        }
    }

    /// Get spinner character (cycles through animation frames)
    fn spinner_char(&self, started_at: Instant) -> char {
        const FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        let frame_duration = Duration::from_millis(80);
        let elapsed = started_at.elapsed();
        let frame_index = (elapsed.as_millis() / frame_duration.as_millis()) as usize % FRAMES.len();
        FRAMES[frame_index]
    }
}

fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}
```

### StreamingOutputPanel

Renders live result chunks with smart truncation.

```rust
pub struct StreamingOutputPanel {
    /// Accumulated chunks per call_id
    chunks: HashMap<String, ChunkBuffer>,
    /// Scroll offset for each call_id
    scroll: HashMap<String, usize>,
}

struct ChunkBuffer {
    /// All chunks received
    chunks: Vec<ToolResultChunk>,
    /// Merged text lines (for display)
    lines: Vec<String>,
    /// Total lines (including truncated)
    total_lines: usize,
    /// Truncation config
    config: TruncationConfig,
}

impl StreamingOutputPanel {
    /// Add a chunk for a call_id
    pub fn add_chunk(&mut self, call_id: &str, chunk: ToolResultChunk) {
        let buffer = self.chunks.entry(call_id.to_string())
            .or_insert_with(|| ChunkBuffer {
                chunks: Vec::new(),
                lines: Vec::new(),
                total_lines: 0,
                config: TruncationConfig::default(),
            });

        buffer.chunks.push(chunk.clone());

        // Append lines to buffer
        if chunk.content_type == "text" {
            for line in chunk.content.lines() {
                buffer.total_lines += 1;

                // Smart truncation: keep head + tail window
                if buffer.total_lines <= buffer.config.head_lines {
                    // In head window
                    buffer.lines.push(line.to_string());
                } else if buffer.total_lines > buffer.config.max_lines - buffer.config.tail_lines {
                    // In tail window
                    // If we just crossed the threshold, add the omission marker
                    if buffer.lines.len() == buffer.config.head_lines {
                        buffer.lines.push(format!(
                            "... [{} lines omitted, streaming...] ...",
                            buffer.total_lines - buffer.config.head_lines
                        ));
                    } else {
                        // We're in tail — keep overwriting the marker line
                        if buffer.lines.len() > buffer.config.head_lines {
                            // Remove old tail lines and marker
                            buffer.lines.truncate(buffer.config.head_lines);
                        }
                        // Update marker
                        let omitted = buffer.total_lines - buffer.config.head_lines - 
                            buffer.chunks.iter()
                                .rev()
                                .take(buffer.config.tail_lines)
                                .flat_map(|c| c.content.lines())
                                .count();
                        buffer.lines.push(format!(
                            "... [{} lines omitted, streaming...] ...",
                            omitted
                        ));
                        // Add current line to tail
                        buffer.lines.push(line.to_string());
                    }
                }
                // Middle lines (not in head or tail) are dropped
            }
        }
    }

    /// Render output for a call_id
    pub fn render(&self, call_id: &str, area: Rect, buf: &mut Buffer) {
        let Some(buffer) = self.chunks.get(call_id) else {
            return;
        };

        let scroll_offset = self.scroll.get(call_id).copied().unwrap_or(0);
        let visible_lines = area.height as usize;

        let start = scroll_offset.min(buffer.lines.len().saturating_sub(1));
        let end = (start + visible_lines).min(buffer.lines.len());

        for (i, line) in buffer.lines[start..end].iter().enumerate() {
            let y = area.y + i as u16;
            if y >= area.y + area.height {
                break;
            }

            // Highlight omission marker
            let style = if line.contains("lines omitted") {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::DIM)
            } else {
                Style::default()
            };

            buf.set_string(area.x, y, line, style);
        }

        // Show scrollbar if content is taller than viewport
        if buffer.lines.len() > visible_lines {
            let scrollbar_area = Rect {
                x: area.x + area.width - 1,
                y: area.y,
                width: 1,
                height: area.height,
            };
            self.render_scrollbar(scrollbar_area, buf, scroll_offset, buffer.lines.len(), visible_lines);
        }

        // Show stats at bottom
        let stats = format!(
            "Lines: {} ({} total) | Chunks: {}",
            buffer.lines.len(),
            buffer.total_lines,
            buffer.chunks.len()
        );
        let stats_y = area.y + area.height - 1;
        buf.set_string(area.x, stats_y, stats, Style::default().fg(Color::DarkGray));
    }

    fn render_scrollbar(&self, area: Rect, buf: &mut Buffer, offset: usize, total: usize, visible: usize) {
        let thumb_height = ((visible as f32 / total as f32) * area.height as f32).max(1.0) as u16;
        let thumb_offset = ((offset as f32 / total as f32) * area.height as f32) as u16;

        for y in 0..area.height {
            let abs_y = area.y + y;
            if y >= thumb_offset && y < thumb_offset + thumb_height {
                buf.get_mut(area.x, abs_y).set_char('█');
            } else {
                buf.get_mut(area.x, abs_y).set_char('│');
            }
        }
    }

    /// Scroll down
    pub fn scroll_down(&mut self, call_id: &str, amount: usize) {
        let offset = self.scroll.entry(call_id.to_string()).or_insert(0);
        if let Some(buffer) = self.chunks.get(call_id) {
            *offset = (*offset + amount).min(buffer.lines.len().saturating_sub(1));
        }
    }

    /// Scroll up
    pub fn scroll_up(&mut self, call_id: &str, amount: usize) {
        let offset = self.scroll.entry(call_id.to_string()).or_insert(0);
        *offset = offset.saturating_sub(amount);
    }

    /// Scroll to bottom (auto-follow mode)
    pub fn scroll_to_bottom(&mut self, call_id: &str) {
        if let Some(buffer) = self.chunks.get(call_id) {
            self.scroll.insert(call_id.to_string(), buffer.lines.len().saturating_sub(1));
        }
    }
}
```

## Integration into Tool Panel

The existing tool execution panel (`tool_panel.rs` or similar) needs to handle
the new events.

```rust
impl ToolPanel {
    pub fn handle_event(&mut self, event: AgentEvent) {
        match event {
            AgentEvent::ToolExecutionStart { call_id, tool_name } => {
                // Existing: create panel entry
                self.active_tools.insert(call_id.clone(), ToolExecution {
                    tool_name,
                    started_at: Instant::now(),
                    progress: None,
                    output: None,
                });
            }

            AgentEvent::ToolProgressUpdate { call_id, progress } => {
                // NEW: update progress renderer
                self.progress_renderer.update(&call_id, progress);
            }

            AgentEvent::ToolResultChunk { call_id, chunk } => {
                // NEW: add chunk to output panel
                self.output_panel.add_chunk(&call_id, chunk);
                // Auto-follow (scroll to bottom) if enabled
                if self.auto_follow {
                    self.output_panel.scroll_to_bottom(&call_id);
                }
            }

            AgentEvent::ToolExecutionUpdate { call_id, partial } => {
                // EXISTING: backward-compatible text updates
                if let Some(text) = partial.as_text() {
                    self.output_panel.add_chunk(
                        &call_id,
                        ToolResultChunk::text(text)
                    );
                }
            }

            AgentEvent::ToolExecutionEnd { call_id, .. } => {
                // Existing: mark complete, move to history
                self.active_tools.remove(&call_id);
                self.progress_renderer.states.remove(&call_id);
            }

            _ => {}
        }
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        // Layout: split into progress section + output section
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Progress bars
                Constraint::Min(0),     // Streaming output
            ])
            .split(area);

        // Render progress for active tools
        for (call_id, _) in &self.active_tools {
            self.progress_renderer.render(call_id, chunks[0], buf);
        }

        // Render streaming output
        if let Some(call_id) = self.selected_tool() {
            self.output_panel.render(&call_id, chunks[1], buf);
        }
    }
}
```

## Keybindings

New keybindings for the tool panel:

- `j` / `↓` — Scroll output down
- `k` / `↑` — Scroll output up
- `g` — Scroll to top
- `G` — Scroll to bottom
- `f` — Toggle auto-follow mode (scroll to bottom on new chunks)
- `d` — Toggle detail view (show progress history, chunk metadata)

## File Locations

- `src/tui/components/progress_renderer.rs` — new file
- `src/tui/components/streaming_output_panel.rs` — new file
- `src/tui/components/tool_panel.rs` — extend existing to integrate new renderers
