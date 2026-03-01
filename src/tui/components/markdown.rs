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

/// Render markdown text into a list of ratatui Lines.
///
/// Each returned `Line` corresponds to one visual line of output.
/// The caller is responsible for adding any border prefix (e.g. `"│ "`)
/// before each line.
pub fn render_markdown(text: &str, style: &MarkdownStyle) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut in_code_block = false;
    let mut code_lang = String::new();

    for raw_line in text.lines() {
        // ── Code fences ──────────────────────────────
        if let Some(rest) = raw_line.strip_prefix("```") {
            if !in_code_block {
                // Opening fence
                in_code_block = true;
                code_lang = rest.trim().to_string();
                let label = if code_lang.is_empty() {
                    "───".to_string()
                } else {
                    format!("─── {} ", code_lang)
                };
                lines.push(Line::from(Span::styled(label, style.code_fence)));
            } else {
                // Closing fence
                in_code_block = false;
                code_lang.clear();
                lines.push(Line::from(Span::styled("───", style.code_fence)));
            }
            continue;
        }

        if in_code_block {
            if !code_lang.is_empty() {
                // Syntax-highlighted code line
                let mut spans: Vec<Span<'static>> = vec![Span::raw("  ".to_string())];
                let hl_spans = crate::util::syntax::highlight_ratatui(raw_line, &code_lang);
                if hl_spans.iter().all(|s| matches!(s.style.fg, None | Some(ratatui::style::Color::Reset))) {
                    // syntect didn't produce colors — fall back to code_block style
                    spans.push(Span::styled(raw_line.to_string(), style.code_block));
                } else {
                    spans.extend(hl_spans);
                }
                lines.push(Line::from(spans));
            } else {
                lines.push(Line::from(Span::styled(format!("  {}", raw_line), style.code_block)));
            }
            continue;
        }

        // ── Horizontal rules ─────────────────────────
        {
            let trimmed = raw_line.trim();
            if trimmed.len() >= 3
                && (trimmed.chars().all(|c| c == '-' || c == ' ')
                    || trimmed.chars().all(|c| c == '*' || c == ' ')
                    || trimmed.chars().all(|c| c == '_' || c == ' '))
                && trimmed.chars().filter(|c| !c.is_whitespace()).count() >= 3
            {
                lines.push(Line::from(Span::styled("────────────", style.hrule)));
                continue;
            }
        }

        // ── Headings ─────────────────────────────────
        if let Some(content) = raw_line.strip_prefix("# ") {
            lines.push(Line::from(Span::styled(content.trim().to_string(), style.heading)));
            continue;
        }
        if let Some(content) = raw_line.strip_prefix("## ") {
            lines.push(Line::from(Span::styled(content.trim().to_string(), style.subheading)));
            continue;
        }
        if raw_line.starts_with("### ") || raw_line.starts_with("#### ") {
            let content = raw_line.trim_start_matches('#').trim();
            lines.push(Line::from(Span::styled(content.to_string(), style.subheading)));
            continue;
        }

        // ── Blockquotes ──────────────────────────────
        if let Some(rest) = raw_line.strip_prefix("> ") {
            let mut spans = vec![Span::styled("▎ ", style.blockquote)];
            spans.extend(render_inline_spans(rest, style));
            lines.push(Line::from(spans));
            continue;
        }
        if raw_line == ">" {
            lines.push(Line::from(Span::styled("▎", style.blockquote)));
            continue;
        }

        // ── Unordered lists ──────────────────────────
        if let Some(content) = strip_list_bullet(raw_line) {
            let indent = leading_spaces(raw_line);
            let indent_str: String = " ".repeat(indent);
            let mut spans = vec![Span::raw(indent_str), Span::styled("• ", style.list_marker)];
            spans.extend(render_inline_spans(content, style));
            lines.push(Line::from(spans));
            continue;
        }

        // ── Ordered lists ────────────────────────────
        if let Some((num, content)) = strip_ordered_list(raw_line) {
            let indent = leading_spaces(raw_line);
            let indent_str: String = " ".repeat(indent);
            let mut spans = vec![
                Span::raw(indent_str),
                Span::styled(format!("{}. ", num), style.list_marker),
            ];
            spans.extend(render_inline_spans(content, style));
            lines.push(Line::from(spans));
            continue;
        }

        // ── Regular paragraph text ───────────────────
        let spans = render_inline_spans(raw_line, style);
        lines.push(Line::from(spans));
    }

    // If we ended inside a code block (incomplete stream), close it gracefully
    if in_code_block {
        // Don't add a closing fence — the block is still being streamed
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

    /// Flush the accumulated plain-text buffer into a styled span.
    fn flush(buf: &mut String, spans: &mut Vec<Span<'static>>, sty: Style) {
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

    while i < len {
        // ── Inline code ──────────────────────────────
        if chars[i] == '`' {
            flush(&mut buf, &mut spans, style.base);
            i += 1;
            let start = i;
            while i < len && chars[i] != '`' {
                i += 1;
            }
            let code: String = chars[start..i].iter().collect();
            spans.push(Span::styled(format!(" {} ", code), style.inline_code));
            if i < len {
                i += 1;
            }
            continue;
        }

        // ── Strikethrough (~~text~~) ─────────────────
        if i + 1 < len
            && chars[i] == '~'
            && chars[i + 1] == '~'
            && let Some(close) = find_closing(&chars, i + 2, &['~', '~'])
        {
            flush(&mut buf, &mut spans, style.base);
            let inner: String = chars[i + 2..close].iter().collect();
            spans.push(Span::styled(inner, style.base.add_modifier(Modifier::CROSSED_OUT)));
            i = close + 2;
            continue;
        }

        // ── Bold-italic (***text*** or ___text___) ───
        if i + 2 < len
            && chars[i] == '*'
            && chars[i + 1] == '*'
            && chars[i + 2] == '*'
            && let Some(close) = find_closing(&chars, i + 3, &['*', '*', '*'])
        {
            flush(&mut buf, &mut spans, style.base);
            let inner: String = chars[i + 3..close].iter().collect();
            spans.push(Span::styled(inner, style.bold_italic));
            i = close + 3;
            continue;
        }
        if i + 2 < len
            && chars[i] == '_'
            && chars[i + 1] == '_'
            && chars[i + 2] == '_'
            && let Some(close) = find_closing(&chars, i + 3, &['_', '_', '_'])
        {
            flush(&mut buf, &mut spans, style.base);
            let inner: String = chars[i + 3..close].iter().collect();
            spans.push(Span::styled(inner, style.bold_italic));
            i = close + 3;
            continue;
        }

        // ── Bold (**text** or __text__) ──────────────
        if i + 1 < len
            && chars[i] == '*'
            && chars[i + 1] == '*'
            && let Some(close) = find_closing(&chars, i + 2, &['*', '*'])
        {
            flush(&mut buf, &mut spans, style.base);
            let inner: String = chars[i + 2..close].iter().collect();
            spans.push(Span::styled(inner, style.bold));
            i = close + 2;
            continue;
        }
        if i + 1 < len
            && chars[i] == '_'
            && chars[i + 1] == '_'
            && let Some(close) = find_closing(&chars, i + 2, &['_', '_'])
        {
            flush(&mut buf, &mut spans, style.base);
            let inner: String = chars[i + 2..close].iter().collect();
            spans.push(Span::styled(inner, style.bold));
            i = close + 2;
            continue;
        }

        // ── Italic (*text* or _text_) ────────────────
        if chars[i] == '*'
            && (i + 1 >= len || chars[i + 1] != '*')
            && let Some(close) = find_closing(&chars, i + 1, &['*'])
        {
            flush(&mut buf, &mut spans, style.base);
            let inner: String = chars[i + 1..close].iter().collect();
            spans.push(Span::styled(inner, style.italic));
            i = close + 1;
            continue;
        }
        if chars[i] == '_'
            && (i + 1 >= len || chars[i + 1] != '_')
            && let Some(close) = find_closing(&chars, i + 1, &['_'])
        {
            // Only treat as italic if the underscore is at a word boundary
            // (not in the middle of a_word_like_this)
            let at_start = i == 0 || !chars[i - 1].is_alphanumeric();
            let at_end = close + 1 >= len || !chars[close + 1].is_alphanumeric();
            if at_start && at_end {
                flush(&mut buf, &mut spans, style.base);
                let inner: String = chars[i + 1..close].iter().collect();
                spans.push(Span::styled(inner, style.italic));
                i = close + 1;
                continue;
            }
        }

        // ── Links [text](url) ────────────────────────
        if chars[i] == '[' {
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
                    flush(&mut buf, &mut spans, style.base);
                    spans.push(Span::styled(link_text, style.base.add_modifier(Modifier::UNDERLINED)));
                    i = k + 1;
                    continue;
                }
            }
            // Not a valid link — treat [ as literal
            buf.push(chars[i]);
            i += 1;
            continue;
        }

        // ── Regular character ────────────────────────
        buf.push(chars[i]);
        i += 1;
    }

    // Flush remaining text
    if !buf.is_empty() {
        spans.push(Span::styled(buf, style.base));
    }

    // If empty input, return at least one empty span so the line exists
    if spans.is_empty() {
        spans.push(Span::raw(String::new()));
    }

    spans
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
        render_markdown(text, &style)
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
