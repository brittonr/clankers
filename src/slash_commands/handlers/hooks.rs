//! Hooks slash command handler.

use super::SlashContext;
use super::SlashHandler;

pub struct HooksHandler;

impl SlashHandler for HooksHandler {
    fn command(&self) -> super::super::SlashCommand {
        super::super::SlashCommand {
            name: "hooks",
            description: "Manage lifecycle, git, and plugin hooks",
            help: "Manage the hook system.\n\nUsage:\n  /hooks              — show hook status\n  /hooks list          — list available hook points\n  /hooks install-git   — install git hook shims\n  /hooks uninstall-git — remove git hook shims",
            accepts_args: true,
            subcommands: vec![
                ("list", "List hook points and installed scripts"),
                ("install-git", "Install git hook shims (pre-commit, post-commit)"),
                ("uninstall-git", "Remove git hook shims"),
            ],
            leader_key: None,
        }
    }

    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        let args = args.trim();

        if args.is_empty() {
            show_hook_status(ctx);
            return;
        }

        match args {
            "list" => list_hook_points(ctx),
            "install-git" => install_git_hooks(ctx),
            "uninstall-git" => uninstall_git_hooks(ctx),
            other => {
                ctx.app.push_system(
                    format!("Unknown hooks subcommand: '{}'. Try /hooks list", other),
                    true,
                );
            }
        }
    }
}

fn show_hook_status(ctx: &mut SlashContext<'_>) {
    let hooks_config = load_hooks_config();

    let mut lines = Vec::new();
    lines.push("## Hook System Status".to_string());
    lines.push(String::new());
    lines.push(format!(
        "- **Enabled:** {}",
        if hooks_config.enabled { "yes" } else { "no" }
    ));

    let cwd = std::env::current_dir().unwrap_or_default();
    let hooks_dir = hooks_config.resolve_hooks_dir(&cwd);
    lines.push(format!("- **Scripts dir:** `{}`", hooks_dir.display()));

    // Count installed scripts
    let script_count = clankers_hooks::HookPoint::all()
        .iter()
        .filter(|pt| {
            let path = hooks_dir.join(pt.to_filename());
            path.is_file()
        })
        .count();
    lines.push(format!("- **Installed scripts:** {script_count}"));
    lines.push(format!(
        "- **Script timeout:** {}s",
        hooks_config.script_timeout_secs
    ));
    lines.push(format!(
        "- **Git hooks managed:** {}",
        if hooks_config.manage_git_hooks {
            "yes"
        } else {
            "no"
        }
    ));

    if !hooks_config.disabled_hooks.is_empty() {
        lines.push(format!(
            "- **Disabled hooks:** {}",
            hooks_config.disabled_hooks.join(", ")
        ));
    }

    ctx.app.push_system(lines.join("\n"), false);
}

fn list_hook_points(ctx: &mut SlashContext<'_>) {
    let cwd = std::env::current_dir().unwrap_or_default();
    let hooks_config = load_hooks_config();
    let hooks_dir = hooks_config.resolve_hooks_dir(&cwd);

    let mut lines = Vec::new();
    lines.push("## Hook Points".to_string());
    lines.push(String::new());
    lines.push("| Hook | Type | Script |".to_string());
    lines.push("|------|------|--------|".to_string());

    for point in clankers_hooks::HookPoint::all() {
        let kind = if point.is_pre_hook() {
            "pre (can deny)"
        } else {
            "post (notify)"
        };
        let script_path = hooks_dir.join(point.to_filename());
        let installed = if script_path.is_file() {
            "✅ installed"
        } else {
            "—"
        };
        lines.push(format!("| `{}` | {} | {} |", point.to_filename(), kind, installed));
    }

    lines.push(String::new());
    lines.push(format!(
        "Scripts go in: `{}`",
        hooks_dir.display()
    ));

    ctx.app.push_system(lines.join("\n"), false);
}

fn install_git_hooks(ctx: &mut SlashContext<'_>) {
    let cwd = std::env::current_dir().unwrap_or_default();

    // Find git repo root
    let Some(repo_root) = find_git_root(&cwd) else {
        ctx.app
            .push_system("Not in a git repository.".to_string(), true);
        return;
    };

    let hooks = ["pre-commit", "post-commit"];
    let mut installed = Vec::new();
    let mut errors = Vec::new();

    for hook_name in &hooks {
        match clankers_hooks::git::install_hook_shim(&repo_root, hook_name) {
            Ok(()) => installed.push(*hook_name),
            Err(e) => errors.push(format!("{}: {}", hook_name, e)),
        }
    }

    if !installed.is_empty() {
        ctx.app.push_system(
            format!("Installed git hook shims: {}", installed.join(", ")),
            false,
        );
    }
    if !errors.is_empty() {
        ctx.app.push_system(
            format!("Errors: {}", errors.join("; ")),
            true,
        );
    }
}

fn uninstall_git_hooks(ctx: &mut SlashContext<'_>) {
    let cwd = std::env::current_dir().unwrap_or_default();

    let Some(repo_root) = find_git_root(&cwd) else {
        ctx.app
            .push_system("Not in a git repository.".to_string(), true);
        return;
    };

    let hooks = ["pre-commit", "post-commit"];
    let mut removed = Vec::new();
    let mut errors = Vec::new();

    for hook_name in &hooks {
        match clankers_hooks::git::uninstall_hook_shim(&repo_root, hook_name) {
            Ok(()) => removed.push(*hook_name),
            Err(e) => errors.push(format!("{}: {}", hook_name, e)),
        }
    }

    if !removed.is_empty() {
        ctx.app.push_system(
            format!("Removed git hook shims: {}", removed.join(", ")),
            false,
        );
    }
    if !errors.is_empty() {
        ctx.app.push_system(
            format!("Errors: {}", errors.join("; ")),
            true,
        );
    }
}

/// Load hooks config by reading the merged settings.
fn load_hooks_config() -> clankers_hooks::HooksConfig {
    let paths = crate::config::ClankersPaths::get();
    let project_paths = crate::config::ProjectPaths::resolve(&std::env::current_dir().unwrap_or_default());
    let settings = crate::config::settings::Settings::load(&paths.global_settings, &project_paths.settings);
    settings.hooks
}

fn find_git_root(start: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut current = start;
    loop {
        if current.join(".git").exists() {
            return Some(current.to_path_buf());
        }
        current = current.parent()?;
    }
}
