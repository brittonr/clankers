//! Interactive TUI chat for clankers-router
//!
//! A minimal ratatui-based chat interface that streams completions
//! from the router in real-time.

use std::io;
use std::sync::Arc;

use clankers_router::Router;
use clankers_router::provider::CompletionRequest;
use clankers_router::streaming::ContentDelta;
use clankers_router::streaming::StreamEvent;
use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use crossterm::event::{self};
use crossterm::execute;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
use crossterm::terminal::disable_raw_mode;
use crossterm::terminal::enable_raw_mode;
use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Clear;
use ratatui::widgets::List;
use ratatui::widgets::ListItem;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Scrollbar;
use ratatui::widgets::ScrollbarOrientation;
use ratatui::widgets::ScrollbarState;
use tokio::sync::mpsc;

// ── App state ───────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct ChatMessage {
    role: Role,
    content: String,
}

#[derive(Clone, Debug, PartialEq)]
enum Role {
    User,
    Assistant,
    System,
    Error,
}

#[derive(PartialEq)]
enum AppMode {
    Normal,
    ModelSelector,
}

struct App {
    router: Arc<Router>,
    messages: Vec<ChatMessage>,
    input: String,
    /// Cursor position within the input string (byte offset)
    cursor: usize,
    model: String,
    model_names: Vec<String>,
    system_prompt: Option<String>,
    scroll_offset: u16,
    streaming: bool,
    mode: AppMode,
    model_filter: String,
    model_selected: usize,
    should_quit: bool,
    /// Total message area height from last render (for scroll bounds)
    content_height: u16,
    /// Visible message area height from last render
    visible_height: u16,
}

impl App {
    fn new(router: Arc<Router>, model: String, model_names: Vec<String>, system_prompt: Option<String>) -> Self {
        Self {
            router,
            messages: Vec::new(),
            input: String::new(),
            cursor: 0,
            model,
            model_names,
            system_prompt,
            scroll_offset: 0,
            streaming: false,
            mode: AppMode::Normal,
            model_filter: String::new(),
            model_selected: 0,
            should_quit: false,
            content_height: 0,
            visible_height: 0,
        }
    }

    fn filtered_models(&self) -> Vec<&str> {
        let f = self.model_filter.to_lowercase();
        self.model_names
            .iter()
            .filter(|m| f.is_empty() || m.to_lowercase().contains(&f))
            .map(|s| s.as_str())
            .collect()
    }

    fn scroll_to_bottom(&mut self) {
        self.scroll_offset = self.content_height.saturating_sub(self.visible_height);
    }

    fn build_messages_json(&self) -> Vec<serde_json::Value> {
        self.messages
            .iter()
            .filter(|m| m.role == Role::User || m.role == Role::Assistant)
            .map(|m| {
                let role = match m.role {
                    Role::User => "user",
                    Role::Assistant => "assistant",
                    _ => "user",
                };
                serde_json::json!({"role": role, "content": m.content})
            })
            .collect()
    }
}

// ── Entry point ─────────────────────────────────────────────────────────

