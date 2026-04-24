//! Peers panel — shows swarm peers with online/offline status
//!
//! Displays all known peers from the registry, their connection state,
//! capabilities, and last-seen timestamp. Peers discovered via mDNS or
//! heartbeat are automatically updated.

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;

use super::prelude::*;

/// Connection status for a peer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerStatus {
    /// Successfully responded to last probe
    Online,
    /// Failed to respond within timeout
    Offline,
    /// Never probed or status unknown
    Unknown,
    /// Currently being probed
    Probing,
}

impl PeerStatus {
    pub fn icon(&self) -> &'static str {
        match self {
            PeerStatus::Online => "●",
            PeerStatus::Offline => "○",
            PeerStatus::Unknown => "◌",
            PeerStatus::Probing => "◐",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            PeerStatus::Online => Color::Green,
            PeerStatus::Offline => Color::Red,
            PeerStatus::Unknown => Color::DarkGray,
            PeerStatus::Probing => Color::Yellow,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            PeerStatus::Online => "online",
            PeerStatus::Offline => "offline",
            PeerStatus::Unknown => "unknown",
            PeerStatus::Probing => "probing",
        }
    }
}

/// A peer entry for display
#[derive(Debug, Clone)]
pub struct PeerEntry {
    /// Human-readable name
    pub name: String,
    /// Truncated node ID for display
    pub short_id: String,
    /// Full node ID
    pub node_id: String,
    /// Connection status
    pub status: PeerStatus,
    /// Capability tags
    pub tags: Vec<String>,
    /// Available agents
    pub agents: Vec<String>,
    /// Whether it accepts prompts
    pub accepts_prompts: bool,
    /// Model name (if known)
    pub model: Option<String>,
    /// clankers version (if known)
    pub version: Option<String>,
    /// Last seen (human-readable relative time)
    pub last_seen: Option<String>,
    /// Number of available tools
    pub tool_count: usize,
}

/// State for the peers panel
#[derive(Debug)]
pub struct PeersPanel {
    /// All known peers
    pub entries: Vec<PeerEntry>,
    /// Index of the currently selected peer
    pub selected: usize,
    /// Whether the detail view is open
    pub detail_view: bool,
    /// This node's own ID (short form)
    pub self_id: Option<String>,
    /// Whether the embedded RPC server is running
    pub server_running: bool,
}

impl Default for PeersPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl PeersPanel {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            selected: 0,
            detail_view: false,
            self_id: None,
            server_running: false,
        }
    }

    /// Replace the peer list (called from registry refresh)
    pub fn set_peers(&mut self, entries: Vec<PeerEntry>) {
        // Preserve selection if possible
        let prev_id = self.entries.get(self.selected).map(|e| e.node_id.clone());
        self.entries = entries;
        if let Some(ref prev) = prev_id {
            if let Some(idx) = self.entries.iter().position(|e| e.node_id == *prev) {
                self.selected = idx;
            } else {
                self.selected = 0;
            }
        }
        if self.selected >= self.entries.len() && !self.entries.is_empty() {
            self.selected = self.entries.len() - 1;
        }
    }

    /// Update a single peer's status
    pub fn update_status(&mut self, node_id: &str, status: PeerStatus) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.node_id == node_id) {
            entry.status = status;
        }
    }

    #[cfg_attr(dylint_lib = "tigerstyle", allow(unchecked_division, reason = "divisor guarded by is_empty/non-zero check or TUI layout constraint"))]
    pub fn select_next(&mut self) {
        if !self.entries.is_empty() {
            self.selected = (self.selected + 1) % self.entries.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.entries.is_empty() {
            self.selected = if self.selected == 0 {
                self.entries.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    pub fn toggle_detail(&mut self) {
        self.detail_view = !self.detail_view;
    }

    /// Get the currently selected peer's node_id
    pub fn selected_peer(&self) -> Option<&PeerEntry> {
        self.entries.get(self.selected)
    }
}

// ── Panel trait impl ────────────────────────────────────────────────────────

impl Panel for PeersPanel {
    fn id(&self) -> PanelId {
        PanelId::Peers
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn title(&self) -> String {
        let online = self.entries.iter().filter(|e| e.status == PeerStatus::Online).count();
        let total = self.entries.len();
        format!("Peers ({}/{})", online, total)
    }

    fn focus_hints(&self) -> &'static str {
        " j/k Tab Enter:detail "
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty() && !self.server_running
    }

    fn empty_text(&self) -> &'static str {
        "No peers known. /peers add <id> <name>"
    }

    #[cfg_attr(dylint_lib = "tigerstyle", allow(catch_all_on_enum, reason = "default handler covers many variants uniformly"))]
    fn handle_key_event(&mut self, key: KeyEvent) -> Option<PanelAction> {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.select_next();
                Some(PanelAction::Consumed)
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.select_prev();
                Some(PanelAction::Consumed)
            }
            KeyCode::Enter => {
                self.toggle_detail();
                Some(PanelAction::Consumed)
            }
            KeyCode::Esc if self.detail_view => {
                self.detail_view = false;
                Some(PanelAction::Consumed)
            }
            KeyCode::Esc => Some(PanelAction::Unfocus),
            _ => None,
        }
    }

    fn handle_scroll(&mut self, up: bool, lines: u16) {
        for _ in 0..lines {
            if up {
                self.select_prev();
            } else {
                self.select_next();
            }
        }
    }

    fn draw(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
        if self.detail_view {
            render_detail_view(frame, self, ctx.theme, area, ctx.focused);
        } else {
            render_list_view(frame, self, ctx.theme, area, ctx.focused);
        }
    }
}

