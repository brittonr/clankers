//! Syntax highlighting abstraction.
//!
//! The TUI crate needs to highlight code blocks but shouldn't depend on
//! the main crate's `util::syntax` module directly.

/// A highlighted span of text with optional foreground color.
#[derive(Debug, Clone)]
pub struct HighlightSpan {
    /// The text content.
    pub text: String,
    /// Optional foreground color as (r, g, b).
    pub fg: Option<(u8, u8, u8)>,
}

/// Trait for syntax highlighting providers.
///
/// The main crate implements this using syntect; the TUI crate consumes it
/// without knowing the implementation details.
pub trait SyntaxHighlighter: Send + Sync {
    /// Highlight a line of code in the given language.
    /// Returns styled spans. If the language is unknown, returns a single plain span.
    fn highlight(&self, code: &str, language: &str) -> Vec<HighlightSpan>;
}

/// No-op highlighter that returns unstyled text.
pub struct PlainHighlighter;

impl SyntaxHighlighter for PlainHighlighter {
    fn highlight(&self, code: &str, _language: &str) -> Vec<HighlightSpan> {
        vec![HighlightSpan {
            text: code.to_string(),
            fg: None,
        }]
    }
}