pub async fn run(
    router: Router,
    model: String,
    model_names: Vec<String>,
    system_prompt: Option<String>,
) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(Arc::new(router), model, model_names, system_prompt);

    // Channel for streaming events from the completion task
    let (stream_tx, mut stream_rx) = mpsc::channel::<StreamEvent>(128);

    loop {
        // Draw
        terminal.draw(|f| draw(&app, f))?;

        // Poll for events with a short timeout so we can check stream_rx
        let timeout = std::time::Duration::from_millis(16);

        // Drain any stream events first
        while let Ok(event) = stream_rx.try_recv() {
            handle_stream_event(&mut app, event);
        }

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if app.mode == AppMode::ModelSelector {
                    handle_model_selector_key(&mut app, key);
                } else {
                    handle_key(&mut app, key, &stream_tx).await;
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

// ── Stream event handling ───────────────────────────────────────────────

fn handle_stream_event(app: &mut App, event: StreamEvent) {
    match event {
        StreamEvent::ContentBlockDelta {
            delta: ContentDelta::TextDelta { text },
            ..
        } => {
            if let Some(last) = app.messages.last_mut() {
                if last.role == Role::Assistant {
                    last.content.push_str(&text);
                    app.scroll_to_bottom();
                }
            }
        }
        StreamEvent::MessageStart { .. } => {
            app.messages.push(ChatMessage {
                role: Role::Assistant,
                content: String::new(),
            });
        }
        StreamEvent::MessageStop => {
            app.streaming = false;
            app.scroll_to_bottom();
        }
        StreamEvent::Error { error } => {
            app.messages.push(ChatMessage {
                role: Role::Error,
                content: error,
            });
            app.streaming = false;
        }
        _ => {}
    }
}

// ── Key handling ────────────────────────────────────────────────────────

async fn handle_key(app: &mut App, key: KeyEvent, stream_tx: &mpsc::Sender<StreamEvent>) {
    match (key.modifiers, key.code) {
        // Quit
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
            app.should_quit = true;
        }
        // Model selector
        (KeyModifiers::CONTROL, KeyCode::Char('m')) => {
            app.mode = AppMode::ModelSelector;
            app.model_filter.clear();
            app.model_selected = 0;
        }
        // Submit
        (_, KeyCode::Enter) if !key.modifiers.contains(KeyModifiers::ALT) => {
            if !app.input.trim().is_empty() && !app.streaming {
                submit_message(app, stream_tx.clone()).await;
            }
        }
        // Newline
        (KeyModifiers::ALT, KeyCode::Enter) => {
            app.input.insert(app.cursor, '\n');
            app.cursor += 1;
        }
        // Scroll
        (KeyModifiers::NONE, KeyCode::PageUp) => {
            app.scroll_offset = app.scroll_offset.saturating_sub(10);
        }
        (KeyModifiers::NONE, KeyCode::PageDown) => {
            app.scroll_offset = (app.scroll_offset + 10).min(app.content_height.saturating_sub(app.visible_height));
        }
        (KeyModifiers::CONTROL, KeyCode::Char('u')) => {
            app.scroll_offset = app.scroll_offset.saturating_sub(5);
        }
        (KeyModifiers::CONTROL, KeyCode::Char('d')) => {
            app.scroll_offset = (app.scroll_offset + 5).min(app.content_height.saturating_sub(app.visible_height));
        }
        // Input editing
        (_, KeyCode::Backspace) => {
            if app.cursor > 0 {
                // Remove the character before cursor
                let prev = app.input[..app.cursor].char_indices().next_back().map(|(i, _)| i).unwrap_or(0);
                app.input.replace_range(prev..app.cursor, "");
                app.cursor = prev;
            }
        }
        (_, KeyCode::Delete) => {
            if app.cursor < app.input.len() {
                let next = app.input[app.cursor..]
                    .char_indices()
                    .nth(1)
                    .map(|(i, _)| app.cursor + i)
                    .unwrap_or(app.input.len());
                app.input.replace_range(app.cursor..next, "");
            }
        }
        (_, KeyCode::Left) => {
            if app.cursor > 0 {
                app.cursor = app.input[..app.cursor].char_indices().next_back().map(|(i, _)| i).unwrap_or(0);
            }
        }
        (_, KeyCode::Right) => {
            if app.cursor < app.input.len() {
                app.cursor = app.input[app.cursor..]
                    .char_indices()
                    .nth(1)
                    .map(|(i, _)| app.cursor + i)
                    .unwrap_or(app.input.len());
            }
        }
        (KeyModifiers::CONTROL, KeyCode::Char('a')) | (_, KeyCode::Home) => {
            app.cursor = 0;
        }
        (KeyModifiers::CONTROL, KeyCode::Char('e')) | (_, KeyCode::End) => {
            app.cursor = app.input.len();
        }
        (KeyModifiers::CONTROL, KeyCode::Char('k')) => {
            app.input.truncate(app.cursor);
        }
        (KeyModifiers::CONTROL, KeyCode::Char('l')) => {
            app.messages.clear();
            app.scroll_offset = 0;
        }
        (_, KeyCode::Char(c)) => {
            app.input.insert(app.cursor, c);
            app.cursor += c.len_utf8();
        }
        _ => {}
    }
}

fn handle_model_selector_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        KeyCode::Enter => {
            let filtered = app.filtered_models();
            if let Some(model) = filtered.get(app.model_selected) {
                app.model = model.to_string();
                app.messages.push(ChatMessage {
                    role: Role::System,
                    content: format!("Switched to model: {}", app.model),
                });
            }
            app.mode = AppMode::Normal;
        }
        KeyCode::Up => {
            app.model_selected = app.model_selected.saturating_sub(1);
        }
        KeyCode::Down => {
            let max = app.filtered_models().len().saturating_sub(1);
            app.model_selected = (app.model_selected + 1).min(max);
        }
        KeyCode::Backspace => {
            app.model_filter.pop();
            app.model_selected = 0;
        }
        KeyCode::Char(c) => {
            app.model_filter.push(c);
            app.model_selected = 0;
        }
        _ => {}
    }
}

