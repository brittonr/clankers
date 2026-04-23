use std::io;
use std::io::IsTerminal;
use std::io::Write;

use chrono::Local;
use clankers_tui_types::BlockEntry;
use clankers_tui_types::ConversationBlock;
use clankers_tui_types::DisplayMessage;
use clankers_tui_types::MessageRole;
use rat_inline::InlineMarkdown;
use rat_inline::InlineRenderer;
use rat_inline::InlineText;
use rat_inline::InlineView;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use tracing::warn;

use crate::config::settings::Settings;
use crate::error::Error;
use crate::error::Result;

const SCROLLBACK_DEFAULT_WIDTH: u16 = 80;
const SCROLLBACK_MAX_BLOCKS: usize = 20;
const SCROLLBACK_MAX_TOOL_OUTPUT_LINES: usize = 10;
const SCROLLBACK_SEPARATOR_BAR_COUNT: usize = 4;
const SCROLLBACK_STYLE_RESET: &str = "\x1b[0m\n";
const SCROLLBACK_TIME_FORMAT: &str = "%H:%M:%S";
const SCROLLBACK_OMISSION_PREFIX: &str = "... ";
const SCROLLBACK_OMISSION_SUFFIX: &str = " earlier blocks omitted";
const SCROLLBACK_THINKING_PREFIX: &str = "Thinking... ";
const SCROLLBACK_TOOL_PREFIX: &str = "⚡ ";
const SCROLLBACK_UNKNOWN_TOOL_NAME: &str = "tool";
const SCROLLBACK_MULTI_LINE_SEPARATOR: &str = "\n";
const SCROLLBACK_MORE_LINES_PREFIX: &str = "... (";
const SCROLLBACK_MORE_LINES_SUFFIX: &str = " more lines)";
const SCROLLBACK_BLANK_LINE: &str = "";

struct ScrollbackSelection<'a> {
    omitted_block_count: usize,
    blocks: Vec<&'a ConversationBlock>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScrollbackLineKind {
    OmissionHeader,
    Separator,
    Prompt,
    AssistantMarkdown,
    Thinking,
    ToolCall,
    ToolResult,
    ToolResultError,
    System,
    User,
    Blank,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScrollbackLine {
    key: String,
    kind: ScrollbackLineKind,
    content: String,
}

pub fn finalize_terminal_and_scrollback(
    run_result: Result<()>,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    entries: &[BlockEntry],
    settings: &Settings,
) -> Result<()> {
    super::common::restore_terminal(terminal);
    let dump_result = dump_conversation_to_scrollback(entries, settings);
    merge_terminal_results(run_result, dump_result)
}

pub fn dump_conversation_to_scrollback(entries: &[BlockEntry], settings: &Settings) -> Result<()> {
    let stdout_is_terminal = io::stdout().is_terminal();
    if !scrollback_dump_enabled(settings.scrollback_on_exit, stdout_is_terminal) {
        return Ok(());
    }

    let render_width = scrollback_render_width(crossterm::terminal::size());
    let render_output = render_scrollback_output(entries, render_width);
    if render_output.is_empty() {
        return Ok(());
    }

    let mut stdout = io::stdout();
    write_scrollback_output(&mut stdout, &render_output)
}

fn merge_terminal_results(run_result: Result<()>, dump_result: Result<()>) -> Result<()> {
    match run_result {
        Ok(()) => dump_result,
        Err(run_error) => {
            if let Err(dump_error) = dump_result {
                warn!("scrollback dump failed after terminal restore: {dump_error}");
            }
            Err(run_error)
        }
    }
}

fn scrollback_dump_enabled(scrollback_on_exit: Option<bool>, stdout_is_terminal: bool) -> bool {
    if !stdout_is_terminal {
        return false;
    }
    scrollback_on_exit.unwrap_or(true)
}

fn scrollback_render_width(size_result: io::Result<(u16, u16)>) -> u16 {
    match size_result {
        Ok((width, _height)) => width,
        Err(_error) => SCROLLBACK_DEFAULT_WIDTH,
    }
}

fn render_scrollback_output(entries: &[BlockEntry], width: u16) -> Vec<u8> {
    let selection = select_scrollback_blocks(entries);
    if selection.blocks.is_empty() {
        return Vec::new();
    }

    let view = build_scrollback_view(&selection);
    let mut renderer = InlineRenderer::new(width);
    renderer.rebuild(view);
    renderer.render()
}

