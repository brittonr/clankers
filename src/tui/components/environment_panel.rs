//! Environment panel — shows current configuration at a glance
//!
//! Displays model, thinking mode, session, cwd, and other settings
//! without needing `/status`. Can be rendered as a compact panel.

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::tui::app::App;
    use crate::tui::components::context_gauge::ContextGauge;
    use crate::tui::components::git_status::GitStatus;
    use crate::tui::theme::Theme;

    #[test]
    fn test_environment_panel_no_panic() {
        // Just ensure it doesn't panic with default state
        let _app = App::new("claude-sonnet-4-5".into(), "/tmp".into(), Theme::dark());
        let context = ContextGauge::new("claude-sonnet-4-5");
        let mut git = GitStatus::new("/tmp");
        git.is_repo = false;
        // We can't easily test rendering without a terminal, but
        // we can at least verify the data structures are sound
        let _ = context.summary();
        let _ = git.summary();
    }
}