// ── Submit message ──────────────────────────────────────────────────────

async fn submit_message(app: &mut App, stream_tx: mpsc::Sender<StreamEvent>) {
    let user_text = app.input.trim().to_string();
    app.input.clear();
    app.cursor = 0;

    // Handle slash commands
    if user_text.starts_with('/') {
        handle_slash_command(app, &user_text);
        return;
    }

    app.messages.push(ChatMessage {
        role: Role::User,
        content: user_text,
    });

    let request = CompletionRequest {
        model: app.model.clone(),
        messages: app.build_messages_json(),
        system_prompt: app.system_prompt.clone(),
        max_tokens: None,
        temperature: None,
        tools: vec![],
        thinking: None,
        extra_params: Default::default(),
    };

    app.streaming = true;
    app.scroll_to_bottom();

    let tx = stream_tx.clone();
    let router = Arc::clone(&app.router);

    tokio::spawn(async move {
        if let Err(e) = router.complete(request, tx.clone()).await {
            let _ = tx.send(StreamEvent::Error { error: e.to_string() }).await;
        }
    });
}

fn handle_slash_command(app: &mut App, cmd: &str) {
    let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
    match parts[0] {
        "/model" => {
            if parts.len() > 1 {
                app.model = parts[1].to_string();
                app.messages.push(ChatMessage {
                    role: Role::System,
                    content: format!("Switched to model: {}", app.model),
                });
            } else {
                app.mode = AppMode::ModelSelector;
                app.model_filter.clear();
                app.model_selected = 0;
            }
        }
        "/clear" => {
            app.messages.clear();
            app.scroll_offset = 0;
        }
        "/system" => {
            if parts.len() > 1 {
                app.system_prompt = Some(parts[1].to_string());
                app.messages.push(ChatMessage {
                    role: Role::System,
                    content: format!("System prompt updated."),
                });
            } else {
                let current = app.system_prompt.as_deref().unwrap_or("(none)");
                app.messages.push(ChatMessage {
                    role: Role::System,
                    content: format!("Current system prompt: {}", current),
                });
            }
        }
        "/help" => {
            app.messages.push(ChatMessage {
                role: Role::System,
                content: [
                    "Commands:",
                    "  /model [name]  — switch model (or open picker)",
                    "  /system [text] — set/show system prompt",
                    "  /clear         — clear chat history",
                    "  /help          — show this help",
                    "",
                    "Keys:",
                    "  Enter       — send message",
                    "  Alt+Enter   — newline",
                    "  Ctrl+M      — model picker",
                    "  Ctrl+L      — clear screen",
                    "  PgUp/PgDn   — scroll",
                    "  Ctrl+C      — quit",
                ]
                .join("\n"),
            });
        }
        _ => {
            app.messages.push(ChatMessage {
                role: Role::Error,
                content: format!("Unknown command: {}. Type /help for help.", parts[0]),
            });
        }
    }
}

