//! Plugin command handlers for managing WASM plugins.

use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use snafu::ResultExt;

use crate::cli::PluginAction;
use crate::commands::CommandContext;
use crate::error::Result;
use crate::tools::Tool;
use crate::tools::ToolContext;

/// Run the plugin subcommand.
pub async fn run(ctx: &CommandContext, action: PluginAction) -> Result<()> {
    let plugin_manager = crate::modes::common::init_plugin_manager(
        &ctx.paths.global_plugins_dir,
        Some(&ctx.project_paths.plugins_dir),
        &[&ctx.project_paths.plugins_root_dir],
    );

    let result = match action {
        PluginAction::List { verbose } => handle_list(ctx, &plugin_manager, verbose),
        PluginAction::Show { name } => handle_show(&plugin_manager, &name),
        PluginAction::Call { plugin, tool, args } => handle_call(&plugin_manager, &plugin, &tool, &args).await,
        PluginAction::Install { source, project } => handle_install(ctx, &source, project),
        PluginAction::Uninstall { name, project } => handle_uninstall(ctx, &name, project),
    };

    clankers_plugin::shutdown_plugin_runtime(&plugin_manager, "plugin command complete").await;
    result
}

async fn handle_call(
    pm: &Arc<std::sync::Mutex<clankers_plugin::PluginManager>>,
    plugin_name: &str,
    tool_name: &str,
    args: &str,
) -> Result<()> {
    let params = serde_json::from_str::<serde_json::Value>(args).context(crate::error::JsonSnafu)?;
    if let Some(output) = call_wasm_plugin_direct(pm, plugin_name, tool_name, &params)? {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "is_error": false,
                "content": [{ "type": "text", "text": output }]
            }))
            .context(crate::error::JsonSnafu)?
        );
        return Ok(());
    }
    let tool = wait_for_plugin_tool(pm, plugin_name, tool_name)?;
    let result = tool
        .execute(
            &ToolContext::new(
                format!("plugin-cli-{plugin_name}-{tool_name}"),
                tokio_util::sync::CancellationToken::new(),
                None,
            ),
            params,
        )
        .await;

    println!("{}", serde_json::to_string_pretty(&result).context(crate::error::JsonSnafu)?);
    if result.is_error {
        return Err(crate::error::Error::Tool {
            tool_name: tool_name.to_string(),
            message: "plugin tool returned an error".to_string(),
        });
    }
    Ok(())
}

fn call_wasm_plugin_direct(
    pm: &Arc<std::sync::Mutex<clankers_plugin::PluginManager>>,
    plugin_name: &str,
    tool_name: &str,
    params: &serde_json::Value,
) -> Result<Option<String>> {
    let manager = pm.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(plugin) = manager.get(plugin_name) else {
        return Ok(None);
    };
    if !plugin.manifest.kind.uses_wasm_runtime() {
        return Ok(None);
    }
    let handler = plugin
        .manifest
        .tool_definitions
        .iter()
        .find(|tool| tool.name == tool_name)
        .map(|tool| tool.handler.as_str())
        .or_else(|| plugin.manifest.tools.iter().find(|tool| tool.as_str() == tool_name).map(String::as_str))
        .ok_or_else(|| crate::error::Error::Plugin {
            plugin_name: plugin_name.to_string(),
            message: format!("tool '{tool_name}' is not declared by plugin"),
        })?;
    let input = serde_json::to_string(&serde_json::json!({
        "tool": tool_name,
        "args": params,
    }))
    .context(crate::error::JsonSnafu)?;
    let output = manager.call_plugin(plugin_name, handler, &input).map_err(|message| crate::error::Error::Plugin {
        plugin_name: plugin_name.to_string(),
        message,
    })?;
    Ok(Some(output))
}

fn wait_for_plugin_tool(
    pm: &Arc<std::sync::Mutex<clankers_plugin::PluginManager>>,
    plugin_name: &str,
    tool_name: &str,
) -> Result<Arc<dyn Tool>> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let mut tools = crate::modes::common::build_plugin_tools(&[], pm, None);
        if let Some(index) =
            tools.iter().position(|tool| tool.source() == plugin_name && tool.definition().name == tool_name)
        {
            return Ok(tools.swap_remove(index));
        }
        if Instant::now() >= deadline {
            let known = tools
                .iter()
                .map(|tool| format!("{}::{}", tool.source(), tool.definition().name))
                .collect::<Vec<_>>()
                .join(", ");
            let plugin_states = {
                let manager = pm.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                manager
                    .list()
                    .iter()
                    .map(|plugin| format!("{}={:?}", plugin.name, plugin.state))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            return Err(crate::error::Error::Plugin {
                plugin_name: plugin_name.to_string(),
                message: format!(
                    "tool '{tool_name}' did not become available; known plugin tools: {known}; plugin states: {plugin_states}"
                ),
            });
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn handle_list(
    ctx: &CommandContext,
    pm: &Arc<std::sync::Mutex<clankers_plugin::PluginManager>>,
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
                    clankers_plugin::PluginState::Active => "✓",
                    clankers_plugin::PluginState::Loaded | clankers_plugin::PluginState::Starting => "○",
                    clankers_plugin::PluginState::Backoff(_) => "↺",
                    clankers_plugin::PluginState::Error(_) => "✗",
                    clankers_plugin::PluginState::Disabled => "−",
                };
                println!("{} {} v{} — {}", state, p.name, p.version, p.manifest.description);
            }
        }
    }
    Ok(())
}

fn handle_show(pm: &Arc<std::sync::Mutex<clankers_plugin::PluginManager>>, name: &str) -> Result<()> {
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
        clankers_plugin::manifest::PluginManifest::load(&manifest_path).ok_or_else(|| crate::error::Error::Config {
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