fn render_list_view(frame: &mut Frame, panel: &PeersPanel, _theme: &Theme, area: Rect, _focused: bool) {
    if panel.entries.is_empty() {
        let msg = vec![
            Line::from(Span::styled("No peers known.", Style::default().fg(Color::DarkGray))),
            Line::from(""),
            Line::from(Span::styled("/peers add <id> <name>", Style::default().fg(Color::DarkGray))),
            Line::from(Span::styled("/peers discover", Style::default().fg(Color::DarkGray))),
        ];
        frame.render_widget(Paragraph::new(msg).wrap(Wrap { trim: false }), area);
        return;
    }

    let mut lines = Vec::new();

    // Self indicator
    if let Some(ref self_id) = panel.self_id {
        lines.push(Line::from(vec![
            Span::styled("◆ ", Style::default().fg(Color::Cyan)),
            Span::styled(format!("you ({})", self_id), Style::default().fg(Color::Cyan)),
        ]));
        lines.push(Line::from(""));
    }

    for (i, entry) in panel.entries.iter().enumerate() {
        let is_selected = i == panel.selected;

        let status_icon = Span::styled(format!("{} ", entry.status.icon()), Style::default().fg(entry.status.color()));

        let name_style = if is_selected {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        let mut spans = vec![
            if is_selected {
                Span::styled("▸ ", Style::default().fg(Color::Cyan))
            } else {
                Span::raw("  ")
            },
            status_icon,
            Span::styled(&entry.name, name_style),
        ];

        // Show short ID
        spans.push(Span::styled(format!(" ({})", &entry.short_id), Style::default().fg(Color::DarkGray)));

        lines.push(Line::from(spans));

        // Second line: capabilities summary
        let mut info_spans = vec![Span::raw("    ")];

        if entry.accepts_prompts {
            info_spans.push(Span::styled("⚙ prompts", Style::default().fg(Color::Green)));
            info_spans.push(Span::raw(" "));
        }

        if !entry.tags.is_empty() {
            let tag_str = entry.tags.join(",");
            info_spans.push(Span::styled(format!("#{}", tag_str), Style::default().fg(Color::Yellow)));
            info_spans.push(Span::raw(" "));
        }

        if let Some(ref last) = entry.last_seen {
            info_spans.push(Span::styled(last.clone(), Style::default().fg(Color::DarkGray)));
        }

        lines.push(Line::from(info_spans));
    }

    // Scroll if needed
    let total_lines = lines.len();
    let visible_height = area.height as usize;
    let scroll_offset = if panel.selected * 2 >= visible_height {
        (panel.selected * 2).saturating_sub(visible_height / 2)
    } else {
        0
    };

    frame.render_widget(Paragraph::new(lines).scroll((scroll_offset as u16, 0)).wrap(Wrap { trim: false }), area);
    render_scrollbar(frame, area, total_lines, scroll_offset, visible_height);
}

fn render_detail_view(frame: &mut Frame, panel: &PeersPanel, _theme: &Theme, area: Rect, _focused: bool) {
    let entry = match panel.selected_peer() {
        Some(e) => e,
        None => return,
    };

    let mut lines = vec![
        Line::from(vec![
            Span::styled(entry.status.icon(), Style::default().fg(entry.status.color())),
            Span::raw(" "),
            Span::styled(&entry.name, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Node ID: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&entry.node_id, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("  Status:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(entry.status.label(), Style::default().fg(entry.status.color())),
        ]),
    ];

    if let Some(ref last) = entry.last_seen {
        lines.push(Line::from(vec![
            Span::styled("  Seen:    ", Style::default().fg(Color::DarkGray)),
            Span::styled(last, Style::default().fg(Color::White)),
        ]));
    }

    if let Some(ref version) = entry.version {
        lines.push(Line::from(vec![
            Span::styled("  Version: ", Style::default().fg(Color::DarkGray)),
            Span::styled(version, Style::default().fg(Color::White)),
        ]));
    }

    if let Some(ref model) = entry.model {
        lines.push(Line::from(vec![
            Span::styled("  Model:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(model, Style::default().fg(Color::White)),
        ]));
    }

    lines.push(Line::from(vec![
        Span::styled("  Prompts: ", Style::default().fg(Color::DarkGray)),
        if entry.accepts_prompts {
            Span::styled("yes", Style::default().fg(Color::Green))
        } else {
            Span::styled("no", Style::default().fg(Color::Red))
        },
    ]));

    if entry.tool_count > 0 {
        lines.push(Line::from(vec![
            Span::styled("  Tools:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{} available", entry.tool_count), Style::default().fg(Color::White)),
        ]));
    }

    if !entry.tags.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("  Tags:    ", Style::default().fg(Color::DarkGray)),
            Span::styled(entry.tags.join(", "), Style::default().fg(Color::Yellow)),
        ]));
    }

    if !entry.agents.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("  Agents:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(entry.agents.join(", "), Style::default().fg(Color::White)),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("  Esc: back  d: delegate  p: probe", Style::default().fg(Color::DarkGray))));

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

/// Build a `PeerEntry` list from the peer registry, determining online/offline
/// based on `last_seen` recency.
pub fn entries_from_registry(
    peers: &[clanker_tui_types::PeerInfoView],
    stale_threshold: chrono::Duration,
) -> Vec<PeerEntry> {
    let now = chrono::Utc::now();

    let mut entries: Vec<PeerEntry> = peers
        .iter()
        .map(|peer| {
            let status = match peer.last_seen {
                Some(ts) if (now - ts) < stale_threshold => PeerStatus::Online,
                Some(_) => PeerStatus::Offline,
                None => PeerStatus::Unknown,
            };

            let last_seen = peer.last_seen.map(|ts| {
                let delta = now - ts;
                if delta.num_seconds() < 60 {
                    "just now".to_string()
                } else if delta.num_minutes() < 60 {
                    format!("{}m ago", delta.num_minutes())
                } else if delta.num_hours() < 24 {
                    format!("{}h ago", delta.num_hours())
                } else {
                    format!("{}d ago", delta.num_days())
                }
            });

            let short_id = if peer.node_id.len() > 12 {
                format!("{}…", &peer.node_id[..12])
            } else {
                peer.node_id.clone()
            };

            PeerEntry {
                name: peer.name.clone(),
                short_id,
                node_id: peer.node_id.clone(),
                status,
                tags: peer.capabilities.tags.clone(),
                agents: peer.capabilities.agents.clone(),
                accepts_prompts: peer.capabilities.accepts_prompts,
                model: None,
                version: peer.capabilities.version.clone(),
                last_seen,
                tool_count: peer.capabilities.tools.len(),
            }
        })
        .collect();

    // Sort: online first, then by name
    entries.sort_by(|a, b| {
        let status_ord = |s: &PeerStatus| match s {
            PeerStatus::Online => 0,
            PeerStatus::Probing => 1,
            PeerStatus::Unknown => 2,
            PeerStatus::Offline => 3,
        };
        status_ord(&a.status).cmp(&status_ord(&b.status)).then(a.name.cmp(&b.name))
    });

    entries
}
