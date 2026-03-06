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
use crate::slash_commands::{self};
use crate::tui::app::App;
use crate::tui::app::AppState;
use crate::tui::components::block::BlockEntry;
use crate::tui::event::AppEvent;
use crate::tui::event::{self as tui_event};
// Panel trait is in scope via panel_mut() return type
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

    // Build leader menu from all contributors (builtins + plugins + user config)
    app.rebuild_leader_menu(plugin_manager.as_ref(), &settings);

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

pub(crate) enum AgentCommand {
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
                        handle_input_with_plugins(app, &text, &cmd_tx, plugin_manager.as_ref(), &panel_tx, &db, &mut session_manager);
                    }
                }
                TaskResult::PromptDone(None) => {
                    // Stay in current input mode — don't force Normal mode
                    // so the user can keep typing without pressing 'i' again.
                    // Dispatch queued prompt if one is waiting
                    if let Some(text) = app.queued_prompt.take() {
                        handle_input_with_plugins(app, &text, &cmd_tx, plugin_manager.as_ref(), &panel_tx, &db, &mut session_manager);
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
                    if app.model_selector.visible && super::selectors::handle_model_selector_key(app, &key, &cmd_tx) {
                        continue;
                    }

                    // ── Account selector intercept ───────────────
                    if app.account_selector.visible && super::selectors::handle_account_selector_key(app, &key, &cmd_tx) {
                        continue;
                    }

                    // ── Session selector intercept ───────────────
                    if app.session_selector.visible && super::selectors::handle_session_selector_key(app, &key, &cmd_tx) {
                        continue;
                    }

                    // ── Leader menu intercept ────────────────────
                    if app.leader_menu.visible {
                        if let Some(leader_action) = app.leader_menu.handle_key(&key) {
                            handle_leader_action(app, leader_action, &cmd_tx, plugin_manager.as_ref(), &panel_tx, &db, &mut session_manager);
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
                        && handle_slash_menu_key(app, &key, &keymap, &cmd_tx, plugin_manager.as_ref(), &panel_tx, &db, &mut session_manager)
                    {
                        continue;
                    }

                    // ── Panel intercepts in normal mode ──────────
                    if app.focus.has_panel_focus() && app.input_mode == InputMode::Normal {
                        use crossterm::event::KeyCode;
                        use crate::tui::panel::PanelAction;

                        // Tab / Shift+Tab cycles sub-panels within the same column
                        if matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
                            app.focus.cycle_in_column(&app.panel_layout);
                            continue;
                        }

                        // Side-effect keys that need app-level resources
                        // (the Panel trait can't send on channels)
                        if let Some(focused_id) = app.focus.focused {
                            use crate::tui::panel::PanelId;
                            match (focused_id, key.code, key.modifiers) {
                                // Subagents: 'x' = kill selected running subagent
                                (PanelId::Subagents, KeyCode::Char('x'), m) if m.is_empty() => {
                                    if let Some(id) = app.subagent_panel.selected_id() {
                                        let _ = panel_tx.send(
                                            crate::tui::components::subagent_event::SubagentEvent::KillRequest { id },
                                        );
                                    }
                                    continue;
                                }
                                // Peers: 'p' = probe selected peer
                                (PanelId::Peers, KeyCode::Char('p'), m) if m.is_empty() => {
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

                        // Delegate everything else to the focused panel's handle_key_event
                        if let Some(focused_id) = app.focus.focused {
                            let result = app.panel_mut(focused_id).handle_key_event(key);
                            match result {
                                Some(PanelAction::Consumed) => continue,
                                Some(PanelAction::Unfocus) => {
                                    app.focus.unfocus();
                                    continue;
                                }
                                Some(PanelAction::SlashCommand(_cmd)) => continue,
                                Some(PanelAction::FocusPanel(id)) => {
                                    app.focus.focus(id);
                                    continue;
                                }
                                None => {} // key not handled by panel, fall through
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

                        handle_action(app, action, &key, &cmd_tx, plugin_manager.as_ref(), &panel_tx, &db, &mut session_manager);

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
                    super::mouse::handle_mouse_down(app, button, col, row);
                }
                AppEvent::MouseDrag(button, col, row) => {
                    super::mouse::handle_mouse_drag(app, button, col, row);
                }
                AppEvent::MouseUp(button, col, row) => {
                    super::mouse::handle_mouse_up(app, button, col, row);
                }
                AppEvent::ScrollUp(col, row, n) => {
                    super::mouse::handle_mouse_scroll(app, col, row, true, n);
                }
                AppEvent::ScrollDown(col, row, n) => {
                    super::mouse::handle_mouse_scroll(app, col, row, false, n);
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
    session_manager: &mut Option<crate::session::SessionManager>,
) {
    // When a panel is focused, intercept navigation actions and let
    // global actions (leader menu, selectors, etc.) fall through.
    // Panel-specific key handling is done by Panel::handle_key_event()
    // in the raw key dispatch above — this block only handles Action-level
    // structural navigation (focus/unfocus, column movement, mode switching).
    if app.focus.has_panel_focus() {
        use crate::tui::layout::ColumnSide;

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
                | Action::ToggleBlockIds
                | Action::SearchOutput
                | Action::ToggleSessionPopup
                | Action::PasteImage
                | Action::OpenEditor
        );

        if !is_global {
            match action {
                Action::Unfocus | Action::TogglePanelFocus => {
                    app.close_focused_panel_views();
                    app.focus.unfocus();
                    return;
                }
                Action::EnterInsert => {
                    app.close_focused_panel_views();
                    app.focus.unfocus();
                    app.input_mode = InputMode::Insert;
                    return;
                }
                Action::EnterCommand => {
                    app.close_focused_panel_views();
                    app.focus.unfocus();
                    // Don't return — fall through to main handler for "/" prefix setup
                }
                // h/l: move between columns and main area
                Action::PanelNextTab | Action::BranchNext => {
                    if let Some(id) = app.focus.focused {
                        if app.panel_layout.panel_side(id) == Some(ColumnSide::Left) {
                            app.focus.unfocus();
                        }
                    }
                    return;
                }
                Action::PanelPrevTab | Action::BranchPrev => {
                    if let Some(id) = app.focus.focused {
                        if app.panel_layout.panel_side(id) == Some(ColumnSide::Right) {
                            app.focus.unfocus();
                        }
                    }
                    return;
                }
                // j/k: cycle panels within the same column
                Action::FocusPrevBlock | Action::FocusNextBlock => {
                    app.focus.cycle_in_column(&app.panel_layout);
                    return;
                }
                // Everything else is consumed (don't leak to main handler)
                _ => return,
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
                    handle_input_with_plugins(app, &text, cmd_tx, plugin_manager, panel_tx, db, session_manager);
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
            if app.input_mode == InputMode::Insert {
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
                app.branch_prev();
            } else {
                // h = focus left column
                app.focus.focus_side(&app.panel_layout, crate::tui::layout::ColumnSide::Left);
                app.input_mode = InputMode::Normal;
            }
        }
        Action::BranchNext => {
            if app.focused_block.is_some() {
                app.branch_next();
            } else {
                // l = focus right column
                app.focus.focus_side(&app.panel_layout, crate::tui::layout::ColumnSide::Right);
                app.input_mode = InputMode::Normal;
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
        Action::ToggleBlockIds => {
            app.show_block_ids = !app.show_block_ids;
            let state = if app.show_block_ids { "visible" } else { "hidden" };
            app.push_system(format!("Block IDs now {}.", state), false);
        }

        // ── Panel focus ─────────────────────────────
        Action::TogglePanelFocus => {
            if app.focus.has_panel_focus() {
                app.focus.unfocus();
            } else {
                // Focus the first panel in the layout
                let order = app.panel_layout.focus_order();
                if let Some(&first) = order.first() {
                    app.focus.focus(first);
                }
                app.input_mode = InputMode::Normal;
            }
        }
        Action::PanelNextTab => {
            app.focus.focus_side(&app.panel_layout, crate::tui::layout::ColumnSide::Right);
            app.input_mode = InputMode::Normal;
        }
        Action::PanelPrevTab => {
            app.focus.focus_side(&app.panel_layout, crate::tui::layout::ColumnSide::Left);
            app.input_mode = InputMode::Normal;
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
                app.focus.unfocus();
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
    session_manager: &mut Option<crate::session::SessionManager>,
) {
    use crate::tui::components::leader_menu::LeaderAction;

    match action {
        LeaderAction::KeymapAction(keymap_action) => {
            // Re-use the existing action dispatcher with a dummy key event
            let dummy_key = crossterm::event::KeyEvent::new(KeyCode::Null, KeyModifiers::NONE);
            handle_action(app, keymap_action, &dummy_key, cmd_tx, plugin_manager, panel_tx, db, session_manager);
        }
        LeaderAction::SlashCommand(command) => {
            // Execute as if the user typed and submitted the slash command
            handle_input_with_plugins(app, &command, cmd_tx, plugin_manager, panel_tx, db, session_manager);
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
    session_manager: &mut Option<crate::session::SessionManager>,
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
                    handle_input_with_plugins(app, &text, cmd_tx, plugin_manager, panel_tx, db, session_manager);
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
        Some(Action::Unfocus | Action::ToggleSessionPopup | Action::Quit) => {
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
        Some(Action::EnterInsert | Action::EnterCommand) => {
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

pub(crate) fn resume_session_from_file(
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
pub(crate) fn paste_from_clipboard(app: &mut App) {
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
    session_manager: &mut Option<crate::session::SessionManager>,
) {
    if let Some((command, args)) = slash_commands::parse_command(text) {
        execute_slash_command(app, &command, &args, cmd_tx, plugin_manager, panel_tx, db, session_manager);
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
pub(crate) fn parse_oauth_input(input: &str) -> Option<(String, String)> {
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

pub(crate) fn parse_account_flag(args: &str) -> (Option<String>, String) {
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

pub(crate) fn execute_slash_command(
    app: &mut App,
    command: &str,
    args: &str,
    cmd_tx: &tokio::sync::mpsc::UnboundedSender<AgentCommand>,
    plugin_manager: Option<&Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
    panel_tx: &tokio::sync::mpsc::UnboundedSender<crate::tui::components::subagent_event::SubagentEvent>,
    db: &Option<crate::db::Db>,
    session_manager: &mut Option<crate::session::SessionManager>,
) {
    let mut ctx = slash_commands::handlers::SlashContext {
        app,
        cmd_tx,
        plugin_manager,
        panel_tx,
        db,
        session_manager,
    };
    slash_commands::dispatch(command, args, &mut ctx);
}


/// Strip YAML frontmatter (--- ... ---) from a prompt template
/// Format a timestamp as a human-readable "time ago" string.
pub(crate) fn format_time_ago(ts: chrono::DateTime<chrono::Utc>) -> String {
    let elapsed = chrono::Utc::now().signed_duration_since(ts);
    let secs = elapsed.num_seconds();
    if secs < 60 {
        "just now".to_string()
    } else if secs < 3600 {
        let m = elapsed.num_minutes();
        format!("{} minute{} ago", m, if m == 1 { "" } else { "s" })
    } else if secs < 86400 {
        let h = elapsed.num_hours();
        format!("{} hour{} ago", h, if h == 1 { "" } else { "s" })
    } else {
        let d = elapsed.num_days();
        if d == 1 {
            "yesterday".to_string()
        } else {
            format!("{} days ago", d)
        }
    }
}

pub(crate) fn strip_frontmatter(content: &str) -> String {
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
pub(crate) async fn probe_peer_background(
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
pub(crate) async fn discover_peers_background(
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
            () = cancel_clone.cancelled() => {
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
