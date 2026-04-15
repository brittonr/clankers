//! Auth slash command handlers.

use super::SlashContext;
use super::SlashHandler;
use crate::modes::interactive::AgentCommand;
use crate::provider::auth::AuthStoreExt;

pub struct LoginHandler;

impl SlashHandler for LoginHandler {
    fn command(&self) -> super::super::SlashCommand {
        super::super::SlashCommand {
            name: "login",
            description: "Authenticate with an OAuth provider",
            help: "Start the OAuth login flow.\n\n\
                   Usage:\n  \
                     /login                              — start Anthropic login\n  \
                     /login <provider>                   — start login for a specific provider\n  \
                     /login <code#state>                 — complete login with code from browser\n  \
                     /login <callback URL>               — complete login with the full callback URL\n  \
                     /login --account <name>             — login to a specific account\n  \
                     /login <provider> --account <name>  — combine provider + account\n\n\
                   See also: /account (list, switch, logout, status)",
            accepts_args: true,
            subcommands: vec![],
        }
    }

    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        let (provider_name, account_name, remaining_args) = parse_login_args(args);

        if remaining_args.is_empty() {
            handle_login_start(ctx, &provider_name, &account_name);
        } else if let Some(verifier) = ctx.app.login_verifiers.remove(&(provider_name.clone(), account_name.clone())) {
            handle_login_complete(ctx, &remaining_args, verifier, &provider_name, &account_name);
        } else {
            handle_login_complete_from_disk(ctx, &remaining_args, &provider_name, &account_name);
        }
    }
}

fn parse_login_args(args: &str) -> (String, String, String) {
    let (account_name, remaining_args) = crate::modes::interactive::parse_account_flag(args);
    let account_name = account_name.unwrap_or_else(|| "default".to_string());
    let trimmed = remaining_args.trim();

    if trimmed.is_empty() {
        return (crate::provider::auth::DEFAULT_OAUTH_PROVIDER.to_string(), account_name, String::new());
    }

    if let Some(split_at) = trimmed.find(char::is_whitespace) {
        let first = &trimmed[..split_at];
        let rest = trimmed[split_at..].trim_start();
        if matches!(first, "anthropic" | "openai-codex") {
            return (first.to_string(), account_name, rest.to_string());
        }
    } else if matches!(trimmed, "anthropic" | "openai-codex") {
        return (trimmed.to_string(), account_name, String::new());
    }

    (crate::provider::auth::DEFAULT_OAUTH_PROVIDER.to_string(), account_name, trimmed.to_string())
}

fn handle_login_start(ctx: &mut SlashContext<'_>, provider_name: &str, account_name: &str) {
    let oauth_flow = match crate::provider::auth::OAuthFlow::from_provider(Some(provider_name)) {
        Ok(flow) => flow,
        Err(e) => {
            ctx.app.push_system(e.to_string(), true);
            return;
        }
    };

    let (url, verifier) = match oauth_flow.build_auth_url() {
        Ok(flow) => flow,
        Err(e) => {
            ctx.app.push_system(e.to_string(), true);
            return;
        }
    };

    let pending = crate::provider::auth::PendingOAuthLogin::new(provider_name, account_name, verifier);
    ctx.app
        .login_verifiers
        .insert((pending.provider.clone(), pending.account.clone()), pending.verifier.clone());

    let paths = crate::config::ClankersPaths::get();
    let verifier_path = crate::provider::auth::pending_oauth_login_path(
        &paths.global_config_dir,
        &pending.provider,
        &pending.account,
    );
    if let Err(e) = pending.save(&verifier_path) {
        ctx.app.push_system(format!("Failed to persist login verifier: {e}"), true);
        return;
    }

    // Try to auto-open the browser (detached so it doesn't block the TUI)
    let was_browser_opened = open::that_detached(&url).is_ok();

    let browser_msg = if was_browser_opened {
        "Opening browser automatically..."
    } else {
        "Could not open browser automatically."
    };

    ctx.app.push_system(
        format!(
            "Logging in to provider '{}' as account '{}'.\n\n\
             {}\n\n\
             Open this URL in your browser to authenticate:\n\n  {}\n\n\
             After authorizing, paste the code with:\n  /login <code#state>\n  /login <callback URL>\n\n\
             Or from another terminal:\n  clankers auth login --provider {} --code <code#state>",
            provider_name, account_name, browser_msg, url, provider_name
        ),
        false,
    );
}