fn write_scrollback_output(writer: &mut dyn Write, render_output: &[u8]) -> Result<()> {
    writer.write_all(render_output).map_err(|source| Error::Io { source })?;
    writer.write_all(SCROLLBACK_STYLE_RESET.as_bytes()).map_err(|source| Error::Io { source })?;
    writer.flush().map_err(|source| Error::Io { source })
}

fn select_scrollback_blocks(entries: &[BlockEntry]) -> ScrollbackSelection<'_> {
    let blocks = entries
        .iter()
        .filter_map(|entry| match entry {
            BlockEntry::Conversation(block) => Some(block),
            BlockEntry::System(_message) => None,
        })
        .collect::<Vec<_>>();

    let total_block_count = blocks.len();
    let omitted_block_count = total_block_count.saturating_sub(SCROLLBACK_MAX_BLOCKS);
    let visible_blocks = if omitted_block_count == 0 {
        blocks
    } else {
        blocks[omitted_block_count..].to_vec()
    };

    ScrollbackSelection {
        omitted_block_count,
        blocks: visible_blocks,
    }
}

fn build_scrollback_view(selection: &ScrollbackSelection<'_>) -> InlineView {
    let lines = build_scrollback_lines(selection);
    let mut view = InlineView::new();
    for line in lines {
        view = append_scrollback_line(view, line);
    }
    view
}

fn build_scrollback_lines(selection: &ScrollbackSelection<'_>) -> Vec<ScrollbackLine> {
    let mut lines = Vec::new();

    if selection.omitted_block_count > 0 {
        lines.push(ScrollbackLine {
            key: "scrollback-omitted".to_string(),
            kind: ScrollbackLineKind::OmissionHeader,
            content: scrollback_omission_header(selection.omitted_block_count),
        });
        lines.push(ScrollbackLine {
            key: "scrollback-omitted-blank".to_string(),
            kind: ScrollbackLineKind::Blank,
            content: SCROLLBACK_BLANK_LINE.to_string(),
        });
    }

    let block_count = selection.blocks.len();
    for (block_index, block) in selection.blocks.iter().enumerate() {
        let block_key_prefix = format!("scrollback-block-{block_index}");
        lines.push(ScrollbackLine {
            key: format!("{block_key_prefix}-separator"),
            kind: ScrollbackLineKind::Separator,
            content: scrollback_block_separator(block),
        });
        lines.push(ScrollbackLine {
            key: format!("{block_key_prefix}-prompt"),
            kind: ScrollbackLineKind::Prompt,
            content: block.prompt.clone(),
        });

        for (message_index, message) in block.responses.iter().enumerate() {
            append_response_lines(&mut lines, &block_key_prefix, message_index, message);
        }

        let is_last_block = block_index + 1 == block_count;
        if !is_last_block {
            lines.push(ScrollbackLine {
                key: format!("{block_key_prefix}-blank"),
                kind: ScrollbackLineKind::Blank,
                content: SCROLLBACK_BLANK_LINE.to_string(),
            });
        }
    }

    lines
}

fn append_response_lines(
    lines: &mut Vec<ScrollbackLine>,
    block_key_prefix: &str,
    message_index: usize,
    message: &DisplayMessage,
) {
    let message_key_prefix = format!("{block_key_prefix}-message-{message_index}");
    match message.role {
        MessageRole::Assistant => {
            lines.push(ScrollbackLine {
                key: format!("{message_key_prefix}-assistant"),
                kind: ScrollbackLineKind::AssistantMarkdown,
                content: message.content.clone(),
            });
        }
        MessageRole::Thinking => {
            if let Some(summary) = thinking_summary(&message.content) {
                lines.push(ScrollbackLine {
                    key: format!("{message_key_prefix}-thinking"),
                    kind: ScrollbackLineKind::Thinking,
                    content: summary,
                });
            }
        }
        MessageRole::ToolCall => {
            lines.push(ScrollbackLine {
                key: format!("{message_key_prefix}-tool-call"),
                kind: ScrollbackLineKind::ToolCall,
                content: tool_call_header(message),
            });
        }
        MessageRole::ToolResult => {
            lines.push(ScrollbackLine {
                key: format!("{message_key_prefix}-tool-result"),
                kind: if message.is_error {
                    ScrollbackLineKind::ToolResultError
                } else {
                    ScrollbackLineKind::ToolResult
                },
                content: truncate_tool_output(&message.content, SCROLLBACK_MAX_TOOL_OUTPUT_LINES),
            });
        }
        MessageRole::System => {
            lines.push(ScrollbackLine {
                key: format!("{message_key_prefix}-system"),
                kind: ScrollbackLineKind::System,
                content: message.content.clone(),
            });
        }
        MessageRole::User => {
            lines.push(ScrollbackLine {
                key: format!("{message_key_prefix}-user"),
                kind: ScrollbackLineKind::User,
                content: message.content.clone(),
            });
        }
    }
}

