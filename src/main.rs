//! Clankers entry point.
//!
//! This file is a thin dispatcher: parse CLI args, set up logging,
//! resolve paths/settings, then delegate to the appropriate command
//! handler in [`clankers::commands`].

use clankers::cli::AgentScopeArg;
use clankers::cli::Cli;
use clankers::cli::Commands;
use clankers::cli::OutputMode;
use clankers::commands::CommandContext;
use clankers::error::ConfigSnafu;
use clankers::error::Result;
use clap::Parser;
use tracing::info;

#[snafu::report]
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // ── Logging ────────────────────────────────────────────────────
    let is_tui = matches!(cli.mode, OutputMode::Interactive) && cli.print.is_none() && !cli.stdin;
    let has_log_file = cli.log_file.is_some();
    let log_level = if let Some(ref level) = cli.log_level {
        level.parse().unwrap_or(tracing::Level::INFO)
    } else if cli.verbose {
        tracing::Level::DEBUG
    } else if has_log_file {
        tracing::Level::INFO
    } else if is_tui {
        tracing::Level::ERROR
    } else {
        tracing::Level::WARN
    };

    let env_filter = clankers::util::logging::build_env_filter(log_level);
    let subscriber = tracing_subscriber::fmt().with_env_filter(env_filter);

    if let Some(ref log_file) = cli.log_file {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file)
            .expect("failed to open log file");
        subscriber.with_writer(file).init();
    } else {
        subscriber.with_writer(std::io::stderr).init();
    }

    info!("starting clankers");
    info!(?cli, "parsed CLI arguments");

    // Clean up stale tool output temp files (older than 24h)
    clankers_loop::cleanup_temp_files(std::time::Duration::from_secs(24 * 3600));

    // ── CLI validation ─────────────────────────────────────────────
    validate_cli(&cli)?;

    // ── Environment variables ──────────────────────────────────────
    if !cli.env.is_empty() {
        for env_var in &cli.env {
            if let Some((key, value)) = env_var.split_once('=') {
                // SAFETY: Setting env vars early in main before threads spawn.
                unsafe {
                    std::env::set_var(key, value);
                }
                info!("set environment variable: {}={}", key, value);
            } else {
                return ConfigSnafu {
                    message: format!("invalid environment variable format: {}", env_var),
                }
                .fail();
            }
        }
    }

    // ── Resolve paths and settings ─────────────────────────────────
    let cwd = cli
        .cwd
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default().to_string_lossy().to_string());
    clankers::util::direnv::load_direnv_if_needed(std::path::Path::new(&cwd));

    let paths = clankers::config::ClankersPaths::resolve();
    let project_paths = clankers::config::ProjectPaths::resolve(std::path::Path::new(&cwd));
    let settings = clankers::config::Settings::load_with_pi_fallback(
        paths.pi_settings.as_deref(),
        &paths.global_settings,
        &project_paths.settings,
    );

    let model = cli.model.clone().unwrap_or(settings.model.clone());

    // Detect system capabilities for conditional prompt sections
    let nix_available = clankers::agent::system_prompt::detect_nix();
    let has_multi_model = settings.model_roles.is_configured();

    // Build system prompt from multiple sources
    let base_prompt = cli
        .system_prompt
        .clone()
        .or_else(|| cli.system_prompt_file.as_ref().and_then(|f| std::fs::read_to_string(f).ok()))
        .unwrap_or_else(|| {
            use clankers::agent::system_prompt::PromptFeatures;
            use clankers::agent::system_prompt::default_system_prompt;
            // In the main entrypoint we don't know the mode yet (dispatch
            // happens later), so we include interactive-appropriate sections.
            // Daemon mode builds its own system prompt via DaemonConfig.
            default_system_prompt(&PromptFeatures {
                nix_available,
                multi_model: has_multi_model,
                daemon_mode: false,
                process_monitor: true,
            })
        });

    let resources = clankers::agent::system_prompt::discover_resources(&paths, &project_paths);
    let system_prompt = clankers::agent::system_prompt::assemble_system_prompt(
        &base_prompt,
        &resources,
        cli.system_prompt_prefix.as_deref().or(settings.system_prompt_prefix.as_deref()),
        cli.system_prompt_suffix.as_deref().or(settings.system_prompt_suffix.as_deref()),
    );

    let ctx = CommandContext {
        paths,
        project_paths,
        settings,
        model,
        system_prompt,
        cwd,
        api_key: cli.api_key.clone(),
        api_base: cli.api_base.clone(),
        account: cli.account.clone(),
    };

    // ── Dispatch ───────────────────────────────────────────────────
    Box::pin(dispatch(cli, ctx, resources)).await
}