fn handle_login_complete(
    ctx: &mut SlashContext<'_>,
    input: &str,
    verifier: String,
    provider: &str,
    account: &str,
) {
    let parsed = crate::modes::interactive::parse_oauth_input(input);
    match parsed {
        Some((code, state)) => {
            ctx.app.push_system(
                format!("Exchanging code for provider '{}' account '{}'...", provider, account),
                false,
            );
            ctx.cmd_tx.send(AgentCommand::Login {
                code,
                state,
                verifier,
                provider: provider.to_string(),
                account: account.to_string(),
            }).ok();
        }
        None => {
            ctx.app
                .login_verifiers
                .insert((provider.to_string(), account.to_string()), verifier);
            ctx.app.push_system(
                "Invalid code format. Expected:\n  /login code#state\n  /login https://...?code=CODE&state=STATE"
                    .to_string(),
                true,
            );
        }
    }
}

fn handle_login_complete_from_disk(ctx: &mut SlashContext<'_>, input: &str, provider: &str, account: &str) {
    // No in-memory verifier — try recovering from disk (e.g. login started in another clankers
    // instance)
    let paths = crate::config::ClankersPaths::get();
    let verifier_path = crate::provider::auth::pending_oauth_login_path(&paths.global_config_dir, provider, account);
    let legacy_path = crate::provider::auth::legacy_pending_oauth_login_path(&paths.global_config_dir);

    if let Some(pending) = crate::provider::auth::PendingOAuthLogin::load(&verifier_path)
        .or_else(|| crate::provider::auth::PendingOAuthLogin::load(&legacy_path))
    {
        if let Some((code, state)) = crate::modes::interactive::parse_oauth_input(input) {
            ctx.app.push_system(
                format!(
                    "Exchanging code for provider '{}' account '{}'...",
                    pending.provider, pending.account
                ),
                false,
            );
            std::fs::remove_file(&verifier_path).ok();
            std::fs::remove_file(&legacy_path).ok();
            ctx.cmd_tx.send(AgentCommand::Login {
                code,
                state,
                verifier: pending.verifier,
                provider: pending.provider,
                account: pending.account,
            }).ok();
        } else {
            ctx.app.push_system(
                "Invalid code format. Expected:\n  /login code#state\n  /login https://...?code=CODE&state=STATE"
                    .to_string(),
                true,
            );
        }
    } else {
        ctx.app.push_system("No login in progress. Run /login first to get the auth URL.".to_string(), true);
    }
}

pub struct AccountHandler;

impl SlashHandler for AccountHandler {
    fn command(&self) -> super::super::SlashCommand {
        super::super::SlashCommand {
            name: "account",
            description: "Switch or inspect accounts",
            help: "Manage multiple authenticated accounts.\n\n\
                   Usage:\n  \
                     /account                             — show Anthropic-compatible default status\n  \
                     /account --all                       — show grouped status for all providers\n  \
                     /account switch <name>               — switch active Anthropic account\n  \
                     /account switch <provider> <name>    — switch active account for a provider\n  \
                     /account login [name]                — login to an Anthropic account\n  \
                     /account login <provider> [name]     — login to a provider account\n  \
                     /account logout [name]               — logout an Anthropic account\n  \
                     /account logout <provider> [name]    — logout a provider account\n  \
                     /account status [provider] [name]    — show account status\n  \
                     /account list                        — list Anthropic accounts",
            accepts_args: true,
            subcommands: vec![
                ("switch <name>", "switch active Anthropic account"),
                ("switch <provider> <name>", "switch active account for a provider"),
                ("login [name]", "login to an Anthropic account"),
                ("login <provider> [name]", "login to a provider account"),
                ("logout [name]", "logout an Anthropic account"),
                ("logout <provider> [name]", "logout a provider account"),
                ("remove <provider> <name>", "remove an account"),
                ("status [provider] [name]", "show account status"),
                ("list", "list Anthropic accounts"),
            ],
        }
    }

    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        let paths = crate::config::ClankersPaths::get();
        let mut store = crate::provider::auth::AuthStore::load(&paths.global_auth);
        let trimmed = args.trim();

