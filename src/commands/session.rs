use crate::cli::{ExportFormat, SessionAction};
use crate::commands::CommandContext;
use crate::error::Result;
use crate::session::store;
use crate::util::fs;

pub fn run(ctx: &CommandContext, action: SessionAction) -> Result<()> {
    match action {
        SessionAction::List { limit, all } => {
            let files = if all {
                store::list_all_sessions(&ctx.paths.global_sessions_dir)
            } else {
                store::list_sessions(&ctx.paths.global_sessions_dir, &ctx.cwd)
            };
            if files.is_empty() {
                println!("No sessions found.");
            } else {
                for (i, path) in files.iter().take(limit).enumerate() {
                    if let Some(summary) = store::read_session_summary(path) {
                        let date = summary.created_at.format("%Y-%m-%d %H:%M");
                        let preview = summary.first_user_message.as_deref().unwrap_or("(empty)");
                        let cwd_info = if all {
                            format!(" [{}]", summary.cwd)
                        } else {
                            String::new()
                        };
                        println!(
                            "  {}. {} | {} | {} msgs | {}{}\n     {}",
                            i + 1,
                            &summary.session_id[..8.min(summary.session_id.len())],
                            date,
                            summary.message_count,
                            summary.model,
                            cwd_info,
                            preview,
                        );
                    } else {
                        println!("  {}. {}", i + 1, path.display());
                    }
                }
                if files.len() > limit {
                    println!("\n  ({} more sessions)", files.len() - limit);
                }
            }
        }
        SessionAction::Show { session_id, full } => {
            let found = store::find_session_by_id(
                &ctx.paths.global_sessions_dir,
                &ctx.cwd,
                &session_id,
            );
            if let Some(path) = found {
                if full {
                    // Dump raw JSONL
                    let content = std::fs::read_to_string(&path).unwrap_or_default();
                    println!("{}", content);
                } else {
                    // Human-readable text format
                    match store::export_text(&path) {
                        Ok(text) => print!("{}", text),
                        Err(e) => eprintln!("Failed to read session: {}", e),
                    }
                }
            } else {
                eprintln!("Session not found: {}", session_id);
                return Err(crate::error::Error::Session {
                    message: format!("session not found: {}", session_id),
                });
            }
        }
        SessionAction::Delete { session_id, .. } => {
            let found = store::find_session_by_id(
                &ctx.paths.global_sessions_dir,
                &ctx.cwd,
                &session_id,
            );
            if let Some(path) = found {
                std::fs::remove_file(&path).map_err(|e| crate::error::Error::SessionStore {
                    message: format!("failed to delete session: {}", session_id),
                    source: e,
                })?;
                println!("Session deleted.");
            } else {
                eprintln!("Session not found: {}", session_id);
                return Err(crate::error::Error::Session {
                    message: format!("session not found: {}", session_id),
                });
            }
        }
        SessionAction::DeleteAll { force } => {
            if !force {
                eprintln!("This will delete ALL sessions for the current directory.");
                eprintln!("Use --force to confirm.");
                return Err(crate::error::Error::Session {
                    message: "delete-all requires --force flag".to_string(),
                });
            }
            match store::purge_sessions(&ctx.paths.global_sessions_dir, &ctx.cwd) {
                Ok(count) => println!("Deleted {} session(s).", count),
                Err(e) => {
                    eprintln!("Failed to purge sessions: {}", e);
                    return Err(crate::error::Error::SessionStore {
                        message: "failed to purge sessions".to_string(),
                        source: e,
                    });
                }
            }
        }
        SessionAction::Export {
            session_id,
            output,
            format,
        } => {
            let found = store::find_session_by_id(
                &ctx.paths.global_sessions_dir,
                &ctx.cwd,
                &session_id,
            );
            if let Some(path) = found {
                let result = match format {
                    ExportFormat::Json => store::export_json(&path),
                    ExportFormat::Markdown => store::export_markdown(&path),
                    ExportFormat::Text => store::export_text(&path),
                };
                let content = result?;
                if let Some(ref out_path) = output {
                    let out = std::path::Path::new(out_path);
                    // If the path is just a filename, place it in .clankers/exports/
                    let resolved = if out.parent().is_none_or(|p| p.as_os_str().is_empty()) {
                        let cwd_path = std::path::Path::new(&ctx.cwd);
                        let exports_dir = cwd_path.join(".clankers").join("exports");
                        std::fs::create_dir_all(&exports_dir)
                            .map_err(|e| crate::error::Error::Io { source: e })?;
                        fs::ensure_gitignore_entry(cwd_path, ".clankers/exports");
                        exports_dir.join(out)
                    } else {
                        out.to_path_buf()
                    };
                    std::fs::write(&resolved, &content)
                        .map_err(|e| crate::error::Error::Io { source: e })?;
                    println!("Exported to {}", resolved.display());
                } else {
                    print!("{}", content);
                }
            } else {
                eprintln!("Session not found: {}", session_id);
                return Err(crate::error::Error::Session {
                    message: format!("session not found: {}", session_id),
                });
            }
        }
        SessionAction::Import { file } => {
            let source = std::path::Path::new(&file);
            if !source.is_file() {
                eprintln!("File not found: {}", file);
                return Err(crate::error::Error::Session {
                    message: format!("file not found: {}", file),
                });
            }
            let dest = store::import_session(&ctx.paths.global_sessions_dir, source)?;
            println!("Imported session to {}", dest.display());
        }
    }
    Ok(())
}
