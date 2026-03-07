//! Plugin command handlers for managing WASM plugins.

use crate::cli::PluginAction;
use crate::commands::CommandContext;
use crate::error::Result;
use snafu::ResultExt;

/// Run the plugin subcommand.
pub fn run(ctx: &CommandContext, action: PluginAction) -> Result<()> {
    let plugin_manager = crate::modes::common::init_plugin_manager(
        &ctx.paths.global_plugins_dir,
        Some(&ctx.project_paths.plugins_dir),
        &[&ctx.project_paths.plugins_root_dir],
    );

    match action {
        PluginAction::List { verbose } => {
            let mgr = plugin_manager.lock().unwrap_or_else(|e| e.into_inner());
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
                            crate::plugin::PluginState::Loaded => "○",
                            crate::plugin::PluginState::Error(_) => "✗",
                            crate::plugin::PluginState::Disabled => "−",
                        };
                        println!("{} {} v{} — {}", state, p.name, p.version, p.manifest.description);
                    }
                }
            }
        }
        PluginAction::Show { name } => {
            let mgr = plugin_manager.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(p) = mgr.get(&name) {
                println!("Name:        {}", p.name);
                println!("Version:     {}", p.version);
                println!("State:       {:?}", p.state);
                println!("Description: {}", p.manifest.description);
                println!("Path:        {}", p.path.display());
                println!("WASM:        {}", p.manifest.wasm.as_deref().unwrap_or("plugin.wasm"));
                println!("Kind:        {:?}", p.manifest.kind);
                println!(
                    "Tools:       {}",
                    if p.manifest.tools.is_empty() {
                        "(none)".to_string()
                    } else {
                        p.manifest.tools.join(", ")
                    }
                );
                println!(
                    "Commands:    {}",
                    if p.manifest.commands.is_empty() {
                        "(none)".to_string()
                    } else {
                        p.manifest.commands.join(", ")
                    }
                );
                println!(
                    "Events:      {}",
                    if p.manifest.events.is_empty() {
                        "(none)".to_string()
                    } else {
                        p.manifest.events.join(", ")
                    }
                );
                println!(
                    "Permissions: {}",
                    if p.manifest.permissions.is_empty() {
                        "(none)".to_string()
                    } else {
                        p.manifest.permissions.join(", ")
                    }
                );
                if !p.manifest.tool_definitions.is_empty() {
                    println!("\nTool definitions:");
                    for td in &p.manifest.tool_definitions {
                        println!("  {} — {}", td.name, td.description);
                        println!("    Handler: {}", td.handler);
                        println!(
                            "    Schema:  {}",
                            serde_json::to_string(&td.input_schema).unwrap_or_default()
                        );
                    }
                }
            } else {
                return Err(crate::error::Error::Config {
                    message: format!("Plugin '{}' not found.", name),
                });
            }
        }
        PluginAction::Install { source, project } => {
            let source_path = std::path::Path::new(&source);
            let manifest_path = source_path.join("plugin.json");
            if !manifest_path.is_file() {
                return Err(crate::error::Error::Config {
                    message: format!("No plugin.json found at: {}", manifest_path.display()),
                });
            }
            let manifest = crate::plugin::manifest::PluginManifest::load(&manifest_path).ok_or_else(|| {
                crate::error::Error::Config {
                    message: format!("Failed to parse plugin.json at: {}", manifest_path.display()),
                }
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
            // Copy plugin directory
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
        }
        PluginAction::Uninstall { name, project } => {
            let dest_dir = if project {
                ctx.project_paths.plugins_dir.join(&name)
            } else {
                ctx.paths.global_plugins_dir.join(&name)
            };
            if !dest_dir.exists() {
                return Err(crate::error::Error::Config {
                    message: format!("Plugin '{}' not found at: {}", name, dest_dir.display()),
                });
            }
            std::fs::remove_dir_all(&dest_dir).context(crate::error::IoSnafu)?;
            println!("Uninstalled plugin '{}'.", name);
        }
    }

    Ok(())
}
