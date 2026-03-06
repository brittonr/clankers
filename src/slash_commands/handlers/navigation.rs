//! Navigation slash command handlers.

use super::SlashContext;
use super::SlashHandler;

pub struct CdHandler;

impl SlashHandler for CdHandler {
    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        if args.is_empty() {
            ctx.app.push_system(format!("Current directory: {}\n\nUsage: /cd <path>", ctx.app.cwd), false);
        } else {
            let new_path = if args.starts_with('/') {
                std::path::PathBuf::from(args)
            } else {
                std::path::PathBuf::from(&ctx.app.cwd).join(args)
            };
            match new_path.canonicalize() {
                Ok(canonical) if canonical.is_dir() => {
                    ctx.app.cwd = canonical.to_string_lossy().to_string();
                    ctx.app.git_status.set_cwd(&ctx.app.cwd);
                    ctx.app.push_system(format!("Changed directory to: {}", ctx.app.cwd), false);
                }
                Ok(_) => {
                    ctx.app.push_system(format!("Not a directory: {}", args), true);
                }
                Err(e) => {
                    ctx.app.push_system(format!("Invalid path '{}': {}", args, e), true);
                }
            }
        }
    }
}

pub struct ShellHandler;

impl SlashHandler for ShellHandler {
    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        if args.is_empty() {
            ctx.app.push_system("Usage: /shell <command>".to_string(), false);
        } else {
            match std::process::Command::new("sh").arg("-c").arg(args).current_dir(&ctx.app.cwd).output() {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let mut result = String::new();
                    if !stdout.is_empty() {
                        result.push_str(&stdout);
                    }
                    if !stderr.is_empty() {
                        if !result.is_empty() {
                            result.push('\n');
                        }
                        result.push_str(&stderr);
                    }
                    if result.is_empty() {
                        result = format!("(exit code: {})", output.status.code().unwrap_or(-1));
                    }
                    ctx.app.push_system(result, !output.status.success());
                }
                Err(e) => {
                    ctx.app.push_system(format!("Failed to run command: {}", e), true);
                }
            }
        }
    }
}
