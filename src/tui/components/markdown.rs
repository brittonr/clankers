//! Inline markdown rendering for chat messages
//!
//! Converts markdown text into styled ratatui `Span`s for display inside
//! conversation blocks. Supports code blocks (with language labels),
//! headings, bullet/numbered lists, bold, italic, bold-italic, inline code,
//! links, blockquotes, and horizontal rules.

use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;

use crate::tui::theme::Theme;

/// Colors/styles used by the markdown renderer.
#[derive(Debug, Clone)]
pub struct MarkdownStyle {
    /// Base text style (passed by caller — usually `assistant_msg` color)
    pub base: Style,
    /// Fenced code block content
    pub code_block: Style,
    /// Code block language label / fence line
    pub code_fence: Style,
    /// Inline `code`
    pub inline_code: Style,
    /// **bold**
    pub bold: Style,
    /// *italic*
    pub italic: Style,
    /// ***bold italic***
    pub bold_italic: Style,
    /// # Heading
    pub heading: Style,
    /// ## Subheading
    pub subheading: Style,
    /// List bullet / number
    pub list_marker: Style,
    /// > blockquote
    pub blockquote: Style,
    /// Horizontal rule
    pub hrule: Style,
}

impl MarkdownStyle {
    /// Build a markdown style from a `Theme`, using `base` as the text color.
    pub fn from_theme(theme: &Theme, base: Style) -> Self {
        Self {
            base,
            code_block: Style::default().fg(theme.md_code_block),
            code_fence: Style::default().fg(theme.md_code_fence),
            inline_code: Style::default().fg(theme.md_inline_code_fg).bg(theme.md_inline_code_bg),
            bold: base.add_modifier(Modifier::BOLD),
            italic: base.add_modifier(Modifier::ITALIC),
            bold_italic: base.add_modifier(Modifier::BOLD | Modifier::ITALIC),
            heading: base.add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            subheading: base.add_modifier(Modifier::BOLD),
            list_marker: Style::default().fg(theme.md_list_marker),
            blockquote: Style::default().fg(theme.md_blockquote).add_modifier(Modifier::ITALIC),
            hrule: Style::default().fg(theme.md_hrule),
        }
    }

    /// Build a markdown style from a base (text) style with default colors.
    /// Useful for tests or contexts where no Theme is available.
    pub fn from_base(base: Style) -> Self {
        Self {
            base,
            code_block: Style::default().fg(Color::Rgb(180, 220, 140)),
            code_fence: Style::default().fg(Color::Rgb(100, 100, 100)),
            inline_code: Style::default().fg(Color::Rgb(230, 190, 80)).bg(Color::Rgb(45, 45, 45)),
            bold: base.add_modifier(Modifier::BOLD),
            italic: base.add_modifier(Modifier::ITALIC),
            bold_italic: base.add_modifier(Modifier::BOLD | Modifier::ITALIC),
            heading: base.add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            subheading: base.add_modifier(Modifier::BOLD),
            list_marker: Style::default().fg(Color::Rgb(100, 100, 100)),
            blockquote: Style::default().fg(Color::Rgb(160, 160, 160)).add_modifier(Modifier::ITALIC),
            hrule: Style::default().fg(Color::Rgb(80, 80, 80)),
        }
    }
}

// ── Block-level rendering helpers ────────────────────────────────────────────

/// Try to render a code fence line (opening or closing ```).
/// Returns Some(Line) if this is a fence, updating the in_code_block state.
fn try_render_code_fence(
    line: &str,
    in_code_block: &mut bool,
    code_lang: &mut String,
    style: &MarkdownStyle,
) -> Option<Line<'static>> {
    let rest = line.strip_prefix("```")?;

    if !*in_code_block {
        // Opening fence
        *in_code_block = true;
        *code_lang = rest.trim().to_string();
        let label = if code_lang.is_empty() {
            "───".to_string()
        } else {
            format!("─── {} ", code_lang)
        };
        Some(Line::from(Span::styled(label, style.code_fence)))
    } else {
        // Closing fence
        *in_code_block = false;
        code_lang.clear();
        Some(Line::from(Span::styled("───", style.code_fence)))
    }
}