        if trimmed.is_empty() || trimmed == "list" {
            handle_account_list(ctx, &store, false);
        } else if trimmed == "--all" || trimmed == "all" || trimmed == "list --all" {
            handle_account_list(ctx, &store, true);
        } else {
            let parts: Vec<&str> = trimmed.splitn(2, char::is_whitespace).collect();
            let subcmd = parts[0].trim();
            let subcmd_args = parts.get(1).map(|s| s.trim()).unwrap_or("");

            match subcmd {
                "switch" | "use" => handle_account_switch(ctx, &store, subcmd_args),
                "login" => handle_account_login(ctx, &store, subcmd_args),
                "logout" => handle_account_logout(ctx, &mut store, paths, subcmd_args),
                "remove" | "rm" => handle_account_remove(ctx, &mut store, paths, subcmd_args),
                "status" => handle_account_status(ctx, &store, subcmd_args),
                _ => {
                    ctx.app.push_system(
                        format!(
                            "Unknown subcommand '{}'. Available:\n  \
                             switch <name>\n  \
                             switch <provider> <name>\n  \
                             login [name]\n  \
                             login <provider> [name]\n  \
                             logout [name]\n  \
                             logout <provider> [name]\n  \
                             remove <provider> <name>\n  \
                             status [provider] [name]\n  \
                             list\n  \
                             --all",
                            subcmd
                        ),
                        true,
                    );
                }
            }
        }
    }
}

fn account_status_detail(
    store: &crate::provider::auth::AuthStore,
    provider: &str,
    account: &str,
    cred: &crate::provider::auth::StoredCredential,
) -> String {
    let base = crate::commands::auth::describe_credential(cred);
    if provider == crate::provider::openai_codex::OPENAI_CODEX_PROVIDER {
        if let Some(suffix) = crate::provider::openai_codex::codex_status_suffix(store, account) {
            return format!("{}; {}", base, suffix);
        }
    }
    base
}

fn handle_account_list(ctx: &mut SlashContext<'_>, store: &crate::provider::auth::AuthStore, all: bool) {
    use std::fmt::Write;

    if all {
        let pi_store = crate::config::ClankersPaths::get()
            .pi_auth
            .as_ref()
            .map(|path| crate::provider::auth::AuthStore::load(path));
        ctx.app.push_system(
            crate::commands::auth::render_grouped_status_with_fallback(store, pi_store.as_ref()),
            false,
        );
        return;
    }

    let accounts = store.list_anthropic_accounts();
    if accounts.is_empty() {
        let mut msg = String::from("No accounts configured.\n\n");
        if std::env::var("ANTHROPIC_API_KEY").is_ok() {
            msg.push_str("  Using ANTHROPIC_API_KEY from environment.\n");
        }
        msg.push_str("\n  Use /account login [name] or /login to add one.");
        ctx.app.push_system(msg, false);
    } else {
        let mut out = String::from("Accounts:\n\n");
        for info in &accounts {
            let marker = if info.is_active { "▸" } else { " " };
            let status = if info.is_expired { "✗ expired" } else { "✓ valid" };
            let label = info.label.as_ref().map(|l| format!(" ({})", l)).unwrap_or_default();
            writeln!(out, "  {} {}{} — {}", marker, info.name, label, status).ok();
        }
        write!(out, "\n  {} account(s). Use /account switch <name> to change.", accounts.len()).ok();
        ctx.app.push_system(out, false);
    }
}

