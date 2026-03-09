//! Full TUI interactive mode

use std::io;
use std::sync::Arc;

use crossterm::event::DisableBracketedPaste;
use crossterm::event::DisableMouseCapture;
use crossterm::event::EnableBracketedPaste;
use crossterm::event::EnableMouseCapture;
use crossterm::execute;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
use crossterm::terminal::{self};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio_util::sync::CancellationToken;

use crate::agent::Agent;
use crate::agent::events::AgentEvent;
use crate::config::keybindings::Keymap;
use crate::error::Result;
use crate::provider::auth::AuthStoreExt;
use crate::tui::app::App;
// Panel trait is in scope via panel_mut() return type
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

    // Build slash command registry with plugin contributions
    app.rebuild_slash_registry(plugin_manager.as_ref());

    // ── Session persistence setup ────────────────────────────────────────
    let paths = crate::config::ClankersPaths::get();
    let original_cwd = cwd.clone();

    // Open the global database for worktree registry + GC
    let db_path = paths.global_config_dir.join("clankers.db");
    let db = crate::db::Db::open(&db_path).ok();

    // Run startup GC in the background if we're in a git repo with worktrees enabled
    if settings.use_worktrees
        && let Some(ref db) = db
        && let Some(repo_root) = crate::worktree::WorktreeManager::find_repo_root(std::path::Path::new(&cwd))
    {
        let db_clone = db.clone();
        crate::worktree::gc::spawn_startup_gc(db_clone, repo_root);
    }

    let (session_manager, seed_messages, worktree_setup) =
        setup_session(&mut app, &cwd, &model, &db, &settings, resume_opts);

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
        let paths = crate::config::ClankersPaths::get();
        let store = crate::provider::auth::AuthStore::load(&paths.global_auth);
        app.active_account = store.active_account_name().to_string();
    }

    let (agent, event_rx, mut bash_confirm_rx) = build_agent_with_tools(
        provider.clone(),
        &settings,
        model.clone(),
        system_prompt.clone(),
        &mut app,
        panel_tx.clone(),
        todo_tx.clone(),
        plugin_manager.as_ref(),
        &paths,
        &db,
    );

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
                {
                    let peers_panel = peers_panel(&mut app);
                    peers_panel.self_id = Some(short_id);
                    peers_panel.server_running = true;
                    // Load initial peer list
                    let registry =
                        crate::modes::rpc::peers::PeerRegistry::load(&crate::modes::rpc::peers::registry_path(paths));
                    let entries = crate::tui::components::peers_panel::entries_from_registry(
                        &registry,
                        chrono::Duration::minutes(5),
                    );
                    peers_panel.set_peers(entries);
                }
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
        &settings,
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

/// Set up session persistence: create new session or resume existing one
fn setup_session(
    app: &mut App,
    cwd: &str,
    model: &str,
    db: &Option<crate::db::Db>,
    settings: &crate::config::settings::Settings,
    resume_opts: ResumeOptions,
) -> (
    Option<crate::session::SessionManager>,
    Vec<crate::provider::message::AgentMessage>,
    Option<crate::worktree::session_bridge::WorktreeSetup>,
) {
    let paths = crate::config::ClankersPaths::get();
    let sessions_dir = &paths.global_sessions_dir;
    let use_worktrees = settings.use_worktrees;

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

    if resume_opts.no_session {
        (None, Vec::new(), None)
    } else if resume_opts.continue_last {
        // Find the most recent session for this cwd
        let files = crate::session::store::list_sessions(sessions_dir, cwd);
        if let Some(latest_file) = files.into_iter().next() {
            match crate::session::SessionManager::open(latest_file) {
                Ok(mgr) => resume_session(app, mgr, "continue"),
                Err(e) => {
                    app.push_system(format!("Failed to resume last session: {}", e), true);
                    create_new_session(app, cwd, db)
                }
            }
        } else {
            app.push_system("No previous session found. Starting new session.".to_string(), false);
            create_new_session(app, cwd, db)
        }
    } else if let Some(ref session_id) = resume_opts.session_id {
        // Resume a specific session by ID
        let files = crate::session::store::list_sessions(sessions_dir, cwd);
        let found = files
            .into_iter()
            .find(|f| f.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.contains(session_id)));
        if let Some(file) = found {
            match crate::session::SessionManager::open(file) {
                Ok(mgr) => resume_session(app, mgr, "resume"),
                Err(e) => {
                    app.push_system(format!("Failed to resume session '{}': {}", session_id, e), true);
                    create_new_session(app, cwd, db)
                }
            }
        } else {
            app.push_system(format!("Session '{}' not found.", session_id), true);
            create_new_session(app, cwd, db)
        }
    } else {
        // Default: create a new session
        create_new_session(app, cwd, db)
    }
}