/// Render a line of code inside a code block (with optional syntax highlighting).
fn render_code_block_line(
    line: &str,
    code_lang: &str,
    style: &MarkdownStyle,
    highlighter: &dyn clankers_tui_types::SyntaxHighlighter,
) -> Line<'static> {
    if !code_lang.is_empty() {
        // Syntax-highlighted code line
        let mut spans: Vec<Span<'static>> = vec![Span::raw("  ".to_string())];
        let hl_spans: Vec<Span<'static>> = highlighter.highlight(line, code_lang)
            .into_iter()
            .map(|s| {
                let style = match s.fg {
                    Some((r, g, b)) => Style::default().fg(Color::Rgb(r, g, b)),
                    None => Style::default(),
                };
                Span::styled(s.text, style)
            })
            .collect();
        if hl_spans.iter().all(|s| matches!(s.style.fg, None | Some(Color::Reset))) {
            // Highlighter didn't produce colors — fall back to code_block style
            spans.push(Span::styled(line.to_string(), style.code_block));
        } else {
            spans.extend(hl_spans);
        }
        Line::from(spans)
    } else {
        Line::from(Span::styled(format!("  {}", line), style.code_block))
    }
}

/// Try to render a horizontal rule (---, ***, ___).
fn try_render_horizontal_rule(line: &str, style: &MarkdownStyle) -> Option<Line<'static>> {
    let trimmed = line.trim();
    if trimmed.len() >= 3
        && (trimmed.chars().all(|c| c == '-' || c == ' ')
            || trimmed.chars().all(|c| c == '*' || c == ' ')
            || trimmed.chars().all(|c| c == '_' || c == ' '))
        && trimmed.chars().filter(|c| !c.is_whitespace()).count() >= 3
    {
        Some(Line::from(Span::styled("────────────", style.hrule)))
    } else {
        None
    }
}

/// Try to render a heading (# H1, ## H2, ### H3, etc.).
fn try_render_heading(line: &str, style: &MarkdownStyle) -> Option<Line<'static>> {
    if let Some(content) = line.strip_prefix("# ") {
        Some(Line::from(Span::styled(content.trim().to_string(), style.heading)))
    } else if let Some(content) = line.strip_prefix("## ") {
        Some(Line::from(Span::styled(content.trim().to_string(), style.subheading)))
    } else if line.starts_with("### ") || line.starts_with("#### ") {
        let content = line.trim_start_matches('#').trim();
        Some(Line::from(Span::styled(content.to_string(), style.subheading)))
    } else {
        None
    }
}

/// Try to render a blockquote (> text).
fn try_render_blockquote(line: &str, style: &MarkdownStyle) -> Option<Line<'static>> {
    if let Some(rest) = line.strip_prefix("> ") {
        let mut spans = vec![Span::styled("▎ ", style.blockquote)];
        spans.extend(render_inline_spans(rest, style));
        Some(Line::from(spans))
    } else if line == ">" {
        Some(Line::from(Span::styled("▎", style.blockquote)))
    } else {
        None
    }
}

/// Try to render a list item (unordered: `- ` or `* `, ordered: `1. `).
fn try_render_list_item(line: &str, style: &MarkdownStyle) -> Option<Line<'static>> {
    // Try unordered list first
    if let Some(content) = strip_list_bullet(line) {
        let indent = leading_spaces(line);
        let indent_str: String = " ".repeat(indent);
        let mut spans = vec![Span::raw(indent_str), Span::styled("• ", style.list_marker)];
        spans.extend(render_inline_spans(content, style));
        return Some(Line::from(spans));
    }

    // Try ordered list
    if let Some((num, content)) = strip_ordered_list(line) {
        let indent = leading_spaces(line);
        let indent_str: String = " ".repeat(indent);
        let mut spans = vec![
            Span::raw(indent_str),
            Span::styled(format!("{}. ", num), style.list_marker),
        ];
        spans.extend(render_inline_spans(content, style));
        return Some(Line::from(spans));
    }

    None
}