// ── Drawing ─────────────────────────────────────────────────────────────

fn draw(app: &App, frame: &mut Frame) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Min(5),    // messages
            Constraint::Length(3), // input
            Constraint::Length(1), // status bar
        ])
        .split(area);

    draw_header(app, frame, chunks[0]);
    draw_messages(app, frame, chunks[1]);
    draw_input(app, frame, chunks[2]);
    draw_status_bar(app, frame, chunks[3]);

    // Overlays
    if app.mode == AppMode::ModelSelector {
        draw_model_selector(app, frame, area);
    }
}

fn draw_header(app: &App, frame: &mut Frame, area: Rect) {
    let header = Paragraph::new(Line::from(vec![
        Span::styled(" clankers-router ", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {} ", app.model), Style::default().fg(Color::Cyan)),
        Span::styled(" Ctrl+C quit  Enter send  Ctrl+M model  /help ", Style::default().fg(Color::DarkGray)),
    ]));
    frame.render_widget(header, area);
}

fn draw_messages(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::LEFT | Borders::RIGHT)
        .border_style(Style::default().fg(Color::Rgb(50, 50, 50)));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.messages.is_empty() {
        let welcome = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Welcome to clankers-router chat!",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled("  Type a message and press Enter to send.", Style::default().fg(Color::DarkGray))),
            Line::from(Span::styled("  Type /help for commands.", Style::default().fg(Color::DarkGray))),
        ]);
        frame.render_widget(welcome, inner);
        return;
    }

    // Build all lines
    let mut lines: Vec<Line> = Vec::new();
    for msg in &app.messages {
        let (prefix, style) = match msg.role {
            Role::User => ("You", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Role::Assistant => ("AI", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Role::System => ("ℹ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Role::Error => ("✗", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        };

        let content_style = match msg.role {
            Role::User => Style::default().fg(Color::White),
            Role::Assistant => Style::default().fg(Color::Rgb(210, 210, 210)),
            Role::System => Style::default().fg(Color::Yellow),
            Role::Error => Style::default().fg(Color::Red),
        };

        // Prefix line
        lines.push(Line::from(Span::styled(format!(" {} ", prefix), style)));

        // Content lines (wrap manually)
        for text_line in msg.content.lines() {
            // Simple word-wrap at inner.width
            let max_w = inner.width.saturating_sub(2) as usize;
            if max_w == 0 {
                lines.push(Line::from(Span::styled(format!("  {}", text_line), content_style)));
            } else {
                let mut remaining = text_line;
                while !remaining.is_empty() {
                    let take = if remaining.len() <= max_w {
                        remaining.len()
                    } else {
                        // Try to break at a space
                        remaining[..max_w].rfind(' ').map(|i| i + 1).unwrap_or(max_w)
                    };
                    lines.push(Line::from(Span::styled(format!("  {}", &remaining[..take]), content_style)));
                    remaining = &remaining[take..];
                }
            }
        }

        // Blank line between messages
        lines.push(Line::from(""));
    }

    // Streaming indicator
    if app.streaming {
        lines.push(Line::from(Span::styled(
            "  ⏳ streaming...",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::SLOW_BLINK),
        )));
    }

    let total_lines = lines.len() as u16;

    // Auto-scroll to bottom
    let scroll = if total_lines > inner.height {
        total_lines - inner.height
    } else {
        0
    };
    let scroll = if app.scroll_offset < scroll {
        // User scrolled up — honour their offset, but clamp
        app.scroll_offset.min(scroll)
    } else {
        scroll
    };

    let paragraph = Paragraph::new(lines).scroll((scroll, 0));
    frame.render_widget(paragraph, inner);

    // Scrollbar
    if total_lines > inner.height {
        let mut scrollbar_state = ScrollbarState::new(total_lines as usize)
            .position(scroll as usize)
            .viewport_content_length(inner.height as usize);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight).style(Style::default().fg(Color::DarkGray)),
            inner,
            &mut scrollbar_state,
        );
    }
}