/// Validate mutually exclusive CLI flags.
fn validate_cli(cli: &Cli) -> Result<()> {
    if cli.print.is_some() && cli.r#continue {
        return ConfigSnafu {
            message: "cannot use --print with --continue",
        }
        .fail();
    }
    if cli.resume.is_some() && cli.r#continue {
        return ConfigSnafu {
            message: "cannot use --resume with --continue",
        }
        .fail();
    }
    if cli.stream && cli.no_stream {
        return ConfigSnafu {
            message: "cannot use --stream with --no-stream",
        }
        .fail();
    }
    if (cli.zellij || cli.swarm) && cli.no_zellij {
        return ConfigSnafu {
            message: "cannot use --zellij/--swarm with --no-zellij",
        }
        .fail();
    }
    if cli.dry_run && cli.auto_approve {
        return ConfigSnafu {
            message: "cannot use --dry-run with --auto-approve",
        }
        .fail();
    }
    Ok(())
}

/// Route to the appropriate command handler.
async fn dispatch(
    cli: Cli,
    ctx: CommandContext,
    resources: clankers::agent::system_prompt::PromptResources,
) -> Result<()> {
    match cli.command {
        Some(Commands::Version { verbose }) => {
            print!("clankers {}", env!("CARGO_PKG_VERSION"));
            if verbose {
                println!(" ({})", option_env!("CARGO_PKG_DESCRIPTION").unwrap_or("Rust terminal coding agent"));
            } else {
                println!();
            }
        }
        Some(Commands::Auth { action }) => {
            clankers::commands::auth::run(&ctx, action).await?;
        }
        Some(Commands::Config { action }) => {
            clankers::commands::config::run(&ctx, action)?;
        }
        Some(Commands::Session { action }) => {
            clankers::commands::session::run(&ctx, action)?;
        }
        #[cfg(feature = "zellij-share")]
        Some(Commands::Share { read_only }) => {
            clankers::commands::share::run_share(&ctx, read_only).await?;
        }
        #[cfg(feature = "zellij-share")]
        Some(Commands::Join { node_id, psk }) => {
            clankers::commands::share::run_join(&node_id, &psk).await?;
        }
        Some(Commands::Rpc { identity, action }) => {
            clankers::commands::rpc::run(&ctx, identity, action).await?;
        }
        Some(Commands::Token { action }) => {
            clankers::commands::token::run(&ctx, action)?;
        }
        Some(Commands::Daemon { action }) => {
            clankers::commands::daemon::dispatch(&ctx, action).await?;
        }
        Some(Commands::MergeDaemon { interval, once }) => {
            clankers::commands::daemon::run_merge_daemon(&ctx, interval, once).await?;
        }
        Some(Commands::Plugin { action }) => {
            clankers::commands::plugin::run(&ctx, action)?;
        }
        Some(Commands::Attach { session_id, new, model, remote }) => {
            if let Some(ref remote_id) = remote {
                clankers::modes::attach::run_remote_attach(
                    remote_id,
                    session_id,
                    new,
                    model,
                    &ctx.settings,
                    &ctx.paths,
                ).await?;
            } else {
                clankers::modes::attach::run_attach(session_id, new, model, &ctx.settings).await?;
            }
        }
        Some(Commands::Ps { all }) => {
            clankers::commands::daemon::dispatch_sessions(all).await?;
        }
        Some(_) => {
            eprintln!("This command is not yet implemented.");
            std::process::exit(1);
        }
        None => {
            Box::pin(run_agent_mode(cli, ctx, resources)).await?;
        }
    }

    Ok(())
}