/// Render markdown text into a list of ratatui Lines.
///
/// Each returned `Line` corresponds to one visual line of output.
/// The caller is responsible for adding any border prefix (e.g. `"│ "`)
/// before each line.
pub fn render_markdown(
    text: &str,
    style: &MarkdownStyle,
    highlighter: &dyn clankers_tui_types::SyntaxHighlighter,
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut in_code_block = false;
    let mut code_lang = String::new();

    for raw_line in text.lines() {
        // ── Code fences ──────────────────────────────
        if let Some(fence_line) = try_render_code_fence(raw_line, &mut in_code_block, &mut code_lang, style) {
            lines.push(fence_line);
            continue;
        }

        if in_code_block {
            lines.push(render_code_block_line(raw_line, &code_lang, style, highlighter));
            continue;
        }

        // ── Horizontal rules ─────────────────────────
        if let Some(hrule) = try_render_horizontal_rule(raw_line, style) {
            lines.push(hrule);
            continue;
        }

        // ── Headings ─────────────────────────────────
        if let Some(heading) = try_render_heading(raw_line, style) {
            lines.push(heading);
            continue;
        }

        // ── Blockquotes ──────────────────────────────
        if let Some(blockquote) = try_render_blockquote(raw_line, style) {
            lines.push(blockquote);
            continue;
        }

        // ── Lists ────────────────────────────────────
        if let Some(list_item) = try_render_list_item(raw_line, style) {
            lines.push(list_item);
            continue;
        }

        // ── Regular paragraph text ───────────────────
        let spans = render_inline_spans(raw_line, style);
        lines.push(Line::from(spans));
    }

    lines
}

/// Count leading space characters.
fn leading_spaces(s: &str) -> usize {
    s.len() - s.trim_start_matches(' ').len()
}

/// Strip an unordered list bullet (`- ` or `* `) allowing leading whitespace.
/// Returns the content after the bullet, or `None`.
fn strip_list_bullet(line: &str) -> Option<&str> {
    let trimmed = line.trim_start_matches(' ');
    if let Some(rest) = trimmed.strip_prefix("- ") {
        Some(rest)
    } else {
        trimmed.strip_prefix("* ")
    }
}

/// Strip an ordered list marker (`1. `, `2. `, etc.) allowing leading whitespace.
/// Returns `(number, content)` or `None`.
fn strip_ordered_list(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.trim_start_matches(' ');
    let dot_pos = trimmed.find(". ")?;
    let num_part = &trimmed[..dot_pos];
    if num_part.chars().all(|c| c.is_ascii_digit()) && !num_part.is_empty() {
        Some((num_part, &trimmed[dot_pos + 2..]))
    } else {
        None
    }
}

/// Render inline markdown into a Vec of Spans.
///
/// Handles: `code`, ***bold italic***, **bold**, __bold__, *italic*, _italic_,
/// ~~strikethrough~~, and [links](url).
/// Processes markers from left to right, using a simple state machine.
fn render_inline_spans(text: &str, style: &MarkdownStyle) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut buf = String::new();

    while i < len {
        // Try inline code first
        if let Some(new_i) = try_render_inline_code(&chars, i, &mut buf, &mut spans, style) {
            i = new_i;
            continue;
        }

        // Try emphasis/formatting (strikethrough, bold, italic)
        if let Some(new_i) = try_render_emphasis(&chars, i, &mut buf, &mut spans, style) {
            i = new_i;
            continue;
        }

        // Try links
        if let Some(new_i) = try_render_link(&chars, i, &mut buf, &mut spans, style) {
            i = new_i;
            continue;
        }

        // Regular character
        buf.push(chars[i]);
        i += 1;
    }

    // Flush remaining text
    flush_buf(&mut buf, &mut spans, style.base);

    // If empty input, return at least one empty span so the line exists
    if spans.is_empty() {
        spans.push(Span::raw(String::new()));
    }

    spans
}

