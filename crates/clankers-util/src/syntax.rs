//! Syntax highlighting using `syntect`.
//!
//! Provides syntax highlighting for 100+ languages using syntect's built-in
//! grammar definitions and a terminal-friendly color theme. Outputs can be
//! ANSI-colored strings, ratatui `Span`s, or raw `HighlightSpan` tokens.

use std::sync::LazyLock;

use syntect::easy::HighlightLines;
use syntect::highlighting::Color as SynColor;
use syntect::highlighting::FontStyle;
use syntect::highlighting::Style as SynStyle;
use syntect::highlighting::Theme;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

/// Shared, lazily-initialized syntax set and theme.
static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
#[cfg_attr(dylint_lib = "tigerstyle", allow(no_unwrap, reason = "syntect ships with at least one theme"))]
static THEME: LazyLock<Theme> = LazyLock::new(|| {
    let ts = ThemeSet::load_defaults();
    // "base16-eighties.dark" is a good terminal-friendly dark theme
    ts.themes
        .get("base16-eighties.dark")
        .cloned()
        .unwrap_or_else(|| ts.themes.values().next().cloned().expect("syntect ships with at least one theme"))
});

/// A highlighted span of text with a token classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightSpan {
    pub text: String,
    pub kind: TokenKind,
    /// The RGB foreground color from syntect (if available).
    pub fg: Option<(u8, u8, u8)>,
}

/// Token classification for syntax highlighting.
///
/// When syntect is used, `Syntect` carries the exact RGB color.
/// The legacy variants are kept for API compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    /// Token colored by syntect — carries foreground RGB
    Syntect {
        r: u8,
        g: u8,
        b: u8,
        bold: bool,
        italic: bool,
    },
    /// Language keyword (fn, let, if, etc.)
    Keyword,
    /// String literal
    String,
    /// Comment
    Comment,
    /// Numeric literal
    Number,
    /// Type name
    Type,
    /// Function call
    Function,
    /// Operator
    Operator,
    /// Punctuation
    Punctuation,
    /// Plain text / identifier
    Plain,
}

impl TokenKind {
    /// Get the ANSI color code for this token kind.
    pub fn ansi_color(self) -> &'static str {
        match self {
            TokenKind::Syntect { .. } => "", // handled separately
            TokenKind::Keyword => "\x1b[38;5;198m",
            TokenKind::String => "\x1b[38;5;113m",
            TokenKind::Comment => "\x1b[38;5;244m",
            TokenKind::Number => "\x1b[38;5;141m",
            TokenKind::Type => "\x1b[38;5;81m",
            TokenKind::Function => "\x1b[38;5;222m",
            TokenKind::Operator => "\x1b[38;5;215m",
            TokenKind::Punctuation => "\x1b[38;5;248m",
            TokenKind::Plain => "\x1b[0m",
        }
    }

    /// Get a ratatui `Style` for this token kind.
    pub fn to_style(self) -> ratatui::style::Style {
        use ratatui::style::Color;
        use ratatui::style::Modifier;
        use ratatui::style::Style;
        match self {
            TokenKind::Syntect { r, g, b, bold, italic } => {
                let mut s = Style::default().fg(Color::Rgb(r, g, b));
                if bold {
                    s = s.add_modifier(Modifier::BOLD);
                }
                if italic {
                    s = s.add_modifier(Modifier::ITALIC);
                }
                s
            }
            TokenKind::Keyword => Style::default().fg(Color::Indexed(198)),
            TokenKind::String => Style::default().fg(Color::Indexed(113)),
            TokenKind::Comment => Style::default().fg(Color::Indexed(244)),
            TokenKind::Number => Style::default().fg(Color::Indexed(141)),
            TokenKind::Type => Style::default().fg(Color::Indexed(81)),
            TokenKind::Function => Style::default().fg(Color::Indexed(222)),
            TokenKind::Operator => Style::default().fg(Color::Indexed(215)),
            TokenKind::Punctuation => Style::default().fg(Color::Indexed(248)),
            TokenKind::Plain => Style::default(),
        }
    }
}

