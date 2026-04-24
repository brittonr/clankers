//! Process panel — displays process monitor data
//!
//! Shows CPU/memory usage for tracked processes, allowing users to see
//! what child processes (bash, subagents, etc.) are running and their resource usage.

use std::time::Duration;

use clanker_tui_types::ProcessDataSource;
use clanker_tui_types::ProcessDisplayState;
use clanker_tui_types::ProcessSnapshot;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;

use super::prelude::*;
use crate::panel::ListNav;

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
struct ProcessEntry {
    pid: u32,
    cpu_percent: f32,
    rss_bytes: u64,
    command: String,
    tool_name: String,
    call_id: String,
    elapsed: Duration,
    depth: u8,
    state: EntryState,
    peak_rss: u64,
    /// CPU% history for sparkline
    cpu_history: Vec<f32>,
    /// RSS history for sparkline
    mem_history: Vec<f32>,
    /// Child PIDs
    children: Vec<u32>,
}

// ── Panel state ─────────────────────────────────────────────────────────────

/// Process monitor panel
pub struct ProcessPanel {
    nav: ListNav,
    entries: Vec<ProcessEntry>,
    show_completed: bool,
    monitor: Option<std::sync::Arc<dyn ProcessDataSource>>,
    sort_mode: SortMode,
    /// When set, show detail view for this PID instead of the list
    detail_pid: Option<u32>,
}

impl Default for ProcessPanel {
    fn default() -> Self {
        Self {
            nav: ListNav::new(),
            entries: Vec::new(),
            show_completed: false,
            monitor: None,
            sort_mode: SortMode::Cpu,
            detail_pid: None,
        }
    }
}

