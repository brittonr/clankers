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
/// Set up the crossterm terminal (raw mode, alternate screen, mouse capture).
fn init_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
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
    Terminal::new(backend).map_err(|e| crate::error::Error::Tui {
        message: format!("Failed to create terminal: {}", e),
    })
}

/// Tear down the crossterm terminal (restore normal mode).
fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) {
    terminal::disable_raw_mode().ok();
    execute!(terminal.backend_mut(), DisableBracketedPaste, DisableMouseCapture, LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();
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
    let mut terminal = init_terminal()?;

    let theme = Theme::dark();
    let keymap = settings.keymap.clone().into_keymap();

    let mut app = App::new(model.clone(), cwd.clone(), theme);
    app.highlighter = Box::new(crate::util::syntax::SyntectHighlighter);

    // Build slash command registry and set completion source on app
    let slash_registry = build_slash_registry(plugin_manager.as_ref());
    app.set_completion_source(Box::new(clankers_tui_types::CompletionSnapshot::from_source(&slash_registry)));

    // Build leader menu from all contributors (builtins + plugins + user config)
    rebuild_leader_menu(&mut app, plugin_manager.as_ref(), &settings);

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

    // Populate disabled tools from settings (global + project merged)
    app.disabled_tools = settings.disabled_tools.iter().cloned().collect();

    let (agent, event_rx, mut bash_confirm_rx) = build_agent_with_tools(
        provider.clone(),
        &settings,
        model.clone(),
        system_prompt.clone(),
        &mut app,
        panel_tx.clone(),
        todo_tx.clone(),
        plugin_manager.as_ref(),
        paths,
        &db,
    );

    // Start embedded RPC for swarm presence (non-fatal if unavailable)
    let _rpc_cancel = maybe_start_rpc(&mut app, paths).await;

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
        slash_registry,
    )
    .await;

    // ── Shut down embedded RPC server ─────────────────────────────────
    if let Some(cancel) = _rpc_cancel {
        cancel.cancel();
    }

    restore_terminal(&mut terminal);

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

    // Create and start the process monitor (bridge ProcessEvent → AgentEvent)
    let process_monitor = {
        let config = crate::procmon::ProcessMonitorConfig::default();
        let (proc_tx, mut proc_rx) = tokio::sync::broadcast::channel::<crate::procmon::ProcessEvent>(256);
        let monitor = std::sync::Arc::new(crate::procmon::ProcessMonitor::new(config, Some(proc_tx)));
        monitor.clone().start();
        let agent_tx = event_tx.clone();
        tokio::spawn(async move {
            loop {
                match proc_rx.recv().await {
                    Ok(pe) => {
                        let _ = agent_tx.send(crate::agent::events::process_event_to_agent(pe));
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                }
            }
        });
        monitor
    };

    // Wire process monitor into the TUI panel
    *process_panel(app) = crate::tui::components::process_panel::ProcessPanel::new()
        .with_monitor(process_monitor.clone() as std::sync::Arc<dyn clankers_tui_types::ProcessDataSource>);

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
            crate::plugin::ui::apply_ui_action(&mut app.plugin_ui, action);
        }
    }

    // Filter out disabled tools before giving them to the agent.
    // tool_info keeps the full list so the toggle menu shows everything.
    let active_tools: Vec<std::sync::Arc<dyn crate::tools::Tool>> = tools
        .into_iter()
        .filter(|t| !app.disabled_tools.contains(&t.definition().name))
        .collect();

    // Build the final agent with tools, db, routing, and cost tracking
    let mut agent_builder = crate::agent::builder::AgentBuilder::new(provider, settings.clone(), model, system_prompt)
        .with_tools(active_tools)
        .with_paths(paths.clone());

    // Attach the global database so the agent can read memories and record usage
    if let Some(db) = db {
        agent_builder = agent_builder.with_db(db.clone());
    }

    // Build the agent (automatically wires routing and cost tracking from settings)
    let agent = agent_builder.build();

    // Extract cost tracker reference for the app UI
    if settings.cost_tracking.is_some() {
        app.cost_tracker =
            agent.cost_tracker().map(|ct| ct.clone() as std::sync::Arc<dyn clankers_tui_types::CostProvider>);
    }

    let event_rx = agent.subscribe();

    (agent, event_rx, bash_confirm_rx)
}

// ---------------------------------------------------------------------------
// Agent commands / results (re-exported from agent_commands module)
// ---------------------------------------------------------------------------

