//! Process panel — displays process monitor data
//!
//! Shows CPU/memory usage for tracked processes, allowing users to see
//! what child processes (bash, subagents, etc.) are running and their resource usage.

use std::time::Duration;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;

use crate::procmon::ProcessMonitorHandle;
use crate::procmon::ProcessState;
use crate::tui::panel::DrawContext;
use crate::tui::panel::ListNav;
use crate::tui::panel::Panel;
use crate::tui::panel::PanelAction;
use crate::tui::panel::PanelId;

// ── Data types ──────────────────────────────────────────────────────────────

/// Sort mode for process list
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    Cpu,
    Memory,
    Time,
    Name,
}

impl SortMode {
    pub fn label(&self) -> &'static str {
        match self {
            SortMode::Cpu => "CPU%",
            SortMode::Memory => "MEM",
            SortMode::Time => "TIME",
            SortMode::Name => "NAME",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            SortMode::Cpu => SortMode::Memory,
            SortMode::Memory => SortMode::Time,
            SortMode::Time => SortMode::Name,
            SortMode::Name => SortMode::Cpu,
        }
    }
}

/// Entry state for display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntryState {
    Running,
    Exited { code: Option<i32> },
}

/// A process entry for display
#[derive(Debug, Clone)]
#[allow(dead_code)] // tool_name used in future detail view
struct ProcessEntry {
    pid: u32,
    cpu_percent: f32,
    rss_bytes: u64,
    command: String,
    tool_name: String,
    elapsed: Duration,
    depth: u8,
    state: EntryState,
    peak_rss: u64,
}

// ── Panel state ─────────────────────────────────────────────────────────────

/// Process monitor panel
pub struct ProcessPanel {
    nav: ListNav,
    entries: Vec<ProcessEntry>,
    show_completed: bool,
    monitor: Option<ProcessMonitorHandle>,
    sort_mode: SortMode,
}

impl Default for ProcessPanel {
    fn default() -> Self {
        Self {
            nav: ListNav::new(),
            entries: Vec::new(),
            show_completed: false,
            monitor: None,
            sort_mode: SortMode::Cpu,
        }
    }
}

impl ProcessPanel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the process monitor handle
    pub fn with_monitor(mut self, monitor: ProcessMonitorHandle) -> Self {
        self.monitor = Some(monitor);
        self
    }

    /// Refresh the entries list from the monitor
    fn refresh(&mut self) {
        self.entries.clear();

        let Some(ref monitor) = self.monitor else {
            return;
        };

        // Get active processes
        let snapshot = monitor.snapshot();
        
        for (pid, tracked) in snapshot {
            let (cpu_percent, rss_bytes) = if let Some(last) = tracked.snapshots.last() {
                (last.cpu_percent, last.rss_bytes)
            } else {
                (0.0, 0)
            };

            let elapsed = tracked.start_time.elapsed();
            
            self.entries.push(ProcessEntry {
                pid,
                cpu_percent,
                rss_bytes,
                command: tracked.meta.command.clone(),
                tool_name: tracked.meta.tool_name.clone(),
                elapsed,
                depth: 0,
                state: EntryState::Running,
                peak_rss: tracked.peak_rss,
            });

            // Add children with depth=1
            for &child_pid in &tracked.children {
                self.entries.push(ProcessEntry {
                    pid: child_pid,
                    cpu_percent: 0.0, // Children stats are tracked separately if registered
                    rss_bytes: 0,
                    command: format!("child of {}", pid),
                    tool_name: tracked.meta.tool_name.clone(),
                    elapsed: Duration::ZERO,
                    depth: 1,
                    state: EntryState::Running,
                    peak_rss: 0,
                });
            }
        }

        // Add completed processes if requested
        if self.show_completed {
            let history = monitor.history();
            
            for (pid, tracked) in history {
                let (cpu_percent, rss_bytes) = if let Some(last) = tracked.snapshots.last() {
                    (last.cpu_percent, last.rss_bytes)
                } else {
                    (0.0, 0)
                };

                let (elapsed, code) = match &tracked.state {
                    ProcessState::Running => (Duration::ZERO, None),
                    ProcessState::Exited { code, wall_time } => (*wall_time, *code),
                };

                self.entries.push(ProcessEntry {
                    pid,
                    cpu_percent,
                    rss_bytes,
                    command: tracked.meta.command.clone(),
                    tool_name: tracked.meta.tool_name.clone(),
                    elapsed,
                    depth: 0,
                    state: EntryState::Exited { code },
                    peak_rss: tracked.peak_rss,
                });
            }
        }

        // Sort entries
        match self.sort_mode {
            SortMode::Cpu => {
                self.entries.sort_by(|a, b| b.cpu_percent.partial_cmp(&a.cpu_percent).unwrap_or(std::cmp::Ordering::Equal));
            }
            SortMode::Memory => {
                self.entries.sort_by(|a, b| b.rss_bytes.cmp(&a.rss_bytes));
            }
            SortMode::Time => {
                self.entries.sort_by(|a, b| b.elapsed.cmp(&a.elapsed));
            }
            SortMode::Name => {
                self.entries.sort_by(|a, b| a.command.cmp(&b.command));
            }
        }

        // Clamp navigation
        self.nav.clamp(self.entries.len());
    }

    /// Get the count of active processes
    pub fn active_count(&self) -> usize {
        self.entries.iter().filter(|e| matches!(e.state, EntryState::Running)).count()
    }
}

