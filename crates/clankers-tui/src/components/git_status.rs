//! Git status — shows branch name and working tree status
//!
//! Rendered as a status bar segment. Refreshes periodically via git2
//! (non-blocking, cached).

use std::path::Path;
use std::time::Duration;
use std::time::Instant;

use git2::Repository;
use git2::StatusOptions;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Span;

// ── State ───────────────────────────────────────────────────────────────────

/// Cached git status for the status bar
#[derive(Debug, Clone)]
pub struct GitStatus {
    /// Current branch name (or HEAD sha if detached)
    pub branch: Option<String>,
    /// Number of modified/staged/untracked files
    pub dirty_count: usize,
    /// Number of staged files
    pub staged_count: usize,
    /// Number of untracked files
    pub untracked_count: usize,
    /// Whether we're in a git repo at all
    pub is_repo: bool,
    /// Last refresh time
    last_refresh: Instant,
    /// How often to refresh
    refresh_interval: Duration,
    /// Working directory to check
    cwd: String,
}

impl GitStatus {
    pub fn new(cwd: &str) -> Self {
        let mut s = Self {
            branch: None,
            dirty_count: 0,
            staged_count: 0,
            untracked_count: 0,
            is_repo: false,
            last_refresh: Instant::now().checked_sub(Duration::from_secs(60)).unwrap(), // force initial refresh
            refresh_interval: Duration::from_secs(5),
            cwd: cwd.to_string(),
        };
        s.refresh();
        s
    }

    /// Update cwd (e.g. after /cd)
    pub fn set_cwd(&mut self, cwd: &str) {
        self.cwd = cwd.to_string();
        self.last_refresh = Instant::now().checked_sub(Duration::from_secs(60)).unwrap();
    }

    /// Check if a refresh is needed and do it
    pub fn maybe_refresh(&mut self) {
        if self.last_refresh.elapsed() >= self.refresh_interval {
            self.refresh();
        }
    }

    /// Force a refresh using git2
    pub fn refresh(&mut self) {
        self.last_refresh = Instant::now();
        let cwd = Path::new(&self.cwd);

        // Try to discover and open the git repository
        let repo = match Repository::discover(cwd) {
            Ok(r) => r,
            Err(_) => {
                self.is_repo = false;
                self.branch = None;
                self.dirty_count = 0;
                self.staged_count = 0;
                self.untracked_count = 0;
                return;
            }
        };

        self.is_repo = true;

        // Get branch name or short OID for detached HEAD
        self.branch = match repo.head() {
            Ok(head) => {
                if head.is_branch() {
                    head.shorthand().map(|s| s.to_string())
                } else {
                    // Detached HEAD — format as :shortsha
                    head.target().map(|oid| format!(":{}", &oid.to_string()[..7]))
                }
            }
            Err(_) => None,
        };

        // Get file status counts
        let mut opts = StatusOptions::new();
        opts.include_untracked(true);
        opts.recurse_untracked_dirs(true);

        self.dirty_count = 0;
        self.staged_count = 0;
        self.untracked_count = 0;

        if let Ok(statuses) = repo.statuses(Some(&mut opts)) {
            for entry in statuses.iter() {
                let status = entry.status();

                // Untracked files
                if status.is_wt_new() {
                    self.untracked_count += 1;
                    continue;
                }

                // Staged changes (index)
                if status.is_index_new()
                    || status.is_index_modified()
                    || status.is_index_deleted()
                    || status.is_index_renamed()
                    || status.is_index_typechange()
                {
                    self.staged_count += 1;
                }

                // Working tree changes
                if status.is_wt_modified()
                    || status.is_wt_deleted()
                    || status.is_wt_typechange()
                    || status.is_wt_renamed()
                {
                    self.dirty_count += 1;
                }
            }
        }
    }

    /// Whether the working tree has changes
    pub fn is_dirty(&self) -> bool {
        self.dirty_count > 0 || self.staged_count > 0 || self.untracked_count > 0
    }

    /// Total changed files
    pub fn total_changes(&self) -> usize {
        self.dirty_count + self.staged_count + self.untracked_count
    }

    /// Render as a status bar span:  main *3
    pub fn status_bar_span(&self) -> Option<Span<'static>> {
        if !self.is_repo {
            return None;
        }

        let branch = self.branch.clone().unwrap_or_else(|| "???".to_string());
        let branch_color = Color::Magenta;

        let text = if self.is_dirty() {
            let mut parts = Vec::new();
            if self.staged_count > 0 {
                parts.push(format!("+{}", self.staged_count));
            }
            if self.dirty_count > 0 {
                parts.push(format!("~{}", self.dirty_count));
            }
            if self.untracked_count > 0 {
                parts.push(format!("?{}", self.untracked_count));
            }
            format!("  {} *{} ", branch, parts.join(""))
        } else {
            format!("  {} ", branch)
        };

        let style = if self.is_dirty() {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(branch_color).add_modifier(Modifier::BOLD)
        };

        Some(Span::styled(text, style))
    }

    /// Detailed summary string
    pub fn summary(&self) -> String {
        if !self.is_repo {
            return "Not a git repository.".to_string();
        }
        let branch = self.branch.as_deref().unwrap_or("???");
        let mut out = format!("Branch: {}\n", branch);
        if self.is_dirty() {
            out.push_str(&format!("  Staged:    {}\n", self.staged_count));
            out.push_str(&format!("  Modified:  {}\n", self.dirty_count));
            out.push_str(&format!("  Untracked: {}\n", self.untracked_count));
        } else {
            out.push_str("  Clean working tree\n");
        }
        out
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_status_in_repo() {
        // This test runs inside the clankers repo, so should find git.
        // In Nix sandbox builds the source lives in /nix/store without .git,
        // so skip when no repo is found.
        let status = GitStatus::new(env!("CARGO_MANIFEST_DIR"));
        if !status.is_repo {
            eprintln!("skipping: not inside a git repo (Nix sandbox build)");
            return;
        }
        assert!(status.branch.is_some(), "Should have a branch name");
    }

    #[test]
    fn test_git_status_outside_repo() {
        let status = GitStatus::new("/tmp");
        // /tmp might or might not be a git repo, so just check it doesn't panic
        let _ = status.summary();
    }

    #[test]
    fn test_status_bar_span_none_outside_repo() {
        let mut status = GitStatus::new("/tmp");
        status.is_repo = false;
        assert!(status.status_bar_span().is_none());
    }

    #[test]
    fn test_summary_not_repo() {
        let mut status = GitStatus::new("/tmp");
        status.is_repo = false;
        assert!(status.summary().contains("Not a git"));
    }

    #[test]
    fn test_is_dirty() {
        let mut status = GitStatus::new("/tmp");
        status.is_repo = true;
        status.dirty_count = 0;
        status.staged_count = 0;
        status.untracked_count = 0;
        assert!(!status.is_dirty());
        status.dirty_count = 1;
        assert!(status.is_dirty());
    }

    #[test]
    fn test_total_changes() {
        let mut status = GitStatus::new("/tmp");
        status.dirty_count = 2;
        status.staged_count = 3;
        status.untracked_count = 1;
        assert_eq!(status.total_changes(), 6);
    }
}
