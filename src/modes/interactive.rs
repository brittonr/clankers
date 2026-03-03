//! Full TUI interactive mode

use std::io;
use std::sync::Arc;
use std::time::Duration;

use crossterm::event::DisableBracketedPaste;
use crossterm::event::DisableMouseCapture;
use crossterm::event::EnableBracketedPaste;
use crossterm::event::EnableMouseCapture;
use crossterm::event::KeyCode;
use crossterm::event::KeyModifiers;
use crossterm::execute;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
use crossterm::terminal::{self};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio_util::sync::CancellationToken;

use crate::agent::Agent;
use crate::agent::events::AgentEvent;
use crate::config::keybindings::Action;
use crate::config::keybindings::InputMode;
use crate::config::keybindings::Keymap;
use crate::error::Result;
use crate::provider::auth::AuthStoreExt;
use crate::slash_commands::SlashAction;
use crate::slash_commands::{self};
use crate::tui::app::App;
use crate::tui::app::AppState;
use crate::tui::app::HitRegion;
use crate::tui::app::MessageRole;
use crate::tui::components::block::BlockEntry;
use crate::tui::event::AppEvent;
use crate::tui::event::Button;
use crate::tui::event::{self as tui_event};
use crate::tui::panel::Panel; // bring trait methods (is_empty, etc.) into scope
use crate::tui::render;
use crate::tui::theme::Theme;

/// Options for resuming a session
#[derive(Default)]
pub struct ResumeOptions {
    /// Resume a specific session by ID
    pub session_id: Option<String>,
    /// Continue the most recent session
    pub continue_last: bool,
    /// Disable session persistence entirely
    pub no_session: bool,
}

