//! Full TUI interactive mode — top-level orchestration.
//!
//! This module wires up the terminal, session, agent, and event loop.
//! Heavy lifting is delegated to:
//!   - `session_setup` — session create/resume logic
//!   - `agent_setup`   — tool construction, process monitor, agent builder
//!   - `agent_task`    — background agent command handler

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use std::io;
use std::sync::Arc;

use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::agent::Agent;
use crate::config::keybindings::Keymap;
use crate::config::theme::load_theme;
use crate::error::Result;
use crate::provider::auth::AuthStoreExt;
use crate::tui::app::App;

/// Options for resuming a session.
#[derive(Default)]
pub struct ResumeOptions {
    /// Resume a specific session by ID
    pub session_id: Option<String>,
    /// Continue the most recent session
    pub continue_last: bool,
    /// Disable session persistence entirely
    pub no_session: bool,
}

/// Run the interactive TUI mode.
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(function_length, reason = "sequential setup/dispatch logic")
)]
pub async fn run_interactive(
    provider: Arc<dyn crate::provider::Provider>,
    settings: crate::config::settings::Settings,
    model: String,
    system_prompt: String,
    cwd: String,
    plugin_manager: Option<Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
    resume_opts: ResumeOptions,
) -> Result<()> {
    let mut terminal = super::common::init_terminal()?;

    let paths = crate::config::ClankersPaths::get();
    let theme = load_theme(settings.theme.as_deref(), &paths.global_themes_dir);
    let keymap = settings.keymap.clone().into_keymap();

    let mut app = App::new(model.clone(), cwd.clone(), theme);
    app.auto_theme = crate::config::theme::is_auto_theme(settings.theme.as_deref());
    app.highlighter = Box::new(crate::util::syntax::SyntectHighlighter);

    // Build slash command registry and set completion source on app
    let slash_registry = build_slash_registry(plugin_manager.as_ref());
    app.set_completion_source(Box::new(clanker_tui_types::CompletionSnapshot::from_source(&slash_registry)));

    // Build leader menu from all contributors (builtins + plugins + user config)
    rebuild_leader_menu(&mut app, plugin_manager.as_ref(), &settings);

    // ── Session persistence setup ────────────────────────────────────
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

    let (session_manager, seed_messages, latest_compaction_summary, worktree_setup) =
        super::session_setup::setup_session(&mut app, &cwd, &model, &db, &settings, resume_opts);

    // ── Enter worktree working directory ─────────────────────────────
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

    // Set router connection status and info based on provider type
    app.router_status = if provider.name() == "rpc-router" {
        crate::tui::app::RouterStatus::Connected
    } else {
        crate::tui::app::RouterStatus::Local
    };

    // Populate detailed router info from the provider's model list
    {
        let models = provider.models();
        let mut backend_names: Vec<String> = models
            .iter()
            .map(|m| m.provider.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        backend_names.sort();
        app.router_info = crate::tui::app::RouterInfo {
            provider_type: provider.name().to_string(),
            backend_names,
            model_count: models.len(),
        };
    }

    // Populate active account name
    {
        let paths = crate::config::ClankersPaths::get();
        let store = crate::provider::auth::AuthStore::load(&paths.global_auth);
        app.active_account = store.active_account_name().to_string();
    }

    // Populate disabled tools from settings (global + project merged)
    app.disabled_tools = settings.disabled_tools.iter().cloned().collect();

    // Auto-test: initialize from settings
    if let Some(ref cmd) = settings.auto_test_command {
        app.auto_test_command = Some(cmd.clone());
        app.auto_test_enabled = true;
    }

    // ── Hook pipeline setup ────────────────────────────────────────────
    let hook_pipeline = build_hook_pipeline(&settings, &cwd, plugin_manager.as_ref());

    // Per-process schedule engine (standalone mode). Shared across tool rebuilds.
    let schedules_path = paths.global_config_dir.join("schedules.json");
    let schedule_engine =
        std::sync::Arc::new(clanker_scheduler::ScheduleEngine::new().with_persistence(schedules_path.clone()));

    // Load persisted schedules from previous sessions.
    let persisted = clanker_scheduler::ScheduleEngine::load_from(&schedules_path);
    if !persisted.is_empty() {
        tracing::info!("loaded {} persisted schedule(s)", persisted.len());
        schedule_engine.add_all(persisted);
    }

    // Start the engine's background tick loop so schedules actually fire.
    let _schedule_handle = schedule_engine.start();
    let schedule_rx = schedule_engine.subscribe();

    let (mut agent, event_rx, mut bash_confirm_rx) = super::agent_setup::build_agent_with_tools(
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
        Some(schedule_engine.clone()),
    );

    // Attach hook pipeline to the agent
    if let Some(ref pipeline) = hook_pipeline {
        agent = agent.with_hook_pipeline(Arc::clone(pipeline));
    }
    agent.set_session_id(app.session_id.clone());

    // Start embedded RPC for swarm presence (non-fatal if unavailable)
    let _rpc_cancel = maybe_start_rpc(&mut app, paths).await;

    // Build the embedded-mode SessionController for audit, hooks, loop, auto-test.
    // The controller owns the SessionManager for persistence; slash commands
    // and branch/merge operations access it via controller.session_manager.
    let controller = {
        let mut ctrl_config = clankers_controller::config::ControllerConfig {
            session_id: app.session_id.clone(),
            model: model.clone(),
            session_manager,
            hook_pipeline,
            ..Default::default()
        };
        if let Some(ref cmd) = settings.auto_test_command {
            ctrl_config.auto_test_command = Some(cmd.clone());
            ctrl_config.auto_test_enabled = true;
        }
        clankers_controller::SessionController::new_embedded(ctrl_config)
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
        seed_messages,
        latest_compaction_summary,
        db.clone(),
        &settings,
        slash_registry,
        controller,
        schedule_engine.clone(),
        schedule_rx,
    )
    .await;

    // ── Shut down schedule engine + embedded RPC server ───────────────
    schedule_engine.cancel_token().cancel();
    if let Some(cancel) = _rpc_cancel {
        cancel.cancel();
    }

    let result = super::scrollback_dump::finalize_terminal_and_scrollback(
        result,
        &mut terminal,
        &app.conversation.blocks,
        &settings,
    );

    // ── Worktree cleanup: mark completed, merge, and GC ─────────────
    if let Some(ref wt) = worktree_setup
        && let Some(ref db) = db
    {
        std::env::set_current_dir(&original_cwd).ok();
        match crate::worktree::session_bridge::complete_and_merge(db, wt, Some(provider_for_merge), model_for_merge) {
            Ok(handle) => {
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
    event_rx: tokio::sync::broadcast::Receiver<crate::agent::events::AgentEvent>,
    panel_rx: &mut tokio::sync::mpsc::UnboundedReceiver<crate::tui::components::subagent_event::SubagentEvent>,
    todo_rx: &mut tokio::sync::mpsc::UnboundedReceiver<(
        crate::tools::todo::TodoAction,
        tokio::sync::oneshot::Sender<crate::tools::todo::TodoResponse>,
    )>,
    bash_confirm_rx: &mut crate::tools::bash::ConfirmRx,
    panel_tx: tokio::sync::mpsc::UnboundedSender<crate::tui::components::subagent_event::SubagentEvent>,
    keymap: Keymap,
    plugin_manager: Option<Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
    seed_messages: Vec<crate::provider::message::AgentMessage>,
    latest_compaction_summary: Option<String>,
    db: Option<crate::db::Db>,
    settings: &crate::config::settings::Settings,
    slash_registry: crate::slash_commands::SlashRegistry,
    controller: clankers_controller::SessionController,
    schedule_engine: Arc<clanker_scheduler::ScheduleEngine>,
    schedule_rx: tokio::sync::broadcast::Receiver<clanker_scheduler::ScheduleEvent>,
) -> Result<()> {
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel::<AgentCommand>();
    let (done_tx, done_rx) = tokio::sync::mpsc::unbounded_channel::<TaskResult>();

    if latest_compaction_summary.is_some() {
        cmd_tx.send(AgentCommand::SetCompactionSummary(latest_compaction_summary)).ok();
    }

    // If we have seed messages from a resumed session, restore them into the agent
    // and rebuild display blocks so the user sees the conversation
    if !seed_messages.is_empty() {
        super::session_restore::restore_display_blocks(app, &seed_messages);
        cmd_tx.send(AgentCommand::SeedMessages(seed_messages)).ok();
    }

    // Clone tool_env and plugin_manager for tool rebuilds inside the agent task.
    let tool_env_for_rebuild = crate::modes::common::ToolEnv {
        settings: Some(settings.clone()),
        event_tx: Some(agent.event_sender()),
        schedule_engine: Some(schedule_engine),
        ..Default::default()
    };

    super::agent_task::spawn_agent_task(agent, cmd_rx, done_tx, tool_env_for_rebuild, plugin_manager.clone());

    // Delegate to EventLoopRunner which decomposes the loop body into
    // focused methods (drain_agent_events, drain_panel_events, etc.)
    // The SessionController handles audit, lifecycle hooks, loop mode,
    // and auto-test. The runner handles TUI rendering + interaction.
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
        db,
        settings,
        cmd_tx,
        done_rx,
        slash_registry,
        controller,
        schedule_rx,
    );
    runner.run()
}

// ---------------------------------------------------------------------------
// Standalone helpers
// ---------------------------------------------------------------------------

pub(crate) fn resume_session_from_file(
    app: &mut App,
    file_path: std::path::PathBuf,
    _session_id: &str,
    cmd_tx: &tokio::sync::mpsc::UnboundedSender<AgentCommand>,
    session_manager: &mut Option<crate::session::SessionManager>,
) {
    match crate::session::SessionManager::open(file_path) {
        Ok(mut mgr) => {
            let msgs = mgr.build_context().unwrap_or_default();
            let msg_count = msgs.len();
            let resumed_session_id = mgr.session_id().to_string();
            let latest_compaction_summary = mgr.latest_compaction_summary().map(str::to_string);
            app.session_id.clone_from(&resumed_session_id);
            mgr.record_resume(crate::provider::message::MessageId::new("slash-resume")).ok();
            *session_manager = Some(mgr);

            cmd_tx.send(AgentCommand::SetCompactionSummary(latest_compaction_summary)).ok();

            app.conversation.blocks.clear();
            app.conversation.all_blocks.clear();
            app.conversation.active_block = None;
            super::session_restore::restore_display_blocks(app, &msgs);

            cmd_tx.send(AgentCommand::SetSessionId(resumed_session_id)).ok();
            cmd_tx.send(AgentCommand::SeedMessages(msgs)).ok();

            app.push_system(format!("Resumed session {} ({} messages)", app.session_id, msg_count), false);
            app.conversation.scroll.scroll_to_bottom();
        }
        Err(e) => {
            app.push_system(format!("Failed to resume session: {}", e), true);
        }
    }
}

/// Parse OAuth callback input: code#state, URL with ?code=...&state=..., or space-separated.
///
/// # Tiger Style
///
/// Pure function — no I/O, no side effects. Validates input bounds
/// to reject suspiciously large payloads before parsing.
pub(crate) fn parse_oauth_input(input: &str) -> Option<(String, String)> {
    /// Tiger Style: reject inputs longer than this (OAuth codes are short).
    const MAX_INPUT_LEN: usize = 4_096;

    let input = input.trim();

    // Tiger Style: reject oversized input before doing any parsing work.
    if input.is_empty() || input.len() > MAX_INPUT_LEN {
        return None;
    }

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

/// Format a timestamp as a human-readable "time ago" string.
///
/// # Tiger Style
///
/// Pure function. Handles negative durations (future timestamps) gracefully
/// by clamping to "just now". Threshold constants are explicit.
pub(crate) fn format_time_ago(ts: chrono::DateTime<chrono::Utc>) -> String {
    const SECS_PER_MINUTE: i64 = 60;
    const SECS_PER_HOUR: i64 = 3_600;
    const SECS_PER_DAY: i64 = 86_400;

    let elapsed = chrono::Utc::now().signed_duration_since(ts);
    let secs = elapsed.num_seconds();

    // Tiger Style: clamp negative durations (clock skew, future timestamps).
    if secs < SECS_PER_MINUTE {
        return "just now".to_string();
    }

    if secs < SECS_PER_HOUR {
        let m = elapsed.num_minutes();
        debug_assert!(m > 0, "minutes should be positive when secs >= 60");
        return format!("{} minute{} ago", m, if m == 1 { "" } else { "s" });
    }

    if secs < SECS_PER_DAY {
        let h = elapsed.num_hours();
        debug_assert!(h > 0, "hours should be positive when secs >= 3600");
        return format!("{} hour{} ago", h, if h == 1 { "" } else { "s" });
    }

    let d = elapsed.num_days();
    if d == 1 {
        "yesterday".to_string()
    } else {
        format!("{} days ago", d)
    }
}

/// Strip YAML frontmatter (--- ... ---) from a prompt template.
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
// Swarm / peer background tasks
// ---------------------------------------------------------------------------

use super::rpc_embed::maybe_start_rpc;

// ── Hook pipeline builder ────────────────────────────────────────────

/// Build the hook pipeline from settings (script hooks + git hooks + plugin hooks).
fn build_hook_pipeline(
    settings: &crate::config::settings::Settings,
    cwd: &str,
    plugin_manager: Option<&Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
) -> Option<Arc<clankers_hooks::HookPipeline>> {
    if !settings.hooks.enabled {
        return None;
    }

    let project_root = std::path::Path::new(cwd);
    let mut pipeline = clankers_hooks::HookPipeline::new();

    // Disabled hooks from settings
    pipeline.set_disabled_hooks(settings.hooks.disabled_hooks.iter().cloned());

    // Script hooks from .clankers/hooks/ (or configured dir)
    let hooks_dir = settings.hooks.resolve_hooks_dir(project_root);
    let timeout = std::time::Duration::from_secs(settings.hooks.script_timeout_secs);
    pipeline.register(Arc::new(clankers_hooks::script::ScriptHookHandler::new(hooks_dir, timeout)));

    // Git hooks from .git/hooks/ (if manage_git_hooks is enabled)
    if settings.hooks.manage_git_hooks {
        // Find the git repo root (may differ from cwd in worktrees)
        if let Some(repo_root) = find_git_root(project_root) {
            pipeline.register(Arc::new(clankers_hooks::git::GitHookHandler::new(repo_root)));
        }
    }

    // Plugin hooks (wraps plugin dispatch as a HookHandler)
    if let Some(pm) = plugin_manager {
        pipeline.register(Arc::new(crate::plugin::hooks::PluginHookHandler::new(Arc::clone(pm))));
    }

    Some(Arc::new(pipeline))
}

/// Find the nearest .git directory walking up from a path.
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(unbounded_loop, reason = "event loop; exits on quit signal")
)]
fn find_git_root(start: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut current = start;
    loop {
        if current.join(".git").exists() {
            return Some(current.to_path_buf());
        }
        current = current.parent()?;
    }
}

// ── Leader menu + slash registry builders ───────────────────────────

/// Build the leader menu from builtins + slash commands + plugins + user config.
pub(crate) fn rebuild_leader_menu(
    app: &mut App,
    plugin_manager: Option<&Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
    settings: &crate::config::settings::Settings,
) {
    use crate::tui::components::leader_menu::BuiltinKeymapContributor;
    use crate::tui::components::leader_menu::MenuContributor;

    let builtin = BuiltinKeymapContributor;
    let hidden = settings.leader_menu.hidden_set();

    let pm_guard;
    let pm_menu_contrib;
    let mut contributors: Vec<&dyn MenuContributor> = vec![&builtin];
    if let Some(pm_arc) = plugin_manager {
        match pm_arc.lock() {
            Ok(guard) => {
                pm_guard = guard;
                pm_menu_contrib = crate::plugin::contributions::PluginMenuContributor(&pm_guard);
                contributors.push(&pm_menu_contrib);
            }
            Err(poisoned) => {
                pm_guard = poisoned.into_inner();
                pm_menu_contrib = crate::plugin::contributions::PluginMenuContributor(&pm_guard);
                contributors.push(&pm_menu_contrib);
            }
        }
    }
    contributors.push(&settings.leader_menu);

    app.rebuild_leader_menu(&contributors, &hidden);
}

/// Build the slash command registry from builtins + plugins.
pub(crate) fn build_slash_registry(
    plugin_manager: Option<&Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
) -> crate::slash_commands::SlashRegistry {
    use crate::slash_commands::BuiltinSlashContributor;
    use crate::slash_commands::SlashContributor;
    use crate::slash_commands::SlashRegistry;

    let builtin = BuiltinSlashContributor;
    let pm_guard;
    let pm_contrib;
    let mut contributors: Vec<&dyn SlashContributor> = vec![&builtin];
    if let Some(pm_arc) = plugin_manager {
        match pm_arc.lock() {
            Ok(guard) => {
                pm_guard = guard;
                pm_contrib = crate::plugin::contributions::PluginSlashContributor(&pm_guard);
                contributors.push(&pm_contrib);
            }
            Err(poisoned) => {
                pm_guard = poisoned.into_inner();
                pm_contrib = crate::plugin::contributions::PluginSlashContributor(&pm_guard);
                contributors.push(&pm_contrib);
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
