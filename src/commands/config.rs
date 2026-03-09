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
            println!("Global auth:     {}", ctx.paths.global_auth.display());
            println!("Global agents:   {}", ctx.paths.global_agents_dir.display());
            println!("Global sessions: {}", ctx.paths.global_sessions_dir.display());
            println!("Project root:    {}", ctx.project_paths.root.display());
            println!("Project config:  {}", ctx.project_paths.config_dir.display());
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
        _ => {
            eprintln!("This config command is not yet implemented.");
            return Err(crate::error::Error::Config {
                message: "config command not yet implemented".to_string(),
            });
        }
    }
    Ok(())
}
