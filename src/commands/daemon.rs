//! Daemon command handlers for running persistent background processes.

use crate::commands::CommandContext;
use crate::error::Result;

/// Run the main daemon (iroh + optional Matrix RPC listener).
pub async fn run_daemon(
    ctx: &CommandContext,
    tags: Vec<String>,
    allow_all: bool,
    matrix: bool,
    heartbeat: u64,
    max_sessions: usize,
) -> Result<()> {
    let provider = crate::provider::discovery::build_router(
        ctx.api_key.as_deref(),
        ctx.api_base.clone(),
        &ctx.paths.global_auth,
        ctx.paths.pi_auth.as_deref(),
        ctx.account.as_deref(),
    )?;

    let process_monitor = {
        let config = crate::procmon::ProcessMonitorConfig::default();
        let monitor = std::sync::Arc::new(crate::procmon::ProcessMonitor::new(config, None));
        monitor.clone().start();
        monitor
    };
    let env = crate::modes::common::ToolEnv {
        process_monitor: Some(process_monitor),
        ..Default::default()
    };
    // Daemon mode: all tiers active (needs matrix, orchestration, everything)
    let tiered = crate::modes::common::build_tiered_tools(&env);
    let tool_set = crate::modes::common::ToolSet::new(tiered, [
        crate::modes::common::ToolTier::Core,
        crate::modes::common::ToolTier::Orchestration,
        crate::modes::common::ToolTier::Specialty,
        crate::modes::common::ToolTier::Matrix,
    ]);
    let tools = tool_set.active_tools();

    let config = crate::modes::daemon::DaemonConfig {
        model: ctx.model.clone(),
        system_prompt: ctx.system_prompt.clone(),
        settings: ctx.settings.clone(),
        tags,
        allow_all,
        enable_matrix: matrix,
        heartbeat_secs: heartbeat,
        max_sessions,
        ..Default::default()
    };

    crate::modes::daemon::run_daemon(provider, tools, config, &ctx.paths).await?;
    Ok(())
}

/// Run the merge daemon (watches for completed workers and auto-merges).
pub async fn run_merge_daemon(ctx: &CommandContext, interval: u64, once: bool) -> Result<()> {
    let repo_root = std::path::PathBuf::from(&ctx.cwd);

    // Try to build a provider for LLM conflict resolution
    let provider = crate::provider::discovery::build_router(
        ctx.api_key.as_deref(),
        ctx.api_base.clone(),
        &ctx.paths.global_auth,
        ctx.paths.pi_auth.as_deref(),
        None,
    )
    .ok();

    let db_path = ctx.paths.global_config_dir.join("clankers.db");
    let db = crate::db::Db::open(&db_path).map_err(|e| crate::error::Error::Io {
        source: std::io::Error::other(format!("failed to open database: {}", e)),
    })?;
    crate::worktree::merge_daemon::run_polling(db, repo_root, interval, once, provider, ctx.model.clone()).await;
    Ok(())
}
