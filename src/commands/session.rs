use crate::cli::ExportFormat;
use crate::cli::SessionAction;
use crate::commands::CommandContext;
use crate::error::Result;
use crate::session::automerge_store;
use crate::session::export;
use crate::session::store;
use crate::util::fs;

pub fn run(ctx: &CommandContext, action: SessionAction) -> Result<()> {
    match action {
        SessionAction::List { limit, all } => handle_list(ctx, limit, all),
        SessionAction::Show { session_id, full } => handle_show(ctx, &session_id, full),
        SessionAction::Delete { session_id, .. } => handle_delete(ctx, &session_id),
        SessionAction::DeleteAll { force } => handle_delete_all(ctx, force),
        SessionAction::Export {
            session_id,
            output,
            format,
        } => handle_export(ctx, &session_id, output, format),
        SessionAction::Import { file } => handle_import(ctx, &file),
        SessionAction::Migrate { session_id, all } => handle_migrate(ctx, session_id.as_deref(), all),
    }
}

fn handle_list(ctx: &CommandContext, limit: usize, all: bool) -> Result<()> {
    let files = if all {
        store::list_all_sessions(&ctx.paths.global_sessions_dir)
    } else {
        store::list_sessions(&ctx.paths.global_sessions_dir, &ctx.cwd)
    };
    if files.is_empty() {
        println!("No sessions found.");
        return Ok(());
    }
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
    Ok(())
}

fn handle_show(ctx: &CommandContext, session_id: &str, full: bool) -> Result<()> {
    let path = find_session(ctx, session_id)?;
    if full {
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        println!("{}", content);
    } else {
        match export::export_text(&path) {
            Ok(text) => print!("{}", text),
            Err(e) => eprintln!("Failed to read session: {}", e),
        }
    }
    Ok(())
}

fn handle_delete(ctx: &CommandContext, session_id: &str) -> Result<()> {
    let path = find_session(ctx, session_id)?;
    std::fs::remove_file(&path).map_err(|e| crate::error::Error::SessionStore {
        message: format!("failed to delete session: {}", session_id),
        source: e,
    })?;
    println!("Session deleted.");
    Ok(())
}

fn handle_delete_all(ctx: &CommandContext, force: bool) -> Result<()> {
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
            return Err(crate::error::Error::SessionStore {
                message: "failed to purge sessions".to_string(),
                source: e,
            });
        }
    }
    Ok(())
}

fn handle_export(ctx: &CommandContext, session_id: &str, output: Option<String>, format: ExportFormat) -> Result<()> {
    let path = find_session(ctx, session_id)?;
    let content = match format {
        ExportFormat::Json => export::export_json(&path),
        ExportFormat::Markdown => export::export_markdown(&path),
        ExportFormat::Text => export::export_text(&path),
    }?;
    if let Some(ref out_path) = output {
        let out = std::path::Path::new(out_path);
        let resolved = if out.parent().is_none_or(|p| p.as_os_str().is_empty()) {
            let cwd_path = std::path::Path::new(&ctx.cwd);
            let exports_dir = cwd_path.join(".clankers").join("exports");
            std::fs::create_dir_all(&exports_dir).map_err(|e| crate::error::Error::Io { source: e })?;
            fs::ensure_gitignore_entry(cwd_path, ".clankers/exports");
            exports_dir.join(out)
        } else {
            out.to_path_buf()
        };
        std::fs::write(&resolved, &content).map_err(|e| crate::error::Error::Io { source: e })?;
        println!("Exported to {}", resolved.display());
    } else {
        print!("{}", content);
    }
    Ok(())
}

fn handle_import(ctx: &CommandContext, file: &str) -> Result<()> {
    let source = std::path::Path::new(file);
    if !source.is_file() {
        return Err(crate::error::Error::Session {
            message: format!("file not found: {}", file),
        });
    }
    let dest = store::import_session(&ctx.paths.global_sessions_dir, source)?;
    println!("Imported session to {}", dest.display());
    Ok(())
}

#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(no_unwrap, reason = "session_id is Some when not all mode, checked above")
)]
fn handle_migrate(ctx: &CommandContext, session_id: Option<&str>, all: bool) -> Result<()> {
    if !all && session_id.is_none() {
        return Err(crate::error::Error::Session {
            message: "specify a session ID or use --all".to_string(),
        });
    }

    let jsonl_files: Vec<std::path::PathBuf> = if all {
        store::list_all_sessions(&ctx.paths.global_sessions_dir)
            .into_iter()
            .filter(|p| p.extension().is_some_and(|ext| ext == "jsonl"))
            .collect()
    } else {
        let id = session_id.expect("checked above");
        let path = store::find_session_by_id(&ctx.paths.global_sessions_dir, &ctx.cwd, id).ok_or_else(|| {
            crate::error::Error::Session {
                message: format!("session not found: {}", id),
            }
        })?;
        if path.extension().is_some_and(|ext| ext == "automerge") {
            println!("Session {} is already in Automerge format.", id);
            return Ok(());
        }
        vec![path]
    };

    if jsonl_files.is_empty() {
        println!("No JSONL sessions to migrate.");
        return Ok(());
    }

    let mut migrated = 0usize;
    let mut skipped = 0usize;
    let mut failed = 0usize;

    for path in &jsonl_files {
        match automerge_store::migrate_jsonl_to_automerge(path) {
            Ok(automerge_store::MigrateResult::Migrated { message_count, .. }) => {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                println!("  ✓ {} ({} messages)", name, message_count);
                migrated += 1;
            }
            Ok(automerge_store::MigrateResult::Skipped) => {
                skipped += 1;
            }
            Err(e) => {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                eprintln!("  ✗ {}: {}", name, e);
                failed += 1;
            }
        }
    }

    println!("\nMigration complete: {} migrated, {} skipped, {} failed", migrated, skipped, failed);
    Ok(())
}

/// Find a session file by ID prefix, or return an error.
fn find_session(ctx: &CommandContext, session_id: &str) -> Result<std::path::PathBuf> {
    store::find_session_by_id(&ctx.paths.global_sessions_dir, &ctx.cwd, session_id).ok_or_else(|| {
        crate::error::Error::Session {
            message: format!("session not found: {}", session_id),
        }
    })
}