fn handle_account_switch(ctx: &mut SlashContext<'_>, store: &crate::provider::auth::AuthStore, args: &str) {
    let (provider, remainder) = crate::commands::auth::split_provider_prefix(args);
    let account = remainder.trim();
    if account.is_empty() {
        let names: Vec<String> = store.list_provider_accounts(&provider).iter().map(|a| a.name.clone()).collect();
        let usage = if provider == crate::provider::auth::DEFAULT_OAUTH_PROVIDER {
            "Usage: /account switch <name>"
        } else {
            "Usage: /account switch <provider> <name>"
        };
        ctx.app.push_system(format!("{}\n\nAvailable: {}", usage, names.join(", ")), true);
    } else if provider == crate::provider::auth::DEFAULT_OAUTH_PROVIDER {
        ctx.cmd_tx.send(AgentCommand::SwitchAccount(account.to_string())).ok();
    } else {
        ctx.cmd_tx
            .send(AgentCommand::SwitchProviderAccount {
                provider,
                account: account.to_string(),
            })
            .ok();
    }
}

fn handle_account_login(ctx: &mut SlashContext<'_>, store: &crate::provider::auth::AuthStore, args: &str) {
    let (provider, remainder) = crate::commands::auth::split_provider_prefix(args);
    let account_name = if remainder.trim().is_empty() {
        store.active_account_name_for(&provider).to_string()
    } else {
        remainder.trim().to_string()
    };
    let login_args = if provider == crate::provider::auth::DEFAULT_OAUTH_PROVIDER {
        format!("--account {}", account_name)
    } else {
        format!("{} --account {}", provider, account_name)
    };
    super::auth::LoginHandler.handle(&login_args, ctx);
}

fn handle_account_logout(
    ctx: &mut SlashContext<'_>,
    store: &mut crate::provider::auth::AuthStore,
    paths: &crate::config::ClankersPaths,
    args: &str,
) {
    let (provider, remainder) = crate::commands::auth::split_provider_prefix(args);
    let name = if remainder.trim().is_empty() {
        store.active_account_name_for(&provider).to_string()
    } else {
        remainder.trim().to_string()
    };

    if store.remove_provider_account(&provider, &name) {
        if let Err(e) = store.save(&paths.global_auth) {
            ctx.app.push_system(format!("Failed to save: {}", e), true);
        } else {
            crate::provider::openai_codex::reset_entitlement(&provider, None);
            ctx.cmd_tx.send(AgentCommand::ReloadCredentials).ok();
            let new_active = store.active_account_name_for(&provider).to_string();
            if provider == crate::provider::auth::DEFAULT_OAUTH_PROVIDER {
                ctx.app.push_system(format!("Logged out '{}'. Active account: '{}'.", name, new_active), false);
            } else {
                ctx.app.push_system(
                    format!("Logged out '{}' from provider '{}'. Active account: '{}'.", name, provider, new_active),
                    false,
                );
            }
        }
    } else if provider == crate::provider::auth::DEFAULT_OAUTH_PROVIDER {
        ctx.app.push_system(format!("No account '{}'.", name), true);
    } else {
        ctx.app.push_system(format!("No account '{}' for provider '{}'.", name, provider), true);
    }
}