// ── Panel trait impl ────────────────────────────────────────────────────────

impl Panel for ProcessPanel {
    fn id(&self) -> PanelId {
        PanelId::Processes
    }

    fn title(&self) -> String {
        let active = self.entries.iter().filter(|e| matches!(e.state, EntryState::Running)).count();
        format!("Processes ({} active)", active)
    }

    fn focus_hints(&self) -> &'static str {
        " j/k s:sort c:completed "
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn empty_text(&self) -> &'static str {
        "No processes tracked."
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> Option<PanelAction> {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.nav.next(self.entries.len());
                Some(PanelAction::Consumed)
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.nav.prev(self.entries.len());
                Some(PanelAction::Consumed)
            }
            KeyCode::Char('s') => {
                self.sort_mode = self.sort_mode.next();
                Some(PanelAction::Consumed)
            }
            KeyCode::Char('c') => {
                self.show_completed = !self.show_completed;
                Some(PanelAction::Consumed)
            }
            KeyCode::Esc => Some(PanelAction::Unfocus),
            _ => None,
        }
    }

    fn handle_scroll(&mut self, up: bool, lines: u16) {
        let len = self.entries.len();
        for _ in 0..lines {
            if up {
                self.nav.prev(len);
            } else {
                self.nav.next(len);
            }
        }
    }

    fn draw(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
        // Refresh is const since we use interior mutability in the monitor
        // But we can't call it here since draw takes &self. Instead, the caller
        // should call refresh() before draw in the render loop.
        
        let mut lines = Vec::new();

        // Header row
        lines.push(Line::from(vec![
            Span::styled(
                format!("{:>6}  {:>6}  {:>8}  {:>8}  {}", "PID", "CPU%", "MEM(MB)", "TIME", "COMMAND"),
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD),
            ),
        ]));

        let active_entries: Vec<_> = self.entries.iter().filter(|e| matches!(e.state, EntryState::Running)).collect();
        let completed_entries: Vec<_> = self.entries.iter().filter(|e| matches!(e.state, EntryState::Exited { .. })).collect();

        // Render active processes
        for (i, entry) in active_entries.iter().enumerate() {
            let mem_mb = entry.rss_bytes as f64 / (1024.0 * 1024.0);
            let elapsed_secs = entry.elapsed.as_secs();
            let time_str = if elapsed_secs >= 3600 {
                format!("{}h{}m", elapsed_secs / 3600, (elapsed_secs % 3600) / 60)
            } else if elapsed_secs >= 60 {
                format!("{}m{}s", elapsed_secs / 60, elapsed_secs % 60)
            } else {
                format!("{}s", elapsed_secs)
            };

            // Color coding
            let cpu_color = if entry.cpu_percent > 80.0 {
                Color::Red
            } else if entry.cpu_percent > 50.0 {
                Color::Yellow
            } else {
                ctx.theme.fg
            };

            let mem_color = if mem_mb > 1024.0 {
                Color::Red
            } else if mem_mb > 512.0 {
                Color::Yellow
            } else {
                ctx.theme.fg
            };

            let prefix = if entry.depth > 0 { " └─" } else { "" };
            let command_display = if entry.command.len() > 40 {
                format!("{}...", &entry.command[..37])
            } else {
                entry.command.clone()
            };

            let spans = vec![
                self.nav.prefix_span(i, ctx.focused),
                Span::styled(format!("{:>6}", entry.pid), Style::default().fg(ctx.theme.fg)),
                Span::raw("  "),
                Span::styled(format!("{:>6.1}", entry.cpu_percent), Style::default().fg(cpu_color)),
                Span::raw("  "),
                Span::styled(format!("{:>8.1}", mem_mb), Style::default().fg(mem_color)),
                Span::raw("  "),
                Span::styled(format!("{:>8}", time_str), Style::default().fg(ctx.theme.fg)),
                Span::raw("  "),
                Span::styled(
                    format!("{}{}", prefix, command_display),
                    self.nav.item_style(i, ctx.focused, Style::default().fg(ctx.theme.fg)),
                ),
            ];

            lines.push(Line::from(spans));
        }

        // Separator if showing completed
        if self.show_completed && !completed_entries.is_empty() {
            lines.push(Line::from(Span::styled(
                "─ completed ─",
                Style::default().fg(Color::DarkGray),
            )));

            // Render completed processes
            let offset = active_entries.len();
            for (i, entry) in completed_entries.iter().enumerate() {
                let mem_mb = entry.peak_rss as f64 / (1024.0 * 1024.0);
                let elapsed_secs = entry.elapsed.as_secs();
                let time_str = if elapsed_secs >= 3600 {
                    format!("{}h{}m", elapsed_secs / 3600, (elapsed_secs % 3600) / 60)
                } else if elapsed_secs >= 60 {
                    format!("{}m{}s", elapsed_secs / 60, elapsed_secs % 60)
                } else {
                    format!("{}s", elapsed_secs)
                };

                let code_str = if let EntryState::Exited { code: Some(c) } = entry.state {
                    format!("exit:{}", c)
                } else {
                    "exit".to_string()
                };

                let command_display = if entry.command.len() > 35 {
                    format!("{}...", &entry.command[..32])
                } else {
                    entry.command.clone()
                };

                let index = offset + i;
                let spans = vec![
                    self.nav.prefix_span(index, ctx.focused),
                    Span::styled(format!("{:>6}", entry.pid), Style::default().fg(Color::DarkGray)),
                    Span::raw("  "),
                    Span::styled(code_str, Style::default().fg(Color::DarkGray)),
                    Span::raw("  "),
                    Span::styled(format!("{:>8.1}", mem_mb), Style::default().fg(Color::DarkGray)),
                    Span::raw("  "),
                    Span::styled(format!("{:>8}", time_str), Style::default().fg(Color::DarkGray)),
                    Span::raw("  "),
                    Span::styled(
                        command_display,
                        Style::default().fg(Color::DarkGray).add_modifier(Modifier::CROSSED_OUT),
                    ),
                ];

                lines.push(Line::from(spans));
            }
        }

        let scroll = self.nav.scroll_offset(area.height as usize, 1);
        let para = Paragraph::new(lines).scroll((scroll, 0)).wrap(Wrap { trim: false });
        frame.render_widget(para, area);
    }
}