impl ProcessPanel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the process data source
    pub fn with_monitor(mut self, monitor: std::sync::Arc<dyn ProcessDataSource>) -> Self {
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
        for snap in monitor.active_processes() {
            // Add children with depth=1
            let children = snap.children.clone();
            let tool_name = snap.tool_name.clone();
            self.entries.push(snapshot_to_entry(snap, 0));

            for child_pid in children {
                self.entries.push(ProcessEntry {
                    pid: child_pid,
                    cpu_percent: 0.0,
                    rss_bytes: 0,
                    command: format!("child of {}", self.entries.last().map(|e| e.pid).unwrap_or(0)),
                    tool_name: tool_name.clone(),
                    call_id: String::new(),
                    elapsed: Duration::ZERO,
                    depth: 1,
                    state: EntryState::Running,
                    peak_rss: 0,
                    cpu_history: Vec::new(),
                    mem_history: Vec::new(),
                    children: Vec::new(),
                });
            }
        }

        // Add completed processes if requested
        if self.show_completed {
            for snap in monitor.completed_processes() {
                self.entries.push(snapshot_to_entry(snap, 0));
            }
        }

        // Sort entries
        match self.sort_mode {
            SortMode::Cpu => {
                self.entries
                    .sort_by(|a, b| b.cpu_percent.partial_cmp(&a.cpu_percent).unwrap_or(std::cmp::Ordering::Equal));
            }
            SortMode::Memory => {
                self.entries.sort_by_key(|e| std::cmp::Reverse(e.rss_bytes));
            }
            SortMode::Time => {
                self.entries.sort_by_key(|e| std::cmp::Reverse(e.elapsed));
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

fn snapshot_to_entry(snap: ProcessSnapshot, depth: u8) -> ProcessEntry {
    let state = match snap.state {
        ProcessDisplayState::Running => EntryState::Running,
        ProcessDisplayState::Exited { code, .. } => EntryState::Exited { code },
    };
    let elapsed = match snap.state {
        ProcessDisplayState::Running => snap.elapsed,
        ProcessDisplayState::Exited { wall_time, .. } => wall_time,
    };
    ProcessEntry {
        pid: snap.pid,
        cpu_percent: snap.cpu_percent,
        rss_bytes: snap.rss_bytes,
        command: snap.command,
        tool_name: snap.tool_name,
        call_id: snap.call_id,
        elapsed,
        depth,
        state,
        peak_rss: snap.peak_rss,
        cpu_history: snap.cpu_history,
        mem_history: snap.mem_history,
        children: snap.children,
    }
}

// ── Panel trait impl ────────────────────────────────────────────────────────

impl Panel for ProcessPanel {
    fn id(&self) -> PanelId {
        PanelId::Processes
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn title(&self) -> String {
        let active = self.entries.iter().filter(|e| matches!(e.state, EntryState::Running)).count();
        format!("Processes ({} active)", active)
    }

    fn focus_hints(&self) -> &'static str {
        if self.detail_pid.is_some() {
            " esc:back "
        } else {
            " j/k s:sort c:completed ↵:detail "
        }
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn empty_text(&self) -> &'static str {
        "No processes tracked."
    }

    #[cfg_attr(dylint_lib = "tigerstyle", allow(catch_all_on_enum, reason = "default handler covers many variants uniformly"))]
    fn handle_key_event(&mut self, key: KeyEvent) -> Option<PanelAction> {
        // Detail mode: Esc goes back to list, other keys ignored
        if self.detail_pid.is_some() {
            return match key.code {
                KeyCode::Esc => {
                    self.detail_pid = None;
                    Some(PanelAction::Consumed)
                }
                _ => None,
            };
        }

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
            KeyCode::Enter => {
                if let Some(entry) = self.entries.get(self.nav.selected) {
                    self.detail_pid = Some(entry.pid);
                    Some(PanelAction::Consumed)
                } else {
                    None
                }
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
        if let Some(pid) = self.detail_pid {
            self.draw_detail(frame, area, ctx, pid);
        } else {
            self.draw_list(frame, area, ctx);
        }
    }
}

impl ProcessPanel {
    /// Refresh entries - called from render loop
    pub fn refresh_entries(&mut self) {
        self.refresh();
    }

    fn draw_list(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
        let mut lines = Vec::new();

        // Header row
        lines.push(Line::from(vec![Span::styled(
            format!("{:>6}  {:>6}  {:>8}  {:>8}  {:>8}  {}", "PID", "CPU%", "SPARK", "MEM(MB)", "TIME", "COMMAND"),
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD),
        )]));

        let active_entries: Vec<_> =
            self.entries.iter().enumerate().filter(|(_, e)| matches!(e.state, EntryState::Running)).collect();
        let completed_entries: Vec<_> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| matches!(e.state, EntryState::Exited { .. }))
            .collect();

        // Render active processes
        for (display_idx, (global_idx, entry)) in active_entries.iter().enumerate() {
            let _ = display_idx;
            let mem_mb = entry.rss_bytes as f64 / (1024.0 * 1024.0);
            let time_str = format_elapsed(entry.elapsed);

            let cpu_color = cpu_color(entry.cpu_percent, ctx.theme.fg);
            let mem_color = mem_color(mem_mb, ctx.theme.fg);

            let spark = sparkline(&entry.cpu_history, 100.0, 8);

            let prefix = if entry.depth > 0 { " └─" } else { "" };
            let command_display = truncate_command(&entry.command, 40);

            let spans = vec![
                self.nav.prefix_span(*global_idx, ctx.focused),
                Span::styled(format!("{:>6}", entry.pid), Style::default().fg(ctx.theme.fg)),
                Span::raw("  "),
                Span::styled(format!("{:>6.1}", entry.cpu_percent), Style::default().fg(cpu_color)),
                Span::raw("  "),
                Span::styled(format!("{:>8}", spark), Style::default().fg(Color::Green)),
                Span::raw("  "),
                Span::styled(format!("{:>8.1}", mem_mb), Style::default().fg(mem_color)),
                Span::raw("  "),
                Span::styled(format!("{:>8}", time_str), Style::default().fg(ctx.theme.fg)),
                Span::raw("  "),
                Span::styled(
                    format!("{}{}", prefix, command_display),
                    self.nav.item_style(*global_idx, ctx.focused, Style::default().fg(ctx.theme.fg)),
                ),
            ];

            lines.push(Line::from(spans));
        }

        // Separator if showing completed
        if self.show_completed && !completed_entries.is_empty() {
            lines.push(Line::from(Span::styled("─ completed ─", Style::default().fg(Color::DarkGray))));

            for (global_idx, entry) in &completed_entries {
                let mem_mb = entry.peak_rss as f64 / (1024.0 * 1024.0);
                let time_str = format_elapsed(entry.elapsed);

                let code_str = if let EntryState::Exited { code: Some(c) } = entry.state {
                    format!("exit:{}", c)
                } else {
                    "exit".to_string()
                };

                let command_display = truncate_command(&entry.command, 35);

                let spans = vec![
                    self.nav.prefix_span(*global_idx, ctx.focused),
                    Span::styled(format!("{:>6}", entry.pid), Style::default().fg(Color::DarkGray)),
                    Span::raw("  "),
                    Span::styled(format!("{:>6}", code_str), Style::default().fg(Color::DarkGray)),
                    Span::raw("  "),
                    Span::styled("        ", Style::default()), // spark placeholder
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

        let total_lines = lines.len();
        let visible = area.height as usize;
        let scroll = self.nav.scroll_offset(visible, 1);
        let para = Paragraph::new(lines).scroll((scroll, 0)).wrap(Wrap { trim: false });
        frame.render_widget(para, area);
        render_scrollbar(frame, area, total_lines, scroll as usize, visible);
    }

    fn draw_detail(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext, pid: u32) {
        let entry = self.entries.iter().find(|e| e.pid == pid);
        let Some(entry) = entry else {
            let para = Paragraph::new(Line::from(Span::styled(
                format!("Process {} not found", pid),
                Style::default().fg(Color::Red),
            )));
            frame.render_widget(para, area);
            return;
        };

        let label_style = Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD);
        let val_style = Style::default().fg(ctx.theme.fg);

        let mut lines = Vec::new();

        let state_str = match entry.state {
            EntryState::Running => "Running".to_string(),
            EntryState::Exited { code } => {
                let c = code.map(|c| c.to_string()).unwrap_or_else(|| "?".to_string());
                format!("Exited (code {})", c)
            }
        };

        let mem_mb = entry.rss_bytes as f64 / (1024.0 * 1024.0);
        let peak_mb = entry.peak_rss as f64 / (1024.0 * 1024.0);

        // Metadata
        lines.push(Line::from(vec![
            Span::styled("Command:   ", label_style),
            Span::styled(&entry.command, val_style),
        ]));
        lines.push(Line::from(vec![
            Span::styled("PID:       ", label_style),
            Span::styled(entry.pid.to_string(), val_style),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Tool:      ", label_style),
            Span::styled(&entry.tool_name, val_style),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Call ID:   ", label_style),
            Span::styled(&entry.call_id, val_style),
        ]));
        lines.push(Line::from(vec![
            Span::styled("State:     ", label_style),
            Span::styled(
                &state_str,
                Style::default().fg(if matches!(entry.state, EntryState::Running) {
                    Color::Green
                } else {
                    Color::DarkGray
                }),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Wall time: ", label_style),
            Span::styled(format_elapsed(entry.elapsed), val_style),
        ]));

        lines.push(Line::default()); // blank line

        // Resource stats
        lines.push(Line::from(vec![
            Span::styled("CPU:       ", label_style),
            Span::styled(
                format!("{:.1}%", entry.cpu_percent),
                Style::default().fg(cpu_color(entry.cpu_percent, ctx.theme.fg)),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("RSS:       ", label_style),
            Span::styled(format!("{:.1} MB", mem_mb), val_style),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Peak RSS:  ", label_style),
            Span::styled(format!("{:.1} MB", peak_mb), val_style),
        ]));

        lines.push(Line::default());

        // Sparklines — use available width (area minus label)
        let spark_width = (area.width as usize).saturating_sub(14);
        let cpu_spark = sparkline(&entry.cpu_history, 100.0, spark_width);
        let mem_spark = sparkline(&entry.mem_history, entry.peak_rss as f32, spark_width);

        lines.push(Line::from(vec![
            Span::styled("CPU hist:  ", label_style),
            Span::styled(cpu_spark, Style::default().fg(Color::Cyan)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Mem hist:  ", label_style),
            Span::styled(mem_spark, Style::default().fg(Color::Magenta)),
        ]));

        // Children
        if !entry.children.is_empty() {
            lines.push(Line::default());
            lines.push(Line::from(vec![
                Span::styled("Children:  ", label_style),
                Span::styled(entry.children.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(", "), val_style),
            ]));
        }

        let para = Paragraph::new(lines).wrap(Wrap { trim: false });
        frame.render_widget(para, area);
    }

    /// Generate a status bar span showing process count
    pub fn status_bar_span(&self) -> Option<Span<'static>> {
        let active = self.entries.iter().filter(|e| matches!(e.state, EntryState::Running)).count();
        if active == 0 {
            return None;
        }

        let text = format!(" ⚙ {} procs ", active);
        Some(Span::styled(text, Style::default().fg(Color::Black).bg(Color::Blue).add_modifier(Modifier::BOLD)))
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Render values as a Unicode sparkline using block characters.
fn sparkline(values: &[f32], max_val: f32, width: usize) -> String {
    const BLOCKS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

    if values.is_empty() {
        return " ".repeat(width.max(1));
    }

    let start = values.len().saturating_sub(width);
    let slice = &values[start..];

    slice
        .iter()
        .map(|&v| {
            if max_val <= 0.0 {
                BLOCKS[0]
            } else {
                let ratio = (v / max_val).clamp(0.0, 1.0);
                let idx = (ratio * 7.0).round() as usize;
                BLOCKS[idx.min(7)]
            }
        })
        .collect()
}

fn format_elapsed(d: Duration) -> String {
    let secs = d.as_secs();
    if secs >= 3600 {
        format!("{}h{}m", secs / 3600, (secs % 3600) / 60)
    } else if secs >= 60 {
        format!("{}m{}s", secs / 60, secs % 60)
    } else {
        format!("{}s", secs)
    }
}

fn cpu_color(pct: f32, default: Color) -> Color {
    if pct > 80.0 {
        Color::Red
    } else if pct > 50.0 {
        Color::Yellow
    } else {
        default
    }
}

fn mem_color(mb: f64, default: Color) -> Color {
    if mb > 1024.0 {
        Color::Red
    } else if mb > 512.0 {
        Color::Yellow
    } else {
        default
    }
}

fn truncate_command(cmd: &str, max: usize) -> String {
    if cmd.len() > max {
        format!("{}...", &cmd[..max.saturating_sub(3)])
    } else {
        cmd.to_string()
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

    // ── sparkline tests ─────────────────────────────────────────────

    #[test]
    fn test_sparkline_empty_returns_spaces() {
        let s = sparkline(&[], 100.0, 8);
        assert_eq!(s, "        ");
    }

    #[test]
    fn test_sparkline_all_zeros() {
        let s = sparkline(&[0.0; 4], 100.0, 10);
        assert_eq!(s, "▁▁▁▁");
    }

    #[test]
    fn test_sparkline_all_max() {
        let s = sparkline(&[100.0; 3], 100.0, 10);
        assert_eq!(s, "███");
    }

    #[test]
    fn test_sparkline_ascending() {
        let vals = vec![0.0, 50.0, 100.0];
        let s = sparkline(&vals, 100.0, 10);
        let chars: Vec<char> = s.chars().collect();
        assert_eq!(chars[0], '▁');
        assert_eq!(chars[2], '█');
    }

    #[test]
    fn test_sparkline_truncates_to_width() {
        let vals: Vec<f32> = (0..20).map(|i| i as f32).collect();
        let s = sparkline(&vals, 19.0, 5);
        assert_eq!(s.chars().count(), 5);
    }

    #[test]
    fn test_sparkline_zero_max() {
        let s = sparkline(&[50.0, 100.0], 0.0, 10);
        assert_eq!(s, "▁▁");
    }

    // ── detail view tests ───────────────────────────────────────────

    #[test]
    fn test_enter_sets_detail_pid() {
        let mut panel = ProcessPanel::new();
        panel.entries.push(ProcessEntry {
            pid: 42,
            cpu_percent: 10.0,
            rss_bytes: 1000,
            command: "test".to_string(),
            tool_name: "bash".to_string(),
            call_id: "c1".to_string(),
            elapsed: Duration::from_secs(5),
            depth: 0,
            state: EntryState::Running,
            peak_rss: 2000,
            cpu_history: vec![],
            mem_history: vec![],
            children: vec![],
        });

        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(panel.handle_key_event(key), Some(PanelAction::Consumed));
        assert_eq!(panel.detail_pid, Some(42));
    }

    #[test]
    fn test_esc_in_detail_returns_to_list() {
        let mut panel = ProcessPanel::new();
        panel.detail_pid = Some(42);

        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(panel.handle_key_event(key), Some(PanelAction::Consumed));
        assert_eq!(panel.detail_pid, None);
    }

    #[test]
    fn test_esc_in_list_unfocuses() {
        let mut panel = ProcessPanel::new();
        assert_eq!(panel.detail_pid, None);

        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(panel.handle_key_event(key), Some(PanelAction::Unfocus));
    }

    #[test]
    fn test_focus_hints_change_with_mode() {
        let mut panel = ProcessPanel::new();
        assert!(panel.focus_hints().contains("detail"));

        panel.detail_pid = Some(1);
        assert!(panel.focus_hints().contains("back"));
        assert!(!panel.focus_hints().contains("detail"));
    }

    #[test]
    fn test_nav_disabled_in_detail_mode() {
        let mut panel = ProcessPanel::new();
        panel.detail_pid = Some(42);

        let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(panel.handle_key_event(key), None);

        let key = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE);
        assert_eq!(panel.handle_key_event(key), None);
    }
}