fn append_scrollback_line(mut view: InlineView, line: ScrollbackLine) -> InlineView {
    match line.kind {
        ScrollbackLineKind::AssistantMarkdown => {
            view = view.keyed(line.key, InlineMarkdown::new(line.content));
        }
        ScrollbackLineKind::Prompt | ScrollbackLineKind::User => {
            view = view
                .keyed(line.key, InlineText::new(line.content).style(Style::default().add_modifier(Modifier::BOLD)));
        }
        ScrollbackLineKind::Thinking => {
            view = view.keyed(
                line.key,
                InlineText::new(line.content)
                    .style(Style::default().add_modifier(Modifier::DIM).add_modifier(Modifier::ITALIC)),
            );
        }
        ScrollbackLineKind::ToolCall => {
            view = view
                .keyed(line.key, InlineText::new(line.content).style(Style::default().add_modifier(Modifier::BOLD)));
        }
        ScrollbackLineKind::ToolResult => {
            view =
                view.keyed(line.key, InlineText::new(line.content).style(Style::default().add_modifier(Modifier::DIM)));
        }
        ScrollbackLineKind::ToolResultError | ScrollbackLineKind::System => {
            view = view.keyed(
                line.key,
                InlineText::new(line.content).style(Style::default().fg(Color::Red).add_modifier(Modifier::DIM)),
            );
        }
        ScrollbackLineKind::OmissionHeader | ScrollbackLineKind::Separator => {
            view =
                view.keyed(line.key, InlineText::new(line.content).style(Style::default().add_modifier(Modifier::DIM)));
        }
        ScrollbackLineKind::Blank => {
            view = view.keyed(line.key, InlineText::new(line.content));
        }
    }
    view
}

fn scrollback_omission_header(omitted_block_count: usize) -> String {
    format!("{SCROLLBACK_OMISSION_PREFIX}{omitted_block_count}{SCROLLBACK_OMISSION_SUFFIX}")
}

fn scrollback_block_separator(block: &ConversationBlock) -> String {
    let time = block.started_at.with_timezone(&Local).format(SCROLLBACK_TIME_FORMAT);
    let bars = "─".repeat(SCROLLBACK_SEPARATOR_BAR_COUNT);
    format!("{bars} {time} {bars}")
}

fn thinking_summary(content: &str) -> Option<String> {
    let first_line = content.lines().map(str::trim).find(|line| !line.is_empty())?;
    Some(format!("{SCROLLBACK_THINKING_PREFIX}{first_line}"))
}

fn tool_call_header(message: &DisplayMessage) -> String {
    let tool_name =
        message.tool_name.as_deref().filter(|name| !name.is_empty()).unwrap_or(SCROLLBACK_UNKNOWN_TOOL_NAME);
    format!("{SCROLLBACK_TOOL_PREFIX}{tool_name}")
}

