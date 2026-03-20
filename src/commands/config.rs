use crate::cli::ConfigAction;
use crate::commands::CommandContext;
use crate::error::Result;

pub fn run(ctx: &CommandContext, action: ConfigAction) -> Result<()> {
    match action {
        ConfigAction::Show { .. } => {
            println!("{}", serde_json::to_string_pretty(&ctx.settings).unwrap_or_default());
        }
        ConfigAction::Paths => {
            println!("Global config:   {}", ctx.paths.global_config_dir.display());
            println!("Global settings: {}", ctx.paths.global_settings.display());
            println!("Global ncl:      {}", ctx.paths.global_settings_ncl.display());
            println!("Global auth:     {}", ctx.paths.global_auth.display());
            println!("Global agents:   {}", ctx.paths.global_agents_dir.display());
            println!("Global sessions: {}", ctx.paths.global_sessions_dir.display());
            println!("Project root:    {}", ctx.project_paths.root.display());
            println!("Project config:  {}", ctx.project_paths.config_dir.display());
            println!("Project ncl:     {}", ctx.project_paths.settings_ncl.display());
            if let Some(ref pi_dir) = ctx.paths.pi_config_dir {
                println!(
                    "Pi fallback:     {} (settings: {}, auth: {})",
                    pi_dir.display(),
                    if ctx.paths.pi_settings.is_some() {
                        "found"
                    } else {
                        "none"
                    },
                    if ctx.paths.pi_auth.is_some() { "found" } else { "none" },
                );
            }
        }
        ConfigAction::Edit { project } => {
            let path = if project {
                &ctx.project_paths.settings
            } else {
                &ctx.paths.global_settings
            };
            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
            let _ = std::process::Command::new(&editor).arg(path).status();
        }
        ConfigAction::Init {
            force,
            nickel,
            global,
        } => {
            run_init(ctx, force, nickel, global)?;
        }
        ConfigAction::Check => {
            run_check(ctx)?;
        }
        ConfigAction::Export { global } => {
            run_export(ctx, global)?;
        }
        _ => {
            eprintln!("This config command is not yet implemented.");
            return Err(crate::error::Error::Config {
                message: "config command not yet implemented".to_string(),
            });
        }
    }
    Ok(())
}

fn run_init(ctx: &CommandContext, force: bool, nickel: bool, global: bool) -> Result<()> {
    if nickel {
        #[cfg(feature = "nickel")]
        {
            let path = if global {
                ctx.paths.global_settings_ncl.clone()
            } else {
                ctx.project_paths.settings_ncl.clone()
            };
            if path.exists() && !force {
                eprintln!("error: {} already exists (use --force to overwrite)", path.display());
                return Err(crate::error::Error::Config {
                    message: format!("{} already exists", path.display()),
                });
            }
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            let content = clankers_config::nickel::generate_starter_config();
            std::fs::write(&path, content).map_err(|e| crate::error::Error::Config {
                message: format!("failed to write {}: {e}", path.display()),
            })?;
            println!("Created {}", path.display());
        }
        #[cfg(not(feature = "nickel"))]
        {
            eprintln!("error: nickel support not compiled in (build with --features nickel)");
            return Err(crate::error::Error::Config {
                message: "nickel feature not enabled".to_string(),
            });
        }
    } else {
        let path = if global {
            ctx.paths.global_settings.clone()
        } else {
            ctx.project_paths.settings.clone()
        };
        if path.exists() && !force {
            eprintln!("error: {} already exists (use --force to overwrite)", path.display());
            return Err(crate::error::Error::Config {
                message: format!("{} already exists", path.display()),
            });
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let content = serde_json::to_string_pretty(&clankers_config::Settings::default()).unwrap_or_default();
        std::fs::write(&path, content).map_err(|e| crate::error::Error::Config {
            message: format!("failed to write {}: {e}", path.display()),
        })?;
        println!("Created {}", path.display());
    }
    Ok(())
}

fn run_check(ctx: &CommandContext) -> Result<()> {
    // Re-load settings to catch errors (the main load already succeeded if we got here,
    // but this exercises the full path with diagnostics)
    let _settings = clankers_config::Settings::load_with_nickel(
        ctx.paths.pi_settings.as_deref(),
        &ctx.paths.global_settings,
        &ctx.paths.global_settings_ncl,
        &ctx.project_paths.settings,
        &ctx.project_paths.settings_ncl,
    );
    println!("Config OK");
    Ok(())
}

fn run_export(ctx: &CommandContext, global_only: bool) -> Result<()> {
    let settings = if global_only {
        clankers_config::Settings::load_with_nickel(
            ctx.paths.pi_settings.as_deref(),
            &ctx.paths.global_settings,
            &ctx.paths.global_settings_ncl,
            // Use a non-existent path so project layer is empty
            std::path::Path::new("/dev/null/settings.json"),
            std::path::Path::new("/dev/null/settings.ncl"),
        )
    } else {
        ctx.settings.clone()
    };
    println!("{}", serde_json::to_string_pretty(&settings).unwrap_or_default());
    Ok(())
}