/// Build agent with all tools and configuration
#[allow(clippy::type_complexity)]
fn build_agent_with_tools(
    provider: Arc<dyn crate::provider::Provider>,
    settings: &crate::config::settings::Settings,
    model: String,
    system_prompt: String,
    app: &mut App,
    panel_tx: tokio::sync::mpsc::UnboundedSender<crate::tui::components::subagent_event::SubagentEvent>,
    todo_tx: tokio::sync::mpsc::UnboundedSender<(
        crate::tools::todo::TodoAction,
        tokio::sync::oneshot::Sender<crate::tools::todo::TodoResponse>,
    )>,
    plugin_manager: Option<&Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
    paths: &crate::config::ClankersPaths,
    db: &Option<crate::db::Db>,
) -> (Agent, tokio::sync::broadcast::Receiver<AgentEvent>, crate::tools::bash::ConfirmRx) {
    // Create a temporary agent with empty tools to get event_tx for tool construction
    let temp_agent =
        Agent::new(Arc::clone(&provider), Vec::new(), settings.clone(), model.clone(), system_prompt.clone());
    let event_tx = temp_agent.event_sender();
    let (bash_confirm_tx, bash_confirm_rx) = crate::tools::bash::confirm_channel();

    // Create and start the process monitor
    let process_monitor = {
        let config = crate::procmon::ProcessMonitorConfig::default();
        let monitor = std::sync::Arc::new(crate::procmon::ProcessMonitor::new(config, Some(event_tx.clone())));
        monitor.clone().start();
        monitor
    };

    // Wire process monitor into the TUI panel
    *process_panel(app) =
        crate::tui::components::process_panel::ProcessPanel::new().with_monitor(process_monitor.clone());

    let tool_env = crate::modes::common::ToolEnv {
        event_tx: Some(event_tx),
        panel_tx: Some(panel_tx),
        todo_tx: Some(todo_tx),
        bash_confirm_tx: Some(bash_confirm_tx),
        process_monitor: Some(process_monitor),
    };
    let tools = crate::modes::common::build_all_tools_with_env(&tool_env, plugin_manager);

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
    if let Some(pm) = plugin_manager {
        for action in crate::modes::common::fire_plugin_init(pm) {
            app.plugin_ui.apply(action);
        }
    }

    // Build the final agent with tools, db, routing, and cost tracking
    let mut agent_builder = crate::agent::builder::AgentBuilder::new(provider, settings.clone(), model, system_prompt)
        .with_tools(tools)
        .with_paths(paths.clone());

    // Attach the global database so the agent can read memories and record usage
    if let Some(db) = db {
        agent_builder = agent_builder.with_db(db.clone());
    }

    // Build the agent (automatically wires routing and cost tracking from settings)
    let agent = agent_builder.build();

    // Extract cost tracker reference for the app UI
    if settings.cost_tracking.is_some() {
        app.cost_tracker = agent.cost_tracker().cloned();
    }

    let event_rx = agent.subscribe();

    (agent, event_rx, bash_confirm_rx)
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
}