// ── Inline rendering helpers ─────────────────────────────────────────────────

/// Flush the accumulated plain-text buffer into a styled span.
fn flush_buf(buf: &mut String, spans: &mut Vec<Span<'static>>, sty: Style) {
    if !buf.is_empty() {
        spans.push(Span::styled(std::mem::take(buf), sty));
    }
}

/// Find the closing delimiter starting at position `from` in `chars`.
/// Returns the index of the first char of the closing delimiter, or `None`.
fn find_closing(chars: &[char], from: usize, delim: &[char]) -> Option<usize> {
    let dlen = delim.len();
    if dlen == 0 {
        return None;
    }
    let mut j = from;
    while j + dlen <= chars.len() {
        if chars[j..j + dlen] == *delim {
            return Some(j);
        }
        j += 1;
    }
    None
}

/// Try to render inline code (`code`).
/// Returns the new position if successful, None otherwise.
fn try_render_inline_code(
    chars: &[char],
    i: usize,
    buf: &mut String,
    spans: &mut Vec<Span<'static>>,
    style: &MarkdownStyle,
) -> Option<usize> {
    if chars[i] != '`' {
        return None;
    }

    flush_buf(buf, spans, style.base);
    let mut pos = i + 1;
    let start = pos;
    while pos < chars.len() && chars[pos] != '`' {
        pos += 1;
    }
    let code: String = chars[start..pos].iter().collect();
    spans.push(Span::styled(format!(" {} ", code), style.inline_code));
    if pos < chars.len() {
        pos += 1; // Skip closing backtick
    }
    Some(pos)
}

/// Try to render emphasis markers (strikethrough, bold-italic, bold, italic).
/// Returns the new position if successful, None otherwise.
fn try_render_emphasis(
    chars: &[char],
    i: usize,
    buf: &mut String,
    spans: &mut Vec<Span<'static>>,
    style: &MarkdownStyle,
) -> Option<usize> {
    let len = chars.len();

    // ── Strikethrough (~~text~~) ─────────────────
    if i + 1 < len
        && chars[i] == '~'
        && chars[i + 1] == '~'
        && let Some(close) = find_closing(chars, i + 2, &['~', '~'])
    {
        flush_buf(buf, spans, style.base);
        let inner: String = chars[i + 2..close].iter().collect();
        spans.push(Span::styled(inner, style.base.add_modifier(Modifier::CROSSED_OUT)));
        return Some(close + 2);
    }

    // ── Bold-italic (***text*** or ___text___) ───
    if i + 2 < len
        && chars[i] == '*'
        && chars[i + 1] == '*'
        && chars[i + 2] == '*'
        && let Some(close) = find_closing(chars, i + 3, &['*', '*', '*'])
    {
        flush_buf(buf, spans, style.base);
        let inner: String = chars[i + 3..close].iter().collect();
        spans.push(Span::styled(inner, style.bold_italic));
        return Some(close + 3);
    }
    if i + 2 < len
        && chars[i] == '_'
        && chars[i + 1] == '_'
        && chars[i + 2] == '_'
        && let Some(close) = find_closing(chars, i + 3, &['_', '_', '_'])
    {
        flush_buf(buf, spans, style.base);
        let inner: String = chars[i + 3..close].iter().collect();
        spans.push(Span::styled(inner, style.bold_italic));
        return Some(close + 3);
    }

    // ── Bold (**text** or __text__) ──────────────
    if i + 1 < len
        && chars[i] == '*'
        && chars[i + 1] == '*'
        && let Some(close) = find_closing(chars, i + 2, &['*', '*'])
    {
        flush_buf(buf, spans, style.base);
        let inner: String = chars[i + 2..close].iter().collect();
        spans.push(Span::styled(inner, style.bold));
        return Some(close + 2);
    }
    if i + 1 < len
        && chars[i] == '_'
        && chars[i + 1] == '_'
        && let Some(close) = find_closing(chars, i + 2, &['_', '_'])
    {
        flush_buf(buf, spans, style.base);
        let inner: String = chars[i + 2..close].iter().collect();
        spans.push(Span::styled(inner, style.bold));
        return Some(close + 2);
    }

    // ── Italic (*text* or _text_) ────────────────
    if chars[i] == '*'
        && (i + 1 >= len || chars[i + 1] != '*')
        && let Some(close) = find_closing(chars, i + 1, &['*'])
    {
        flush_buf(buf, spans, style.base);
        let inner: String = chars[i + 1..close].iter().collect();
        spans.push(Span::styled(inner, style.italic));
        return Some(close + 1);
    }
    if chars[i] == '_'
        && (i + 1 >= len || chars[i + 1] != '_')
        && let Some(close) = find_closing(chars, i + 1, &['_'])
    {
        // Only treat as italic if the underscore is at a word boundary
        // (not in the middle of a_word_like_this)
        let at_start = i == 0 || !chars[i - 1].is_alphanumeric();
        let at_end = close + 1 >= len || !chars[close + 1].is_alphanumeric();
        if at_start && at_end {
            flush_buf(buf, spans, style.base);
            let inner: String = chars[i + 1..close].iter().collect();
            spans.push(Span::styled(inner, style.italic));
            return Some(close + 1);
        }
    }

    None
}