fn handle_account_remove(
    ctx: &mut SlashContext<'_>,
    store: &mut crate::provider::auth::AuthStore,
    paths: &crate::config::ClankersPaths,
    args: &str,
) {
    let (provider, remainder) = crate::commands::auth::split_provider_prefix(args);
    if remainder.trim().is_empty() {
        if provider == crate::provider::auth::DEFAULT_OAUTH_PROVIDER {
            ctx.app.push_system("Usage: /account remove <name>".to_string(), true);
        } else {
            ctx.app.push_system("Usage: /account remove <provider> <name>".to_string(), true);
        }
        return;
    }

    let name = remainder.trim().to_string();
    if store.remove_provider_account(&provider, &name) {
        if let Err(e) = store.save(&paths.global_auth) {
            ctx.app.push_system(format!("Failed to save: {}", e), true);
        } else {
            crate::provider::openai_codex::reset_entitlement(&provider, None);
            ctx.cmd_tx.send(AgentCommand::ReloadCredentials).ok();
            if provider == crate::provider::auth::DEFAULT_OAUTH_PROVIDER {
                ctx.app.push_system(format!("Removed account '{}'.", name), false);
            } else {
                ctx.app.push_system(format!("Removed account '{}' from provider '{}'.", name, provider), false);
            }
        }
    } else if provider == crate::provider::auth::DEFAULT_OAUTH_PROVIDER {
        ctx.app.push_system(format!("No account '{}'.", name), true);
    } else {
        ctx.app.push_system(format!("No account '{}' for provider '{}'.", name, provider), true);
    }
}

fn handle_account_status(ctx: &mut SlashContext<'_>, store: &crate::provider::auth::AuthStore, args: &str) {
    let trimmed = args.trim();
    if trimmed == "--all" || trimmed == "all" {
        let pi_store = crate::config::ClankersPaths::get()
            .pi_auth
            .as_ref()
            .map(|path| crate::provider::auth::AuthStore::load(path));
        ctx.app.push_system(
            crate::commands::auth::render_grouped_status_with_fallback(store, pi_store.as_ref()),
            false,
        );
        return;
    }

    let (provider, remainder) = crate::commands::auth::split_provider_prefix(trimmed);
    let name = if remainder.trim().is_empty() {
        store.active_account_name_for(&provider).to_string()
    } else {
        remainder.trim().to_string()
    };

    if let Some(cred) = store.credential_for(&provider, &name) {
        ctx.app.push_system(
            format!(
                "{} / {}: {}",
                provider,
                name,
                account_status_detail(store, &provider, &name, cred)
            ),
            false,
        );
        return;
    }

    let pi_store = crate::config::ClankersPaths::get()
        .pi_auth
        .as_ref()
        .map(|path| crate::provider::auth::AuthStore::load(path));
    if let Some(pi_store) = pi_store.as_ref()
        && let Some(cred) = pi_store.credential_for(&provider, &name)
    {
        ctx.app.push_system(
            format!(
                "Using credentials from ~/.pi:\n{} / {}: {}",
                provider,
                name,
                account_status_detail(pi_store, &provider, &name, cred)
            ),
            false,
        );
    } else if let Some(summary) = crate::commands::auth::provider_status_summary(store, &provider, pi_store.as_ref()) {
        ctx.app.push_system(summary, false);
    } else if provider == crate::provider::auth::DEFAULT_OAUTH_PROVIDER {
        ctx.app.push_system(format!("No account '{}'.", name), true);
    } else {
        ctx.app.push_system(format!("No account '{}' for provider '{}'.", name, provider), true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_login_args_defaults_to_anthropic() {
        assert_eq!(
            parse_login_args("--account work"),
            ("anthropic".to_string(), "work".to_string(), String::new())
        );
    }

    #[test]
    fn parse_login_args_extracts_provider_before_code() {
        assert_eq!(
            parse_login_args("openai-codex --account work code#state"),
            ("openai-codex".to_string(), "work".to_string(), "code#state".to_string())
        );
    }

    #[test]
    fn parse_login_args_treats_callback_url_as_input() {
        assert_eq!(
            parse_login_args("https://example.test/callback?code=a&state=b"),
            (
                "anthropic".to_string(),
                "default".to_string(),
                "https://example.test/callback?code=a&state=b".to_string(),
            )
        );
    }

    #[test]
    fn parse_login_args_preserves_omitted_provider_default_for_status_switch_logout_paths() {
        assert_eq!(
            crate::commands::auth::split_provider_prefix("work"),
            ("anthropic".to_string(), "work".to_string())
        );
    }
}
