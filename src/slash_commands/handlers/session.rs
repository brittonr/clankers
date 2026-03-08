//! Session slash command handlers.

use super::SlashContext;
use super::SlashHandler;

pub struct SessionHandler;

impl SlashHandler for SessionHandler {
    fn command(&self) -> super::super::SlashCommand {
        super::super::SlashCommand {
            name: "session",
            description: "Manage sessions",
            help: "Session management:\n  \
                   /session                — show current session info\n  \
                   /session list [n]       — list recent sessions (default: 10)\n  \
                   /session resume [id]    — resume a previous session (opens menu if no id)\n  \
                   /session delete <id>    — delete a session\n  \
                   /session purge          — delete all sessions for this directory",
            accepts_args: true,
            subcommands: vec![
                ("list [n]", "list recent sessions"),
                ("resume [id]", "resume a session (menu if no id)"),
                ("delete <id>", "delete a session"),
                ("purge", "delete all sessions for this directory"),
            ],
            leader_key: None,
        }
    }
    
    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        if args.is_empty() {
            handle_show_current(ctx);
        } else {
            let parts: Vec<&str> = args.splitn(2, char::is_whitespace).collect();
            let subcmd = parts[0].trim();
            let subcmd_args = parts.get(1).map(|s| s.trim()).unwrap_or("");

            match subcmd {
                "list" | "ls" => handle_list(ctx, subcmd_args),
                "resume" | "open" => handle_resume(ctx, subcmd_args),
                "delete" | "rm" => handle_delete(ctx, subcmd_args),
                "purge" => handle_purge(ctx),
                _ => {
                    ctx.app.push_system(
                        format!("Unknown subcommand '{}'. Available: list, resume, delete, purge", subcmd),
                        true,
                    );
                }
            }
        }
    }
}

fn handle_show_current(ctx: &mut SlashContext<'_>) {
    let info = if ctx.app.session_id.is_empty() {
        "No active session.".to_string()
    } else {
        format!(
            "Session ID: {}\nCWD: {}\nModel: {}\n\nUse /session list to see recent sessions.",
            ctx.app.session_id, ctx.app.cwd, ctx.app.model
        )
    };
    ctx.app.push_system(info, false);
}

fn handle_list(ctx: &mut SlashContext<'_>, args: &str) {
    let paths = crate::config::ClankersPaths::get();
    let limit: usize = if args.is_empty() {
        10
    } else {
        args.parse().unwrap_or(10)
    };
    let files = crate::session::store::list_sessions(&paths.global_sessions_dir, &ctx.app.cwd);
    
    if files.is_empty() {
        ctx.app.push_system("No sessions found for this directory.".to_string(), false);
        return;
    }

    let mut out = String::from("Recent sessions:\n\n");
    for (i, path) in files.iter().take(limit).enumerate() {
        let is_current_file = !ctx.app.session_id.is_empty()
            && path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(&ctx.app.session_id));
        let marker = if is_current_file { " ◀ current" } else { "" };

        if let Some(summary) = crate::session::store::read_session_summary(path) {
            let date = summary.created_at.format("%Y-%m-%d %H:%M");
            let preview = summary.first_user_message.as_deref().unwrap_or("(empty)");
            out.push_str(&format!(
                "  {}. [{}] {} ({} msgs, {}){}\n     {}\n\n",
                i + 1,
                &summary.session_id[..8.min(summary.session_id.len())],
                date,
                summary.message_count,
                summary.model,
                marker,
                preview,
            ));
        } else {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
            out.push_str(&format!("  {}. {}{}\n", i + 1, name, marker));
        }
    }
    
    if files.len() > limit {
        out.push_str(&format!("  ({} more sessions)\n", files.len() - limit));
    }
    out.push_str("\nUse /session resume to pick a session, or /session resume <id>.");
    ctx.app.push_system(out, false);
}

fn handle_resume(ctx: &mut SlashContext<'_>, args: &str) {
    let paths = crate::config::ClankersPaths::get();
    let files = crate::session::store::list_sessions(&paths.global_sessions_dir, &ctx.app.cwd);
    
    if files.is_empty() {
        ctx.app.push_system("No sessions found for this directory.".to_string(), true);
        return;
    }

    if args.is_empty() {
        // Open the session selector menu
        let items: Vec<crate::tui::components::session_selector::SessionItem> = files
            .iter()
            .map(|f| {
                if let Some(summary) = crate::session::store::read_session_summary(f) {
                    let date = summary
                        .created_at
                        .with_timezone(&chrono::Local)
                        .format("%Y-%m-%d %H:%M");
                    let preview = summary.first_user_message.as_deref().unwrap_or("(empty)");
                    let label = format!(
                        "[{}] {} — {} ({} msgs, {})",
                        &summary.session_id[..8.min(summary.session_id.len())],
                        date,
                        preview,
                        summary.message_count,
                        summary.model,
                    );
                    crate::tui::components::session_selector::SessionItem {
                        session_id: summary.session_id,
                        label,
                        file_path: f.clone(),
                    }
                } else {
                    let name = f.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                    crate::tui::components::session_selector::SessionItem {
                        session_id: name.to_string(),
                        label: name.to_string(),
                        file_path: f.clone(),
                    }
                }
            })
            .collect();
        ctx.app.overlays.session_selector.open(items);
    } else {
        // Direct resume by ID
        let found = files.into_iter().find(|f| {
            f.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.contains(args))
        });
        if let Some(file) = found {
            let session_id = file.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
            crate::modes::interactive::resume_session_from_file(ctx.app, file, &session_id, ctx.cmd_tx);
        } else {
            ctx.app.push_system(
                format!("No session matching '{}'. Use /session resume to browse.", args),
                true,
            );
        }
    }
}

fn handle_delete(ctx: &mut SlashContext<'_>, args: &str) {
    if args.is_empty() {
        ctx.app.push_system("Usage: /session delete <session-id>".to_string(), true);
        return;
    }

    let paths = crate::config::ClankersPaths::get();
    let found = crate::session::store::find_session_by_id(
        &paths.global_sessions_dir,
        &ctx.app.cwd,
        args,
    );
    
    if let Some(file) = found {
        match std::fs::remove_file(&file) {
            Ok(()) => {
                ctx.app.push_system(format!("Session deleted: {}", file.display()), false);
            }
            Err(e) => {
                ctx.app.push_system(format!("Failed to delete session: {}", e), true);
            }
        }
    } else {
        ctx.app.push_system(
            format!("No session matching '{}'. Use /session list.", args),
            true,
        );
    }
}

fn handle_purge(ctx: &mut SlashContext<'_>) {
    let paths = crate::config::ClankersPaths::get();
    match crate::session::store::purge_sessions(&paths.global_sessions_dir, &ctx.app.cwd) {
        Ok(count) => {
            ctx.app.push_system(format!("Deleted {} session(s) for this directory.", count), false);
        }
        Err(e) => {
            ctx.app.push_system(format!("Failed to purge sessions: {}", e), true);
        }
    }
}