/// Detect language from a file extension.
pub fn detect_language(filename: &str) -> Option<&'static str> {
    let ext = filename.rsplit('.').next()?.to_lowercase();
    match ext.as_str() {
        "rs" => Some("rust"),
        "py" | "pyi" => Some("python"),
        "js" | "mjs" | "cjs" => Some("javascript"),
        "ts" | "mts" | "cts" => Some("typescript"),
        "jsx" => Some("jsx"),
        "tsx" => Some("tsx"),
        "go" => Some("go"),
        "sh" => Some("sh"),
        "bash" | "zsh" => Some("bash"),
        "c" | "h" => Some("c"),
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" => Some("cpp"),
        "java" => Some("java"),
        "rb" => Some("ruby"),
        "toml" => Some("toml"),
        "yaml" | "yml" => Some("yaml"),
        "json" => Some("json"),
        "md" | "markdown" => Some("markdown"),
        "css" => Some("css"),
        "html" | "htm" => Some("html"),
        "xml" => Some("xml"),
        "sql" => Some("sql"),
        "swift" => Some("swift"),
        "kt" | "kts" => Some("kotlin"),
        "scala" => Some("scala"),
        "r" | "R" => Some("r"),
        "lua" => Some("lua"),
        "hs" => Some("haskell"),
        "ex" | "exs" => Some("elixir"),
        "erl" => Some("erlang"),
        "cs" => Some("csharp"),
        "fs" | "fsx" => Some("fsharp"),
        "php" => Some("php"),
        "pl" | "pm" => Some("perl"),
        "clj" | "cljs" => Some("clojure"),
        "ml" | "mli" => Some("ocaml"),
        "zig" => Some("zig"),
        "nim" => Some("nim"),
        "dart" => Some("dart"),
        "tf" => Some("terraform"),
        "dockerfile" => Some("dockerfile"),
        "proto" => Some("protobuf"),
        "graphql" | "gql" => Some("graphql"),
        _ => None,
    }
}

/// Normalize a language identifier from a fenced code block info string.
pub fn normalize_language(info: &str) -> &str {
    let lang = info.split_whitespace().next().unwrap_or(info);
    match lang {
        "rs" | "rust" => "Rust",
        "py" | "python" | "python3" => "Python",
        "js" | "javascript" | "node" => "JavaScript",
        "ts" | "typescript" => "TypeScript",
        "jsx" => "JavaScript (JSX)",
        "tsx" => "TypeScript (TSX)",
        "go" | "golang" => "Go",
        "sh" | "shell" | "posix" => "Bourne Again Shell (bash)",
        "bash" | "zsh" => "Bourne Again Shell (bash)",
        "c" => "C",
        "cpp" | "c++" | "cxx" => "C++",
        "java" => "Java",
        "rb" | "ruby" => "Ruby",
        "toml" => "TOML",
        "yaml" | "yml" => "YAML",
        "json" | "jsonc" => "JSON",
        "md" | "markdown" => "Markdown",
        "css" => "CSS",
        "html" | "htm" => "HTML",
        "xml" => "XML",
        "sql" => "SQL",
        "swift" => "Swift",
        "kt" | "kotlin" => "Kotlin",
        "scala" => "Scala",
        "r" => "R",
        "lua" => "Lua",
        "hs" | "haskell" => "Haskell",
        "ex" | "elixir" => "Elixir",
        "erl" | "erlang" => "Erlang",
        "cs" | "csharp" | "c#" => "C#",
        "fs" | "fsharp" | "f#" => "F#",
        "php" => "PHP",
        "pl" | "perl" => "Perl",
        "clj" | "clojure" => "Clojure",
        "ml" | "ocaml" => "OCaml",
        "dart" => "Dart",
        "diff" | "patch" => "Diff",
        other => other,
    }
}

