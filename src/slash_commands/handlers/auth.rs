//! Auth slash command handlers.

use super::SlashContext;
use super::SlashHandler;
use crate::provider::auth::AuthStoreExt;
use crate::modes::interactive::AgentCommand;

pub struct LoginHandler;

impl SlashHandler for LoginHandler {
    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        // Parse optional --account flag: /login [--account name] [code#state|url]
        let (account_name, remaining_args) = crate::modes::interactive::parse_account_flag(args);
        let account_name = account_name.unwrap_or_else(|| "default".to_string());

        if remaining_args.is_empty() {
            let (url, verifier) = crate::provider::anthropic::oauth::build_auth_url();
            ctx.app.login_verifier = Some((verifier.clone(), account_name.clone()));

            // Also persist verifier to disk so `clankers auth login --code` can use it
            let paths = crate::config::ClankersPaths::get();
            let verifier_path = paths.global_config_dir.join(".login_verifier");
            std::fs::create_dir_all(&paths.global_config_dir).ok();
            std::fs::write(&verifier_path, &verifier).ok();

            // Try to auto-open the browser (detached so it doesn't block the TUI)
            let browser_opened = open::that_detached(&url).is_ok();

            let browser_msg = if browser_opened {
                "Opening browser automatically..."
            } else {
                "Could not open browser automatically."
            };

            ctx.app.push_system(
                format!(
                    "Logging in as account: {}\n\n\
                     {}\n\n\
                     Open this URL in your browser to authenticate:\n\n  {}\n\n\
                     After authorizing, paste the code with:\n  /login <code#state>\n  /login <callback URL>\n\n\
                     Or from another terminal:\n  clankers auth login --code <code#state>",
                    account_name, browser_msg, url
                ),
                false,
            );
        } else if let Some((verifier, acct)) = ctx.app.login_verifier.take() {
            // Parse code+state from various formats (code#state, URL, etc.)
            let parsed = crate::modes::interactive::parse_oauth_input(&remaining_args);
            match parsed {
                Some((code, state)) => {
                    ctx.app.push_system(format!("Exchanging code for account '{}'...", acct), false);
                    let _ = ctx.cmd_tx.send(AgentCommand::Login {
                        code,
                        state,
                        verifier,
                        account: acct,
                    });
                }
                None => {
                    ctx.app.login_verifier = Some((verifier, acct));
                    ctx.app.push_system(
                        "Invalid code format. Expected:\n  /login code#state\n  /login https://...?code=CODE&state=STATE".to_string(),
                        true,
                    );
                }
            }
        } else {
            // No in-memory verifier — try recovering from disk (e.g. login started in another clankers
            // instance)
            let paths = crate::config::ClankersPaths::get();
            let verifier_path = paths.global_config_dir.join(".login_verifier");
            if let Ok(verifier) = std::fs::read_to_string(&verifier_path) {
                if let Some((code, state)) = crate::modes::interactive::parse_oauth_input(&remaining_args) {
                    ctx.app.push_system(format!("Exchanging code for account '{}'...", account_name), false);
                    std::fs::remove_file(&verifier_path).ok();
                    let _ = ctx.cmd_tx.send(AgentCommand::Login {
                        code,
                        state,
                        verifier,
                        account: account_name,
                    });
                } else {
                    ctx.app.push_system(
                        "Invalid code format. Expected:\n  /login code#state\n  /login https://...?code=CODE&state=STATE".to_string(),
                        true,
                    );
                }
            } else {
                ctx.app.push_system("No login in progress. Run /login first to get the auth URL.".to_string(), true);
            }
        }
    }
}

pub struct AccountHandler;

impl SlashHandler for AccountHandler {
    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        let paths = crate::config::ClankersPaths::get();
        let mut store = crate::provider::auth::AuthStore::load(&paths.global_auth);