/// Try to render a markdown link ([text](url)).
/// Returns the new position if successful, None otherwise.
fn try_render_link(
    chars: &[char],
    i: usize,
    buf: &mut String,
    spans: &mut Vec<Span<'static>>,
    style: &MarkdownStyle,
) -> Option<usize> {
    if chars[i] != '[' {
        return None;
    }

    let len = chars.len();
    let bracket_start = i + 1;
    let mut j = bracket_start;
    while j < len && chars[j] != ']' {
        j += 1;
    }
    if j + 1 < len && chars[j] == ']' && chars[j + 1] == '(' {
        let link_text: String = chars[bracket_start..j].iter().collect();
        let paren_start = j + 2;
        let mut k = paren_start;
        while k < len && chars[k] != ')' {
            k += 1;
        }
        if k < len {
            flush_buf(buf, spans, style.base);
            spans.push(Span::styled(link_text, style.base.add_modifier(Modifier::UNDERLINED)));
            return Some(k + 1);
        }
    }

    // Not a valid link
    None
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_style() -> MarkdownStyle {
        MarkdownStyle::from_base(Style::default())
    }

    /// Helper: render markdown and return plain text lines (no styles).
    fn plain_lines(text: &str) -> Vec<String> {
        let style = test_style();
        render_markdown(text, &style, &clankers_tui_types::PlainHighlighter)
            .iter()
            .map(|line| line.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
            .collect()
    }

    /// Helper: render inline spans and return the text content of each span.
    fn inline_texts(text: &str) -> Vec<String> {
        let style = test_style();
        render_inline_spans(text, &style).iter().map(|s| s.content.to_string()).collect()
    }

    // ── Block-level tests ────────────────────────────

    #[test]
    fn plain_text_passthrough() {
        let lines = plain_lines("Hello, world!");
        assert_eq!(lines, vec!["Hello, world!"]);
    }

    #[test]
    fn multiple_lines() {
        let lines = plain_lines("Line one\nLine two\nLine three");
        assert_eq!(lines, vec!["Line one", "Line two", "Line three"]);
    }

    #[test]
    fn heading_h1() {
        let lines = plain_lines("# Big Title");
        assert_eq!(lines, vec!["Big Title"]);
    }

    #[test]
    fn heading_h2_h3() {
        let lines = plain_lines("## Section\n### Subsection");
        assert_eq!(lines, vec!["Section", "Subsection"]);
    }

    #[test]
    fn code_block() {
        let input = "before\n```rust\nfn main() {}\n```\nafter";
        let lines = plain_lines(input);
        assert_eq!(lines, vec!["before", "─── rust ", "  fn main() {}", "───", "after"]);
    }

    #[test]
    fn code_block_no_lang() {
        let input = "```\nsome code\n```";
        let lines = plain_lines(input);
        assert_eq!(lines, vec!["───", "  some code", "───"]);
    }

    #[test]
    fn unordered_list() {
        let lines = plain_lines("- first\n- second\n* third");
        assert_eq!(lines, vec!["• first", "• second", "• third"]);
    }

    #[test]
    fn ordered_list() {
        let lines = plain_lines("1. alpha\n2. beta\n3. gamma");
        assert_eq!(lines, vec!["1. alpha", "2. beta", "3. gamma"]);
    }

    #[test]
    fn blockquote() {
        let lines = plain_lines("> quoted text");
        assert_eq!(lines, vec!["▎ quoted text"]);
    }

    #[test]
    fn horizontal_rule() {
        let lines = plain_lines("---");
        assert_eq!(lines, vec!["────────────"]);

        let lines2 = plain_lines("***");
        assert_eq!(lines2, vec!["────────────"]);
    }

    // ── Inline tests ─────────────────────────────────

    #[test]
    fn inline_code() {
        let spans = inline_texts("use `foo` here");
        assert_eq!(spans, vec!["use ", " foo ", " here"]);
    }

    #[test]
    fn inline_bold() {
        let spans = inline_texts("this is **bold** text");
        assert_eq!(spans, vec!["this is ", "bold", " text"]);
    }

    #[test]
    fn inline_italic() {
        let spans = inline_texts("this is *italic* text");
        assert_eq!(spans, vec!["this is ", "italic", " text"]);
    }

    #[test]
    fn inline_link() {
        let spans = inline_texts("click [here](http://example.com) now");
        assert_eq!(spans, vec!["click ", "here", " now"]);
    }

    #[test]
    fn inline_mixed() {
        let spans = inline_texts("**bold** and `code` and *italic*");
        assert_eq!(spans, vec!["bold", " and ", " code ", " and ", "italic"]);
    }

    #[test]
    fn inline_no_markers() {
        let spans = inline_texts("plain text without any formatting");
        assert_eq!(spans, vec!["plain text without any formatting"]);
    }

    #[test]
    fn inline_bold_italic_stars() {
        let spans = inline_texts("this is ***bold italic*** text");
        assert_eq!(spans, vec!["this is ", "bold italic", " text"]);
    }

    #[test]
    fn inline_bold_italic_underscores() {
        let spans = inline_texts("this is ___bold italic___ text");
        assert_eq!(spans, vec!["this is ", "bold italic", " text"]);
    }

    #[test]
    fn inline_bold_underscores() {
        let spans = inline_texts("this is __bold__ text");
        assert_eq!(spans, vec!["this is ", "bold", " text"]);
    }

    #[test]
    fn inline_italic_underscores() {
        let spans = inline_texts("this is _italic_ text");
        assert_eq!(spans, vec!["this is ", "italic", " text"]);
    }

    #[test]
    fn inline_underscore_in_word_not_italic() {
        // snake_case should not trigger italic
        let spans = inline_texts("some_variable_name here");
        assert_eq!(spans, vec!["some_variable_name here"]);
    }

    #[test]
    fn inline_strikethrough() {
        let spans = inline_texts("this is ~~deleted~~ text");
        assert_eq!(spans, vec!["this is ", "deleted", " text"]);
    }

    #[test]
    fn unclosed_bold_treated_as_literal() {
        // Unclosed ** should not eat the rest of the line
        let spans = inline_texts("oops **unclosed");
        assert_eq!(spans, vec!["oops **unclosed"]);
    }

    #[test]
    fn unclosed_code_block_streaming() {
        // Simulates a half-streamed code block — should not panic
        let input = "```python\ndef hello():";
        let lines = plain_lines(input);
        assert_eq!(lines, vec!["─── python ", "  def hello():"]);
    }

    #[test]
    fn indented_list() {
        let lines = plain_lines("  - nested item");
        assert_eq!(lines, vec!["  • nested item"]);
    }

    #[test]
    fn empty_input() {
        let lines = plain_lines("");
        assert!(lines.is_empty());
    }
}