/// Default mode: print, json, or interactive agent.
async fn run_agent_mode(
    cli: Cli,
    ctx: CommandContext,
    resources: clankers::agent::system_prompt::PromptResources,
) -> Result<()> {
    let prompt = if let Some(ref p) = cli.print {
        Some(p.clone())
    } else if cli.stdin {
        let mut input = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut input).expect("failed to read stdin");
        Some(input)
    } else {
        None
    };

    // If --agent is specified, look up the agent definition and override model/system_prompt
    let (model, system_prompt) = resolve_agent_overrides(&cli, &ctx)?;

    // Initialize sandbox
    clankers::tools::sandbox::init_policy();

    // Initialize plugin manager
    let plugin_manager = clankers::modes::common::init_plugin_manager(
        &ctx.paths.global_plugins_dir,
        Some(&ctx.project_paths.plugins_dir),
        &[&ctx.project_paths.plugins_root_dir],
    );

    if let Some(prompt) = prompt {
        run_headless(&cli, &ctx, model, system_prompt, &prompt, &plugin_manager).await
    } else {
        run_interactive(&cli, &ctx, model, system_prompt, resources, &plugin_manager).await
    }
}

/// Resolve agent definition overrides for model and system prompt.
fn resolve_agent_overrides(cli: &Cli, ctx: &CommandContext) -> Result<(String, String)> {
    if let Some(ref agent_name) = cli.agent {
        let agent_scope = cli
            .agent_scope
            .as_ref()
            .map(|s| match s {
                AgentScopeArg::User => clankers::agent_defs::definition::AgentScope::User,
                AgentScopeArg::Project => clankers::agent_defs::definition::AgentScope::Project,
                AgentScopeArg::Both => clankers::agent_defs::definition::AgentScope::Both,
            })
            .unwrap_or_default();

        let registry = clankers::agent_defs::discovery::discover_agents(
            &ctx.paths.global_agents_dir,
            Some(&ctx.project_paths.agents_dir),
            &agent_scope,
        );

        if let Some(agent_def) = registry.get(agent_name) {
            let m = agent_def.model.clone().unwrap_or_else(|| ctx.model.clone());
            let sp = agent_def.system_prompt.clone();
            Ok((m, sp))
        } else {
            eprintln!("Agent '{}' not found. Available agents:", agent_name);
            for a in registry.list() {
                eprintln!("  - {}: {}", a.name, a.description);
            }
            std::process::exit(1);
        }
    } else {
        Ok((ctx.model.clone(), ctx.system_prompt.clone()))
    }
}