fn draw_input(app: &App, frame: &mut Frame, area: Rect) {
    let border_color = if app.streaming { Color::DarkGray } else { Color::Blue };

    let title = if app.streaming { " streaming... " } else { " Message " };

    let block = Block::default().title(title).borders(Borders::ALL).border_style(Style::default().fg(border_color));

    // Show input with cursor
    let before_cursor = &app.input[..app.cursor];
    let cursor_char = app.input.get(app.cursor..).and_then(|s| s.chars().next()).unwrap_or(' ');
    let after_cursor_start = app.cursor + cursor_char.len_utf8().min(app.input.len() - app.cursor);
    let after_cursor = if after_cursor_start <= app.input.len() {
        &app.input[after_cursor_start..]
    } else {
        ""
    };

    let input_line = Line::from(vec![
        Span::raw(before_cursor),
        Span::styled(cursor_char.to_string(), Style::default().bg(Color::White).fg(Color::Black)),
        Span::raw(after_cursor),
    ]);

    let paragraph = Paragraph::new(input_line).block(block);
    frame.render_widget(paragraph, area);
}

fn draw_status_bar(app: &App, frame: &mut Frame, area: Rect) {
    let msg_count = app.messages.iter().filter(|m| m.role == Role::User).count();

    let status = Line::from(vec![
        Span::styled(
            format!(" {} ", app.model),
            Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            if app.streaming { " STREAMING " } else { " READY " },
            Style::default()
                .fg(Color::Black)
                .bg(if app.streaming { Color::Yellow } else { Color::Green })
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {} messages | {} turns ", app.messages.len(), msg_count),
            Style::default().fg(Color::DarkGray),
        ),
    ]);

    frame.render_widget(Paragraph::new(status), area);
}

fn draw_model_selector(app: &App, frame: &mut Frame, area: Rect) {
    let width = 55.min(area.width.saturating_sub(4));
    let height = 18.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Select Model (↑↓ Enter Esc) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    if inner.height < 2 {
        return;
    }

    // Filter input
    let filter_line = Line::from(vec![
        Span::styled("Filter: ", Style::default().fg(Color::DarkGray)),
        Span::styled(&app.model_filter, Style::default().fg(Color::White)),
        Span::styled("█", Style::default().fg(Color::White).add_modifier(Modifier::SLOW_BLINK)),
    ]);
    let filter_area = Rect::new(inner.x, inner.y, inner.width, 1);
    frame.render_widget(Paragraph::new(filter_line), filter_area);

    // Separator
    let sep_area = Rect::new(inner.x, inner.y + 1, inner.width, 1);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "─".repeat(inner.width as usize),
            Style::default().fg(Color::Rgb(60, 60, 60)),
        ))),
        sep_area,
    );

    // Model list
    let list_area = Rect::new(inner.x, inner.y + 2, inner.width, inner.height.saturating_sub(2));
    let filtered = app.filtered_models();

    let items: Vec<ListItem> = filtered
        .iter()
        .enumerate()
        .map(|(i, model)| {
            let is_current = *model == app.model;
            let marker = if is_current { "▸ " } else { "  " };

            let style = if i == app.model_selected {
                Style::default().bg(Color::Rgb(50, 50, 80)).fg(Color::White)
            } else if is_current {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::Rgb(200, 200, 200))
            };

            ListItem::new(Span::styled(format!("{}{}", marker, model), style))
        })
        .collect();

    frame.render_widget(List::new(items), list_area);
}
