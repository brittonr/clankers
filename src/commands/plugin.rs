//! Plugin command handlers for managing WASM plugins.

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use std::sync::Arc;

use snafu::ResultExt;

use crate::cli::PluginAction;
use crate::commands::CommandContext;
use crate::error::Result;

/// Run the plugin subcommand.
pub fn run(ctx: &CommandContext, action: PluginAction) -> Result<()> {
    let plugin_manager = crate::modes::common::init_plugin_manager(
        &ctx.paths.global_plugins_dir,
        Some(&ctx.project_paths.plugins_dir),
        &[&ctx.project_paths.plugins_root_dir],
    );

    match action {
        PluginAction::List { verbose } => handle_list(ctx, &plugin_manager, verbose),
        PluginAction::Show { name } => handle_show(&plugin_manager, &name),
        PluginAction::Install { source, project } => handle_install(ctx, &source, project),
        PluginAction::Uninstall { name, project } => handle_uninstall(ctx, &name, project),
    }
}

fn handle_list(
    ctx: &CommandContext,
    pm: &Arc<std::sync::Mutex<crate::plugin::PluginManager>>,
    verbose: bool,
) -> Result<()> {
    let mgr = pm.lock().unwrap_or_else(|e| e.into_inner());
    let plugins = mgr.list();
    if plugins.is_empty() {
        println!("No plugins found.");
        println!("\nPlugin directories:");
        println!("  Global:  {}", ctx.paths.global_plugins_dir.display());
        println!("  Project: {}", ctx.project_paths.plugins_dir.display());
    } else {
        for p in plugins {
            if verbose {
                println!(
                    "{} v{} [{:?}]\n  {}\n  Path: {}\n  Tools: {}\n  Commands: {}\n  Events: {}\n  Permissions: {}",
                    p.name,
                    p.version,
                    p.state,
                    p.manifest.description,
                    p.path.display(),
                    p.manifest.tools.join(", "),
                    p.manifest.commands.join(", "),
                    p.manifest.events.join(", "),
                    p.manifest.permissions.join(", "),
                );
            } else {
                let state = match &p.state {
                    crate::plugin::PluginState::Active => "✓",
                    crate::plugin::PluginState::Loaded | crate::plugin::PluginState::Starting => "○",
                    crate::plugin::PluginState::Backoff(_) => "↺",
                    crate::plugin::PluginState::Error(_) => "✗",
                    crate::plugin::PluginState::Disabled => "−",
                };
                println!("{} {} v{} — {}", state, p.name, p.version, p.manifest.description);
            }
        }
    }
    Ok(())
}

fn handle_show(pm: &Arc<std::sync::Mutex<crate::plugin::PluginManager>>, name: &str) -> Result<()> {
    let mgr = pm.lock().unwrap_or_else(|e| e.into_inner());
    let p = mgr.get(name).ok_or_else(|| crate::error::Error::Config {
        message: format!("Plugin '{}' not found.", name),
    })?;
    println!("Name:        {}", p.name);
    println!("Version:     {}", p.version);
    println!("State:       {:?}", p.state);
    println!("Description: {}", p.manifest.description);
    println!("Path:        {}", p.path.display());
    println!("WASM:        {}", p.manifest.wasm.as_deref().unwrap_or("plugin.wasm"));
    println!("Kind:        {:?}", p.manifest.kind);
    let join_or_none = |v: &[String]| {
        if v.is_empty() {
            "(none)".to_string()
        } else {
            v.join(", ")
        }
    };
    println!("Tools:       {}", join_or_none(&p.manifest.tools));
    println!("Commands:    {}", join_or_none(&p.manifest.commands));
    println!("Events:      {}", join_or_none(&p.manifest.events));
    println!("Permissions: {}", join_or_none(&p.manifest.permissions));
    if !p.manifest.tool_definitions.is_empty() {
        println!("\nTool definitions:");
        for td in &p.manifest.tool_definitions {
            println!("  {} — {}", td.name, td.description);
            println!("    Handler: {}", td.handler);
            println!("    Schema:  {}", serde_json::to_string(&td.input_schema).unwrap_or_default());
        }
    }
    Ok(())
}

fn handle_install(ctx: &CommandContext, source: &str, project: bool) -> Result<()> {
    let source_path = std::path::Path::new(source);
    let manifest_path = source_path.join("plugin.json");
    if !manifest_path.is_file() {
        return Err(crate::error::Error::Config {
            message: format!("No plugin.json found at: {}", manifest_path.display()),
        });
    }
    let manifest =
        crate::plugin::manifest::PluginManifest::load(&manifest_path).ok_or_else(|| crate::error::Error::Config {
            message: format!("Failed to parse plugin.json at: {}", manifest_path.display()),
        })?;
    manifest.validate().map_err(|error| crate::error::Error::Config {
        message: format!("Invalid plugin.json at {}: {}", manifest_path.display(), error),
    })?;
    let dest_dir = if project {
        ctx.project_paths.plugins_dir.join(&manifest.name)
    } else {
        ctx.paths.global_plugins_dir.join(&manifest.name)
    };
    if dest_dir.exists() {
        return Err(crate::error::Error::Config {
            message: format!(
                "Plugin '{}' already installed at: {}\nRemove it first with: clankers plugin uninstall {}",
                manifest.name,
                dest_dir.display(),
                manifest.name
            ),
        });
    }
    std::fs::create_dir_all(&dest_dir).context(crate::error::IoSnafu)?;
    let dir_entries = std::fs::read_dir(source_path).context(crate::error::IoSnafu)?;
    for entry in dir_entries.flatten() {
        let src = entry.path();
        if src.is_file() {
            let dest = dest_dir.join(entry.file_name());
            std::fs::copy(&src, &dest).context(crate::error::IoSnafu)?;
        }
    }
    let scope = if project { "project" } else { "global" };
    println!("Installed plugin '{}' v{} to {} plugins.", manifest.name, manifest.version, scope);
    println!("  Path: {}", dest_dir.display());
    Ok(())
}

fn handle_uninstall(ctx: &CommandContext, name: &str, project: bool) -> Result<()> {
    let dest_dir = if project {
        ctx.project_paths.plugins_dir.join(name)
    } else {
        ctx.paths.global_plugins_dir.join(name)
    };
    if !dest_dir.exists() {
        return Err(crate::error::Error::Config {
            message: format!("Plugin '{}' not found at: {}", name, dest_dir.display()),
        });
    }
    std::fs::remove_dir_all(&dest_dir).context(crate::error::IoSnafu)?;
    println!("Uninstalled plugin '{}'.", name);
    Ok(())
}