pub(crate) enum TaskResult {
    PromptDone(Option<crate::error::Error>),
    LoginDone(std::result::Result<String, String>),
    ThinkingToggled(String, crate::provider::ThinkingLevel),
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
    event_rx: tokio::sync::broadcast::Receiver<AgentEvent>,
    panel_rx: &mut tokio::sync::mpsc::UnboundedReceiver<crate::tui::components::subagent_event::SubagentEvent>,
    todo_rx: &mut tokio::sync::mpsc::UnboundedReceiver<(
        crate::tools::todo::TodoAction,
        tokio::sync::oneshot::Sender<crate::tools::todo::TodoResponse>,
    )>,
    bash_confirm_rx: &mut crate::tools::bash::ConfirmRx,
    panel_tx: tokio::sync::mpsc::UnboundedSender<crate::tui::components::subagent_event::SubagentEvent>,
    keymap: Keymap,
    plugin_manager: Option<Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
    session_manager: Option<crate::session::SessionManager>,
    seed_messages: Vec<crate::provider::message::AgentMessage>,
    db: Option<crate::db::Db>,
    settings: &crate::config::settings::Settings,
) -> Result<()> {
    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel::<AgentCommand>();
    let (done_tx, done_rx) = tokio::sync::mpsc::unbounded_channel::<TaskResult>();

    let mut agent = agent;

    // If we have seed messages from a resumed session, restore them into the agent
    // and rebuild display blocks so the user sees the conversation
    if !seed_messages.is_empty() {
        super::session_restore::restore_display_blocks(app, &seed_messages);
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
                            let paths = crate::config::ClankersPaths::get();
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
                    let paths = crate::config::ClankersPaths::get();
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
                AgentCommand::Quit => break,
            }
        }
    });

    // Delegate to EventLoopRunner which decomposes the loop body into
    // focused methods (drain_agent_events, drain_panel_events, etc.)
    let mut runner = super::event_loop_runner::EventLoopRunner::new(
        terminal,
        app,
        event_rx,
        panel_rx,
        todo_rx,
        bash_confirm_rx,
        panel_tx,
        keymap,
        plugin_manager,
        session_manager,
        db,
        settings,
        cmd_tx,
        done_rx,
    );
    runner.run()
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
            app.conversation.blocks.clear();
            app.conversation.all_blocks.clear();
            app.conversation.active_block = None;
            super::session_restore::restore_display_blocks(app, &msgs);

            // Seed the agent with restored messages
            let _ = cmd_tx.send(AgentCommand::SeedMessages(msgs));

            app.push_system(format!("Resumed session {} ({} messages)", mgr.session_id(), msg_count), false);
            app.conversation.scroll.scroll_to_bottom();
        }
        Err(e) => {
            app.push_system(format!("Failed to resume session: {}", e), true);
        }
    }
}

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

// execute_slash_command() was removed — slash dispatch is now inlined
// at the call site in event_loop.rs using std::mem::take() to avoid
// the self-referential borrow (registry lives inside App, which is
// mutably borrowed by SlashContext).

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
pub(crate) fn persist_messages(
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

// ---------------------------------------------------------------------------
// Plugin event dispatch
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Swarm / peer background tasks
// ---------------------------------------------------------------------------

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

    let paths = crate::config::ClankersPaths::get();
    let identity_path = iroh::identity_path(paths);
    let identity = iroh::Identity::load_or_generate(&identity_path);
    let node_id = identity.public_key().to_string();

    let endpoint = iroh::start_endpoint(&identity).await?;

    // Build ACL
    let acl = if config.allow_all {
        iroh::AccessControl::open()
    } else {
        let acl_path = iroh::allowlist_path(paths);
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
        let registry_path = crate::modes::rpc::peers::registry_path(paths);
        let heartbeat_cancel = cancel.clone();
        let endpoint_arc = std::sync::Arc::new(endpoint);
        tokio::spawn(iroh::run_heartbeat(endpoint_arc, registry_path, interval, heartbeat_cancel));
    }

    tracing::info!("Embedded RPC server started as {}", &node_id[..12.min(node_id.len())]);
    Ok((node_id, cancel))
}

// ── Panel accessor helpers ──────────────────────────────────────────

/// Helper to access the ProcessPanel. Panics if panel not registered (should never happen).
fn process_panel(app: &mut App) -> &mut crate::tui::components::process_panel::ProcessPanel {
    app.panels
        .downcast_mut::<crate::tui::components::process_panel::ProcessPanel>(crate::tui::panel::PanelId::Processes)
        .expect("process panel registered at startup")
}

/// Helper to access the PeersPanel. Panics if panel not registered (should never happen).
fn peers_panel(app: &mut App) -> &mut crate::tui::components::peers_panel::PeersPanel {
    app.panels
        .downcast_mut::<crate::tui::components::peers_panel::PeersPanel>(crate::tui::panel::PanelId::Peers)
        .expect("peers panel registered at startup")
}