impl ProcessPanel {
    /// Refresh entries - called from render loop
    pub fn refresh_entries(&mut self) {
        self.refresh();
    }

    /// Generate a status bar span showing process count
    pub fn status_bar_span(&self) -> Option<Span<'static>> {
        let active = self.entries.iter().filter(|e| matches!(e.state, EntryState::Running)).count();
        if active == 0 {
            return None;
        }

        let text = format!(" ⚙ {} procs ", active);
        Some(Span::styled(
            text,
            Style::default().fg(Color::Black).bg(Color::Blue).add_modifier(Modifier::BOLD),
        ))
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crossterm::event::KeyModifiers;

    use super::*;

    #[test]
    fn test_panel_trait_title() {
        let panel = ProcessPanel::new();
        assert!(panel.title().contains("0 active"));
    }

    #[test]
    fn test_sort_mode_cycle() {
        let mut mode = SortMode::Cpu;
        mode = mode.next();
        assert_eq!(mode, SortMode::Memory);
        mode = mode.next();
        assert_eq!(mode, SortMode::Time);
        mode = mode.next();
        assert_eq!(mode, SortMode::Name);
        mode = mode.next();
        assert_eq!(mode, SortMode::Cpu);
    }

    #[test]
    fn test_handle_key_event() {
        let mut panel = ProcessPanel::new();

        let key = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE);
        assert_eq!(panel.handle_key_event(key), Some(PanelAction::Consumed));
        assert_eq!(panel.sort_mode, SortMode::Memory);

        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE);
        assert_eq!(panel.handle_key_event(key), Some(PanelAction::Consumed));
        assert!(panel.show_completed);

        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(panel.handle_key_event(key), Some(PanelAction::Unfocus));
    }

    #[test]
    fn test_is_empty() {
        let panel = ProcessPanel::new();
        assert!(panel.is_empty());
    }
}
