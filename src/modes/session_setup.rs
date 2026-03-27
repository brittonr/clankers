//! Session persistence setup — create new sessions or resume existing ones.
//!
//! Extracted from interactive.rs to keep that file focused on the top-level
//! orchestration of the TUI interactive mode.

use crate::tui::app::App;

/// Set up session persistence: create new session or resume existing one.
///
/// Returns the session manager (if persistence is enabled), any seed messages
/// from a resumed session, and the worktree setup (if worktree isolation is active).
pub(crate) fn setup_session(
    app: &mut App,
    cwd: &str,
    model: &str,
    db: &Option<crate::db::Db>,
    settings: &crate::config::settings::Settings,
    resume_opts: super::interactive::ResumeOptions,
) -> (
    Option<crate::session::SessionManager>,
    Vec<crate::provider::message::AgentMessage>,
    Option<crate::worktree::session_bridge::WorktreeSetup>,
) {
    let paths = crate::config::ClankersPaths::get();
    let sessions_dir = &paths.global_sessions_dir;
    let use_worktrees = settings.use_worktrees;

    if resume_opts.no_session {
        return (None, Vec::new(), None);
    }

    if resume_opts.continue_last {
        return resume_latest(app, cwd, model, db, sessions_dir, use_worktrees);
    }

    if let Some(ref session_id) = resume_opts.session_id {
        return resume_by_id(app, cwd, model, db, sessions_dir, use_worktrees, session_id);
    }

    // Default: create a new session
    create_new_session(app, cwd, model, db, sessions_dir, use_worktrees)
}

// ── Helpers ──────────────────────────────────────────────────────────

/// Create a brand new session, optionally with a worktree.
fn create_new_session(
    app: &mut App,
    cwd: &str,
    model: &str,
    db: &Option<crate::db::Db>,
    sessions_dir: &std::path::Path,
    use_worktrees: bool,
) -> (
    Option<crate::session::SessionManager>,
    Vec<crate::provider::message::AgentMessage>,
    Option<crate::worktree::session_bridge::WorktreeSetup>,
) {
    // Try to set up a worktree first so we can record it in the session header
    let wt_setup = match db {
        Some(db) => crate::worktree::session_bridge::setup_worktree_for_session(db, cwd, use_worktrees),
        None => None,
    };
    let (wt_path, wt_branch) = match &wt_setup {
        Some(s) => (Some(s.working_dir.to_string_lossy().to_string()), Some(s.branch.clone())),
        None => (None, None),
    };
    match crate::session::SessionManager::create(
        sessions_dir,
        cwd,
        model,
        None,
        wt_path.as_deref(),
        wt_branch.as_deref(),
    ) {
        Ok(mgr) => {
            app.session_id = mgr.session_id().to_string();
            if let Some(ref s) = wt_setup {
                app.push_system(format!("Worktree: {}", s.branch), false);
            }
            (Some(mgr), Vec::new(), wt_setup)
        }
        Err(e) => {
            tracing::warn!("Failed to create session: {}", e);
            (None, Vec::new(), None)
        }
    }
}

/// Resume a session from a manager, writing a resume entry and re-entering
/// the worktree if the session had one.
fn resume_session(
    app: &mut App,
    mut mgr: crate::session::SessionManager,
    from_label: &str,
) -> (
    Option<crate::session::SessionManager>,
    Vec<crate::provider::message::AgentMessage>,
    Option<crate::worktree::session_bridge::WorktreeSetup>,
) {
    let msgs = mgr.build_context().unwrap_or_default();
    app.session_id = mgr.session_id().to_string();

    mgr.record_resume(crate::provider::message::MessageId::new(from_label)).ok();

    let msg_count = msgs.len();
    app.push_system(format!("Resumed session {} ({} messages)", mgr.session_id(), msg_count), false);

    // Re-enter the worktree if this session had one
    let wt_setup = crate::worktree::session_bridge::resume_worktree(mgr.worktree_path(), mgr.worktree_branch());
    if let Some(ref s) = wt_setup {
        app.push_system(format!("Worktree: {}", s.branch), false);
    }
    (Some(mgr), msgs, wt_setup)
}

/// Resume the most recent session for this working directory.
fn resume_latest(
    app: &mut App,
    cwd: &str,
    model: &str,
    db: &Option<crate::db::Db>,
    sessions_dir: &std::path::Path,
    use_worktrees: bool,
) -> (
    Option<crate::session::SessionManager>,
    Vec<crate::provider::message::AgentMessage>,
    Option<crate::worktree::session_bridge::WorktreeSetup>,
) {
    let files = crate::session::store::list_sessions(sessions_dir, cwd);
    if let Some(latest_file) = files.into_iter().next() {
        match crate::session::SessionManager::open(latest_file) {
            Ok(mgr) => resume_session(app, mgr, "continue"),
            Err(e) => {
                app.push_system(format!("Failed to resume last session: {}", e), true);
                create_new_session(app, cwd, model, db, sessions_dir, use_worktrees)
            }
        }
    } else {
        app.push_system("No previous session found. Starting new session.".to_string(), false);
        create_new_session(app, cwd, model, db, sessions_dir, use_worktrees)
    }
}

/// Resume a specific session by ID.
fn resume_by_id(
    app: &mut App,
    cwd: &str,
    model: &str,
    db: &Option<crate::db::Db>,
    sessions_dir: &std::path::Path,
    use_worktrees: bool,
    session_id: &str,
) -> (
    Option<crate::session::SessionManager>,
    Vec<crate::provider::message::AgentMessage>,
    Option<crate::worktree::session_bridge::WorktreeSetup>,
) {
    let files = crate::session::store::list_sessions(sessions_dir, cwd);
    let found = files
        .into_iter()
        .find(|f| f.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.contains(session_id)));

    if let Some(file) = found {
        match crate::session::SessionManager::open(file) {
            Ok(mgr) => resume_session(app, mgr, "resume"),
            Err(e) => {
                app.push_system(format!("Failed to resume session '{}': {}", session_id, e), true);
                create_new_session(app, cwd, model, db, sessions_dir, use_worktrees)
            }
        }
    } else {
        app.push_system(format!("Session '{}' not found.", session_id), true);
        create_new_session(app, cwd, model, db, sessions_dir, use_worktrees)
    }
}