/// Find the syntect syntax for a language string, trying multiple strategies.
fn find_syntax(language: &str) -> Option<&'static syntect::parsing::SyntaxReference> {
    let ss = &*SYNTAX_SET;

    // 1. Try the normalized name (which maps to syntect's display names)
    let normalized = normalize_language(language);
    if let Some(syn) = ss.find_syntax_by_name(normalized) {
        return Some(syn);
    }

    // 2. Try the raw language string as a name
    if let Some(syn) = ss.find_syntax_by_name(language) {
        return Some(syn);
    }

    // 3. Try as an extension
    if let Some(syn) = ss.find_syntax_by_extension(language) {
        return Some(syn);
    }

    // 4. Try common extension mappings
    let ext = match language {
        "rust" => "rs",
        "python" | "python3" => "py",
        "javascript" | "node" => "js",
        "typescript" => "ts",
        "golang" => "go",
        "bash" | "shell" | "zsh" => "sh",
        "csharp" | "c#" => "cs",
        "fsharp" | "f#" => "fs",
        "cpp" | "c++" | "cxx" => "cpp",
        "ruby" => "rb",
        "haskell" => "hs",
        "kotlin" => "kt",
        "perl" => "pl",
        "clojure" => "clj",
        "ocaml" => "ml",
        "elixir" => "ex",
        "erlang" => "erl",
        _ => return None,
    };
    ss.find_syntax_by_extension(ext)
}

fn syn_style_to_token_kind(style: SynStyle) -> TokenKind {
    let SynColor { r, g, b, .. } = style.foreground;
    let is_bold = style.font_style.contains(FontStyle::BOLD);
    let is_italic = style.font_style.contains(FontStyle::ITALIC);
    TokenKind::Syntect { r, g, b, bold: is_bold, italic: is_italic }
}

/// Highlight a code string, returning spans with token classifications.
///
/// Uses syntect for languages it recognizes. Falls back to plain text
/// for unknown languages.
pub fn highlight(code: &str, language: &str) -> Vec<HighlightSpan> {
    let syntax = match find_syntax(language) {
        Some(s) => s,
        None => {
            return vec![HighlightSpan {
                text: code.to_string(),
                kind: TokenKind::Plain,
                fg: None,
            }];
        }
    };

    let mut h = HighlightLines::new(syntax, &THEME);
    let mut spans = Vec::new();

    for line in LinesWithEndings::from(code) {
        match h.highlight_line(line, &SYNTAX_SET) {
            Ok(ranges) => {
                for (style, text) in ranges {
                    let kind = syn_style_to_token_kind(style);
                    let SynColor { r, g, b, .. } = style.foreground;
                    spans.push(HighlightSpan {
                        text: text.to_string(),
                        kind,
                        fg: Some((r, g, b)),
                    });
                }
            }
            Err(_) => {
                spans.push(HighlightSpan {
                    text: line.to_string(),
                    kind: TokenKind::Plain,
                    fg: None,
                });
            }
        }
    }

    spans
}

/// Highlight code and return an ANSI-colored string for terminal output.
pub fn highlight_ansi(code: &str, language: &str) -> String {
    use std::fmt::Write;
    let spans = highlight(code, language);
    let mut out = String::with_capacity(code.len() * 2);
    for span in &spans {
        match span.kind {
            TokenKind::Syntect { r, g, b, bold, italic } => {
                if bold {
                    out.push_str("\x1b[1m");
                }
                if italic {
                    out.push_str("\x1b[3m");
                }
                write!(out, "\x1b[38;2;{};{};{}m", r, g, b).ok();
            }
            TokenKind::Plain => {
                out.push_str("\x1b[0m");
            }
            other => {
                out.push_str(other.ansi_color());
            }
        }
        out.push_str(&span.text);
    }
    out.push_str("\x1b[0m");
    out
}

/// Highlight code and return a vector of ratatui `Span`s for TUI rendering.
pub fn highlight_ratatui<'a>(code: &str, language: &str) -> Vec<ratatui::text::Span<'a>> {
    let spans = highlight(code, language);
    spans.into_iter().map(|s| ratatui::text::Span::styled(s.text, s.kind.to_style())).collect()
}