        if args.is_empty() || args == "list" {
            // Show accounts with status details
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
                    out.push_str(&format!("  {} {}{} — {}\n", marker, info.name, label, status));
                }
                out.push_str(&format!("\n  {} account(s). Use /account switch <name> to change.", accounts.len()));
                ctx.app.push_system(out, false);
            }
        } else {
            let parts: Vec<&str> = args.splitn(2, char::is_whitespace).collect();
            let subcmd = parts[0].trim();
            let subcmd_args = parts.get(1).map(|s| s.trim()).unwrap_or("");

            match subcmd {
                "switch" | "use" => {
                    if subcmd_args.is_empty() {
                        // Show available accounts as a hint
                        let names: Vec<String> =
                            store.list_anthropic_accounts().iter().map(|a| a.name.clone()).collect();
                        ctx.app.push_system(
                            format!("Usage: /account switch <name>\n\nAvailable: {}", names.join(", ")),
                            true,
                        );
                    } else {
                        let _ = ctx.cmd_tx.send(AgentCommand::SwitchAccount(subcmd_args.to_string()));
                    }
                }
                "login" => {
                    // Delegate to /login with optional account name
                    let account_name = if subcmd_args.is_empty() {
                        store.active_account_name().to_string()
                    } else {
                        subcmd_args.to_string()
                    };
                    let login_args = format!("--account {}", account_name);
                    // Delegate to the login handler directly
                    super::auth::LoginHandler.handle(&login_args, ctx);
                }
                "logout" => {
                    let name = if subcmd_args.is_empty() {
                        store.active_account_name().to_string()
                    } else {
                        subcmd_args.to_string()
                    };
                    if store.remove_anthropic_account(&name) {
                        if let Err(e) = store.save(&paths.global_auth) {
                            ctx.app.push_system(format!("Failed to save: {}", e), true);
                        } else {
                            let new_active = store.active_account_name().to_string();
                            ctx.app.push_system(
                                format!("Logged out '{}'. Active account: '{}'.", name, new_active),
                                false,
                            );
                        }
                    } else {
                        ctx.app.push_system(format!("No account '{}'.", name), true);
                    }
                }
                "remove" | "rm" => {
                    if subcmd_args.is_empty() {
                        ctx.app.push_system("Usage: /account remove <name>".to_string(), true);
                    } else {
                        let name = subcmd_args.to_string();
                        if store.remove_anthropic_account(&name) {
                            if let Err(e) = store.save(&paths.global_auth) {
                                ctx.app.push_system(format!("Failed to save: {}", e), true);
                            } else {
                                ctx.app.push_system(format!("Removed account '{}'.", name), false);
                            }
                        } else {
                            ctx.app.push_system(format!("No account '{}'.", name), true);
                        }
                    }
                }
                "status" => {
                    let name = if subcmd_args.is_empty() {
                        store.active_account_name().to_string()
                    } else {
                        subcmd_args.to_string()
                    };
                    if let Some(cred) = store.credential_for("anthropic", &name) {
                        let status = if cred.is_expired() { "✗ expired" } else { "✓ valid" };
                        let expires_in = if cred.is_expired() {
                            "expired".to_string()
                        } else if let crate::provider::auth::StoredCredential::OAuth { expires_at_ms, .. } = cred {
                            let remaining = expires_at_ms - chrono::Utc::now().timestamp_millis();
                            let mins = remaining / 60_000;
                            if mins > 60 {
                                format!("{}h {}m", mins / 60, mins % 60)
                            } else {
                                format!("{}m", mins)
                            }
                        } else {
                            "n/a (api key)".to_string()
                        };
                        ctx.app.push_system(
                            format!("Account '{}': {} (expires in {})", name, status, expires_in),
                            false,
                        );
                    } else {
                        ctx.app.push_system(format!("No account '{}'.", name), true);
                    }
                }
                _ => {
                    ctx.app.push_system(
                        format!(
                            "Unknown subcommand '{}'. Available:\n  \
                             switch <name>  — switch active account\n  \
                             login [name]   — login to an account\n  \
                             logout [name]  — logout an account\n  \
                             remove <name>  — remove an account\n  \
                             status [name]  — show account status\n  \
                             list           — list all accounts",
                            subcmd
                        ),
                        true,
                    );
                }
            }
        }
    }
}