/// Run in headless mode (print/json/markdown).
async fn run_headless(
    cli: &Cli,
    ctx: &CommandContext,
    model: String,
    system_prompt: String,
    prompt: &str,
    plugin_manager: &std::sync::Arc<std::sync::Mutex<clankers::plugin::PluginManager>>,
) -> Result<()> {
    let provider = clankers::provider::discovery::build_router(
        ctx.api_key.as_deref(),
        ctx.api_base.clone(),
        &ctx.paths.global_auth,
        ctx.paths.pi_auth.as_deref(),
        ctx.account.as_deref(),
    )?;

    let headless_process_monitor = {
        let config = clankers::procmon::ProcessMonitorConfig::default();
        let monitor = std::sync::Arc::new(clankers::procmon::ProcessMonitor::new(config, None));
        monitor.clone().start();
        monitor
    };

    let tools = if cli.tools.as_deref() == Some("none") || cli.tools.as_deref() == Some("") {
        Vec::new()
    } else {
        use clankers::modes::common::ToolSet;
        use clankers::modes::common::ToolTier;
        use clankers::modes::common::build_all_tiered_tools;
        use clankers::modes::common::resolve_tool_tiers;
        let env = clankers::modes::common::ToolEnv {
            process_monitor: Some(headless_process_monitor),
            ..Default::default()
        };
        let tiered = build_all_tiered_tools(&env, Some(plugin_manager));

        if let Some(ref allowed) = cli.tools {
            // Check if the flag value is tier-based or tool-name-based
            if let Some(tiers) = resolve_tool_tiers(Some(allowed)) {
                let tool_set = ToolSet::new(tiered, tiers);
                tool_set.active_tools()
            } else {
                // Legacy: comma-separated tool names
                let flat: Vec<_> = tiered.into_iter().map(|(_, t)| t).collect();
                let allowed_set: std::collections::HashSet<&str> = allowed.split(',').map(|s| s.trim()).collect();
                flat.into_iter().filter(|t| allowed_set.contains(t.definition().name.as_str())).collect()
            }
        } else {
            // Default headless: Core + Specialty (no Orchestration, no Matrix)
            let tool_set = ToolSet::new(tiered, [ToolTier::Core, ToolTier::Specialty]);
            tool_set.active_tools()
        }
    };

    // Apply --attach
    let attach_context = clankers::modes::common::build_attach_context(&cli.attach);
    let full_prompt = if attach_context.is_empty() {
        prompt.to_string()
    } else {
        format!("{}{}", attach_context, prompt)
    };

    // Apply settings overrides
    let mut settings = ctx.settings.clone();
    if let Some(max_tokens) = cli.max_tokens {
        settings.max_tokens = max_tokens;
    }
    if cli.enable_routing && settings.routing.is_none() {
        settings.routing = Some(clankers::model_selection::config::RoutingPolicyConfig::default());
    }
    if let Some(max_cost) = cli.max_cost {
        settings.cost_tracking = Some(clankers::model_selection::cost_tracker::CostTrackerConfig {
            soft_limit: Some(max_cost * 0.8),
            hard_limit: Some(max_cost),
            warning_interval: Some(1.0),
        });
    }

    let thinking_config = if cli.thinking || cli.thinking_budget.is_some() {
        Some(clankers::provider::ThinkingConfig {
            enabled: true,
            budget_tokens: cli.thinking_budget.or(Some(10_000)),
        })
    } else {
        None
    };

    match cli.mode {
        OutputMode::Json => {
            let json_opts = clankers::modes::json::JsonOptions {
                output_file: cli.output.clone(),
                thinking: thinking_config,
            };
            clankers::modes::json::run_json_with_options(
                &full_prompt,
                provider,
                tools,
                settings,
                model,
                system_prompt,
                json_opts,
            )
            .await?;
        }
        OutputMode::Markdown => {
            let print_opts = clankers::modes::print::PrintOptions {
                output_file: cli.output.clone(),
                show_stats: cli.stats,
                show_tools: cli.verbose,
                format: clankers::modes::print::PrintFormat::Markdown,
                thinking: thinking_config,
            };
            clankers::modes::print::run_print_with_options(
                &full_prompt,
                provider,
                tools,
                settings,
                model,
                system_prompt,
                print_opts,
            )
            .await?;
        }
        _ => {
            let print_opts = clankers::modes::print::PrintOptions {
                output_file: cli.output.clone(),
                show_stats: cli.stats,
                show_tools: cli.verbose,
                format: clankers::modes::print::PrintFormat::Text,
                thinking: thinking_config,
            };
            clankers::modes::print::run_print_with_options(
                &full_prompt,
                provider,
                tools,
                settings,
                model,
                system_prompt,
                print_opts,
            )
            .await?;
        }
    }

    Ok(())
}

/// Run in interactive TUI mode.
async fn run_interactive(
    cli: &Cli,
    ctx: &CommandContext,
    model: String,
    system_prompt: String,
    resources: clankers::agent::system_prompt::PromptResources,
    plugin_manager: &std::sync::Arc<std::sync::Mutex<clankers::plugin::PluginManager>>,
) -> Result<()> {
    let provider = clankers::provider::discovery::build_router_with_rpc(
        ctx.api_key.as_deref(),
        ctx.api_base.clone(),
        &ctx.paths.global_auth,
        ctx.paths.pi_auth.as_deref(),
        ctx.account.as_deref(),
    )
    .await?;

    let mut settings = ctx.settings.clone();
    if cli.no_worktree {
        settings.use_worktrees = false;
    }
    if cli.enable_routing && settings.routing.is_none() {
        settings.routing = Some(clankers::model_selection::config::RoutingPolicyConfig::default());
    }
    if let Some(max_cost) = cli.max_cost {
        settings.cost_tracking = Some(clankers::model_selection::cost_tracker::CostTrackerConfig {
            soft_limit: Some(max_cost * 0.8),
            hard_limit: Some(max_cost),
            warning_interval: Some(1.0),
        });
    }

    let resume_opts = clankers::modes::interactive::ResumeOptions {
        session_id: cli.resume.clone(),
        continue_last: cli.r#continue,
        no_session: cli.no_session,
    };

    let template_names: Vec<(String, String)> =
        resources.prompts.iter().map(|p| (p.name.clone(), p.description.clone())).collect();
    clankers::slash_commands::register_prompt_templates(&template_names);

    clankers::modes::interactive::run_interactive(
        provider,
        settings,
        model,
        system_prompt,
        ctx.cwd.clone(),
        Some(plugin_manager.clone()),
        resume_opts,
    )
    .await?;

    Ok(())
}