fn truncate_tool_output(content: &str, max_lines: usize) -> String {
    let lines = content.lines().collect::<Vec<_>>();
    if lines.len() <= max_lines {
        return content.to_string();
    }

    let kept_lines = &lines[..max_lines];
    let omitted_line_count = lines.len() - max_lines;
    format!(
        "{}{SCROLLBACK_MULTI_LINE_SEPARATOR}{SCROLLBACK_MORE_LINES_PREFIX}{omitted_line_count}{SCROLLBACK_MORE_LINES_SUFFIX}",
        kept_lines.join(SCROLLBACK_MULTI_LINE_SEPARATOR)
    )
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;
    use chrono::Utc;
    use serde_json::json;

    use super::*;

    const FIXED_STARTED_AT: &str = "2026-04-22T12:34:56Z";
    const TEST_ERROR_MESSAGE: &str = "test error";
    const RENDER_TEST_PROMPT: &str = "render prompt";
    const RENDER_TEST_ASSISTANT: &str = "assistant reply";
    const RENDER_TEST_TOOL_NAME: &str = "bash";
    const RENDER_TEST_THINKING: &str = "think first\nthink later";
    const LONG_TOOL_RESULT: &str =
        "line-01\nline-02\nline-03\nline-04\nline-05\nline-06\nline-07\nline-08\nline-09\nline-10\nline-11\nline-12";
    const EXPECTED_TOOL_OMISSION: &str = "... (2 more lines)";
    const EXPECTED_OMISSION_HEADER: &str = "... 1 earlier blocks omitted";
    const INTERACTIVE_SOURCE: &str = include_str!("interactive.rs");
    const ATTACH_SOURCE: &str = include_str!("attach.rs");
    const AUTO_DAEMON_SOURCE: &str = include_str!("auto_daemon.rs");
    const SHARED_FINALIZER_CALL: &str = "scrollback_dump::finalize_terminal_and_scrollback";

    fn fixed_started_at() -> DateTime<Utc> {
        match DateTime::parse_from_rfc3339(FIXED_STARTED_AT) {
            Ok(parsed) => parsed.with_timezone(&Utc),
            Err(error) => panic!("fixed test timestamp must parse: {error}"),
        }
    }

    fn conversation_entry(prompt: &str, index: usize) -> BlockEntry {
        let mut block = ConversationBlock::new(index, prompt.to_string(), fixed_started_at());
        block.streaming = false;
        block.responses.push(DisplayMessage {
            role: MessageRole::Assistant,
            content: RENDER_TEST_ASSISTANT.to_string(),
            tool_name: None,
            tool_input: None,
            is_error: false,
            images: Vec::new(),
        });
        block.responses.push(DisplayMessage {
            role: MessageRole::Thinking,
            content: RENDER_TEST_THINKING.to_string(),
            tool_name: None,
            tool_input: None,
            is_error: false,
            images: Vec::new(),
        });
        block.responses.push(DisplayMessage {
            role: MessageRole::ToolCall,
            content: String::new(),
            tool_name: Some(RENDER_TEST_TOOL_NAME.to_string()),
            tool_input: Some(json!({"command": "ls"})),
            is_error: false,
            images: Vec::new(),
        });
        block.responses.push(DisplayMessage {
            role: MessageRole::ToolResult,
            content: LONG_TOOL_RESULT.to_string(),
            tool_name: None,
            tool_input: None,
            is_error: false,
            images: Vec::new(),
        });
        BlockEntry::Conversation(block)
    }

    fn settings_with_scrollback(scrollback_on_exit: Option<bool>) -> Settings {
        Settings {
            scrollback_on_exit,
            ..Settings::default()
        }
    }

    #[test]
    fn scrollback_dump_disabled_when_setting_false() {
        assert!(!scrollback_dump_enabled(Some(false), true));
    }

    #[test]
    fn scrollback_dump_enabled_when_setting_unset_and_stdout_is_terminal() {
        assert!(scrollback_dump_enabled(None, true));
    }

    #[test]
    fn scrollback_dump_enabled_when_setting_true_and_stdout_is_terminal() {
        assert!(scrollback_dump_enabled(Some(true), true));
    }

    #[test]
    fn scrollback_dump_disabled_when_stdout_is_not_terminal() {
        assert!(!scrollback_dump_enabled(None, false));
        assert!(!scrollback_dump_enabled(Some(true), false));
    }

    #[test]
    fn render_width_falls_back_when_terminal_size_lookup_fails() {
        let fallback = scrollback_render_width(Err(io::Error::other(TEST_ERROR_MESSAGE)));
        assert_eq!(fallback, SCROLLBACK_DEFAULT_WIDTH);
    }

    #[test]
    fn render_scrollback_output_contains_required_sections() {
        let entries = [conversation_entry(RENDER_TEST_PROMPT, 1)];
        let selection = select_scrollback_blocks(&entries);
        let lines = build_scrollback_lines(&selection);
        let expected_separator = scrollback_block_separator(selection.blocks[0]);
        let contents = lines.iter().map(|line| line.content.as_str()).collect::<Vec<_>>();
        assert!(contents.iter().any(|content| *content == expected_separator));
        assert!(contents.contains(&RENDER_TEST_PROMPT));
        assert!(contents.contains(&RENDER_TEST_ASSISTANT));
        assert!(contents.iter().any(|content| content.starts_with(SCROLLBACK_THINKING_PREFIX)));
        let expected_tool_header = format!("{SCROLLBACK_TOOL_PREFIX}{RENDER_TEST_TOOL_NAME}");
        assert!(contents.iter().any(|content| *content == expected_tool_header));
        assert!(contents.iter().any(|content| content.contains(EXPECTED_TOOL_OMISSION)));
    }

    #[test]
    fn render_scrollback_output_omits_earlier_blocks_for_long_sessions() {
        let entries = (0..=SCROLLBACK_MAX_BLOCKS)
            .map(|index| conversation_entry(&format!("prompt-{index}"), index + 1))
            .collect::<Vec<_>>();
        let selection = select_scrollback_blocks(&entries);
        let lines = build_scrollback_lines(&selection);
        let contents = lines.iter().map(|line| line.content.as_str()).collect::<Vec<_>>();
        assert!(contents.contains(&EXPECTED_OMISSION_HEADER));
        assert!(!contents.contains(&"prompt-0"));
        let last_visible_prompt = format!("prompt-{}", SCROLLBACK_MAX_BLOCKS);
        assert!(contents.iter().any(|content| *content == last_visible_prompt));
    }

    #[test]
    fn render_scrollback_output_keeps_all_blocks_for_short_sessions() {
        let first_prompt = "first prompt";
        let second_prompt = "second prompt";
        let entries = vec![
            conversation_entry(first_prompt, 1),
            conversation_entry(second_prompt, 2),
        ];
        let selection = select_scrollback_blocks(&entries);
        let lines = build_scrollback_lines(&selection);
        let contents = lines.iter().map(|line| line.content.as_str()).collect::<Vec<_>>();
        assert!(contents.contains(&first_prompt));
        assert!(contents.contains(&second_prompt));
        assert!(!contents.iter().any(|content| content.contains(SCROLLBACK_OMISSION_SUFFIX)));
    }

    #[test]
    fn thinking_summary_uses_first_non_empty_line() {
        let summary = thinking_summary("\nfirst line\nsecond line");
        assert_eq!(summary, Some(format!("{SCROLLBACK_THINKING_PREFIX}first line")));
    }

    #[test]
    fn thinking_summary_omits_blank_content() {
        assert_eq!(thinking_summary("\n   \n"), None);
    }

    #[test]
    fn write_scrollback_output_appends_style_reset_and_flushes() {
        let mut buffer = Vec::new();
        let write_result = write_scrollback_output(&mut buffer, RENDER_TEST_ASSISTANT.as_bytes());
        assert!(write_result.is_ok());
        let written = match String::from_utf8(buffer) {
            Ok(text) => text,
            Err(error) => panic!("buffer must stay utf8: {error}"),
        };
        assert!(written.starts_with(RENDER_TEST_ASSISTANT));
        assert!(written.ends_with(SCROLLBACK_STYLE_RESET));
    }

    #[test]
    fn merge_terminal_results_prefers_original_run_error() {
        let run_result = Err(Error::Agent {
            message: TEST_ERROR_MESSAGE.to_string(),
        });
        let dump_result = Err(Error::Io {
            source: io::Error::other("dump failed"),
        });
        let merged = merge_terminal_results(run_result, dump_result);
        match merged {
            Err(Error::Agent { message }) => assert_eq!(message, TEST_ERROR_MESSAGE),
            other => panic!("expected original run error, got {other:?}"),
        }
    }

    #[test]
    fn interactive_mode_uses_shared_scrollback_finalizer() {
        assert!(INTERACTIVE_SOURCE.contains(SHARED_FINALIZER_CALL));
    }

    #[test]
    fn attach_mode_uses_shared_scrollback_finalizer() {
        assert!(ATTACH_SOURCE.contains(SHARED_FINALIZER_CALL));
    }

    #[test]
    fn auto_daemon_mode_uses_shared_scrollback_finalizer() {
        assert!(AUTO_DAEMON_SOURCE.contains(SHARED_FINALIZER_CALL));
    }

    #[test]
    fn dump_conversation_to_scrollback_skips_when_setting_disabled() {
        let settings = settings_with_scrollback(Some(false));
        let dump_result = dump_conversation_to_scrollback(&[conversation_entry(RENDER_TEST_PROMPT, 1)], &settings);
        assert!(dump_result.is_ok());
    }
}