/// List all language names that syntect supports.
pub fn supported_languages() -> Vec<&'static str> {
    SYNTAX_SET.syntaxes().iter().map(|s| s.name.as_str()).collect()
}

/// Syntect-backed syntax highlighter implementing the TUI trait.
pub struct SyntectHighlighter;

impl clankers_tui_types::SyntaxHighlighter for SyntectHighlighter {
    fn highlight(&self, code: &str, language: &str) -> Vec<clankers_tui_types::HighlightSpan> {
        highlight(code, language)
            .into_iter()
            .map(|s| {
                let fg = s.fg;
                clankers_tui_types::HighlightSpan { text: s.text, fg }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_rust_keyword() {
        let spans = highlight("let x = 42;", "rust");
        // syntect should produce multiple spans; at least one should contain "let"
        let has_let = spans.iter().any(|s| s.text.contains("let"));
        assert!(has_let, "should highlight 'let': {:?}", spans);
    }

    #[test]
    fn test_highlight_rust_produces_colors() {
        let spans = highlight("fn main() {}", "rust");
        // Should have at least some colored spans
        let has_color = spans.iter().any(|s| s.fg.is_some());
        assert!(has_color, "should produce colored spans: {:?}", spans);
    }

    #[test]
    fn test_highlight_python() {
        let spans = highlight("def foo(x):\n    return x + 1\n", "python");
        let has_def = spans.iter().any(|s| s.text.contains("def"));
        assert!(has_def);
    }

    #[test]
    fn test_highlight_unknown_language() {
        let spans = highlight("whatever code", "brainfuck");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].kind, TokenKind::Plain);
    }

    #[test]
    fn test_normalize_language() {
        assert_eq!(normalize_language("rs"), "Rust");
        assert_eq!(normalize_language("python3"), "Python");
        assert_eq!(normalize_language("js"), "JavaScript");
        assert_eq!(normalize_language("golang"), "Go");
    }

    #[test]
    fn test_detect_language() {
        assert_eq!(detect_language("main.rs"), Some("rust"));
        assert_eq!(detect_language("script.py"), Some("python"));
        assert_eq!(detect_language("index.js"), Some("javascript"));
        assert_eq!(detect_language("unknown.xyz"), None);
    }

    #[test]
    fn test_highlight_ansi_contains_escape_codes() {
        let out = highlight_ansi("fn main() {}", "rust");
        assert!(out.contains("\x1b["));
    }

    #[test]
    fn test_highlight_many_languages() {
        // Verify syntect can handle various languages without panicking
        let cases = vec![
            ("rust", "fn main() {}"),
            ("python", "def foo(): pass"),
            ("javascript", "const x = 42;"),
            ("typescript", "let x: number = 42;"),
            ("go", "func main() {}"),
            ("bash", "echo hello"),
            ("c", "int main() { return 0; }"),
            ("cpp", "auto x = std::make_unique<int>(42);"),
            ("java", "public class Main {}"),
            ("ruby", "puts 'hello'"),
            ("json", r#"{"key": "value"}"#),
            ("yaml", "key: value"),
            ("toml", "[section]\nkey = \"value\""),
            ("html", "<div>hello</div>"),
            ("css", "body { color: red; }"),
            ("sql", "SELECT * FROM users;"),
            ("diff", "+added\n-removed"),
        ];
        for (lang, code) in cases {
            let spans = highlight(code, lang);
            assert!(!spans.is_empty(), "should produce spans for {}", lang);
        }
    }

    #[test]
    fn test_supported_languages_not_empty() {
        let langs = supported_languages();
        assert!(langs.len() > 20, "syntect should support many languages, got {}", langs.len());
    }

    #[test]
    fn test_highlight_ratatui() {
        let spans = highlight_ratatui("let x = 42;", "rust");
        assert!(!spans.is_empty());
    }
}