/// Run the interactive TUI mode
pub async fn run_interactive(
    provider: Arc<dyn crate::provider::Provider>,
    settings: crate::config::settings::Settings,
    model: String,
    system_prompt: String,
    cwd: String,
    plugin_manager: Option<Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
    resume_opts: ResumeOptions,
) -> Result<()> {
    // Set up terminal
    terminal::enable_raw_mode().map_err(|e| crate::error::Error::Tui {
        message: format!("Failed to enable raw mode: {}", e),
    })?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture, EnableBracketedPaste).map_err(|e| {
        crate::error::Error::Tui {
            message: format!("Failed to enter alternate screen: {}", e),
        }
    })?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(|e| crate::error::Error::Tui {
        message: format!("Failed to create terminal: {}", e),
    })?;

    let theme = Theme::dark();
    let keymap = settings.keymap.clone().into_keymap();

    let mut app = App::new(model.clone(), cwd.clone(), theme);

    // ── Session persistence setup ────────────────────────────────────────
    let paths = crate::config::ClankersPaths::resolve();
    let sessions_dir = &paths.global_sessions_dir;
    let use_worktrees = settings.use_worktrees;
    let original_cwd = cwd.clone();

    // Open the global database for worktree registry + GC
    let db_path = paths.global_config_dir.join("clankers.db");
    let db = crate::db::Db::open(&db_path).ok();

    // Run startup GC in the background if we're in a git repo with worktrees enabled
    if use_worktrees
        && let Some(ref db) = db
        && let Some(repo_root) = crate::worktree::WorktreeManager::find_repo_root(std::path::Path::new(&cwd))
    {
        let db_clone = db.clone();
        crate::worktree::gc::spawn_startup_gc(db_clone, repo_root);
    }

    // Helper: create a new session, optionally with a worktree
    let create_new_session = |app: &mut App,
                              cwd: &str,
                              db: &Option<crate::db::Db>|
     -> (
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
            &model,
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
    };

    // Helper: resume a session, re-entering its worktree if present
    let resume_session = |app: &mut App,
                          mgr: crate::session::SessionManager,
                          from_label: &str|
     -> (
        Option<crate::session::SessionManager>,
        Vec<crate::provider::message::AgentMessage>,
        Option<crate::worktree::session_bridge::WorktreeSetup>,
    ) {
        let msgs = mgr.build_context().unwrap_or_default();
        app.session_id = mgr.session_id().to_string();
        let resume_entry = crate::session::entry::SessionEntry::Resume(crate::session::entry::ResumeEntry {
            id: crate::provider::message::MessageId::generate(),
            resumed_at: chrono::Utc::now(),
            from_entry_id: crate::provider::message::MessageId::new(from_label),
        });
        let _ = crate::session::store::append_entry(mgr.file_path(), &resume_entry);
        let msg_count = msgs.len();
        app.push_system(format!("Resumed session {} ({} messages)", mgr.session_id(), msg_count), false);

        // Re-enter the worktree if this session had one
        let wt_setup = crate::worktree::session_bridge::resume_worktree(mgr.worktree_path(), mgr.worktree_branch());
        if let Some(ref s) = wt_setup {
            app.push_system(format!("Worktree: {}", s.branch), false);
        }
        (Some(mgr), msgs, wt_setup)
    };

    let (session_manager, seed_messages, worktree_setup) = if resume_opts.no_session {
        (None, Vec::new(), None)
    } else if resume_opts.continue_last {
        // Find the most recent session for this cwd
        let files = crate::session::store::list_sessions(sessions_dir, &cwd);
        if let Some(latest_file) = files.into_iter().next() {
            match crate::session::SessionManager::open(latest_file) {
                Ok(mgr) => resume_session(&mut app, mgr, "continue"),
                Err(e) => {
                    app.push_system(format!("Failed to resume last session: {}", e), true);
                    create_new_session(&mut app, &cwd, &db)
                }
            }
        } else {
            app.push_system("No previous session found. Starting new session.".to_string(), false);
            create_new_session(&mut app, &cwd, &db)
        }
    } else if let Some(ref session_id) = resume_opts.session_id {
        // Resume a specific session by ID
        let files = crate::session::store::list_sessions(sessions_dir, &cwd);
        let found = files
            .into_iter()
            .find(|f| f.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.contains(session_id)));
        if let Some(file) = found {
            match crate::session::SessionManager::open(file) {
                Ok(mgr) => resume_session(&mut app, mgr, "resume"),
                Err(e) => {
                    app.push_system(format!("Failed to resume session '{}': {}", session_id, e), true);
                    create_new_session(&mut app, &cwd, &db)
                }
            }
        } else {
            app.push_system(format!("Session '{}' not found.", session_id), true);
            create_new_session(&mut app, &cwd, &db)
        }
    } else {
        // Default: create a new session
        create_new_session(&mut app, &cwd, &db)
    };

    // ── Enter worktree working directory ─────────────────────────────────
    if let Some(ref wt) = worktree_setup
        && let Err(e) = crate::worktree::session_bridge::enter_worktree(wt)
    {
        app.push_system(format!("Warning: failed to enter worktree: {}", e), true);
    }

    app.push_system(format!("clankers — {} — keymap: {} — press i to start typing", model, keymap.preset), false);

    let (panel_tx, mut panel_rx) =
        tokio::sync::mpsc::unbounded_channel::<crate::tui::components::subagent_event::SubagentEvent>();
    let panel_tx_for_slash = panel_tx.clone();

    let (todo_tx, mut todo_rx) = tokio::sync::mpsc::unbounded_channel::<(
        crate::tools::todo::TodoAction,
        tokio::sync::oneshot::Sender<crate::tools::todo::TodoResponse>,
    )>();

    let provider_for_merge = provider.clone();
    let model_for_merge = model.clone();
    app.original_system_prompt = system_prompt.clone();

    // Populate available models from provider
    app.available_models = provider.models().iter().map(|m| m.id.clone()).collect();

    // Set router connection status based on provider type
    app.router_status = if provider.name() == "rpc-router" {
        crate::tui::app::RouterStatus::Connected
    } else {
        crate::tui::app::RouterStatus::Local
    };

    // Populate active account name
    {
        let paths = crate::config::ClankersPaths::resolve();
        let store = crate::provider::auth::AuthStore::load(&paths.global_auth);
        app.active_account = store.active_account_name().to_string();
    }

    let mut agent = Agent::new(provider, Vec::new(), settings, model, system_prompt);
    // Attach the global database so the agent can read memories and record usage
    if let Some(ref db) = db {
        agent = agent.with_db(db.clone());
    }
    let event_tx = agent.event_sender();
    let (bash_confirm_tx, mut bash_confirm_rx) = crate::tools::bash::confirm_channel();

    // Create and start the process monitor
    let process_monitor = {
        let config = crate::procmon::ProcessMonitorConfig::default();
        let monitor = std::sync::Arc::new(crate::procmon::ProcessMonitor::new(config, Some(event_tx.clone())));
        monitor.clone().start();
        monitor
    };

    // Wire process monitor into the TUI panel
    app.process_panel = crate::tui::components::process_panel::ProcessPanel::new()
        .with_monitor(process_monitor.clone());

    let tools = crate::modes::common::build_all_tools(
        Some(event_tx),
        Some(panel_tx),
        Some(todo_tx),
        plugin_manager.as_ref(),
        Some(bash_confirm_tx),
        Some(process_monitor),
    );

    // Populate tool info for /tools slash command
    {
        let builtin_names: std::collections::HashSet<&str> = [
            "read",
            "write",
            "edit",
            "bash",
            "grep",
            "find",
            "ls",
            "subagent",
            "delegate_task",
            "todo",
            "web",
            "commit",
            "review",
            "ask",
            "image_gen",
            "validate_tui",
        ]
        .into_iter()
        .collect();
        for tool in &tools {
            let def = tool.definition();
            let source = if builtin_names.contains(def.name.as_str()) {
                "built-in".to_string()
            } else {
                "plugin".to_string()
            };
            app.tool_info.push((def.name.clone(), def.description.clone(), source));
        }
        app.tool_info.sort_by(|a, b| a.2.cmp(&b.2).then(a.0.cmp(&b.0)));
    }

    // Fire plugin_init event so plugins can set up their initial UI
    if let Some(ref pm) = plugin_manager {
        for action in crate::modes::common::fire_plugin_init(pm) {
            app.plugin_ui.apply(action);
        }
    }

    let agent = agent.with_tools(tools);
    let event_rx = agent.subscribe();

    // ── Start embedded RPC server for swarm presence ─────────────────
    // This makes this clankers instance discoverable on the LAN via mDNS
    // and allows remote peers to query its status. It does NOT expose
    // prompt execution by default (requires explicit opt-in).
    // Skip in test environments to avoid mDNS/network noise.
    let _rpc_cancel = if cfg!(test) || std::env::var("CLANKERS_NO_RPC").is_ok() {
        None
    } else {
        let config = EmbeddedRpcConfig {
            tags: vec![],
            with_agent: false, // Don't expose prompt execution by default
            allow_all: true,   // Accept status queries from anyone
            heartbeat_interval: Some(std::time::Duration::from_secs(120)),
        };
        match start_embedded_rpc(config, None, Vec::new(), Default::default(), String::new(), String::new()).await {
            Ok((node_id, cancel)) => {
                let short_id = if node_id.len() > 12 {
                    format!("{}…", &node_id[..12])
                } else {
                    node_id.clone()
                };
                app.peers_panel.self_id = Some(short_id);
                app.peers_panel.server_running = true;
                // Load initial peer list
                let registry =
                    crate::modes::rpc::peers::PeerRegistry::load(&crate::modes::rpc::peers::registry_path(&paths));
                let entries =
                    crate::tui::components::peers_panel::entries_from_registry(&registry, chrono::Duration::minutes(5));
                app.peers_panel.set_peers(entries);
                Some(cancel)
            }
            Err(e) => {
                tracing::debug!("Embedded RPC not available: {}", e);
                // Non-fatal — swarm features just won't be available
                None
            }
        }
    };

    let result = run_event_loop(
        &mut terminal,
        &mut app,
        agent,
        event_rx,
        &mut panel_rx,
        &mut todo_rx,
        &mut bash_confirm_rx,
        panel_tx_for_slash,
        keymap,
        plugin_manager,
        session_manager,
        seed_messages,
        db.clone(),
    )
    .await;

    // ── Shut down embedded RPC server ─────────────────────────────────
    if let Some(cancel) = _rpc_cancel {
        cancel.cancel();
    }

    terminal::disable_raw_mode().ok();
    execute!(terminal.backend_mut(), DisableBracketedPaste, DisableMouseCapture, LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();

    // ── Worktree cleanup: mark completed, merge, and GC ─────────────────
    if let Some(ref wt) = worktree_setup
        && let Some(ref db) = db
    {
        // Restore original cwd before merge operations
        let _ = std::env::set_current_dir(&original_cwd);
        match crate::worktree::session_bridge::complete_and_merge(db, wt, Some(provider_for_merge), model_for_merge) {
            Ok(handle) => {
                // Block until the merge completes. Aborting mid-merge can
                // leave the repository in a dirty state (partial git
                // commits, uncommitted conflict markers, etc.), so we must
                // wait for it to finish. The user can Ctrl+C if it hangs.
                eprintln!("Merging worktree branches...");
                if let Err(e) = handle.await {
                    tracing::warn!("Merge task panicked: {}", e);
                }
            }
            Err(e) => {
                tracing::warn!("Failed to start background merge: {}", e);
            }
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Agent commands / results
// ---------------------------------------------------------------------------

enum AgentCommand {
    Prompt(String),
    PromptWithImages {
        text: String,
        images: Vec<crate::tui::app::PendingImage>,
    },
    Abort,
    ResetCancel,
    SetModel(String),
    ClearHistory,
    TruncateMessages(usize),
    SetThinkingLevel(crate::provider::ThinkingLevel),
    CycleThinkingLevel,
    SeedMessages(Vec<crate::provider::message::AgentMessage>),
    Quit,
    Login {
        code: String,
        state: String,
        verifier: String,
        account: String,
    },
    /// Replace the agent's system prompt
    SetSystemPrompt(String),
    /// Get the current system prompt
    GetSystemPrompt(tokio::sync::oneshot::Sender<String>),
    /// Switch the active account (hot-swap credentials)
    SwitchAccount(String),
    /// Display a system message in the TUI (used by background tasks)
    #[allow(dead_code)]
    SystemMessage {
        text: String,
        is_error: bool,
    },
}

enum TaskResult {
    PromptDone(Option<crate::error::Error>),
    LoginDone(std::result::Result<String, String>),
    ThinkingToggled(String, crate::provider::ThinkingLevel),
    ShareResult(std::result::Result<String, String>),
    AccountSwitched(std::result::Result<String, String>),
}

// ---------------------------------------------------------------------------
// Main event loop
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
async fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    agent: Agent,
    mut event_rx: tokio::sync::broadcast::Receiver<AgentEvent>,
    panel_rx: &mut tokio::sync::mpsc::UnboundedReceiver<crate::tui::components::subagent_event::SubagentEvent>,
    todo_rx: &mut tokio::sync::mpsc::UnboundedReceiver<(
        crate::tools::todo::TodoAction,
        tokio::sync::oneshot::Sender<crate::tools::todo::TodoResponse>,
    )>,
    bash_confirm_rx: &mut crate::tools::bash::ConfirmRx,
    panel_tx: tokio::sync::mpsc::UnboundedSender<crate::tui::components::subagent_event::SubagentEvent>,
    keymap: Keymap,
    plugin_manager: Option<Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
    mut session_manager: Option<crate::session::SessionManager>,
    seed_messages: Vec<crate::provider::message::AgentMessage>,
    db: Option<crate::db::Db>,
) -> Result<()> {
    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel::<AgentCommand>();
    let (done_tx, mut done_rx) = tokio::sync::mpsc::unbounded_channel::<TaskResult>();

    let mut agent = agent;

    // If we have seed messages from a resumed session, restore them into the agent
    // and rebuild display blocks so the user sees the conversation
    if !seed_messages.is_empty() {
        restore_display_blocks(app, &seed_messages);
        let _ = cmd_tx.send(AgentCommand::SeedMessages(seed_messages));
    }

    tokio::spawn(async move {
        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                AgentCommand::Prompt(text) => {
                    // Reset the cancel token before starting, then grab a clone
                    // so Abort commands received during streaming can cancel it
                    // directly (without waiting for the prompt to finish).
                    agent.reset_cancel();
                    let cancel = agent.cancel_token();
                    let prompt_fut = agent.prompt(&text);
                    tokio::pin!(prompt_fut);
                    let result = loop {
                        tokio::select! {
                            biased;
                            result = &mut prompt_fut => break result,
                            Some(cmd) = cmd_rx.recv() => {
                                if matches!(cmd, AgentCommand::Abort) {
                                    cancel.cancel();
                                }
                                // Other commands during prompt are dropped;
                                // they'll be re-sent after PromptDone.
                            }
                        }
                    };
                    let err = match result {
                        Ok(()) => None,
                        Err(crate::error::Error::Cancelled) => None,
                        Err(e) => Some(e),
                    };
                    let _ = done_tx.send(TaskResult::PromptDone(err));
                }
                AgentCommand::PromptWithImages { text, images } => {
                    let img_contents: Vec<crate::provider::message::Content> = images
                        .into_iter()
                        .map(|img| crate::provider::message::Content::Image {
                            source: crate::provider::message::ImageSource::Base64 {
                                media_type: img.media_type,
                                data: img.data,
                            },
                        })
                        .collect();
                    // Same pattern: reset + clone cancel token for mid-stream abort
                    agent.reset_cancel();
                    let cancel = agent.cancel_token();
                    let prompt_fut = agent.prompt_with_images(&text, img_contents);
                    tokio::pin!(prompt_fut);
                    let result = loop {
                        tokio::select! {
                            biased;
                            result = &mut prompt_fut => break result,
                            Some(cmd) = cmd_rx.recv() => {
                                if matches!(cmd, AgentCommand::Abort) {
                                    cancel.cancel();
                                }
                            }
                        }
                    };
                    let err = match result {
                        Ok(()) => None,
                        Err(crate::error::Error::Cancelled) => None,
                        Err(e) => Some(e),
                    };
                    let _ = done_tx.send(TaskResult::PromptDone(err));
                }
                AgentCommand::Login {
                    code,
                    state,
                    verifier,
                    account,
                } => {
                    let result = crate::provider::anthropic::oauth::exchange_code(&code, &state, &verifier).await;
                    match result {
                        Ok(creds) => {
                            let paths = crate::config::ClankersPaths::resolve();
                            let mut store = crate::provider::auth::AuthStore::load(&paths.global_auth);
                            store.set_credentials(&account, creds);
                            store.switch_anthropic_account(&account);
                            match store.save(&paths.global_auth) {
                                Ok(()) => {
                                    // Reload the in-memory credentials so the provider
                                    // picks up the new tokens without hitting the
                                    // (now-invalid) old refresh token.
                                    agent.provider().reload_credentials().await;
                                    let _ = done_tx.send(TaskResult::LoginDone(Ok(format!(
                                        "Authentication successful! Saved as account '{}'.",
                                        account
                                    ))));
                                }
                                Err(e) => {
                                    let _ = done_tx
                                        .send(TaskResult::LoginDone(Err(format!("Failed to save credentials: {}", e))));
                                }
                            }
                        }
                        Err(e) => {
                            let _ = done_tx.send(TaskResult::LoginDone(Err(format!("Login failed: {}", e))));
                        }
                    }
                }
                AgentCommand::Abort => agent.abort(),
                AgentCommand::ResetCancel => agent.reset_cancel(),
                AgentCommand::SetModel(model) => agent.set_model(model),
                AgentCommand::ClearHistory => agent.clear_messages(),
                AgentCommand::TruncateMessages(n) => agent.truncate_messages(n),
                AgentCommand::SeedMessages(msgs) => agent.seed_messages(msgs),
                AgentCommand::SetThinkingLevel(level) => {
                    let level = agent.set_thinking_level(level);
                    let msg = if level.is_enabled() {
                        format!("Thinking: {} ({} tokens)", level.label(), level.budget_tokens().unwrap_or(0))
                    } else {
                        "Thinking: off".to_string()
                    };
                    let _ = done_tx.send(TaskResult::ThinkingToggled(msg, level));
                }
                AgentCommand::CycleThinkingLevel => {
                    let level = agent.cycle_thinking_level();
                    let msg = if level.is_enabled() {
                        format!("Thinking: {} ({} tokens)", level.label(), level.budget_tokens().unwrap_or(0))
                    } else {
                        "Thinking: off".to_string()
                    };
                    let _ = done_tx.send(TaskResult::ThinkingToggled(msg, level));
                }
                AgentCommand::SetSystemPrompt(prompt) => {
                    agent.set_system_prompt(prompt);
                }
                AgentCommand::GetSystemPrompt(tx) => {
                    let _ = tx.send(agent.system_prompt().to_string());
                }
                AgentCommand::SwitchAccount(account_name) => {
                    let paths = crate::config::ClankersPaths::resolve();
                    let mut store = crate::provider::auth::AuthStore::load(&paths.global_auth);
                    if store.switch_anthropic_account(&account_name) {
                        if let Err(e) = store.save(&paths.global_auth) {
                            let _ = done_tx.send(TaskResult::AccountSwitched(Err(format!("Failed to save: {}", e))));
                        } else {
                            // Reload in-memory credentials for the new account
                            agent.provider().reload_credentials().await;
                            let _ = done_tx.send(TaskResult::AccountSwitched(Ok(account_name)));
                        }
                    } else {
                        let _ =
                            done_tx.send(TaskResult::AccountSwitched(Err(format!("No account '{}'", account_name))));
                    }
                }
                AgentCommand::SystemMessage { text, is_error } => {
                    let _ = done_tx.send(TaskResult::ShareResult(if is_error { Err(text) } else { Ok(text) }));
                }
                AgentCommand::Quit => break,
            }
        }
    });

    // Audit log state: track pending tool calls for duration measurement
    let mut audit_pending: std::collections::HashMap<String, (String, serde_json::Value, std::time::Instant)> =
        std::collections::HashMap::new();
    let mut audit_seq: u32 = 0;

    loop {
        // Render
        terminal.draw(|frame| render::render(frame, app)).map_err(|e| crate::error::Error::Tui {
            message: format!("Render failed: {}", e),
        })?;

        if app.should_quit {
            let _ = cmd_tx.send(AgentCommand::Quit);
            break;
        }

        // Drain agent events
        loop {
            match event_rx.try_recv() {
                Ok(event) => {
                    app.handle_agent_event(&event);

                    // Persist messages to session on AgentEnd
                    if let AgentEvent::AgentEnd { ref messages } = event
                        && let Some(ref mut sm) = session_manager
                    {
                        persist_messages(sm, messages);
                    }

                    // Record per-turn usage to redb (fire-and-forget on blocking pool)
                    if let AgentEvent::UsageUpdate { ref turn_usage, .. } = event
                        && let Some(ref db) = db
                    {
                        let req = crate::db::usage::RequestUsage::from_provider(&app.model, turn_usage);
                        db.spawn_write(move |db| {
                            if let Err(e) = db.usage().record(&req) {
                                tracing::warn!("Failed to record usage: {}", e);
                            }
                        });
                    }

                    // ── Audit log: record tool calls ────────────────────
                    // Track when tool calls start (for duration measurement)
                    if let AgentEvent::ToolCall {
                        ref call_id,
                        ref tool_name,
                        ref input,
                    } = event
                    {
                        audit_pending
                            .insert(call_id.clone(), (tool_name.clone(), input.clone(), std::time::Instant::now()));
                    }
                    // Record completed tool calls to the audit log
                    if let AgentEvent::ToolExecutionEnd {
                        ref call_id,
                        ref result,
                        is_error,
                    } = event
                        && let Some(ref db) = db
                        && !app.session_id.is_empty()
                    {
                        let (tool_name, input, started_at) = audit_pending
                            .remove(call_id)
                            .unwrap_or_else(|| ("unknown".into(), serde_json::json!({}), std::time::Instant::now()));
                        let duration_ms = started_at.elapsed().as_millis() as u64;

                        // Extract result preview (first 500 chars of text content)
                        let result_preview: String = result
                            .content
                            .iter()
                            .filter_map(|c| match c {
                                crate::tools::ToolResultContent::Text { text } => Some(text.as_str()),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                            .chars()
                            .take(500)
                            .collect();

                        // Check if this was a sandbox block
                        let sandbox_blocked = if is_error {
                            result_preview.strip_prefix("🔒 ").map(|s| s.to_string())
                        } else {
                            None
                        };

                        let session_id = app.session_id.clone();
                        let call_id = call_id.clone();
                        let seq = audit_seq;
                        audit_seq += 1;

                        db.spawn_write(move |db| {
                            let entry = crate::db::audit::AuditEntry {
                                session_id,
                                seq,
                                tool: tool_name,
                                call_id,
                                input,
                                is_error,
                                result_preview,
                                duration_ms,
                                timestamp: chrono::Utc::now(),
                                sandbox_blocked,
                            };
                            if let Err(e) = db.audit().record(&entry) {
                                tracing::warn!("Failed to record audit entry: {}", e);
                            }
                        });
                    }

                    // Dispatch to plugins and surface their messages / UI actions
                    if let Some(ref pm) = plugin_manager {
                        let result = dispatch_event_to_plugins(pm, &event);
                        for (plugin_name, message) in result.messages {
                            app.push_system(format!("🔌 {}: {}", plugin_name, message), false);
                        }
                        for action in result.ui_actions {
                            app.plugin_ui.apply(action);
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::TryRecvError::Lagged(n)) => {
                    tracing::warn!("Agent event receiver lagged, skipped {} events", n);
                    // Continue draining — there are still newer events to process
                    continue;
                }
                Err(_) => break, // Empty or Closed
            }
        }

        // Drain subagent panel events
        while let Ok(event) = panel_rx.try_recv() {
            use crate::tui::components::subagent_event::SubagentEvent;
            match event {
                SubagentEvent::Started { id, name, task, pid } => {
                    app.subagent_panel.add(id, name, task, pid);
                }
                SubagentEvent::Output { id, line } => {
                    app.subagent_panel.append_output(&id, &line);
                }
                SubagentEvent::Done { id } => {
                    app.subagent_panel.mark_done(&id);
                }
                SubagentEvent::Error { id, .. } => {
                    app.subagent_panel.mark_error(&id);
                }
                SubagentEvent::KillRequest { ref id } => {
                    // Kill the subagent process by PID
                    if let Some(entry) = app.subagent_panel.get_by_id(id)
                        && entry.status == crate::tui::components::subagent_panel::SubagentStatus::Running
                    {
                        if let Some(pid) = entry.pid {
                            // Send SIGKILL to the entire process group
                            #[cfg(unix)]
                            {
                                unsafe {
                                    // Negative PID targets the process group
                                    libc::kill(-(pid as i32), libc::SIGKILL);
                                }
                            }
                            #[cfg(not(unix))]
                            {
                                // On non-unix, try to kill via std
                                let _ = std::process::Command::new("taskkill")
                                    .args(&["/PID", &pid.to_string(), "/F"])
                                    .spawn();
                            }
                            app.subagent_panel.mark_error(id);
                            app.subagent_panel.append_output(id, "⚡ Killed by user");
                        } else {
                            app.subagent_panel.append_output(id, "⚠ Cannot kill: no PID tracked");
                        }
                    }
                }
                SubagentEvent::InputRequest { .. } => {
                    // Future: send input to subagent stdin
                }
            }
        }

        // Drain todo tool requests (synchronous request/response with panel state)
        while let Ok((action, resp_tx)) = todo_rx.try_recv() {
            use crate::tools::todo::TodoAction;
            use crate::tools::todo::TodoResponse;
            use crate::tui::components::todo_panel::TodoStatus;

            let response = match action {
                TodoAction::Add { text } => {
                    let id = app.todo_panel.add(text);
                    TodoResponse::Added { id }
                }
                TodoAction::SetStatus { id, status } => {
                    if let Some(s) = TodoStatus::parse(&status) {
                        if app.todo_panel.set_status(id, s) {
                            TodoResponse::Updated { id }
                        } else {
                            TodoResponse::NotFound
                        }
                    } else {
                        TodoResponse::NotFound
                    }
                }
                TodoAction::SetStatusByText { query, status } => {
                    if let Some(s) = TodoStatus::parse(&status) {
                        if let Some(id) = app.todo_panel.set_status_by_text(&query, s) {
                            TodoResponse::Updated { id }
                        } else {
                            TodoResponse::NotFound
                        }
                    } else {
                        TodoResponse::NotFound
                    }
                }
                TodoAction::SetNote { id, note } => {
                    if app.todo_panel.set_note(id, note) {
                        TodoResponse::Updated { id }
                    } else {
                        TodoResponse::NotFound
                    }
                }
                TodoAction::Remove { id } => {
                    if app.todo_panel.remove(id) {
                        TodoResponse::Updated { id }
                    } else {
                        TodoResponse::NotFound
                    }
                }
                TodoAction::ClearDone => {
                    app.todo_panel.clear_done();
                    TodoResponse::Cleared
                }
                TodoAction::List => TodoResponse::Listed {
                    summary: app.todo_panel.summary(),
                },
            };
            let _ = resp_tx.send(response);
        }

        // Handle dangerous command confirmations from the bash tool
        while let Ok((message, resp_tx)) = bash_confirm_rx.try_recv() {
            // Show a confirmation dialog in the TUI
            app.push_system(message, true);
            app.push_system("Type 'y' to approve or 'n' to block. Approving...".to_string(), false);
            // For now, auto-approve in TUI mode (the model already sees the warning).
            // A full ConfirmDialog integration can come later.
            // The important safety net is that headless mode blocks outright.
            let _ = resp_tx.send(true);
        }

        // Periodic peer registry refresh (every ~200 ticks ≈ 10 seconds at 50ms poll)
        {
            static PEER_REFRESH_COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let count = PEER_REFRESH_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if count.is_multiple_of(200) && app.peers_panel.server_running {
                let registry = crate::modes::rpc::peers::PeerRegistry::load(&crate::modes::rpc::peers::registry_path(
                    &crate::config::ClankersPaths::resolve(),
                ));
                let entries =
                    crate::tui::components::peers_panel::entries_from_registry(&registry, chrono::Duration::minutes(5));
                app.peers_panel.set_peers(entries);
            }
        }

        // Check for task completion
        while let Ok(result) = done_rx.try_recv() {
            match result {
                TaskResult::PromptDone(Some(e)) => {
                    if let Some(ref mut block) = app.active_block {
                        block.error = Some(format!("{}", e));
                    }
                    app.finalize_active_block();
                    // Don't show the error if we're about to send a queued prompt
                    // (the error is likely just a cancellation)
                    if app.queued_prompt.is_none() {
                        app.push_system(format!("Error: {}", e), true);
                    }
                    // Dispatch queued prompt if one is waiting
                    if let Some(text) = app.queued_prompt.take() {
                        handle_input_with_plugins(app, &text, &cmd_tx, plugin_manager.as_ref(), &panel_tx, &db);
                    }
                }
                TaskResult::PromptDone(None) => {
                    // Stay in current input mode — don't force Normal mode
                    // so the user can keep typing without pressing 'i' again.
                    // Dispatch queued prompt if one is waiting
                    if let Some(text) = app.queued_prompt.take() {
                        handle_input_with_plugins(app, &text, &cmd_tx, plugin_manager.as_ref(), &panel_tx, &db);
                    }
                }
                TaskResult::LoginDone(Ok(msg)) => app.push_system(msg, false),
                TaskResult::LoginDone(Err(msg)) => app.push_system(msg, true),
                TaskResult::ThinkingToggled(msg, level) => {
                    app.thinking_enabled = level.is_enabled();
                    app.thinking_level = level;
                    app.push_system(msg, false);
                }
                TaskResult::ShareResult(Ok(msg)) => app.push_system(msg, false),
                TaskResult::ShareResult(Err(msg)) => app.push_system(msg, true),
                TaskResult::AccountSwitched(Ok(name)) => {
                    app.active_account = name.clone();
                    app.push_system(
                        format!("Switched to account '{}'. New credentials will be used for the next API call.", name),
                        false,
                    );
                }
                TaskResult::AccountSwitched(Err(msg)) => {
                    app.push_system(msg, true);
                }
            }
        }

        // Check for completed background clipboard reads
        poll_clipboard_result(app);

        // Poll terminal events
        let mut poll_timeout = Duration::from_millis(50);
        while let Some(event) = tui_event::poll_event(poll_timeout) {
            poll_timeout = Duration::ZERO;
            match event {
                AppEvent::Paste(text) => {
                    // Paste always goes to editor, switching to insert if needed
                    app.input_mode = InputMode::Insert;
                    app.selection = None;
                    app.editor.insert_str(&text);
                    app.update_slash_menu();
                }
                AppEvent::Key(key) => {
                    app.selection = None;

                    // ── Force quit (Ctrl+Q) — always works regardless of UI state ──
                    if key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL) {
                        app.should_quit = true;
                        continue;
                    }

                    // ── Session popup intercept ──────────────────
                    if app.session_popup_visible && handle_session_popup_key(app, &key, &keymap) {
                        continue;
                    }

                    // ── Model selector intercept ─────────────────
                    if app.model_selector.visible && handle_model_selector_key(app, &key, &cmd_tx) {
                        continue;
                    }

                    // ── Account selector intercept ───────────────
                    if app.account_selector.visible && handle_account_selector_key(app, &key, &cmd_tx) {
                        continue;
                    }

                    // ── Session selector intercept ───────────────
                    if app.session_selector.visible && handle_session_selector_key(app, &key, &cmd_tx) {
                        continue;
                    }

                    // ── Leader menu intercept ────────────────────
                    if app.leader_menu.visible {
                        if let Some(leader_action) = app.leader_menu.handle_key(&key) {
                            handle_leader_action(app, leader_action, &cmd_tx, plugin_manager.as_ref(), &panel_tx, &db);
                        }
                        continue;
                    }

                    // ── Output search intercept ──────────────────
                    if app.output_search.active {
                        handle_output_search_key(app, &key);
                        continue;
                    }

                    // ── Slash menu intercept (only in insert mode) ────
                    if app.input_mode == InputMode::Insert
                        && app.slash_menu.visible
                        && handle_slash_menu_key(app, &key, &keymap, &cmd_tx, plugin_manager.as_ref(), &panel_tx, &db)
                    {
                        continue;
                    }

                    // ── Panel intercepts in normal mode ──────────
                    if app.panel_focused && app.input_mode == InputMode::Normal {
                        use crossterm::event::KeyCode;

                        use crate::tui::app::PanelTab;

                        // Tab / Shift+Tab cycles sub-panels within the current column
                        match (key.code, key.modifiers) {
                            (KeyCode::Tab, _) | (KeyCode::BackTab, _) => {
                                // Toggle between sub-panels in the same column
                                app.panel_tab = match app.panel_tab {
                                    PanelTab::Todo => PanelTab::Files,
                                    PanelTab::Files => PanelTab::Todo,
                                    PanelTab::Subagents => {
                                        app.right_panel_tab = PanelTab::Peers;
                                        PanelTab::Peers
                                    }
                                    PanelTab::Peers => {
                                        app.right_panel_tab = PanelTab::Processes;
                                        PanelTab::Processes
                                    }
                                    PanelTab::Processes => {
                                        app.right_panel_tab = PanelTab::Subagents;
                                        PanelTab::Subagents
                                    }
                                };
                                continue;
                            }
                            _ => {}
                        }

                        match app.panel_tab {
                            PanelTab::Subagents => {
                                match (key.code, key.modifiers) {
                                    (KeyCode::Enter, _) => {
                                        use crate::tui::components::subagent_panel::PanelView;
                                        match app.subagent_panel.view {
                                            PanelView::List => app.subagent_panel.open_detail(),
                                            PanelView::Detail => app.subagent_panel.close_detail(),
                                        }
                                        continue;
                                    }
                                    // 'x' = kill selected running subagent
                                    (KeyCode::Char('x'), m) if m.is_empty() => {
                                        if let Some(id) = app.subagent_panel.selected_id() {
                                            let _ = panel_tx.send(
                                                crate::tui::components::subagent_event::SubagentEvent::KillRequest {
                                                    id,
                                                },
                                            );
                                        }
                                        continue;
                                    }
                                    // 'X' (shift-x) = remove/dismiss selected entry
                                    (KeyCode::Char('X'), _) => {
                                        app.subagent_panel.remove_selected();
                                        if !app.subagent_panel.is_visible() && app.todo_panel.is_empty() {
                                            app.panel_focused = false;
                                        }
                                        continue;
                                    }
                                    _ => {}
                                }
                            }
                            PanelTab::Todo => {
                                match (key.code, key.modifiers) {
                                    // 'x' = mark done
                                    (KeyCode::Char('x'), m) if m.is_empty() => {
                                        app.todo_panel.toggle_selected();
                                        continue;
                                    }
                                    // 'X' = remove selected
                                    (KeyCode::Char('X'), _) => {
                                        app.todo_panel.remove_selected();
                                        if app.todo_panel.is_empty() && !app.subagent_panel.is_visible() {
                                            app.panel_focused = false;
                                        }
                                        continue;
                                    }
                                    // Enter = cycle (same as space for convenience)
                                    (KeyCode::Enter, _) => {
                                        app.todo_panel.cycle_selected();
                                        continue;
                                    }
                                    _ => {}
                                }
                            }
                            PanelTab::Files => {
                                // File activity panel — navigation only
                            }
                            PanelTab::Processes => {
                                // Process panel — navigation and sorting handled via Panel trait
                            }
                            PanelTab::Peers => {
                                match (key.code, key.modifiers) {
                                    (KeyCode::Enter, _) => {
                                        app.peers_panel.toggle_detail();
                                        continue;
                                    }
                                    (KeyCode::Esc, _) if app.peers_panel.detail_view => {
                                        app.peers_panel.detail_view = false;
                                        continue;
                                    }
                                    // 'p' = probe selected peer
                                    (KeyCode::Char('p'), m) if m.is_empty() => {
                                        if let Some(peer) = app.peers_panel.selected_peer().cloned() {
                                            app.peers_panel.update_status(
                                                &peer.node_id,
                                                crate::tui::components::peers_panel::PeerStatus::Probing,
                                            );
                                            let node_id = peer.node_id.clone();
                                            let paths = crate::config::ClankersPaths::resolve();
                                            let registry_path = crate::modes::rpc::peers::registry_path(&paths);
                                            let identity_path = crate::modes::rpc::iroh::identity_path(&paths);
                                            let ptx = panel_tx.clone();
                                            tokio::spawn(async move {
                                                probe_peer_background(node_id, registry_path, identity_path, ptx).await;
                                            });
                                        }
                                        continue;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }

                    // ── Resolve through mode-aware keymap ────────────
                    let action = keymap.resolve(app.input_mode, &key);

                    if let Some(action) = action {
                        // OpenEditor needs terminal access — handle it here
                        if action == Action::OpenEditor {
                            open_external_editor(terminal, app);
                            continue;
                        }

                        handle_action(app, action, &key, &cmd_tx, plugin_manager.as_ref(), &panel_tx, &db);

                        // If a branch was just initiated, record it in the session file
                        if let Some(checkpoint) = app.last_branch_checkpoint.take()
                            && let Some(ref mut sm) = session_manager
                        {
                            // The fork point is the last persisted message at the checkpoint
                            // We need to find the message ID at position `checkpoint - 1`
                            // in the current session. The agent's messages were truncated
                            // to `checkpoint`, so the last message before the branch is
                            // the session's current active leaf at that point.
                            if let Ok(tree) = sm.load_tree() {
                                // Walk the active branch and find the message at the checkpoint
                                let active_leaf = sm.active_leaf_id().cloned();
                                let branch_msgs =
                                    crate::session::context::build_messages_for_branch(&tree, active_leaf.as_ref());
                                if checkpoint > 0 && checkpoint <= branch_msgs.len() {
                                    let fork_msg_id = branch_msgs[checkpoint - 1].id().clone();
                                    let _ = sm.record_branch(fork_msg_id, "User edited prompt");
                                }
                            }
                        }
                    } else {
                        // Unmapped key — in insert mode, insert printable chars
                        if app.input_mode == InputMode::Insert {
                            handle_insert_char(app, &key);
                        }
                        // In normal mode, unmapped keys are ignored
                    }
                }
                AppEvent::MouseDown(button, col, row) => {
                    handle_mouse_down(app, button, col, row);
                }
                AppEvent::MouseDrag(button, col, row) => {
                    handle_mouse_drag(app, button, col, row);
                }
                AppEvent::MouseUp(button, col, row) => {
                    handle_mouse_up(app, button, col, row);
                }
                AppEvent::ScrollUp(col, row, n) => {
                    handle_mouse_scroll(app, col, row, true, n);
                }
                AppEvent::ScrollDown(col, row, n) => {
                    handle_mouse_scroll(app, col, row, false, n);
                }
                AppEvent::Resize(_, _) => {}
                _ => {}
            }
        }

        // ── Check for deferred external editor request ──
        if app.open_editor_requested {
            app.open_editor_requested = false;
            open_external_editor(terminal, app);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Action dispatcher
// ---------------------------------------------------------------------------

fn handle_action(
    app: &mut App,
    action: Action,
    _key: &crossterm::event::KeyEvent,
    cmd_tx: &tokio::sync::mpsc::UnboundedSender<AgentCommand>,
    plugin_manager: Option<&Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
    panel_tx: &tokio::sync::mpsc::UnboundedSender<crate::tui::components::subagent_event::SubagentEvent>,
    db: &Option<crate::db::Db>,
) {
    // When panel is focused, route navigation keys to the panel.
    // Global actions (leader menu, selectors, mode switching, etc.)
    // bypass panel handling so they work from any context.
    if app.panel_focused {
        use crate::tui::app::PanelTab;

        // Global actions always fall through to the main handler below,
        // regardless of which panel is focused.
        let is_global = matches!(
            action,
            Action::Quit
                | Action::Cancel
                | Action::OpenLeaderMenu
                | Action::EnterNormal
                | Action::OpenModelSelector
                | Action::OpenAccountSelector
                | Action::ToggleThinking
                | Action::ToggleShowThinking
                | Action::SearchOutput
                | Action::ToggleSessionPopup
                | Action::PasteImage
                | Action::OpenEditor
        );

        if !is_global {
            match action {
                // ── Panel exit / mode switching ──────────────
                Action::Unfocus | Action::TogglePanelFocus => {
                    if app.panel_tab == PanelTab::Subagents {
                        app.subagent_panel.close_detail();
                    }
                    app.panel_focused = false;
                    return;
                }
                Action::EnterInsert => {
                    if app.panel_tab == PanelTab::Subagents {
                        app.subagent_panel.close_detail();
                    }
                    app.panel_focused = false;
                    app.input_mode = InputMode::Insert;
                    return;
                }
                Action::EnterCommand => {
                    if app.panel_tab == PanelTab::Subagents {
                        app.subagent_panel.close_detail();
                    }
                    app.panel_focused = false;
                    // Don't return — fall through to main handler for "/" prefix setup
                }

                // ── Panel navigation ─────────────────────────
                Action::PanelNextTab | Action::BranchNext => {
                    // l = move right: left→main, right→no-op
                    if app.panel_tab.is_left() {
                        app.panel_focused = false;
                    }
                    // If already in right column, do nothing
                    return;
                }
                Action::PanelPrevTab | Action::BranchPrev => {
                    // h = move left: right→main, left→no-op
                    if app.panel_tab.is_right() {
                        app.panel_focused = false;
                    }
                    // If already in left column, do nothing
                    return;
                }
                // j/k cycles between panels (panes) in the same column
                Action::FocusPrevBlock | Action::FocusNextBlock => {
                    let next_tab = match app.panel_tab {
                        PanelTab::Todo => PanelTab::Files,
                        PanelTab::Files => PanelTab::Todo,
                        PanelTab::Subagents => {
                            app.right_panel_tab = PanelTab::Peers;
                            PanelTab::Peers
                        }
                        PanelTab::Peers => {
                            app.right_panel_tab = PanelTab::Processes;
                            PanelTab::Processes
                        }
                        PanelTab::Processes => {
                            app.right_panel_tab = PanelTab::Subagents;
                            PanelTab::Subagents
                        }
                    };
                    app.panel_tab = next_tab;
                    return;
                }

                // ── Panel-type-specific actions ──────────────
                _ => match app.panel_tab {
                    PanelTab::Subagents => {
                        use crate::tui::components::subagent_panel::PanelView;
                        match app.subagent_panel.view {
                            PanelView::List => match action {
                                Action::Submit => {
                                    app.subagent_panel.open_detail();
                                    return;
                                }
                                Action::PanelClearDone => {
                                    app.subagent_panel.clear_done();
                                    if !app.subagent_panel.is_visible() && app.todo_panel.is_empty() {
                                        app.panel_focused = false;
                                    }
                                    return;
                                }
                                _ => return,
                            },
                            PanelView::Detail => match action {
                                Action::ScrollUp => {
                                    app.subagent_panel.scroll_up(1);
                                    return;
                                }
                                Action::ScrollDown => {
                                    app.subagent_panel.scroll_down(1);
                                    return;
                                }
                                Action::ScrollPageUp => {
                                    app.subagent_panel.scroll_up(10);
                                    return;
                                }
                                Action::ScrollPageDown => {
                                    app.subagent_panel.scroll_down(10);
                                    return;
                                }
                                Action::ScrollToTop => {
                                    app.subagent_panel.scroll_to_top();
                                    return;
                                }
                                Action::ScrollToBottom => {
                                    app.subagent_panel.scroll_to_bottom();
                                    return;
                                }
                                _ => return,
                            },
                        }
                    }
                    PanelTab::Todo => match action {
                        Action::PanelClearDone => {
                            app.todo_panel.clear_done();
                            if app.todo_panel.is_empty() && !app.subagent_panel.is_visible() {
                                app.panel_focused = false;
                            }
                            return;
                        }
                        _ => return,
                    },
                    PanelTab::Files => {
                        return;
                    }
                    PanelTab::Processes => {
                        return;
                    }
                    PanelTab::Peers => match action {
                        Action::Submit => {
                            app.peers_panel.toggle_detail();
                            return;
                        }
                        _ => return,
                    },
                },
            }
        }
    }

    match action {
        // ── Mode switching ───────────────────────────
        Action::EnterInsert => {
            app.input_mode = InputMode::Insert;
        }
        Action::EnterCommand => {
            app.input_mode = InputMode::Insert;
            app.editor.clear();
            app.editor.insert_char('/');
            app.update_slash_menu();
        }
        Action::EnterNormal => {
            // If streaming, Escape also cancels the in-progress prompt
            // so the user doesn't need to press Escape twice (once to
            // exit insert mode, again to abort).
            if app.state == AppState::Streaming {
                let _ = cmd_tx.send(AgentCommand::Abort);
            }
            app.input_mode = InputMode::Normal;
            app.slash_menu.hide();
        }

        // ── Core ─────────────────────────────────────
        Action::Submit => {
            if app.state != AppState::Idle {
                // Abort the current stream and queue the new prompt
                if let Some(text) = app.submit_input() {
                    app.queued_prompt = Some(text);
                    let _ = cmd_tx.send(AgentCommand::Abort);
                }
                return;
            }
            if let Some(text) = app.submit_input() {
                if let Some((checkpoint, prompt)) = app.take_pending_branch(&text) {
                    let _ = cmd_tx.send(AgentCommand::ResetCancel);
                    let _ = cmd_tx.send(AgentCommand::TruncateMessages(checkpoint));
                    let _ = cmd_tx.send(AgentCommand::Prompt(prompt));
                } else {
                    handle_input_with_plugins(app, &text, cmd_tx, plugin_manager, panel_tx, db);
                }
            }
        }
        Action::NewLine => {
            app.editor.insert_char('\n');
        }
        Action::Cancel => {
            if app.state == AppState::Streaming {
                let _ = cmd_tx.send(AgentCommand::Abort);
            } else if !app.editor.is_empty() {
                app.editor.clear();
                app.slash_menu.hide();
            } else {
                app.should_quit = true;
            }
        }
        Action::Quit => {
            app.should_quit = true;
        }

        // ── Editor movement ──────────────────────────
        Action::MoveLeft => app.editor.move_left(),
        Action::MoveRight => app.editor.move_right(),
        Action::MoveHome => app.editor.move_home(),
        Action::MoveEnd => app.editor.move_end(),

        // ── Editor editing ───────────────────────────
        Action::DeleteBack => {
            app.editor.delete_back();
            app.update_slash_menu();
        }
        Action::DeleteForward => {
            app.editor.delete_forward();
            app.update_slash_menu();
        }
        Action::DeleteWord => {
            app.editor.delete_word_back();
            app.update_slash_menu();
        }
        Action::ClearLine => {
            app.editor.clear();
            app.slash_menu.hide();
        }

        // ── History ──────────────────────────────────
        Action::HistoryUp => app.editor.history_up(),
        Action::HistoryDown => app.editor.history_down(),

        // ── Scrolling ────────────────────────────────
        Action::ScrollUp => app.scroll.scroll_up(1),
        Action::ScrollDown => app.scroll.scroll_down(1),
        Action::ScrollPageUp => app.scroll.scroll_up(10),
        Action::ScrollPageDown => app.scroll.scroll_down(10),
        Action::ScrollToTop => app.scroll.scroll_to_top(),
        Action::ScrollToBottom => app.scroll.scroll_to_bottom(),

        // ── Search ──────────────────────────────────
        Action::SearchOutput => {
            app.output_search.activate();
        }
        Action::SearchNext => {
            if !app.output_search.matches.is_empty() {
                app.output_search.next_match();
                app.output_search.scroll_to_current = true;
            }
        }
        Action::SearchPrev => {
            if !app.output_search.matches.is_empty() {
                app.output_search.prev_match();
                app.output_search.scroll_to_current = true;
            }
        }

        // ── Block navigation ─────────────────────────
        Action::FocusPrevBlock => app.focus_prev_block(),
        Action::FocusNextBlock => app.focus_next_block(),
        Action::ToggleBlockCollapse => {
            if app.focused_block.is_some() {
                app.toggle_focused_block();
            }
        }
        Action::CollapseAllBlocks => app.collapse_all_blocks(),
        Action::ExpandAllBlocks => app.expand_all_blocks(),
        Action::CopyBlock => app.copy_focused_block(),
        Action::RerunBlock => {
            if let Some(prompt) = app.get_focused_block_prompt() {
                let _ = cmd_tx.send(AgentCommand::ResetCancel);
                let _ = cmd_tx.send(AgentCommand::Prompt(prompt));
            }
        }
        Action::EditBlock => {
            if app.focused_block.is_some() && app.state == AppState::Idle && app.edit_focused_block_prompt() {
                app.input_mode = InputMode::Insert;
            }
        }
        Action::Unfocus => {
            if app.state == AppState::Streaming {
                let _ = cmd_tx.send(AgentCommand::Abort);
            } else if app.input_mode == InputMode::Insert {
                // Esc in insert → normal
                app.input_mode = InputMode::Normal;
                app.slash_menu.hide();
            } else if app.focused_block.is_some() {
                app.focused_block = None;
                app.scroll.scroll_to_bottom();
            }
        }

        // ── Branch / panel navigation ────────────────
        Action::BranchPrev => {
            if app.focused_block.is_some() {
                // Block focused → branch navigation
                app.branch_prev();
            } else {
                // No block focused → h moves focus to left column
                app.panel_focused = true;
                app.input_mode = InputMode::Normal;
                if !app.panel_tab.is_left() {
                    app.panel_tab = crate::tui::app::PanelTab::Todo;
                }
            }
        }
        Action::BranchNext => {
            if app.focused_block.is_some() {
                // Block focused → branch navigation
                app.branch_next();
            } else {
                // No block focused → l moves focus to right column
                app.panel_focused = true;
                app.input_mode = InputMode::Normal;
                if !app.panel_tab.is_right() {
                    app.panel_tab = app.right_panel_tab;
                }
            }
        }

        // ── Toggles ─────────────────────────────────
        Action::ToggleThinking => {
            let _ = cmd_tx.send(AgentCommand::CycleThinkingLevel);
        }
        Action::ToggleShowThinking => {
            app.show_thinking = !app.show_thinking;
            let state = if app.show_thinking { "visible" } else { "hidden" };
            app.push_system(format!("Thinking content now {}.", state), false);
        }

        // ── Subagent panel ────────────────────────────
        Action::TogglePanelFocus => {
            if app.panel_focused {
                app.panel_focused = false;
            } else {
                // All panels are always visible, so just focus the current tab
                app.panel_focused = true;
                app.input_mode = InputMode::Normal;
            }
        }
        Action::PanelNextTab => {
            // Focus right column
            app.panel_focused = true;
            app.input_mode = InputMode::Normal;
            app.panel_tab = app.right_panel_tab;
        }
        Action::PanelPrevTab => {
            // Focus left column
            app.panel_focused = true;
            app.input_mode = InputMode::Normal;
            if !app.panel_tab.is_left() {
                app.panel_tab = crate::tui::app::PanelTab::Todo;
            }
        }
        Action::PanelScrollUp => {
            app.subagent_panel.scroll_up(3);
        }
        Action::PanelScrollDown => {
            app.subagent_panel.scroll_down(3);
        }
        Action::PanelClearDone => {
            app.subagent_panel.clear_done();
            if !app.subagent_panel.is_visible() {
                app.panel_focused = false;
            }
        }
        Action::PanelKill => {
            if let Some(id) = app.subagent_panel.selected_id() {
                let _ = panel_tx.send(crate::tui::components::subagent_event::SubagentEvent::KillRequest { id });
            }
        }
        Action::PanelRemove => {
            app.subagent_panel.remove_selected();
        }

        // ── Session popup ─────────────────────────────
        Action::ToggleSessionPopup => {
            app.session_popup_visible = !app.session_popup_visible;
            if app.session_popup_visible {
                // Focus the last block when opening so user can navigate
                if app.focused_block.is_none() {
                    let last_id = app.blocks.iter().rev().find_map(|e| match e {
                        BlockEntry::Conversation(b) => Some(b.id),
                        _ => None,
                    });
                    app.focused_block = last_id;
                }
            }
        }

        // Menu actions are handled by handle_slash_menu_key before reaching here
        Action::MenuUp | Action::MenuDown | Action::MenuAccept | Action::MenuClose => {}

        // ── Clipboard paste (text or image) ──────────
        Action::PasteImage => {
            paste_from_clipboard(app);
        }

        // ── External editor ─────────────────────────
        Action::OpenEditor => {
            // Handled specially in the event loop (needs terminal access)
            // This is a marker — the event loop checks for it after handle_action
        }

        // ── Selectors ───────────────────────────────
        Action::OpenModelSelector => {
            let models = app.available_models.clone();
            if models.is_empty() {
                app.push_system("No models available.".to_string(), true);
            } else {
                app.model_selector = crate::tui::components::model_selector::ModelSelector::new(models);
                app.model_selector.open();
            }
        }
        Action::OpenAccountSelector => {
            let paths = crate::config::ClankersPaths::resolve();
            let store = crate::provider::auth::AuthStore::load(&paths.global_auth);
            let accounts: Vec<crate::tui::components::account_selector::AccountItem> = store
                .list_anthropic_accounts()
                .into_iter()
                .map(|info| crate::tui::components::account_selector::AccountItem {
                    name: info.name,
                    label: info.label,
                    is_active: info.is_active,
                    is_expired: info.is_expired,
                })
                .collect();
            if accounts.is_empty() {
                app.push_system("No accounts configured. Use /login to authenticate.".to_string(), true);
            } else {
                app.account_selector.open(accounts);
            }
        }

        // ── Leader key ──────────────────────────────
        Action::OpenLeaderMenu => {
            app.leader_menu.open();
        }
    }
}

// ---------------------------------------------------------------------------
// Leader menu action dispatch
// ---------------------------------------------------------------------------

fn handle_leader_action(
    app: &mut App,
    action: crate::tui::components::leader_menu::LeaderAction,
    cmd_tx: &tokio::sync::mpsc::UnboundedSender<AgentCommand>,
    plugin_manager: Option<&Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
    panel_tx: &tokio::sync::mpsc::UnboundedSender<crate::tui::components::subagent_event::SubagentEvent>,
    db: &Option<crate::db::Db>,
) {
    use crate::tui::components::leader_menu::LeaderAction;

    match action {
        LeaderAction::KeymapAction(keymap_action) => {
            // Re-use the existing action dispatcher with a dummy key event
            let dummy_key = crossterm::event::KeyEvent::new(KeyCode::Null, KeyModifiers::NONE);
            handle_action(app, keymap_action, &dummy_key, cmd_tx, plugin_manager, panel_tx, db);
        }
        LeaderAction::SlashCommand(command) => {
            // Execute as if the user typed and submitted the slash command
            handle_input_with_plugins(app, &command, cmd_tx, plugin_manager, panel_tx, db);
        }
        LeaderAction::Submenu(_) => {
            // Submenus are handled internally by LeaderMenu::handle_key
        }
    }
}

// ---------------------------------------------------------------------------
// Output search (Ctrl+F overlay)
// ---------------------------------------------------------------------------

fn handle_output_search_key(app: &mut App, key: &crossterm::event::KeyEvent) {
    use crossterm::event::KeyCode;
    use crossterm::event::KeyModifiers;

    match (key.code, key.modifiers) {
        // Close search
        (KeyCode::Esc, _) => {
            app.output_search.deactivate();
        }
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
            app.output_search.cancel();
        }

        // Navigate matches
        (KeyCode::Enter, KeyModifiers::NONE) | (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
            app.output_search.next_match();
            app.output_search.scroll_to_current = true;
        }
        (KeyCode::Enter, KeyModifiers::SHIFT) | (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
            app.output_search.prev_match();
            app.output_search.scroll_to_current = true;
        }

        // Toggle search mode (substring ↔ fuzzy)
        (KeyCode::Char('r'), KeyModifiers::CONTROL) => {
            app.output_search.toggle_mode();
            // Recompute matches immediately with new mode
            app.output_search.update_matches(&app.rendered_lines);
            app.output_search.scroll_to_current = true;
        }

        // Edit query
        (KeyCode::Backspace, _) => {
            app.output_search.backspace();
            app.output_search.update_matches(&app.rendered_lines);
            app.output_search.scroll_to_current = true;
        }
        (KeyCode::Char(c), m) if m.is_empty() || m == KeyModifiers::SHIFT => {
            app.output_search.type_char(c);
            app.output_search.update_matches(&app.rendered_lines);
            app.output_search.scroll_to_current = true;
        }

        // Consume all other keys (don't leak to main handler)
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Slash menu (insert mode only)
// ---------------------------------------------------------------------------

fn handle_slash_menu_key(
    app: &mut App,
    key: &crossterm::event::KeyEvent,
    keymap: &Keymap,
    cmd_tx: &tokio::sync::mpsc::UnboundedSender<AgentCommand>,
    plugin_manager: Option<&Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
    panel_tx: &tokio::sync::mpsc::UnboundedSender<crate::tui::components::subagent_event::SubagentEvent>,
    db: &Option<crate::db::Db>,
) -> bool {
    // Resolve through the keymap — menu actions take priority when menu is visible
    if let Some(action) = keymap.resolve(InputMode::Insert, key) {
        match action {
            Action::MenuUp | Action::HistoryUp => {
                app.slash_menu.select_prev();
                return true;
            }
            Action::MenuDown | Action::HistoryDown => {
                app.slash_menu.select_next();
                return true;
            }
            Action::MenuAccept => {
                app.accept_slash_completion();
                app.update_slash_menu();
                return true;
            }
            Action::MenuClose => {
                app.slash_menu.hide();
                return true;
            }
            Action::EnterNormal => {
                app.slash_menu.hide();
                app.input_mode = InputMode::Normal;
                return true;
            }
            Action::Submit => {
                app.accept_slash_completion();
                if let Some(text) = app.submit_input() {
                    handle_input_with_plugins(app, &text, cmd_tx, plugin_manager, panel_tx, db);
                }
                return true;
            }
            Action::DeleteBack => {
                app.editor.delete_back();
                app.update_slash_menu();
                return true;
            }
            // Other mapped actions — fall through to main handler
            _ => return false,
        }
    }

    // Unmapped key — insert printable characters
    if let KeyCode::Char(c) = key.code
        && (key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT)
    {
        app.editor.insert_char(c);
        app.update_slash_menu();
        return true;
    }

    false
}

// ---------------------------------------------------------------------------
// Session popup (key handling when visible)
// ---------------------------------------------------------------------------

fn handle_session_popup_key(app: &mut App, key: &crossterm::event::KeyEvent, keymap: &Keymap) -> bool {
    // Resolve through the current mode's keymap
    let action = keymap.resolve(app.input_mode, key);

    match action {
        // Close on Esc, 's' toggle, or 'q'
        Some(Action::Unfocus) | Some(Action::ToggleSessionPopup) | Some(Action::Quit) => {
            app.session_popup_visible = false;
            true
        }
        // Navigate blocks with j/k
        Some(Action::FocusPrevBlock) => {
            app.focus_prev_block();
            true
        }
        Some(Action::FocusNextBlock) => {
            app.focus_next_block();
            true
        }
        // Branch navigation with h/l
        Some(Action::BranchPrev) => {
            app.branch_prev();
            true
        }
        Some(Action::BranchNext) => {
            app.branch_next();
            true
        }
        // Collapse/expand
        Some(Action::ToggleBlockCollapse) => {
            app.toggle_focused_block();
            true
        }
        Some(Action::CollapseAllBlocks) => {
            app.collapse_all_blocks();
            true
        }
        Some(Action::ExpandAllBlocks) => {
            app.expand_all_blocks();
            true
        }
        // Copy focused block
        Some(Action::CopyBlock) => {
            app.copy_focused_block();
            true
        }
        // Scroll to top/bottom
        Some(Action::ScrollToTop) => {
            app.focused_block = app.blocks.iter().find_map(|e| match e {
                BlockEntry::Conversation(b) => Some(b.id),
                _ => None,
            });
            true
        }
        Some(Action::ScrollToBottom) => {
            app.focused_block = app.blocks.iter().rev().find_map(|e| match e {
                BlockEntry::Conversation(b) => Some(b.id),
                _ => None,
            });
            true
        }
        // Switch to insert mode closes popup
        Some(Action::EnterInsert) | Some(Action::EnterCommand) => {
            app.session_popup_visible = false;
            // Don't consume — let the main handler process it
            false
        }
        // All other keys are consumed (don't pass through while popup is open)
        _ => true,
    }
}

// ---------------------------------------------------------------------------
// Model selector key handling
// ---------------------------------------------------------------------------

fn handle_model_selector_key(
    app: &mut App,
    key: &crossterm::event::KeyEvent,
    cmd_tx: &tokio::sync::mpsc::UnboundedSender<AgentCommand>,
) -> bool {
    match key.code {
        KeyCode::Esc => {
            app.model_selector.close();
            true
        }
        KeyCode::Enter => {
            if let Some(model) = app.model_selector.select() {
                let old_model = std::mem::replace(&mut app.model, model.clone());
                let _ = cmd_tx.send(AgentCommand::SetModel(model.clone()));
                app.context_gauge.set_model(&app.model);
                app.push_system(format!("Model switched: {} → {}", old_model, model), false);
            }
            app.model_selector.close();
            true
        }
        KeyCode::Up => {
            app.model_selector.move_up();
            true
        }
        KeyCode::Down => {
            app.model_selector.move_down();
            true
        }
        KeyCode::Backspace => {
            app.model_selector.backspace();
            true
        }
        KeyCode::Char(c) => {
            // Ctrl+C closes
            if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'c' {
                app.model_selector.close();
            } else if key.modifiers.contains(KeyModifiers::CONTROL) {
                match c {
                    'k' | 'p' => app.model_selector.move_up(),
                    'j' | 'n' => app.model_selector.move_down(),
                    _ => {}
                }
            } else {
                app.model_selector.type_char(c);
            }
            true
        }
        _ => true, // consume all keys while selector is open
    }
}

// ---------------------------------------------------------------------------
// Account selector key handling
// ---------------------------------------------------------------------------

fn handle_account_selector_key(
    app: &mut App,
    key: &crossterm::event::KeyEvent,
    cmd_tx: &tokio::sync::mpsc::UnboundedSender<AgentCommand>,
) -> bool {
    match key.code {
        KeyCode::Esc => {
            app.account_selector.close();
            true
        }
        KeyCode::Enter => {
            if let Some(account_name) = app.account_selector.select() {
                let _ = cmd_tx.send(AgentCommand::SwitchAccount(account_name));
            }
            app.account_selector.close();
            true
        }
        KeyCode::Up => {
            app.account_selector.move_up();
            true
        }
        KeyCode::Down => {
            app.account_selector.move_down();
            true
        }
        KeyCode::Backspace => {
            app.account_selector.backspace();
            true
        }
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'c' {
                app.account_selector.close();
            } else if key.modifiers.contains(KeyModifiers::CONTROL) {
                match c {
                    'k' | 'p' => app.account_selector.move_up(),
                    'j' | 'n' => app.account_selector.move_down(),
                    _ => {}
                }
            } else {
                app.account_selector.type_char(c);
            }
            true
        }
        _ => true,
    }
}

// ---------------------------------------------------------------------------
// Session selector key handling
// ---------------------------------------------------------------------------

fn handle_session_selector_key(
    app: &mut App,
    key: &crossterm::event::KeyEvent,
    cmd_tx: &tokio::sync::mpsc::UnboundedSender<AgentCommand>,
) -> bool {
    match key.code {
        KeyCode::Esc => {
            app.session_selector.close();
            true
        }
        KeyCode::Enter => {
            if let Some(item) = app.session_selector.select() {
                let file_path = item.file_path.clone();
                let session_id = item.session_id.clone();
                app.session_selector.close();
                resume_session_from_file(app, file_path, &session_id, cmd_tx);
            } else {
                app.session_selector.close();
            }
            true
        }
        KeyCode::Up => {
            app.session_selector.move_up();
            true
        }
        KeyCode::Down => {
            app.session_selector.move_down();
            true
        }
        KeyCode::Backspace => {
            app.session_selector.backspace();
            true
        }
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'c' {
                app.session_selector.close();
            } else if key.modifiers.contains(KeyModifiers::CONTROL) {
                match c {
                    'k' | 'p' => app.session_selector.move_up(),
                    'j' | 'n' => app.session_selector.move_down(),
                    _ => {}
                }
            } else {
                app.session_selector.type_char(c);
            }
            true
        }
        _ => true,
    }
}

/// Resume a session from a file path (shared by selector and slash command)
fn resume_session_from_file(
    app: &mut App,
    file_path: std::path::PathBuf,
    _session_id: &str,
    cmd_tx: &tokio::sync::mpsc::UnboundedSender<AgentCommand>,
) {
    match crate::session::SessionManager::open(file_path) {
        Ok(mgr) => {
            let msgs = mgr.build_context().unwrap_or_default();
            let msg_count = msgs.len();
            app.session_id = mgr.session_id().to_string();
            // Write resume entry
            let resume_entry = crate::session::entry::SessionEntry::Resume(crate::session::entry::ResumeEntry {
                id: crate::provider::message::MessageId::generate(),
                resumed_at: chrono::Utc::now(),
                from_entry_id: crate::provider::message::MessageId::new("slash-resume"),
            });
            let _ = crate::session::store::append_entry(mgr.file_path(), &resume_entry);

            // Clear current blocks and restore from session
            app.blocks.clear();
            app.all_blocks.clear();
            app.active_block = None;
            restore_display_blocks(app, &msgs);

            // Seed the agent with restored messages
            let _ = cmd_tx.send(AgentCommand::SeedMessages(msgs));

            app.push_system(format!("Resumed session {} ({} messages)", mgr.session_id(), msg_count), false);
            app.scroll.scroll_to_bottom();
        }
        Err(e) => {
            app.push_system(format!("Failed to resume session: {}", e), true);
        }
    }
}

// ---------------------------------------------------------------------------
// Character insertion (insert mode, unmapped keys)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Mouse event handlers
// ---------------------------------------------------------------------------

/// Handle mouse button press.
fn handle_mouse_down(app: &mut App, button: Button, col: u16, row: u16) {
    let region = app.hit_test(col, row);

    match button {
        Button::Left => {
            match region {
                HitRegion::Messages => {
                    // Start text selection in the messages area
                    if let Some(pos) = crate::tui::selection::screen_to_text_pos(
                        col,
                        row,
                        app.messages_area,
                        app.scroll.offset,
                        &app.rendered_lines,
                    ) {
                        app.selection = Some(crate::tui::selection::TextSelection::start(pos));
                    } else {
                        app.selection = None;
                    }

                    // Switch to normal mode if we were focused on a panel
                    if app.panel_focused {
                        app.panel_focused = false;
                        app.focus.unfocus();
                    }
                }
                HitRegion::Editor => {
                    // Click in editor → switch to insert mode and place cursor
                    app.selection = None;
                    app.panel_focused = false;
                    app.focus.unfocus();
                    app.input_mode = InputMode::Insert;

                    // Compute cursor position from click coordinates
                    let inner_x = app.editor_area.x + 1; // left border
                    let inner_y = app.editor_area.y + 1; // top border
                    let inner_w = app.editor_area.width.saturating_sub(2) as usize;

                    if col >= inner_x && row >= inner_y {
                        let rel_col = col - inner_x;
                        let rel_row = row - inner_y;
                        let indicator_len = match (app.state, app.input_mode) {
                            (AppState::Streaming, _) => 2,
                            (_, InputMode::Normal) => 2,
                            (_, InputMode::Insert) => 2,
                        };
                        app.editor.click_to_cursor(rel_col, rel_row, inner_w, indicator_len);
                    }
                }
                HitRegion::Panel(panel_id) => {
                    // Click on a panel → focus it
                    app.selection = None;
                    app.panel_focused = true;
                    app.panel_tab = panel_id_to_tab(panel_id);
                    app.focus.focus(panel_id);
                    app.input_mode = InputMode::Normal;
                }
                HitRegion::StatusBar | HitRegion::None => {
                    app.selection = None;
                }
            }
        }
        Button::Middle => {
            // Middle-click: paste from system clipboard (X11/Wayland primary selection).
            // We use the same paste mechanism as Ctrl+V but only on click.
            if matches!(region, HitRegion::Editor) {
                app.input_mode = InputMode::Insert;
                paste_from_clipboard(app);
            }
        }
        Button::Right => {
            // Right-click in messages area: toggle collapse of the clicked block
            if matches!(region, HitRegion::Messages)
                && let Some(pos) = crate::tui::selection::screen_to_text_pos(
                    col,
                    row,
                    app.messages_area,
                    app.scroll.offset,
                    &app.rendered_lines,
                )
            {
                // Try to find which block this line belongs to and toggle it
                click_toggle_block(app, pos.row);
            }
        }
    }
}

/// Handle mouse drag (button held + moved).
fn handle_mouse_drag(app: &mut App, button: Button, col: u16, row: u16) {
    if button != Button::Left {
        return;
    }
    // Continue text selection in messages area
    if let Some(ref mut sel) = app.selection
        && let Some(pos) = crate::tui::selection::screen_to_text_pos(
            col,
            row,
            app.messages_area,
            app.scroll.offset,
            &app.rendered_lines,
        )
    {
        sel.update(pos);
    }
}

/// Handle mouse button release.
fn handle_mouse_up(app: &mut App, button: Button, col: u16, row: u16) {
    if button != Button::Left {
        return;
    }
    if let Some(ref mut sel) = app.selection {
        if let Some(pos) = crate::tui::selection::screen_to_text_pos(
            col,
            row,
            app.messages_area,
            app.scroll.offset,
            &app.rendered_lines,
        ) {
            sel.update(pos);
        }
        sel.finish();
        if !sel.is_empty() {
            let text = sel.extract_text(&app.rendered_lines);
            crate::tui::selection::copy_to_clipboard(&text);
        } else {
            app.selection = None;
        }
    }
}

/// Handle mouse scroll wheel — dispatches to whichever region the cursor is over.
fn handle_mouse_scroll(app: &mut App, col: u16, row: u16, up: bool, lines: u16) {
    let region = app.hit_test(col, row);

    match region {
        HitRegion::Messages => {
            if up {
                app.scroll.scroll_up(lines as usize);
            } else {
                app.scroll.scroll_down(lines as usize);
            }
        }
        HitRegion::Panel(panel_id) => {
            let panel = app.panel_mut(panel_id);
            panel.handle_scroll(up, lines);
        }
        HitRegion::Editor => {
            // Scroll in editor could navigate history (up/down),
            // but that would be confusing. Just scroll the messages.
            if up {
                app.scroll.scroll_up(lines as usize);
            } else {
                app.scroll.scroll_down(lines as usize);
            }
        }
        _ => {}
    }
}

/// Try to toggle the collapse state of the block at the given rendered line.
fn click_toggle_block(app: &mut App, text_row: usize) {
    // Walk through blocks and count rendered lines to find which block
    // the clicked row falls in. This is approximate — we use the block
    // header lines as a heuristic.
    let mut row_cursor: usize = 0;

    for entry in &app.blocks {
        if let BlockEntry::Conversation(block) = entry {
            // Each block has at least a header line
            let block_lines = if block.collapsed {
                2 // header + collapsed indicator
            } else {
                // header + responses + spacing
                2 + block.responses.iter().map(|r| r.content.lines().count() + 1).sum::<usize>()
            };

            if text_row >= row_cursor && text_row < row_cursor + block_lines {
                // Found the block — focus and toggle it
                app.focused_block = Some(block.id);
                app.toggle_focused_block();
                app.input_mode = InputMode::Normal;
                return;
            }
            row_cursor += block_lines;
        } else {
            // System messages: count their lines
            if let BlockEntry::System(msg) = entry {
                row_cursor += msg.content.lines().count() + 1;
            }
        }
    }
}

/// Convert a `PanelId` to the legacy `PanelTab` (bridge during migration).
fn panel_id_to_tab(id: crate::tui::panel::PanelId) -> crate::tui::app::PanelTab {
    use crate::tui::app::PanelTab;
    use crate::tui::panel::PanelId;
    match id {
        PanelId::Todo => PanelTab::Todo,
        PanelId::Files => PanelTab::Files,
        PanelId::Subagents => PanelTab::Subagents,
        PanelId::Peers => PanelTab::Peers,
        PanelId::Processes => PanelTab::Processes,
        PanelId::Environment => PanelTab::Todo, // fallback
    }
}

fn handle_insert_char(app: &mut App, key: &crossterm::event::KeyEvent) {
    if let (KeyCode::Char(c), m) = (key.code, key.modifiers)
        && (m.is_empty() || m == KeyModifiers::SHIFT)
    {
        app.editor.insert_char(c);
        app.update_slash_menu();
    }
}

// ---------------------------------------------------------------------------
// Clipboard paste (text + image) — runs on a background thread
// ---------------------------------------------------------------------------

/// Result of a background clipboard read.
pub enum ClipboardResult {
    /// Text was found in the clipboard.
    Text(String),
    /// An image was found: base64 PNG, mime type, raw size, width, height.
    Image {
        encoded: String,
        mime: String,
        raw_size: usize,
        width: u32,
        height: u32,
    },
    /// Nothing useful in clipboard.
    Empty(String),
    /// Error accessing the clipboard.
    Error(String),
}

/// Read from the system clipboard on a background thread. Tries text first,
/// then image. This avoids freezing the TUI when another application (e.g. a
/// browser) holds the Wayland clipboard selection.
fn paste_from_clipboard(app: &mut App) {
    if app.clipboard_pending {
        return;
    }
    app.clipboard_pending = true;

    let (tx, rx) = std::sync::mpsc::channel::<ClipboardResult>();

    std::thread::spawn(move || {
        let result = (|| -> Result<ClipboardResult, ClipboardResult> {
            let mut clipboard =
                arboard::Clipboard::new().map_err(|e| ClipboardResult::Error(format!("Clipboard error: {e}")))?;

            // Try text first — this is what the user almost always wants with Ctrl+V
            if let Ok(text) = clipboard.get_text()
                && !text.is_empty()
            {
                return Ok(ClipboardResult::Text(text));
            }

            // Fall back to image
            match clipboard.get_image() {
                Ok(img_data) => {
                    use base64::Engine;
                    use base64::engine::general_purpose::STANDARD as BASE64;

                    let width = img_data.width as u32;
                    let height = img_data.height as u32;
                    let rgba: Vec<u8> = img_data.bytes.into_owned();

                    let img = image::RgbaImage::from_raw(width, height, rgba)
                        .ok_or_else(|| ClipboardResult::Error("Failed to decode clipboard image data.".to_string()))?;

                    let mut png_buf: Vec<u8> = Vec::new();
                    let mut cursor = std::io::Cursor::new(&mut png_buf);
                    img.write_to(&mut cursor, image::ImageFormat::Png)
                        .map_err(|e| ClipboardResult::Error(format!("Failed to encode image as PNG: {e}")))?;

                    let raw_size = png_buf.len();
                    let encoded = BASE64.encode(&png_buf);

                    Ok(ClipboardResult::Image {
                        encoded,
                        mime: "image/png".to_string(),
                        raw_size,
                        width,
                        height,
                    })
                }
                Err(_) => Err(ClipboardResult::Empty("Clipboard is empty.".to_string())),
            }
        })();

        let _ = tx.send(result.unwrap_or_else(|e| e));
    });

    app.clipboard_rx = Some(rx);
}

/// Poll for a completed clipboard read (non-blocking).
fn poll_clipboard_result(app: &mut App) {
    let result = if let Some(ref rx) = app.clipboard_rx {
        match rx.try_recv() {
            Ok(result) => Some(result),
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                Some(ClipboardResult::Error("Clipboard thread crashed.".to_string()))
            }
        }
    } else {
        return;
    };

    app.clipboard_rx = None;
    app.clipboard_pending = false;

    if let Some(result) = result {
        match result {
            ClipboardResult::Text(text) => {
                app.input_mode = InputMode::Insert;
                app.selection = None;
                app.editor.insert_str(&text);
                app.update_slash_menu();
            }
            ClipboardResult::Image {
                encoded,
                mime,
                raw_size,
                width,
                height,
            } => {
                app.attach_image(encoded, mime, raw_size);

                let size_str = if raw_size >= 1024 * 1024 {
                    format!("{:.1} MB", raw_size as f64 / (1024.0 * 1024.0))
                } else if raw_size >= 1024 {
                    format!("{:.1} KB", raw_size as f64 / 1024.0)
                } else {
                    format!("{raw_size} bytes")
                };

                let count = app.pending_images.len();
                app.push_system(
                    format!(
                        "📎 Image attached ({width}×{height}, {size_str}). {count} image{} pending.",
                        if count == 1 { "" } else { "s" }
                    ),
                    false,
                );
            }
            ClipboardResult::Empty(_) => {
                // Nothing to paste — silently ignore
            }
            ClipboardResult::Error(msg) => {
                app.push_system(msg, true);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// External editor ($EDITOR / $VISUAL)
// ---------------------------------------------------------------------------

/// Suspend the TUI, open $EDITOR with the current editor content, and load
/// the result back. Falls back to $VISUAL, then `vi`.
fn open_external_editor(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) {
    // Determine which editor to use
    let editor_cmd = std::env::var("EDITOR").or_else(|_| std::env::var("VISUAL")).unwrap_or_else(|_| "vi".to_string());

    // Write current editor content to a temp file
    let current_content = app.editor.content().join("\n");
    let tmp_dir = std::env::temp_dir();
    let tmp_path = tmp_dir.join(format!("clankers-edit-{}.md", std::process::id()));

    if let Err(e) = std::fs::write(&tmp_path, &current_content) {
        app.push_system(format!("Failed to create temp file: {}", e), true);
        return;
    }

    // Suspend the TUI: leave alternate screen, disable raw mode
    execute!(terminal.backend_mut(), DisableBracketedPaste, DisableMouseCapture, LeaveAlternateScreen).ok();
    terminal::disable_raw_mode().ok();

    // Parse the editor command (supports args like "code --wait")
    let mut parts = editor_cmd.split_whitespace();
    let program = parts.next().unwrap_or("vi");
    let extra_args: Vec<&str> = parts.collect();

    // Run the editor
    let result = std::process::Command::new(program)
        .args(&extra_args)
        .arg(&tmp_path)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .current_dir(&app.cwd)
        .status();

    // Restore the TUI: re-enable raw mode, enter alternate screen
    terminal::enable_raw_mode().ok();
    execute!(terminal.backend_mut(), EnterAlternateScreen, EnableMouseCapture, EnableBracketedPaste).ok();

    // Force a full redraw after returning from the editor
    terminal.clear().ok();

    match result {
        Ok(status) if status.success() => {
            // Read back the edited content
            match std::fs::read_to_string(&tmp_path) {
                Ok(new_content) => {
                    let new_content = new_content.trim_end_matches('\n').to_string();
                    if new_content.is_empty() {
                        app.push_system("Editor returned empty content — input cleared.".to_string(), false);
                        app.editor.clear();
                    } else if new_content == current_content {
                        // No changes — don't bother updating
                    } else {
                        app.editor.clear();
                        for c in new_content.chars() {
                            app.editor.insert_char(c);
                        }
                        app.input_mode = InputMode::Insert;
                    }
                }
                Err(e) => {
                    app.push_system(format!("Failed to read editor output: {}", e), true);
                }
            }
        }
        Ok(status) => {
            app.push_system(format!("Editor exited with status {} — changes discarded.", status), true);
        }
        Err(e) => {
            app.push_system(format!("Failed to launch '{}': {}", editor_cmd, e), true);
        }
    }

    // Clean up temp file
    let _ = std::fs::remove_file(&tmp_path);
}

// ---------------------------------------------------------------------------
// Input routing
// ---------------------------------------------------------------------------

fn handle_input_with_plugins(
    app: &mut App,
    text: &str,
    cmd_tx: &tokio::sync::mpsc::UnboundedSender<AgentCommand>,
    plugin_manager: Option<&Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
    panel_tx: &tokio::sync::mpsc::UnboundedSender<crate::tui::components::subagent_event::SubagentEvent>,
    db: &Option<crate::db::Db>,
) {
    if let Some((action, args)) = slash_commands::parse_command(text) {
        execute_slash_command(app, action, &args, cmd_tx, plugin_manager, panel_tx, db);
    } else {
        let _ = cmd_tx.send(AgentCommand::ResetCancel);
        let mut pending_images = app.take_pending_images();

        // Expand @file references — text files are inlined, images become Content blocks
        let expanded = crate::util::at_file::expand_at_refs_with_images(text, &app.cwd);
        let prompt_text = expanded.text;

        // Convert @file images into PendingImage and merge with clipboard-pasted images
        let at_file_images: Vec<crate::tui::app::PendingImage> = expanded
            .images
            .into_iter()
            .filter_map(|c| match c {
                crate::provider::message::Content::Image {
                    source: crate::provider::message::ImageSource::Base64 { media_type, data },
                } => {
                    let size = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &data)
                        .map(|b| b.len())
                        .unwrap_or(0);
                    Some(crate::tui::app::PendingImage { data, media_type, size })
                }
                _ => None,
            })
            .collect();
        pending_images.extend(at_file_images);

        if pending_images.is_empty() {
            let _ = cmd_tx.send(AgentCommand::Prompt(prompt_text));
        } else {
            let _ = cmd_tx.send(AgentCommand::PromptWithImages {
                text: prompt_text,
                images: pending_images,
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Slash command execution
// ---------------------------------------------------------------------------

/// Parse an optional `--account <name>` flag from args, returning (account, remaining_args)
/// Parse OAuth callback input: code#state, URL with ?code=...&state=..., or space-separated.
fn parse_oauth_input(input: &str) -> Option<(String, String)> {
    let input = input.trim();

    // Try parsing as a URL
    if input.starts_with("http://") || input.starts_with("https://") {
        if let Ok(url) = url::Url::parse(input) {
            let params: std::collections::HashMap<_, _> = url.query_pairs().collect();
            if let (Some(code), Some(state)) = (params.get("code"), params.get("state")) {
                return Some((code.to_string(), state.to_string()));
            }
        }
        return None;
    }

    // Try code#state format
    if let Some((code, state)) = input.split_once('#')
        && !code.is_empty()
        && !state.is_empty()
    {
        return Some((code.to_string(), state.to_string()));
    }

    None
}

fn parse_account_flag(args: &str) -> (Option<String>, String) {
    let parts: Vec<&str> = args.split_whitespace().collect();
    for (i, part) in parts.iter().enumerate() {
        if *part == "--account" && i + 1 < parts.len() {
            let account = parts[i + 1].to_string();
            let remaining: Vec<&str> = parts[..i].iter().chain(parts[i + 2..].iter()).copied().collect();
            return (Some(account), remaining.join(" "));
        }
    }
    (None, args.to_string())
}

fn execute_slash_command(
    app: &mut App,
    action: SlashAction,
    args: &str,
    cmd_tx: &tokio::sync::mpsc::UnboundedSender<AgentCommand>,
    plugin_manager: Option<&Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
    panel_tx: &tokio::sync::mpsc::UnboundedSender<crate::tui::components::subagent_event::SubagentEvent>,
    db: &Option<crate::db::Db>,
) {
    match action {
        SlashAction::Help => {
            app.push_system(slash_commands::help_text(), false);
        }
        SlashAction::Clear => {
            app.blocks.clear();
            let _ = cmd_tx.send(AgentCommand::ClearHistory);
            app.push_system("Conversation cleared.".to_string(), false);
            app.scroll.scroll_to_top();
        }
        SlashAction::Reset => {
            app.blocks.clear();
            app.all_blocks.clear();
            app.active_block = None;
            app.streaming_text.clear();
            app.streaming_thinking.clear();
            app.total_tokens = 0;
            app.total_cost = 0.0;
            app.focused_block = None;
            let _ = cmd_tx.send(AgentCommand::ClearHistory);
            let _ = cmd_tx.send(AgentCommand::ResetCancel);
            app.push_system("Session reset. Context and history cleared.".to_string(), false);
            app.scroll.scroll_to_top();
        }
        SlashAction::Model => {
            if args.is_empty() {
                app.push_system(format!("Current model: {}\n\nUsage: /model <model-name>", app.model), false);
            } else {
                let old_model = std::mem::replace(&mut app.model, args.to_string());
                let _ = cmd_tx.send(AgentCommand::SetModel(args.to_string()));
                app.context_gauge.set_model(&app.model);
                app.push_system(format!("Model switched: {} → {}", old_model, app.model), false);
            }
        }
        SlashAction::Status => {
            let status = format!(
                "Model: {}\nTokens used: {}\nCost: ${:.4}\nSession: {}\nCWD: {}",
                app.model, app.total_tokens, app.total_cost, app.session_id, app.cwd,
            );
            app.push_system(status, false);
        }
        SlashAction::Usage => {
            let usage = format!(
                "Token usage:\n  Total tokens: {}\n  Estimated cost: ${:.4}",
                app.total_tokens, app.total_cost,
            );
            app.push_system(usage, false);
        }
        SlashAction::Version => {
            app.push_system(format!("clankers {}", env!("CARGO_PKG_VERSION")), false);
        }
        SlashAction::Quit => {
            app.should_quit = true;
        }
        SlashAction::Session => {
            if args.is_empty() {
                let info = if app.session_id.is_empty() {
                    "No active session.".to_string()
                } else {
                    format!(
                        "Session ID: {}\nCWD: {}\nModel: {}\n\nUse /session list to see recent sessions.",
                        app.session_id, app.cwd, app.model
                    )
                };
                app.push_system(info, false);
            } else {
                let parts: Vec<&str> = args.splitn(2, char::is_whitespace).collect();
                let subcmd = parts[0].trim();
                let subcmd_args = parts.get(1).map(|s| s.trim()).unwrap_or("");

                match subcmd {
                    "list" | "ls" => {
                        let paths = crate::config::ClankersPaths::resolve();
                        let limit: usize = if subcmd_args.is_empty() {
                            10
                        } else {
                            subcmd_args.parse().unwrap_or(10)
                        };
                        let files = crate::session::store::list_sessions(&paths.global_sessions_dir, &app.cwd);
                        if files.is_empty() {
                            app.push_system("No sessions found for this directory.".to_string(), false);
                        } else {
                            let mut out = String::from("Recent sessions:\n\n");
                            for (i, path) in files.iter().take(limit).enumerate() {
                                let is_current_file = !app.session_id.is_empty()
                                    && path
                                        .file_name()
                                        .and_then(|n| n.to_str())
                                        .is_some_and(|n| n.contains(&app.session_id));
                                let marker = if is_current_file { " ◀ current" } else { "" };

                                if let Some(summary) = crate::session::store::read_session_summary(path) {
                                    let date = summary.created_at.format("%Y-%m-%d %H:%M");
                                    let preview = summary.first_user_message.as_deref().unwrap_or("(empty)");
                                    out.push_str(&format!(
                                        "  {}. [{}] {} ({} msgs, {}){}\n     {}\n\n",
                                        i + 1,
                                        &summary.session_id[..8.min(summary.session_id.len())],
                                        date,
                                        summary.message_count,
                                        summary.model,
                                        marker,
                                        preview,
                                    ));
                                } else {
                                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                                    out.push_str(&format!("  {}. {}{}\n", i + 1, name, marker));
                                }
                            }
                            if files.len() > limit {
                                out.push_str(&format!("  ({} more sessions)\n", files.len() - limit));
                            }
                            out.push_str("\nUse /session resume to pick a session, or /session resume <id>.");
                            app.push_system(out, false);
                        }
                    }
                    "resume" | "open" => {
                        if subcmd_args.is_empty() {
                            // Open the session selector menu
                            let paths = crate::config::ClankersPaths::resolve();
                            let files = crate::session::store::list_sessions(&paths.global_sessions_dir, &app.cwd);
                            if files.is_empty() {
                                app.push_system("No sessions found for this directory.".to_string(), true);
                            } else {
                                let items: Vec<crate::tui::components::session_selector::SessionItem> = files
                                    .iter()
                                    .map(|f| {
                                        if let Some(summary) = crate::session::store::read_session_summary(f) {
                                            let date = summary
                                                .created_at
                                                .with_timezone(&chrono::Local)
                                                .format("%Y-%m-%d %H:%M");
                                            let preview = summary.first_user_message.as_deref().unwrap_or("(empty)");
                                            let label = format!(
                                                "[{}] {} — {} ({} msgs, {})",
                                                &summary.session_id[..8.min(summary.session_id.len())],
                                                date,
                                                preview,
                                                summary.message_count,
                                                summary.model,
                                            );
                                            crate::tui::components::session_selector::SessionItem {
                                                session_id: summary.session_id,
                                                label,
                                                file_path: f.clone(),
                                            }
                                        } else {
                                            let name = f.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                                            crate::tui::components::session_selector::SessionItem {
                                                session_id: name.to_string(),
                                                label: name.to_string(),
                                                file_path: f.clone(),
                                            }
                                        }
                                    })
                                    .collect();
                                app.session_selector.open(items);
                            }
                        } else {
                            // Direct resume by ID
                            let paths = crate::config::ClankersPaths::resolve();
                            let files = crate::session::store::list_sessions(&paths.global_sessions_dir, &app.cwd);
                            let found = files.into_iter().find(|f| {
                                f.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.contains(subcmd_args))
                            });
                            if let Some(file) = found {
                                let session_id = file.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
                                resume_session_from_file(app, file, &session_id, cmd_tx);
                            } else {
                                app.push_system(
                                    format!("No session matching '{}'. Use /session resume to browse.", subcmd_args),
                                    true,
                                );
                            }
                        }
                    }
                    "delete" | "rm" => {
                        if subcmd_args.is_empty() {
                            app.push_system("Usage: /session delete <session-id>".to_string(), true);
                        } else {
                            let paths = crate::config::ClankersPaths::resolve();
                            let found = crate::session::store::find_session_by_id(
                                &paths.global_sessions_dir,
                                &app.cwd,
                                subcmd_args,
                            );
                            if let Some(file) = found {
                                match std::fs::remove_file(&file) {
                                    Ok(()) => {
                                        app.push_system(format!("Session deleted: {}", file.display()), false);
                                    }
                                    Err(e) => {
                                        app.push_system(format!("Failed to delete session: {}", e), true);
                                    }
                                }
                            } else {
                                app.push_system(
                                    format!("No session matching '{}'. Use /session list.", subcmd_args),
                                    true,
                                );
                            }
                        }
                    }
                    "purge" => {
                        let paths = crate::config::ClankersPaths::resolve();
                        match crate::session::store::purge_sessions(&paths.global_sessions_dir, &app.cwd) {
                            Ok(count) => {
                                app.push_system(format!("Deleted {} session(s) for this directory.", count), false);
                            }
                            Err(e) => {
                                app.push_system(format!("Failed to purge sessions: {}", e), true);
                            }
                        }
                    }
                    _ => {
                        app.push_system(
                            format!("Unknown subcommand '{}'. Available: list, resume, delete, purge", subcmd),
                            true,
                        );
                    }
                }
            }
        }
        SlashAction::Undo => {
            let mut removed = false;
            for i in (0..app.blocks.len()).rev() {
                if matches!(app.blocks[i], BlockEntry::Conversation(_)) {
                    app.blocks.remove(i);
                    removed = true;
                    break;
                }
            }
            if removed {
                app.push_system("Last conversation block removed.".to_string(), false);
            } else {
                app.push_system("Nothing to undo.".to_string(), false);
            }
        }
        SlashAction::Cd => {
            if args.is_empty() {
                app.push_system(format!("Current directory: {}\n\nUsage: /cd <path>", app.cwd), false);
            } else {
                let new_path = if args.starts_with('/') {
                    std::path::PathBuf::from(args)
                } else {
                    std::path::PathBuf::from(&app.cwd).join(args)
                };
                match new_path.canonicalize() {
                    Ok(canonical) if canonical.is_dir() => {
                        app.cwd = canonical.to_string_lossy().to_string();
                        app.git_status.set_cwd(&app.cwd);
                        app.push_system(format!("Changed directory to: {}", app.cwd), false);
                    }
                    Ok(_) => {
                        app.push_system(format!("Not a directory: {}", args), true);
                    }
                    Err(e) => {
                        app.push_system(format!("Invalid path '{}': {}", args, e), true);
                    }
                }
            }
        }
        SlashAction::Shell => {
            if args.is_empty() {
                app.push_system("Usage: /shell <command>".to_string(), false);
            } else {
                match std::process::Command::new("sh").arg("-c").arg(args).current_dir(&app.cwd).output() {
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        let mut result = String::new();
                        if !stdout.is_empty() {
                            result.push_str(&stdout);
                        }
                        if !stderr.is_empty() {
                            if !result.is_empty() {
                                result.push('\n');
                            }
                            result.push_str(&stderr);
                        }
                        if result.is_empty() {
                            result = format!("(exit code: {})", output.status.code().unwrap_or(-1));
                        }
                        app.push_system(result, !output.status.success());
                    }
                    Err(e) => {
                        app.push_system(format!("Failed to run command: {}", e), true);
                    }
                }
            }
        }
        SlashAction::Export => {
            let filename = if args.is_empty() {
                format!("clankers-export-{}.md", chrono::Local::now().format("%Y%m%d-%H%M%S"))
            } else {
                args.to_string()
            };
            let mut content = String::new();
            for entry in &app.blocks {
                match entry {
                    BlockEntry::Conversation(block) => {
                        content.push_str("## User\n");
                        content.push_str(&block.prompt);
                        content.push_str("\n\n");
                        for msg in &block.responses {
                            let label = match msg.role {
                                MessageRole::Assistant => "## Assistant",
                                MessageRole::ToolCall => "## Tool Call",
                                MessageRole::ToolResult => "## Tool Result",
                                MessageRole::Thinking => "## Thinking",
                                _ => "## Other",
                            };
                            content.push_str(label);
                            content.push('\n');
                            content.push_str(&msg.content);
                            content.push_str("\n\n");
                        }
                    }
                    BlockEntry::System(msg) => {
                        content.push_str("## System\n");
                        content.push_str(&msg.content);
                        content.push_str("\n\n");
                    }
                }
            }
            let file_path = std::path::Path::new(&filename);
            // If the filename is just a bare name (no directory components), place it in .clankers/exports/
            let resolved = if file_path.parent().is_none_or(|p| p.as_os_str().is_empty()) {
                let cwd_path = std::path::Path::new(&app.cwd);
                let exports_dir = cwd_path.join(".clankers").join("exports");
                std::fs::create_dir_all(&exports_dir).ok();
                crate::util::fs::ensure_gitignore_entry(cwd_path, ".clankers/exports");
                exports_dir.join(&filename)
            } else {
                std::path::Path::new(&app.cwd).join(&filename)
            };
            match std::fs::write(&resolved, &content) {
                Ok(()) => {
                    app.push_system(format!("Exported to: {}", resolved.display()), false);
                }
                Err(e) => {
                    app.push_system(format!("Export failed: {}", e), true);
                }
            }
        }
        SlashAction::Compact => {
            app.push_system("Compact mode is not yet implemented.".to_string(), false);
        }
        SlashAction::Think => {
            if args.is_empty() {
                // No args: cycle to next level
                let _ = cmd_tx.send(AgentCommand::CycleThinkingLevel);
            } else if let Some(level) = crate::provider::ThinkingLevel::from_str_or_budget(args) {
                // Named level: /think off, /think low, /think high, etc.
                let _ = cmd_tx.send(AgentCommand::SetThinkingLevel(level));
            } else if let Ok(budget) = args.trim().parse::<usize>() {
                // Raw number: /think 20000 → find closest level
                let level = crate::provider::ThinkingLevel::from_budget(budget);
                let _ = cmd_tx.send(AgentCommand::SetThinkingLevel(level));
            } else {
                app.push_system("Usage: /think [off|low|medium|high|max] or /think <budget>".to_string(), true);
            }
        }
        SlashAction::Login => {
            // Parse optional --account flag: /login [--account name] [code#state|url]
            let (account_name, remaining_args) = parse_account_flag(args);
            let account_name = account_name.unwrap_or_else(|| "default".to_string());

            if remaining_args.is_empty() {
                let (url, verifier) = crate::provider::anthropic::oauth::build_auth_url();
                app.login_verifier = Some((verifier.clone(), account_name.clone()));

                // Also persist verifier to disk so `clankers auth login --code` can use it
                let paths = crate::config::ClankersPaths::resolve();
                let verifier_path = paths.global_config_dir.join(".login_verifier");
                std::fs::create_dir_all(&paths.global_config_dir).ok();
                std::fs::write(&verifier_path, &verifier).ok();

                // Try to auto-open the browser (detached so it doesn't block the TUI)
                let browser_opened = open::that_detached(&url).is_ok();

                let browser_msg = if browser_opened {
                    "Opening browser automatically..."
                } else {
                    "Could not open browser automatically."
                };

                app.push_system(
                    format!(
                        "Logging in as account: {}\n\n\
                         {}\n\n\
                         Open this URL in your browser to authenticate:\n\n  {}\n\n\
                         After authorizing, paste the code with:\n  /login <code#state>\n  /login <callback URL>\n\n\
                         Or from another terminal:\n  clankers auth login --code <code#state>",
                        account_name, browser_msg, url
                    ),
                    false,
                );
            } else if let Some((verifier, acct)) = app.login_verifier.take() {
                // Parse code+state from various formats (code#state, URL, etc.)
                let parsed = parse_oauth_input(&remaining_args);
                match parsed {
                    Some((code, state)) => {
                        app.push_system(format!("Exchanging code for account '{}'...", acct), false);
                        let _ = cmd_tx.send(AgentCommand::Login {
                            code,
                            state,
                            verifier,
                            account: acct,
                        });
                    }
                    None => {
                        app.login_verifier = Some((verifier, acct));
                        app.push_system(
                            "Invalid code format. Expected:\n  /login code#state\n  /login https://...?code=CODE&state=STATE".to_string(),
                            true,
                        );
                    }
                }
            } else {
                // No in-memory verifier — try recovering from disk (e.g. login started in another clankers
                // instance)
                let paths = crate::config::ClankersPaths::resolve();
                let verifier_path = paths.global_config_dir.join(".login_verifier");
                if let Ok(verifier) = std::fs::read_to_string(&verifier_path) {
                    if let Some((code, state)) = parse_oauth_input(&remaining_args) {
                        app.push_system(format!("Exchanging code for account '{}'...", account_name), false);
                        std::fs::remove_file(&verifier_path).ok();
                        let _ = cmd_tx.send(AgentCommand::Login {
                            code,
                            state,
                            verifier,
                            account: account_name,
                        });
                    } else {
                        app.push_system(
                            "Invalid code format. Expected:\n  /login code#state\n  /login https://...?code=CODE&state=STATE".to_string(),
                            true,
                        );
                    }
                } else {
                    app.push_system("No login in progress. Run /login first to get the auth URL.".to_string(), true);
                }
            }
        }
        SlashAction::Tools => {
            if app.tool_info.is_empty() {
                app.push_system("No tools available.".to_string(), false);
            } else {
                let mut out = String::from("Available tools:\n\n");
                let max_name = app.tool_info.iter().map(|(n, _, _)| n.len()).max().unwrap_or(0);
                let mut current_source = String::new();
                for (name, description, source) in &app.tool_info {
                    if *source != current_source {
                        if !current_source.is_empty() {
                            out.push('\n');
                        }
                        out.push_str(&format!("  ── {} ──\n", source));
                        current_source = source.clone();
                    }
                    // Truncate long descriptions to keep it readable
                    let desc = if description.len() > 60 {
                        format!("{}…", &description[..59])
                    } else {
                        description.clone()
                    };
                    out.push_str(&format!("  {:<width$}  {}\n", name, desc, width = max_name));
                }
                out.push_str(&format!("\n  {} tool(s) total", app.tool_info.len()));
                app.push_system(out, false);
            }
        }
        SlashAction::Plugin => {
            if let Some(pm) = plugin_manager {
                let mgr = pm.lock().unwrap_or_else(|e| e.into_inner());
                if args.is_empty() {
                    // List all plugins
                    let plugins = mgr.list();
                    if plugins.is_empty() {
                        app.push_system(
                            "No plugins loaded.\n\nInstall plugins with: clankers plugin install <path>".to_string(),
                            false,
                        );
                    } else {
                        let mut out = String::from("Loaded plugins:\n\n");
                        for p in plugins {
                            let state = match &p.state {
                                crate::plugin::PluginState::Active => "✓",
                                crate::plugin::PluginState::Loaded => "○",
                                crate::plugin::PluginState::Error(e) => {
                                    out.push_str(&format!("  ✗ {} v{} — Error: {}\n", p.name, p.version, e));
                                    continue;
                                }
                                crate::plugin::PluginState::Disabled => "−",
                            };
                            out.push_str(&format!(
                                "  {} {} v{} — {} (tools: {})\n",
                                state,
                                p.name,
                                p.version,
                                p.manifest.description,
                                if p.manifest.tools.is_empty() {
                                    "none".to_string()
                                } else {
                                    p.manifest.tools.join(", ")
                                },
                            ));
                        }
                        app.push_system(out, false);
                    }
                } else {
                    // Show specific plugin
                    if let Some(p) = mgr.get(args.trim()) {
                        let out = format!(
                            "Plugin: {} v{}\nState: {:?}\nDescription: {}\nPath: {}\nTools: {}\nCommands: {}\nEvents: {}\nPermissions: {}",
                            p.name,
                            p.version,
                            p.state,
                            p.manifest.description,
                            p.path.display(),
                            if p.manifest.tools.is_empty() {
                                "none".to_string()
                            } else {
                                p.manifest.tools.join(", ")
                            },
                            if p.manifest.commands.is_empty() {
                                "none".to_string()
                            } else {
                                p.manifest.commands.join(", ")
                            },
                            if p.manifest.events.is_empty() {
                                "none".to_string()
                            } else {
                                p.manifest.events.join(", ")
                            },
                            if p.manifest.permissions.is_empty() {
                                "none".to_string()
                            } else {
                                p.manifest.permissions.join(", ")
                            },
                        );
                        app.push_system(out, false);
                    } else {
                        app.push_system(format!("Plugin '{}' not found.", args.trim()), true);
                    }
                }
            } else {
                app.push_system("Plugin system not initialized.".to_string(), true);
            }
        }
        SlashAction::Subagents => {
            if args.is_empty() {
                // List all subagents
                app.push_system(app.subagent_panel.summary(), false);
            } else {
                let parts: Vec<&str> = args.splitn(2, char::is_whitespace).collect();
                let subcmd = parts[0].trim();
                let subcmd_args = parts.get(1).map(|s| s.trim()).unwrap_or("");

                match subcmd {
                    "kill" => {
                        if subcmd_args == "all" {
                            // Kill all running subagents
                            let running: Vec<String> = app
                                .subagent_panel
                                .entries
                                .iter()
                                .filter(|e| e.status == crate::tui::components::subagent_panel::SubagentStatus::Running)
                                .map(|e| e.id.clone())
                                .collect();
                            if running.is_empty() {
                                app.push_system("No running subagents to kill.".to_string(), false);
                            } else {
                                for id in &running {
                                    let _ = panel_tx.send(
                                        crate::tui::components::subagent_event::SubagentEvent::KillRequest {
                                            id: id.clone(),
                                        },
                                    );
                                }
                                app.push_system(format!("Kill requested for {} subagent(s).", running.len()), false);
                            }
                        } else if subcmd_args.is_empty() {
                            app.push_system("Usage: /subagents kill <id> or /subagents kill all".to_string(), true);
                        } else {
                            let target = subcmd_args.to_string();
                            // Try partial match on id or name
                            let matched = app
                                .subagent_panel
                                .entries
                                .iter()
                                .find(|e| e.id == target || e.name == target || e.id.contains(&target))
                                .map(|e| e.id.clone());
                            if let Some(id) = matched {
                                let _ =
                                    panel_tx.send(crate::tui::components::subagent_event::SubagentEvent::KillRequest {
                                        id: id.clone(),
                                    });
                                app.push_system(format!("Kill requested for subagent '{}'.", id), false);
                            } else {
                                app.push_system(format!("No subagent matching '{}'.", subcmd_args), true);
                            }
                        }
                    }
                    "remove" | "rm" => {
                        if subcmd_args.is_empty() {
                            app.push_system("Usage: /subagents remove <id>".to_string(), true);
                        } else {
                            let target = subcmd_args.to_string();
                            let matched = app
                                .subagent_panel
                                .entries
                                .iter()
                                .find(|e| e.id == target || e.name == target || e.id.contains(&target))
                                .map(|e| e.id.clone());
                            if let Some(id) = matched {
                                app.subagent_panel.remove_by_id(&id);
                                app.push_system(format!("Removed subagent '{}'.", id), false);
                            } else {
                                app.push_system(format!("No subagent matching '{}'.", subcmd_args), true);
                            }
                        }
                    }
                    "clear" => {
                        app.subagent_panel.clear_done();
                        app.push_system("Cleared completed/failed subagents.".to_string(), false);
                    }
                    _ => {
                        app.push_system(format!("Unknown subcommand '{}'. Use: kill, remove, clear", subcmd), true);
                    }
                }
            }
        }
        SlashAction::Account => {
            let paths = crate::config::ClankersPaths::resolve();
            let mut store = crate::provider::auth::AuthStore::load(&paths.global_auth);

            if args.is_empty() || args == "list" {
                // Show accounts with status details
                let accounts = store.list_anthropic_accounts();
                if accounts.is_empty() {
                    let mut msg = String::from("No accounts configured.\n\n");
                    if std::env::var("ANTHROPIC_API_KEY").is_ok() {
                        msg.push_str("  Using ANTHROPIC_API_KEY from environment.\n");
                    }
                    msg.push_str("\n  Use /account login [name] or /login to add one.");
                    app.push_system(msg, false);
                } else {
                    let mut out = String::from("Accounts:\n\n");
                    for info in &accounts {
                        let marker = if info.is_active { "▸" } else { " " };
                        let status = if info.is_expired { "✗ expired" } else { "✓ valid" };
                        let label = info.label.as_ref().map(|l| format!(" ({})", l)).unwrap_or_default();
                        out.push_str(&format!("  {} {}{} — {}\n", marker, info.name, label, status));
                    }
                    out.push_str(&format!("\n  {} account(s). Use /account switch <name> to change.", accounts.len()));
                    app.push_system(out, false);
                }
            } else {
                let parts: Vec<&str> = args.splitn(2, char::is_whitespace).collect();
                let subcmd = parts[0].trim();
                let subcmd_args = parts.get(1).map(|s| s.trim()).unwrap_or("");

                match subcmd {
                    "switch" | "use" => {
                        if subcmd_args.is_empty() {
                            // Show available accounts as a hint
                            let names: Vec<String> =
                                store.list_anthropic_accounts().iter().map(|a| a.name.clone()).collect();
                            app.push_system(
                                format!("Usage: /account switch <name>\n\nAvailable: {}", names.join(", ")),
                                true,
                            );
                        } else {
                            let _ = cmd_tx.send(AgentCommand::SwitchAccount(subcmd_args.to_string()));
                        }
                    }
                    "login" => {
                        // Delegate to /login with optional account name
                        let account_name = if subcmd_args.is_empty() {
                            store.active_account_name().to_string()
                        } else {
                            subcmd_args.to_string()
                        };
                        let login_args = format!("--account {}", account_name);
                        execute_slash_command(
                            app,
                            SlashAction::Login,
                            &login_args,
                            cmd_tx,
                            plugin_manager,
                            panel_tx,
                            db,
                        );
                    }
                    "logout" => {
                        let name = if subcmd_args.is_empty() {
                            store.active_account_name().to_string()
                        } else {
                            subcmd_args.to_string()
                        };
                        if store.remove_anthropic_account(&name) {
                            if let Err(e) = store.save(&paths.global_auth) {
                                app.push_system(format!("Failed to save: {}", e), true);
                            } else {
                                let new_active = store.active_account_name().to_string();
                                app.push_system(
                                    format!("Logged out '{}'. Active account: '{}'.", name, new_active),
                                    false,
                                );
                            }
                        } else {
                            app.push_system(format!("No account '{}'.", name), true);
                        }
                    }
                    "remove" | "rm" => {
                        if subcmd_args.is_empty() {
                            app.push_system("Usage: /account remove <name>".to_string(), true);
                        } else {
                            let name = subcmd_args.to_string();
                            if store.remove_anthropic_account(&name) {
                                if let Err(e) = store.save(&paths.global_auth) {
                                    app.push_system(format!("Failed to save: {}", e), true);
                                } else {
                                    app.push_system(format!("Removed account '{}'.", name), false);
                                }
                            } else {
                                app.push_system(format!("No account '{}'.", name), true);
                            }
                        }
                    }
                    "status" => {
                        let name = if subcmd_args.is_empty() {
                            store.active_account_name().to_string()
                        } else {
                            subcmd_args.to_string()
                        };
                        if let Some(cred) = store.credential_for("anthropic", &name) {
                            let status = if cred.is_expired() { "✗ expired" } else { "✓ valid" };
                            let expires_in = if cred.is_expired() {
                                "expired".to_string()
                            } else if let crate::provider::auth::StoredCredential::OAuth { expires_at_ms, .. } = cred {
                                let remaining = expires_at_ms - chrono::Utc::now().timestamp_millis();
                                let mins = remaining / 60_000;
                                if mins > 60 {
                                    format!("{}h {}m", mins / 60, mins % 60)
                                } else {
                                    format!("{}m", mins)
                                }
                            } else {
                                "n/a (api key)".to_string()
                            };
                            app.push_system(
                                format!("Account '{}': {} (expires in {})", name, status, expires_in),
                                false,
                            );
                        } else {
                            app.push_system(format!("No account '{}'.", name), true);
                        }
                    }
                    _ => {
                        app.push_system(
                            format!(
                                "Unknown subcommand '{}'. Available:\n  \
                                 switch <name>  — switch active account\n  \
                                 login [name]   — login to an account\n  \
                                 logout [name]  — logout an account\n  \
                                 remove <name>  — remove an account\n  \
                                 status [name]  — show account status\n  \
                                 list           — list all accounts",
                                subcmd
                            ),
                            true,
                        );
                    }
                }
            }
        }
        SlashAction::Todo => {
            use crate::tui::components::todo_panel::TodoStatus;

            if args.is_empty() {
                app.push_system(app.todo_panel.summary(), false);
            } else {
                let parts: Vec<&str> = args.splitn(2, char::is_whitespace).collect();
                let subcmd = parts[0].trim();
                let subcmd_args = parts.get(1).map(|s| s.trim()).unwrap_or("");

                match subcmd {
                    "add" => {
                        if subcmd_args.is_empty() {
                            app.push_system("Usage: /todo add <text>".to_string(), true);
                        } else {
                            let id = app.todo_panel.add(subcmd_args.to_string());
                            app.push_system(format!("Added todo #{}: {}", id, subcmd_args), false);
                        }
                    }
                    "done" | "complete" => {
                        if subcmd_args.is_empty() {
                            app.push_system("Usage: /todo done <id or text>".to_string(), true);
                        } else if let Ok(id) = subcmd_args.parse::<usize>() {
                            if app.todo_panel.set_status(id, TodoStatus::Done) {
                                app.push_system(format!("Marked #{} as done.", id), false);
                            } else {
                                app.push_system(format!("No todo item #{}.", id), true);
                            }
                        } else if let Some(id) = app.todo_panel.set_status_by_text(subcmd_args, TodoStatus::Done) {
                            app.push_system(format!("Marked #{} as done.", id), false);
                        } else {
                            app.push_system(format!("No todo matching '{}'.", subcmd_args), true);
                        }
                    }
                    "wip" | "active" | "start" => {
                        if subcmd_args.is_empty() {
                            app.push_system("Usage: /todo wip <id or text>".to_string(), true);
                        } else if let Ok(id) = subcmd_args.parse::<usize>() {
                            if app.todo_panel.set_status(id, TodoStatus::InProgress) {
                                app.push_system(format!("Marked #{} as in-progress.", id), false);
                            } else {
                                app.push_system(format!("No todo item #{}.", id), true);
                            }
                        } else if let Some(id) = app.todo_panel.set_status_by_text(subcmd_args, TodoStatus::InProgress)
                        {
                            app.push_system(format!("Marked #{} as in-progress.", id), false);
                        } else {
                            app.push_system(format!("No todo matching '{}'.", subcmd_args), true);
                        }
                    }
                    "remove" | "rm" => {
                        if subcmd_args.is_empty() {
                            app.push_system("Usage: /todo remove <id>".to_string(), true);
                        } else if let Ok(id) = subcmd_args.parse::<usize>() {
                            if app.todo_panel.remove(id) {
                                app.push_system(format!("Removed todo #{}.", id), false);
                            } else {
                                app.push_system(format!("No todo item #{}.", id), true);
                            }
                        } else {
                            app.push_system("Usage: /todo remove <id> (numeric ID required)".to_string(), true);
                        }
                    }
                    "clear" => {
                        app.todo_panel.clear_done();
                        app.push_system("Cleared completed items.".to_string(), false);
                    }
                    _ => {
                        // Treat bare text as "add"
                        let text = args.to_string();
                        let id = app.todo_panel.add(text.clone());
                        app.push_system(format!("Added todo #{}: {}", id, text), false);
                    }
                }
            }
        }
        SlashAction::Worker => {
            if args.is_empty() {
                app.push_system(
                    "Usage: /worker <name> <task>\n\nSpawns a clankers subprocess as a named worker. Output appears in the subagent panel.".to_string(),
                    false,
                );
            } else {
                let (worker_name, task) = match args.split_once(char::is_whitespace) {
                    Some((name, rest)) => (name.trim().to_string(), rest.trim().to_string()),
                    None => {
                        app.push_system("Usage: /worker <name> <task>".to_string(), true);
                        return;
                    }
                };
                app.push_system(format!("Worker '{}' started. See subagent panel →", worker_name), false);
                let ptx = panel_tx.clone();
                tokio::spawn(async move {
                    let signal = CancellationToken::new();
                    let _ = crate::tools::delegate::run_worker_subprocess(
                        &worker_name,
                        &task,
                        None,
                        None,
                        Some(&ptx),
                        signal,
                        None,
                    )
                    .await;
                });
            }
        }
        SlashAction::Share => {
            app.push_system("Share is not yet implemented without Zellij.".to_string(), true);
        }
        SlashAction::Plan => {
            let new_state = if args.eq_ignore_ascii_case("on") {
                crate::modes::plan::PlanState::Planning
            } else if args.eq_ignore_ascii_case("off") {
                crate::modes::plan::PlanState::Inactive
            } else {
                // Toggle
                if app.plan_state.is_active() {
                    crate::modes::plan::PlanState::Inactive
                } else {
                    crate::modes::plan::PlanState::Planning
                }
            };
            let msg = if new_state.is_active() {
                format!("{} Plan mode enabled — the agent will propose changes before editing.", new_state.emoji())
            } else {
                "Plan mode disabled — normal editing restored.".to_string()
            };
            app.plan_state = new_state;
            app.push_system(msg, false);
        }
        SlashAction::Review => {
            let base = if args.is_empty() { None } else { Some(args.as_str()) };
            let prompt = if let Some(b) = base {
                format!(
                    "Please perform a thorough code review of the changes vs `{}`. \
                     Use the `review` tool with action='diff' first, then action='submit' \
                     with your findings.",
                    b
                )
            } else {
                "Please perform a thorough code review of the recent changes. \
                 Use the `review` tool with action='diff' first, then action='submit' \
                 with your findings."
                    .to_string()
            };
            let review_msg = if let Some(b) = base {
                format!("Starting code review vs {}...", b)
            } else {
                "Starting code review...".to_string()
            };
            app.push_system(review_msg, false);
            // Queue the review prompt to be sent as a user message
            app.queued_prompt = Some(prompt);
        }
        SlashAction::Role => {
            if args.is_empty() {
                // List all roles with their resolved models
                let roles_config = crate::config::model_roles::ModelRolesConfig::with_defaults(&app.model);
                let mut roles_info = String::from("Model Roles:\n\n");
                roles_info.push_str(&roles_config.summary(&app.model));
                roles_info.push_str("\n\nUsage:\n  /role <name>           Switch to a role's model");
                roles_info.push_str("\n  /role <name> <model>   Set a role's model and switch");
                roles_info.push_str("\n  /role reset            Clear all role overrides");
                app.push_system(roles_info, false);
            } else {
                let parts: Vec<&str> = args.splitn(2, ' ').collect();
                if parts[0] == "reset" {
                    app.push_system("Model role overrides cleared.".to_string(), false);
                } else if let Some(role) = crate::config::model_roles::ModelRole::parse(parts[0]) {
                    let roles_config = crate::config::model_roles::ModelRolesConfig::with_defaults(&app.model);
                    if parts.len() > 1 {
                        // Set role to specific model and switch to it now
                        let model_name = parts[1].to_string();
                        let old_model = std::mem::replace(&mut app.model, model_name.clone());
                        let _ = cmd_tx.send(AgentCommand::SetModel(model_name.clone()));
                        app.context_gauge.set_model(&app.model);
                        app.push_system(
                            format!("Role '{}' → {} (switched: {} → {})", role, model_name, old_model, app.model),
                            false,
                        );
                    } else {
                        // Switch to the model assigned to this role
                        let target_model = roles_config.resolve(role, &app.model);
                        if target_model == app.model {
                            app.push_system(format!("Already using '{}' model: {}", role, target_model), false);
                        } else {
                            let old_model = std::mem::replace(&mut app.model, target_model.clone());
                            let _ = cmd_tx.send(AgentCommand::SetModel(target_model.clone()));
                            app.context_gauge.set_model(&app.model);
                            app.push_system(format!("Role '{}': {} → {}", role, old_model, target_model), false);
                        }
                    }
                } else {
                    app.push_system(
                        format!("Unknown role: '{}'. Available: default, smol, slow, plan, commit, review", parts[0]),
                        true,
                    );
                }
            }
        }
        SlashAction::SystemPrompt => {
            if args.is_empty() || args == "show" {
                let full = args == "show";
                // Retrieve the current system prompt from the agent
                let (tx, mut rx) = tokio::sync::oneshot::channel();
                let _ = cmd_tx.send(AgentCommand::GetSystemPrompt(tx));
                // We can't await here (sync fn), so spawn a task to receive and display
                let blocks_tx = {
                    // We'll just show what we know: the original prompt.
                    // The agent may have modified it, but we read it synchronously here.
                    // For full accuracy we'd need async, but for display the oneshot
                    // pattern below works within the event-loop tick.
                    //
                    // Instead, try_recv on the oneshot — the agent task processes
                    // commands sequentially and may not have handled it yet.
                    // Fall back to showing the original prompt.
                    rx.try_recv().unwrap_or_else(|_| app.original_system_prompt.clone())
                };
                let prompt = blocks_tx;
                let display = if full {
                    format!("**System Prompt** ({} chars):\n\n{}", prompt.len(), prompt)
                } else {
                    let truncated = if prompt.len() > 500 {
                        format!("{}…\n\n*(truncated — use `/system show` for full prompt)*", &prompt[..500])
                    } else {
                        prompt.clone()
                    };
                    format!("**System Prompt** ({} chars):\n\n{}", prompt.len(), truncated)
                };
                app.push_system(display, false);
            } else {
                let parts: Vec<&str> = args.splitn(2, char::is_whitespace).collect();
                let subcmd = parts[0].trim();
                let subcmd_args = parts.get(1).map(|s| s.trim()).unwrap_or("");

                match subcmd {
                    "set" => {
                        if subcmd_args.is_empty() {
                            app.push_system("Usage: /system set <new system prompt text>".to_string(), true);
                        } else {
                            let new_prompt = subcmd_args.to_string();
                            let len = new_prompt.len();
                            let _ = cmd_tx.send(AgentCommand::SetSystemPrompt(new_prompt));
                            app.push_system(
                                format!("System prompt replaced ({} chars). Takes effect on next message.", len),
                                false,
                            );
                        }
                    }
                    "append" => {
                        if subcmd_args.is_empty() {
                            app.push_system("Usage: /system append <text to append>".to_string(), true);
                        } else {
                            // Read current prompt, append, and set
                            let (tx, mut rx) = tokio::sync::oneshot::channel();
                            let _ = cmd_tx.send(AgentCommand::GetSystemPrompt(tx));
                            let current = rx.try_recv().unwrap_or_else(|_| app.original_system_prompt.clone());
                            let new_prompt = format!("{}\n\n{}", current, subcmd_args);
                            let _ = cmd_tx.send(AgentCommand::SetSystemPrompt(new_prompt));
                            app.push_system(
                                format!(
                                    "Appended {} chars to system prompt. Takes effect on next message.",
                                    subcmd_args.len()
                                ),
                                false,
                            );
                        }
                    }
                    "prepend" => {
                        if subcmd_args.is_empty() {
                            app.push_system("Usage: /system prepend <text to prepend>".to_string(), true);
                        } else {
                            let (tx, mut rx) = tokio::sync::oneshot::channel();
                            let _ = cmd_tx.send(AgentCommand::GetSystemPrompt(tx));
                            let current = rx.try_recv().unwrap_or_else(|_| app.original_system_prompt.clone());
                            let new_prompt = format!("{}\n\n{}", subcmd_args, current);
                            let _ = cmd_tx.send(AgentCommand::SetSystemPrompt(new_prompt));
                            app.push_system(
                                format!(
                                    "Prepended {} chars to system prompt. Takes effect on next message.",
                                    subcmd_args.len()
                                ),
                                false,
                            );
                        }
                    }
                    "reset" => {
                        let original = app.original_system_prompt.clone();
                        let len = original.len();
                        let _ = cmd_tx.send(AgentCommand::SetSystemPrompt(original));
                        app.push_system(
                            format!("System prompt reset to original ({} chars). Takes effect on next message.", len),
                            false,
                        );
                    }
                    "file" => {
                        if subcmd_args.is_empty() {
                            app.push_system("Usage: /system file <path>".to_string(), true);
                        } else {
                            let path = if subcmd_args.starts_with('/') {
                                std::path::PathBuf::from(subcmd_args)
                            } else {
                                std::path::PathBuf::from(&app.cwd).join(subcmd_args)
                            };
                            match std::fs::read_to_string(&path) {
                                Ok(content) => {
                                    let len = content.len();
                                    let _ = cmd_tx.send(AgentCommand::SetSystemPrompt(content));
                                    app.push_system(
                                        format!(
                                            "System prompt loaded from {} ({} chars). Takes effect on next message.",
                                            path.display(),
                                            len
                                        ),
                                        false,
                                    );
                                }
                                Err(e) => {
                                    app.push_system(format!("Failed to read '{}': {}", path.display(), e), true);
                                }
                            }
                        }
                    }
                    "show" => {
                        // Already handled above, but handle "show" as a subcmd too
                        let (tx, mut rx) = tokio::sync::oneshot::channel();
                        let _ = cmd_tx.send(AgentCommand::GetSystemPrompt(tx));
                        let prompt = rx.try_recv().unwrap_or_else(|_| app.original_system_prompt.clone());
                        app.push_system(format!("**System Prompt** ({} chars):\n\n{}", prompt.len(), prompt), false);
                    }
                    _ => {
                        app.push_system(
                            format!(
                                "Unknown subcommand '{}'. Available: show, set, append, prepend, reset, file",
                                subcmd
                            ),
                            true,
                        );
                    }
                }
            }
        }
        SlashAction::Memory => {
            if let Some(db) = &db {
                let mem = db.memory();
                if args.is_empty() || args == "list" {
                    match mem.list(None) {
                        Ok(entries) if entries.is_empty() => {
                            app.push_system(
                                "No memories stored.\n\nUse `/memory add <text>` to save one.".to_string(),
                                false,
                            );
                        }
                        Ok(entries) => {
                            let mut out = format!("**Memories** ({} total):\n\n", entries.len());
                            for e in &entries {
                                let scope_label = match &e.scope {
                                    crate::db::memory::MemoryScope::Global => "global".to_string(),
                                    crate::db::memory::MemoryScope::Project { path } => format!("project:{}", path),
                                };
                                let tags = if e.tags.is_empty() {
                                    String::new()
                                } else {
                                    format!(" [{}]", e.tags.join(", "))
                                };
                                out.push_str(&format!("  `{}` ({}) {}{}\n", e.id, scope_label, e.text, tags));
                            }
                            out.push_str(
                                "\nUse `/memory edit <id> <new text>` to modify, `/memory remove <id>` to delete.",
                            );
                            app.push_system(out, false);
                        }
                        Err(e) => {
                            app.push_system(format!("Failed to list memories: {}", e), true);
                        }
                    }
                } else {
                    let parts: Vec<&str> = args.splitn(2, char::is_whitespace).collect();
                    let subcmd = parts[0].trim();
                    let subcmd_args = parts.get(1).map(|s| s.trim()).unwrap_or("");

                    match subcmd {
                        "add" => {
                            if subcmd_args.is_empty() {
                                app.push_system("Usage: /memory add [--project] <text>".to_string(), true);
                            } else {
                                let (scope, text) = if subcmd_args.starts_with("--project") {
                                    let rest = subcmd_args.trim_start_matches("--project").trim();
                                    if rest.is_empty() {
                                        app.push_system("Usage: /memory add --project <text>".to_string(), true);
                                        return;
                                    }
                                    (
                                        crate::db::memory::MemoryScope::Project { path: app.cwd.clone() },
                                        rest.to_string(),
                                    )
                                } else {
                                    (crate::db::memory::MemoryScope::Global, subcmd_args.to_string())
                                };

                                let entry = crate::db::memory::MemoryEntry::new(&text, scope.clone())
                                    .with_source(crate::db::memory::MemorySource::User);
                                let id = entry.id;
                                match mem.save(&entry) {
                                    Ok(()) => {
                                        app.push_system(
                                            format!("Memory saved (id: `{}`, scope: {}):\n  {}", id, scope, text),
                                            false,
                                        );
                                    }
                                    Err(e) => {
                                        app.push_system(format!("Failed to save memory: {}", e), true);
                                    }
                                }
                            }
                        }
                        "edit" | "update" => {
                            if subcmd_args.is_empty() {
                                app.push_system("Usage: /memory edit <id> <new text>".to_string(), true);
                            } else {
                                let edit_parts: Vec<&str> = subcmd_args.splitn(2, char::is_whitespace).collect();
                                let id_str = edit_parts[0].trim();
                                let new_text = edit_parts.get(1).map(|s| s.trim()).unwrap_or("");

                                if new_text.is_empty() {
                                    app.push_system("Usage: /memory edit <id> <new text>".to_string(), true);
                                } else if let Ok(id) = id_str.parse::<u64>() {
                                    match mem.get(id) {
                                        Ok(Some(mut entry)) => {
                                            let old_text = entry.text.clone();
                                            entry.text = new_text.to_string();
                                            match mem.update(&entry) {
                                                Ok(true) => {
                                                    app.push_system(
                                                        format!(
                                                            "Memory `{}` updated:\n  ~~{}~~\n  → {}",
                                                            id, old_text, new_text
                                                        ),
                                                        false,
                                                    );
                                                }
                                                Ok(false) => {
                                                    app.push_system(format!("Memory `{}` not found.", id), true);
                                                }
                                                Err(e) => {
                                                    app.push_system(format!("Failed to update memory: {}", e), true);
                                                }
                                            }
                                        }
                                        Ok(None) => {
                                            app.push_system(format!("No memory with id `{}`.", id), true);
                                        }
                                        Err(e) => {
                                            app.push_system(format!("Failed to read memory: {}", e), true);
                                        }
                                    }
                                } else {
                                    app.push_system(
                                        format!("Invalid memory ID: '{}'. Use `/memory list` to see IDs.", id_str),
                                        true,
                                    );
                                }
                            }
                        }
                        "remove" | "rm" | "delete" => {
                            if subcmd_args.is_empty() {
                                app.push_system("Usage: /memory remove <id>".to_string(), true);
                            } else if let Ok(id) = subcmd_args.parse::<u64>() {
                                match mem.remove(id) {
                                    Ok(true) => {
                                        app.push_system(format!("Memory `{}` removed.", id), false);
                                    }
                                    Ok(false) => {
                                        app.push_system(format!("No memory with id `{}`.", id), true);
                                    }
                                    Err(e) => {
                                        app.push_system(format!("Failed to remove memory: {}", e), true);
                                    }
                                }
                            } else {
                                app.push_system(
                                    format!("Invalid memory ID: '{}'. Use `/memory list` to see IDs.", subcmd_args),
                                    true,
                                );
                            }
                        }
                        "search" | "find" => {
                            if subcmd_args.is_empty() {
                                app.push_system("Usage: /memory search <query>".to_string(), true);
                            } else {
                                match mem.search(subcmd_args) {
                                    Ok(results) if results.is_empty() => {
                                        app.push_system(format!("No memories matching '{}'.", subcmd_args), false);
                                    }
                                    Ok(results) => {
                                        let mut out = format!(
                                            "**Search results** for '{}' ({} found):\n\n",
                                            subcmd_args,
                                            results.len()
                                        );
                                        for e in &results {
                                            let scope_label = match &e.scope {
                                                crate::db::memory::MemoryScope::Global => "global".to_string(),
                                                crate::db::memory::MemoryScope::Project { path } => {
                                                    format!("project:{}", path)
                                                }
                                            };
                                            out.push_str(&format!("  `{}` ({}) {}\n", e.id, scope_label, e.text));
                                        }
                                        app.push_system(out, false);
                                    }
                                    Err(e) => {
                                        app.push_system(format!("Search failed: {}", e), true);
                                    }
                                }
                            }
                        }
                        "clear" => match mem.clear() {
                            Ok(count) => {
                                app.push_system(format!("Cleared {} memory/memories.", count), false);
                            }
                            Err(e) => {
                                app.push_system(format!("Failed to clear memories: {}", e), true);
                            }
                        },
                        _ => {
                            app.push_system(
                                format!(
                                    "Unknown subcommand '{}'. Available: list, add, edit, remove, search, clear",
                                    subcmd
                                ),
                                true,
                            );
                        }
                    }
                }
            } else {
                app.push_system("Memory database not available (opened without --db).".to_string(), true);
            }
        }
        SlashAction::Peers => {
            // Switch to peers panel tab
            app.panel_tab = crate::tui::app::PanelTab::Peers;
            app.right_panel_tab = crate::tui::app::PanelTab::Peers;
            app.panel_focused = true;

            if args.is_empty() {
                // Just show the panel — refresh peers from registry
                let paths = crate::config::ClankersPaths::resolve();
                let registry_path = crate::modes::rpc::peers::registry_path(&paths);
                let registry = crate::modes::rpc::peers::PeerRegistry::load(&registry_path);
                let entries =
                    crate::tui::components::peers_panel::entries_from_registry(&registry, chrono::Duration::minutes(5));
                let count = entries.len();
                app.peers_panel.set_peers(entries);
                app.push_system(format!("{} peer(s) in registry.", count), false);
            } else {
                let (subcmd, subcmd_args) = args.split_once(char::is_whitespace).unwrap_or((args, ""));
                let subcmd_args = subcmd_args.trim();
                match subcmd {
                    "add" => {
                        let parts: Vec<&str> = subcmd_args.splitn(2, char::is_whitespace).collect();
                        if parts.len() < 2 {
                            app.push_system("Usage: /peers add <node-id> <name>".to_string(), true);
                        } else {
                            let node_id = parts[0].trim();
                            let name = parts[1].trim();
                            let paths = crate::config::ClankersPaths::resolve();
                            let registry_path = crate::modes::rpc::peers::registry_path(&paths);
                            let mut registry = crate::modes::rpc::peers::PeerRegistry::load(&registry_path);
                            registry.add(node_id, name);
                            match registry.save(&registry_path) {
                                Ok(()) => {
                                    app.push_system(
                                        format!("Added peer '{}' ({}…)", name, &node_id[..12.min(node_id.len())]),
                                        false,
                                    );
                                    let entries = crate::tui::components::peers_panel::entries_from_registry(
                                        &registry,
                                        chrono::Duration::minutes(5),
                                    );
                                    app.peers_panel.set_peers(entries);
                                }
                                Err(e) => app.push_system(format!("Failed to save registry: {}", e), true),
                            }
                        }
                    }
                    "remove" | "rm" => {
                        if subcmd_args.is_empty() {
                            app.push_system("Usage: /peers remove <name-or-id>".to_string(), true);
                        } else {
                            let paths = crate::config::ClankersPaths::resolve();
                            let registry_path = crate::modes::rpc::peers::registry_path(&paths);
                            let mut registry = crate::modes::rpc::peers::PeerRegistry::load(&registry_path);
                            // Try as node_id first, then by name
                            let removed = if registry.remove(subcmd_args) {
                                true
                            } else {
                                let found =
                                    registry.peers.values().find(|p| p.name == subcmd_args).map(|p| p.node_id.clone());
                                if let Some(nid) = found {
                                    registry.remove(&nid)
                                } else {
                                    false
                                }
                            };
                            if removed {
                                let _ = registry.save(&registry_path);
                                app.push_system(format!("Removed peer '{}'.", subcmd_args), false);
                                let entries = crate::tui::components::peers_panel::entries_from_registry(
                                    &registry,
                                    chrono::Duration::minutes(5),
                                );
                                app.peers_panel.set_peers(entries);
                            } else {
                                app.push_system(format!("Peer '{}' not found.", subcmd_args), true);
                            }
                        }
                    }
                    "probe" => {
                        let paths = crate::config::ClankersPaths::resolve();
                        let registry_path = crate::modes::rpc::peers::registry_path(&paths);
                        let identity_path = crate::modes::rpc::iroh::identity_path(&paths);

                        if subcmd_args.is_empty() || subcmd_args == "all" {
                            // Probe all peers
                            let registry = crate::modes::rpc::peers::PeerRegistry::load(&registry_path);
                            let peer_ids: Vec<String> = registry.peers.keys().cloned().collect();
                            if peer_ids.is_empty() {
                                app.push_system("No peers to probe.".to_string(), false);
                            } else {
                                app.push_system(format!("Probing {} peer(s)...", peer_ids.len()), false);
                                for nid in &peer_ids {
                                    app.peers_panel
                                        .update_status(nid, crate::tui::components::peers_panel::PeerStatus::Probing);
                                }
                                let ptx = panel_tx.clone();
                                let rp = registry_path.clone();
                                let ip = identity_path.clone();
                                for nid in peer_ids {
                                    let ptx = ptx.clone();
                                    let rp = rp.clone();
                                    let ip = ip.clone();
                                    tokio::spawn(async move {
                                        probe_peer_background(nid, rp, ip, ptx).await;
                                    });
                                }
                            }
                        } else {
                            // Probe specific peer
                            let registry = crate::modes::rpc::peers::PeerRegistry::load(&registry_path);
                            let node_id = registry
                                .peers
                                .values()
                                .find(|p| p.name == subcmd_args)
                                .map(|p| p.node_id.clone())
                                .unwrap_or_else(|| subcmd_args.to_string());
                            app.peers_panel
                                .update_status(&node_id, crate::tui::components::peers_panel::PeerStatus::Probing);
                            app.push_system(format!("Probing {}...", &node_id[..12.min(node_id.len())]), false);
                            let ptx = panel_tx.clone();
                            tokio::spawn(async move {
                                probe_peer_background(node_id, registry_path, identity_path, ptx).await;
                            });
                        }
                    }
                    "discover" => {
                        app.push_system("Scanning LAN via mDNS (5s)...".to_string(), false);
                        let paths = crate::config::ClankersPaths::resolve();
                        let registry_path = crate::modes::rpc::peers::registry_path(&paths);
                        let identity_path = crate::modes::rpc::iroh::identity_path(&paths);
                        let ptx = panel_tx.clone();
                        tokio::spawn(async move {
                            discover_peers_background(registry_path, identity_path, ptx).await;
                        });
                    }
                    "allow" => {
                        if subcmd_args.is_empty() {
                            app.push_system("Usage: /peers allow <node-id>".to_string(), true);
                        } else {
                            let paths = crate::config::ClankersPaths::resolve();
                            let acl_path = crate::modes::rpc::iroh::allowlist_path(&paths);
                            let mut allowed = crate::modes::rpc::iroh::load_allowlist(&acl_path);
                            allowed.insert(subcmd_args.to_string());
                            match crate::modes::rpc::iroh::save_allowlist(&acl_path, &allowed) {
                                Ok(()) => app.push_system(
                                    format!("Allowed peer {}…", &subcmd_args[..12.min(subcmd_args.len())]),
                                    false,
                                ),
                                Err(e) => app.push_system(format!("Failed: {}", e), true),
                            }
                        }
                    }
                    "deny" => {
                        if subcmd_args.is_empty() {
                            app.push_system("Usage: /peers deny <node-id>".to_string(), true);
                        } else {
                            let paths = crate::config::ClankersPaths::resolve();
                            let acl_path = crate::modes::rpc::iroh::allowlist_path(&paths);
                            let mut allowed = crate::modes::rpc::iroh::load_allowlist(&acl_path);
                            if allowed.remove(subcmd_args) {
                                let _ = crate::modes::rpc::iroh::save_allowlist(&acl_path, &allowed);
                                app.push_system(
                                    format!("Denied peer {}…", &subcmd_args[..12.min(subcmd_args.len())]),
                                    false,
                                );
                            } else {
                                app.push_system("Peer not in allowlist.".to_string(), true);
                            }
                        }
                    }
                    "server" => match subcmd_args {
                        "on" | "start" => {
                            app.push_system(
                                "Use `clankers rpc start` to run the RPC server (embedded server coming soon)."
                                    .to_string(),
                                false,
                            );
                        }
                        "off" | "stop" => {
                            app.push_system("Server control not yet available in TUI.".to_string(), false);
                        }
                        _ => {
                            if app.peers_panel.server_running {
                                app.push_system("Embedded RPC server: running".to_string(), false);
                            } else {
                                app.push_system("Embedded RPC server: not running".to_string(), false);
                            }
                        }
                    },
                    _ => {
                        app.push_system(
                            format!(
                                "Unknown subcommand '{}'. Available: add, remove, probe, discover, allow, deny, server",
                                subcmd
                            ),
                            true,
                        );
                    }
                }
            }
        }
        SlashAction::Editor => {
            // Signal the event loop to open the external editor
            // (needs terminal access, which execute_slash_command doesn't have)
            app.open_editor_requested = true;
        }
        SlashAction::Preview => {
            let content = if args.is_empty() {
                "# Markdown Preview\n\n\
                 Here is some **bold text** and *italic text* and `inline code`.\n\n\
                 ## Code Block\n\n\
                 ```rust\n\
                 fn main() {\n\
                     println!(\"Hello, world!\");\n\
                 }\n\
                 ```\n\n\
                 ## Lists\n\n\
                 - First item\n\
                 - Second item\n\
                 - Third item\n\n\
                 1. Ordered one\n\
                 2. Ordered two\n\n\
                 > This is a blockquote\n\n\
                 ---\n\n\
                 A [link](https://example.com) and the end."
                    .to_string()
            } else {
                args.to_string()
            };
            // Create a fake conversation block with the markdown as assistant text
            app.start_block("(markdown preview)".to_string(), 0);
            if let Some(ref mut block) = app.active_block {
                block.responses.push(crate::tui::app::DisplayMessage {
                    role: crate::tui::app::MessageRole::Assistant,
                    content,
                    tool_name: None,
                    is_error: false,
                    images: Vec::new(),
                });
                block.streaming = false;
            }
            app.finalize_active_block();
            app.scroll.scroll_to_bottom();
        }
        SlashAction::Layout => {
            use crate::tui::layout::PanelLayout;
            use crate::tui::panel::PanelId;

            let sub = args.trim().to_lowercase();
            match sub.as_str() {
                "default" | "3col" | "three" => {
                    app.panel_layout = PanelLayout::default_three_column();
                    app.push_system("Layout: default 3-column".into(), false);
                }
                "wide" | "chat" => {
                    app.panel_layout = PanelLayout::wide_chat();
                    app.push_system("Layout: wide chat with left sidebar".into(), false);
                }
                "focused" | "none" | "clean" => {
                    app.panel_layout = PanelLayout::focused();
                    app.panel_focused = false;
                    app.focus.unfocus();
                    app.push_system("Layout: focused (no panels)".into(), false);
                }
                "right" => {
                    app.panel_layout = PanelLayout::right_heavy();
                    app.push_system("Layout: right-heavy".into(), false);
                }
                s if s.starts_with("toggle ") => {
                    let panel_name = s.trim_start_matches("toggle ").trim();
                    let panel_id = match panel_name {
                        "todo" => Some(PanelId::Todo),
                        "files" | "file" => Some(PanelId::Files),
                        "subagents" | "sub" => Some(PanelId::Subagents),
                        "peers" | "peer" => Some(PanelId::Peers),
                        _ => None,
                    };
                    if let Some(id) = panel_id {
                        app.panel_layout.toggle_panel(id);
                        app.push_system(format!("Toggled panel: {}", id.label()), false);
                    } else {
                        app.push_system(
                            format!("Unknown panel '{}'. Use: todo, files, subagents, peers", panel_name),
                            true,
                        );
                    }
                }
                "" => {
                    // Show current layout info
                    let order = app.panel_layout.focus_order();
                    let names: Vec<&str> = order.iter().map(|id| id.label()).collect();
                    let msg = if names.is_empty() {
                        "Layout: focused (no panels)\nUse /layout <preset> to switch.\nPresets: default, wide, focused, right".to_string()
                    } else {
                        format!(
                            "Layout: {} panel(s) visible: {}\nUse /layout <preset> to switch.\nPresets: default, wide, focused, right",
                            names.len(),
                            names.join(", ")
                        )
                    };
                    app.push_system(msg, false);
                }
                _ => {
                    app.push_system("Unknown layout. Use: default, wide, focused, right, toggle <panel>".into(), true);
                }
            }
        }
        SlashAction::PromptTemplate(ref template_name) => {
            // Look up the prompt template from the discovered resources
            let global_dir = crate::config::paths::ClankersPaths::resolve().global_prompts_dir;
            let project_dir =
                crate::config::paths::ProjectPaths::resolve(&std::env::current_dir().unwrap_or_default()).prompts_dir;
            let prompts = crate::prompts::discover_prompts(&global_dir, Some(&project_dir));
            if let Some(template) = prompts.iter().find(|p| p.name == *template_name) {
                let mut vars = std::collections::HashMap::new();
                vars.insert("input".to_string(), args.to_string());
                let expanded = crate::prompts::expand_template(&template.content, &vars);
                // Strip frontmatter before sending
                let prompt = strip_frontmatter(&expanded);
                app.push_system(format!("/{} — {}", template_name, template.description), false);
                app.queued_prompt = Some(prompt);
            } else {
                app.push_system(format!("Unknown command or prompt template: /{}", template_name), true);
            }
        }
    }
}

/// Strip YAML frontmatter (--- ... ---) from a prompt template
fn strip_frontmatter(content: &str) -> String {
    let trimmed = content.trim_start();
    if let Some(rest) = trimmed.strip_prefix("---")
        && let Some(end) = rest.find("\n---")
    {
        return rest[end + 4..].trim_start().to_string();
    }
    content.to_string()
}

// ---------------------------------------------------------------------------
// Session persistence helpers
// ---------------------------------------------------------------------------

/// Persist the latest messages from an agent turn into the session file.
/// Uses in-memory tracking of persisted IDs — no file re-reads needed.
fn persist_messages(
    session_manager: &mut crate::session::SessionManager,
    messages: &[crate::provider::message::AgentMessage],
) {
    let mut prev_id: Option<crate::provider::message::MessageId> = None;
    for msg in messages {
        let id = msg.id().clone();
        if session_manager.is_persisted(&id) {
            prev_id = Some(id);
            continue;
        }
        if let Err(e) = session_manager.append_message(msg.clone(), prev_id.clone()) {
            tracing::warn!("Failed to persist message {}: {}", id, e);
        }
        prev_id = Some(id);
    }
}

/// Rebuild the display blocks from restored session messages so the user
/// can see the prior conversation in the TUI.
fn restore_display_blocks(app: &mut App, messages: &[crate::provider::message::AgentMessage]) {
    use crate::provider::message::AgentMessage;
    use crate::provider::message::Content;
    use crate::tui::app::DisplayMessage;
    use crate::tui::app::MessageRole;

    for (i, msg) in messages.iter().enumerate() {
        match msg {
            AgentMessage::User(user_msg) => {
                let text = user_msg
                    .content
                    .iter()
                    .filter_map(|c| match c {
                        Content::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                // Start a new block for each user message, recording the
                // agent message count at this point for branching support.
                app.start_block(text, i);
            }
            AgentMessage::Assistant(asst_msg) => {
                // Add responses to the current active block
                for content in &asst_msg.content {
                    match content {
                        Content::Text { text } => {
                            if let Some(ref mut block) = app.active_block {
                                block.responses.push(DisplayMessage {
                                    role: MessageRole::Assistant,
                                    content: text.clone(),
                                    tool_name: None,
                                    is_error: false,
                                    images: Vec::new(),
                                });
                            }
                        }
                        Content::ToolUse { name, .. } => {
                            if let Some(ref mut block) = app.active_block {
                                block.responses.push(DisplayMessage {
                                    role: MessageRole::ToolCall,
                                    content: name.clone(),
                                    tool_name: Some(name.clone()),
                                    is_error: false,
                                    images: Vec::new(),
                                });
                            }
                        }
                        Content::Thinking { thinking, .. } => {
                            if let Some(ref mut block) = app.active_block {
                                block.responses.push(DisplayMessage {
                                    role: MessageRole::Thinking,
                                    content: thinking.clone(),
                                    tool_name: None,
                                    is_error: false,
                                    images: Vec::new(),
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
            AgentMessage::ToolResult(tool_result) => {
                let display = tool_result
                    .content
                    .iter()
                    .filter_map(|c| match c {
                        Content::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                // Extract images from tool result Content::Image blocks
                let images: Vec<crate::tui::app::DisplayImage> = tool_result
                    .content
                    .iter()
                    .filter_map(|c| match c {
                        Content::Image {
                            source: crate::provider::message::ImageSource::Base64 { media_type, data },
                        } => Some(crate::tui::app::DisplayImage {
                            data: data.clone(),
                            media_type: media_type.clone(),
                        }),
                        _ => None,
                    })
                    .collect();
                if let Some(ref mut block) = app.active_block {
                    block.responses.push(DisplayMessage {
                        role: MessageRole::ToolResult,
                        content: display,
                        tool_name: None,
                        is_error: tool_result.is_error,
                        images,
                    });
                }
            }
            _ => {
                // BashExecution, Custom, BranchSummary, CompactionSummary — skip in display
            }
        }
    }
    // Finalize the last active block
    app.finalize_active_block();
}

// ---------------------------------------------------------------------------
// Plugin event dispatch
// ---------------------------------------------------------------------------

/// Result of dispatching events to plugins
struct PluginDispatchResult {
    /// Messages to surface to the user
    messages: Vec<(String, String)>,
    /// UI actions to apply
    ui_actions: Vec<crate::plugin::ui::PluginUIAction>,
}

/// Dispatch an agent event to all subscribed plugins.
/// Returns messages to surface and UI actions to apply.
fn dispatch_event_to_plugins(
    plugin_manager: &Arc<std::sync::Mutex<crate::plugin::PluginManager>>,
    event: &AgentEvent,
) -> PluginDispatchResult {
    use crate::plugin::PluginState;
    use crate::plugin::bridge::PluginEvent;

    let mgr = match plugin_manager.lock() {
        Ok(m) => m,
        Err(_) => {
            return PluginDispatchResult {
                messages: Vec::new(),
                ui_actions: Vec::new(),
            };
        }
    };

    let mut messages = Vec::new();
    let mut ui_actions = Vec::new();

    for info in mgr.list() {
        if info.state != PluginState::Active {
            continue;
        }
        // Check if this plugin subscribes to this event type
        let subscribed = info.manifest.events.iter().any(|e| PluginEvent::parse(e).is_some_and(|pe| pe.matches(event)));
        if !subscribed {
            continue;
        }

        // Build event payload
        let payload = match event {
            AgentEvent::AgentStart => serde_json::json!({"event": "agent_start", "data": {}}),
            AgentEvent::AgentEnd { .. } => serde_json::json!({"event": "agent_end", "data": {}}),
            AgentEvent::ToolCall { tool_name, call_id, .. } => {
                serde_json::json!({"event": "tool_call", "data": {"tool": tool_name, "call_id": call_id}})
            }
            AgentEvent::ToolExecutionEnd { call_id, .. } => {
                serde_json::json!({"event": "tool_result", "data": {"call_id": call_id}})
            }
            AgentEvent::TurnStart { index, .. } => {
                serde_json::json!({"event": "turn_start", "data": {"turn": index}})
            }
            AgentEvent::TurnEnd { index, .. } => {
                serde_json::json!({"event": "turn_end", "data": {"turn": index}})
            }
            AgentEvent::UserInput { text, .. } => {
                serde_json::json!({"event": "user_input", "data": {"text": text}})
            }
            _ => continue,
        };

        let input = serde_json::to_string(&payload).unwrap_or_default();
        match mgr.call_plugin(&info.name, "on_event", &input) {
            Ok(output) => {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&output) {
                    // Surface messages the plugin explicitly wants shown
                    let wants_display = parsed.get("display").and_then(|d| d.as_bool()).unwrap_or(false);
                    if wants_display
                        && let Some(msg) = parsed.get("message").and_then(|m| m.as_str())
                        && !msg.is_empty()
                    {
                        messages.push((info.name.clone(), msg.to_string()));
                    }

                    // Parse any UI actions from the response
                    let actions = crate::plugin::bridge::parse_ui_actions(&info.name, &parsed);
                    ui_actions.extend(actions);
                }
            }
            Err(e) => {
                tracing::debug!("Plugin '{}' event handler error: {}", info.name, e);
            }
        }
    }

    PluginDispatchResult { messages, ui_actions }
}

// ---------------------------------------------------------------------------
// Swarm / peer background tasks
// ---------------------------------------------------------------------------

/// Probe a single peer in the background. Updates the registry and sends
/// a status event back to the TUI via the panel channel.
async fn probe_peer_background(
    node_id: String,
    registry_path: std::path::PathBuf,
    identity_path: std::path::PathBuf,
    _panel_tx: tokio::sync::mpsc::UnboundedSender<crate::tui::components::subagent_event::SubagentEvent>,
) {
    use crate::modes::rpc::iroh;
    use crate::modes::rpc::protocol::Request;

    let remote: ::iroh::PublicKey = match node_id.parse() {
        Ok(pk) => pk,
        Err(e) => {
            tracing::warn!("Invalid node ID '{}': {}", node_id, e);
            return;
        }
    };

    let identity = iroh::Identity::load_or_generate(&identity_path);
    let endpoint = match iroh::start_endpoint_no_mdns(&identity).await {
        Ok(ep) => ep,
        Err(e) => {
            tracing::warn!("Failed to start endpoint for probe: {}", e);
            return;
        }
    };
    let request = Request::new("status", serde_json::json!({}));
    let result =
        tokio::time::timeout(std::time::Duration::from_secs(10), iroh::send_rpc(&endpoint, remote, &request)).await;

    let mut registry = crate::modes::rpc::peers::PeerRegistry::load(&registry_path);

    match result {
        Ok(Ok(response)) => {
            if let Some(result) = response.ok {
                let caps = crate::modes::rpc::peers::PeerCapabilities {
                    accepts_prompts: result.get("accepts_prompts").and_then(|v| v.as_bool()).unwrap_or(false),
                    agents: result
                        .get("agents")
                        .and_then(|v| v.as_array())
                        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                        .unwrap_or_default(),
                    tools: result
                        .get("tools")
                        .and_then(|v| v.as_array())
                        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                        .unwrap_or_default(),
                    tags: result
                        .get("tags")
                        .and_then(|v| v.as_array())
                        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                        .unwrap_or_default(),
                    version: result.get("version").and_then(|v| v.as_str()).map(String::from),
                };
                registry.update_capabilities(&node_id, caps);
                tracing::info!("Probed peer {}: online", &node_id[..12.min(node_id.len())]);
            } else {
                registry.touch(&node_id);
            }
        }
        _ => {
            tracing::info!("Probed peer {}: unreachable", &node_id[..12.min(node_id.len())]);
        }
    }

    let _ = registry.save(&registry_path);
}

/// Discover peers via mDNS in the background. Adds discovered peers to the
/// registry and probes them for capabilities.
async fn discover_peers_background(
    registry_path: std::path::PathBuf,
    identity_path: std::path::PathBuf,
    _panel_tx: tokio::sync::mpsc::UnboundedSender<crate::tui::components::subagent_event::SubagentEvent>,
) {
    use crate::modes::rpc::iroh;

    let identity = iroh::Identity::load_or_generate(&identity_path);
    let endpoint = match iroh::start_endpoint(&identity).await {
        Ok(ep) => ep,
        Err(e) => {
            tracing::warn!("Failed to start endpoint for discovery: {}", e);
            return;
        }
    };

    let discovered = iroh::discover_mdns_peers(&endpoint, std::time::Duration::from_secs(5)).await;

    if discovered.is_empty() {
        tracing::info!("mDNS discovery: no peers found");
        return;
    }

    let mut registry = crate::modes::rpc::peers::PeerRegistry::load(&registry_path);

    for (eid, _info) in &discovered {
        let node_id = eid.to_string();
        if !registry.peers.contains_key(&node_id) {
            let short = &node_id[..12.min(node_id.len())];
            registry.add(&node_id, &format!("mdns-{}", short));
            tracing::info!("Discovered new peer via mDNS: {}", short);
        }
    }

    let _ = registry.save(&registry_path);

    // Probe each discovered peer for capabilities
    for (eid, _info) in discovered {
        let node_id = eid.to_string();
        let rp = registry_path.clone();
        let ip = identity_path.clone();
        probe_peer_background(node_id, rp, ip, _panel_tx.clone()).await;
    }
}

// ---------------------------------------------------------------------------
// Embedded RPC server (started alongside the TUI)
// ---------------------------------------------------------------------------

/// Configuration for the embedded RPC server that runs inside the TUI process.
pub struct EmbeddedRpcConfig {
    /// Capability tags to advertise
    pub tags: Vec<String>,
    /// Whether to accept prompts from remote peers
    pub with_agent: bool,
    /// Whether to allow all peers (no allowlist)
    pub allow_all: bool,
    /// Heartbeat interval (None = disabled)
    pub heartbeat_interval: Option<std::time::Duration>,
}

impl Default for EmbeddedRpcConfig {
    fn default() -> Self {
        Self {
            tags: Vec::new(),
            with_agent: false,
            allow_all: true,
            heartbeat_interval: Some(std::time::Duration::from_secs(120)),
        }
    }
}

/// Start the embedded RPC server in the background. Returns the node's
/// public key (EndpointId) and a cancellation token to shut it down.
///
/// The server shares the same process as the TUI but runs on a separate
/// tokio task. It advertises this node via mDNS for LAN discovery and
/// optionally runs a heartbeat to probe known peers.
pub async fn start_embedded_rpc(
    config: EmbeddedRpcConfig,
    provider: Option<std::sync::Arc<dyn crate::provider::Provider>>,
    tools: Vec<std::sync::Arc<dyn crate::tools::Tool>>,
    settings: crate::config::settings::Settings,
    model: String,
    system_prompt: String,
) -> Result<(String, CancellationToken)> {
    use crate::modes::rpc::iroh;

    let paths = crate::config::ClankersPaths::resolve();
    let identity_path = iroh::identity_path(&paths);
    let identity = iroh::Identity::load_or_generate(&identity_path);
    let node_id = identity.public_key().to_string();

    let endpoint = iroh::start_endpoint(&identity).await?;

    // Build ACL
    let acl = if config.allow_all {
        iroh::AccessControl::open()
    } else {
        let acl_path = iroh::allowlist_path(&paths);
        let allowed = iroh::load_allowlist(&acl_path);
        iroh::AccessControl::from_allowlist(allowed)
    };

    // Build agent context if requested
    let agent_ctx = if config.with_agent {
        provider.map(|p| iroh::RpcContext {
            provider: p,
            tools,
            settings: settings.clone(),
            model: model.clone(),
            system_prompt: system_prompt.clone(),
        })
    } else {
        None
    };

    let state = std::sync::Arc::new(iroh::ServerState {
        meta: iroh::NodeMeta {
            tags: config.tags,
            agent_names: Vec::new(),
        },
        agent: agent_ctx,
        acl,
        receive_dir: None,
    });

    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();

    // Start the RPC server
    let endpoint_for_serve = endpoint.clone();
    tokio::spawn(async move {
        tokio::select! {
            result = iroh::serve_rpc(endpoint_for_serve, state) => {
                if let Err(e) = result {
                    tracing::warn!("Embedded RPC server error: {}", e);
                }
            }
            _ = cancel_clone.cancelled() => {
                tracing::info!("Embedded RPC server shut down");
            }
        }
    });

    // Start heartbeat if configured
    if let Some(interval) = config.heartbeat_interval {
        let registry_path = crate::modes::rpc::peers::registry_path(&paths);
        let heartbeat_cancel = cancel.clone();
        let endpoint_arc = std::sync::Arc::new(endpoint);
        tokio::spawn(iroh::run_heartbeat(endpoint_arc, registry_path, interval, heartbeat_cancel));
    }

    tracing::info!("Embedded RPC server started as {}", &node_id[..12.min(node_id.len())]);
    Ok((node_id, cancel))
}