pub(crate) use super::agent_commands::AgentCommand;
pub(crate) use super::agent_commands::TaskResult;

// ---------------------------------------------------------------------------
// Main event loop
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments, clippy::unused_async)]
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
    slash_registry: crate::slash_commands::SlashRegistry,
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

    // Clone tool_env and plugin_manager for tool rebuilds inside the agent task.
    // The ToolEnv channels were set up during build_agent_with_tools — cloning
    // the Arc/sender handles is cheap and gives the spawn block access.
    let tool_env_for_rebuild = crate::modes::common::ToolEnv {
        event_tx: Some(agent.event_sender()),
        panel_tx: None,     // Rebuild doesn't need panel routing
        todo_tx: None,      // These stay wired from the original build
        bash_confirm_tx: None,
        process_monitor: None,
    };
    let plugin_manager_for_rebuild = plugin_manager.clone();

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
                    let result = clankers_router::oauth::exchange_code(&code, &state, &verifier).await;
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
                AgentCommand::SetDisabledTools(disabled) => {
                    // Rebuild tools, filtering out disabled ones
                    let all_tools = crate::modes::common::build_all_tools_with_env(
                        &tool_env_for_rebuild,
                        plugin_manager_for_rebuild.as_ref(),
                    );
                    let filtered: Vec<std::sync::Arc<dyn crate::tools::Tool>> = all_tools
                        .into_iter()
                        .filter(|t| !disabled.contains(&t.definition().name))
                        .collect();
                    agent = agent.with_tools(filtered);
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
        slash_registry,
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
// Swarm / peer background tasks (delegated to rpc_embed module)
// ---------------------------------------------------------------------------

use super::rpc_embed::maybe_start_rpc;

// ── Panel accessor helpers ──────────────────────────────────────────

/// Helper to access the ProcessPanel. Panics if panel not registered (should never happen).
fn process_panel(app: &mut App) -> &mut crate::tui::components::process_panel::ProcessPanel {
    app.panels
        .downcast_mut::<crate::tui::components::process_panel::ProcessPanel>(crate::tui::panel::PanelId::Processes)
        .expect("process panel registered at startup")
}

/// Build the leader menu from builtins + slash commands + plugins + user config.
fn rebuild_leader_menu(
    app: &mut App,
    plugin_manager: Option<&std::sync::Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
    settings: &crate::config::settings::Settings,
) {
    use crate::tui::components::leader_menu::BuiltinKeymapContributor;
    use crate::tui::components::leader_menu::MenuContributor;
    use crate::tui::components::leader_menu::SlashCommandContributor;

    let builtin = BuiltinKeymapContributor;
    let slash_cmds = clankers_tui_types::CompletionSource::slash_commands(&*app.completion_source);
    let slash_commands = SlashCommandContributor::new(slash_cmds);
    let hidden = settings.leader_menu.hidden_set();

    let pm_guard;
    let mut contributors: Vec<&dyn MenuContributor> = vec![&builtin, &slash_commands];
    if let Some(pm_arc) = plugin_manager {
        match pm_arc.lock() {
            Ok(guard) => {
                pm_guard = guard;
                contributors.push(&*pm_guard);
            }
            Err(poisoned) => {
                pm_guard = poisoned.into_inner();
                contributors.push(&*pm_guard);
            }
        }
    }
    contributors.push(&settings.leader_menu);

    app.rebuild_leader_menu(&contributors, &hidden);
}

/// Build the slash command registry from builtins + plugins.
fn build_slash_registry(
    plugin_manager: Option<&std::sync::Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
) -> crate::slash_commands::SlashRegistry {
    use crate::slash_commands::BuiltinSlashContributor;
    use crate::slash_commands::SlashContributor;
    use crate::slash_commands::SlashRegistry;

    let builtin = BuiltinSlashContributor;
    let pm_guard;
    let mut contributors: Vec<&dyn SlashContributor> = vec![&builtin];
    if let Some(pm_arc) = plugin_manager {
        match pm_arc.lock() {
            Ok(guard) => {
                pm_guard = guard;
                contributors.push(&*pm_guard);
            }
            Err(poisoned) => {
                pm_guard = poisoned.into_inner();
                contributors.push(&*pm_guard);
            }
        }
    }
    let (registry, conflicts) = SlashRegistry::build(&contributors);
    for c in &conflicts {
        tracing::debug!(
            registry = c.registry,
            key = %c.key,
            winner = %c.winner,
            loser = %c.loser,
            "slash command conflict"
        );
    }
    registry
}
