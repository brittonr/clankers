//! Authentication command handler

use std::collections::BTreeSet;
use std::fmt::Write;
use std::io::Read;

use crate::cli::AuthAction;
use crate::commands::CommandContext;
use crate::error::Result;
use crate::provider::auth::AuthStoreExt;

/// Parse OAuth callback input in various formats
///
/// Accepts:
/// - `code#state` format
/// - Full callback URL with query parameters
/// - Space-separated `code state`
fn parse_oauth_callback_input(input: &str) -> Result<(String, String)> {
    let input = input.trim();

    // Try parsing as a URL first
    if input.starts_with("http://") || input.starts_with("https://") {
        if let Ok(url) = url::Url::parse(input) {
            let params: std::collections::HashMap<_, _> = url.query_pairs().collect();
            if let (Some(code), Some(state)) = (params.get("code"), params.get("state")) {
                return Ok((code.to_string(), state.to_string()));
            }
        }
        return Err(crate::error::Error::ProviderAuth {
            message: "URL missing 'code' and/or 'state' query parameters.".to_string(),
        });
    }

    // Try code#state format
    if let Some((code, state)) = input.split_once('#')
        && !code.is_empty()
        && !state.is_empty()
    {
        return Ok((code.to_string(), state.to_string()));
    }

    // Try space-separated
    if let Some((code, state)) = input.split_once(' ') {
        let code = code.trim();
        let state = state.trim();
        if !code.is_empty() && !state.is_empty() {
            return Ok((code.to_string(), state.to_string()));
        }
    }

    Err(crate::error::Error::ProviderAuth {
        message: format!(
            "Invalid code format: '{}'. Expected one of:\n  \
             code#state\n  \
             https://...?code=CODE&state=STATE",
            if input.len() > 40 { &input[..40] } else { input }
        ),
    })
}

pub(crate) fn is_known_provider_name(provider: &str) -> bool {
    provider == "openai-codex" || crate::provider::auth::env_var_for_provider(provider).is_some()
}

pub(crate) fn split_provider_prefix(input: &str) -> (String, String) {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return (crate::provider::auth::DEFAULT_OAUTH_PROVIDER.to_string(), String::new());
    }

    if let Some(split_at) = trimmed.find(char::is_whitespace) {
        let first = &trimmed[..split_at];
        let rest = trimmed[split_at..].trim_start();
        if is_known_provider_name(first) {
            return (first.to_string(), rest.to_string());
        }
    } else if is_known_provider_name(trimmed) {
        return (trimmed.to_string(), String::new());
    }

    (crate::provider::auth::DEFAULT_OAUTH_PROVIDER.to_string(), trimmed.to_string())
}

fn format_expires_in(expires_at_ms: i64) -> String {
    let remaining_ms = expires_at_ms - chrono::Utc::now().timestamp_millis();
    if remaining_ms <= 0 {
        return "expired".to_string();
    }
    let mins = remaining_ms / 60_000;
    if mins > 60 {
        format!("{}h {}m", mins / 60, mins % 60)
    } else {
        format!("{}m", mins)
    }
}

pub(crate) fn describe_credential(cred: &crate::provider::auth::StoredCredential) -> String {
    match cred {
        crate::provider::auth::StoredCredential::ApiKey { .. } => "api key".to_string(),
        crate::provider::auth::StoredCredential::OAuth { expires_at_ms, .. } => {
            if cred.is_expired() {
                "oauth expired".to_string()
            } else {
                format!("oauth valid (expires in {})", format_expires_in(*expires_at_ms))
            }
        }
    }
}

fn describe_provider_account_detail(
    store: &crate::provider::auth::AuthStore,
    provider: &str,
    account: &str,
    cred: &crate::provider::auth::StoredCredential,
) -> String {
    let base = describe_credential(cred);
    if provider == crate::provider::openai_codex::OPENAI_CODEX_PROVIDER
        && let Some(suffix) = crate::provider::openai_codex::codex_status_suffix(store, account)
    {
        return format!("{}; {}", base, suffix);
    }
    base
}

pub(crate) fn render_provider_accounts(store: &crate::provider::auth::AuthStore, provider: &str) -> Option<String> {
    let mut accounts = store.list_provider_accounts(provider);
    if accounts.is_empty() {
        return None;
    }

    accounts.sort_by(|a, b| b.is_active.cmp(&a.is_active).then_with(|| a.name.cmp(&b.name)));

    let mut out = String::new();
    writeln!(out, "{}:", provider).ok();
    for info in accounts {
        let marker = if info.is_active { "▸" } else { " " };
        let label = info.label.as_ref().map(|l| format!(" ({})", l)).unwrap_or_default();
        let detail = store
            .credential_for(provider, &info.name)
            .map(|cred| describe_provider_account_detail(store, provider, &info.name, cred))
            .unwrap_or_else(|| "unknown".to_string());
        writeln!(out, "  {} {}{} — {}", marker, info.name, label, detail).ok();
    }
    Some(out.trim_end().to_string())
}

pub(crate) fn provider_status_summary(
    store: &crate::provider::auth::AuthStore,
    provider: &str,
    pi_store: Option<&crate::provider::auth::AuthStore>,
) -> Option<String> {
    if let Some(summary) = render_provider_accounts(store, provider) {
        return Some(summary);
    }

    if let Some(env_var) = crate::provider::auth::env_var_for_provider(provider)
        && std::env::var(env_var).is_ok()
    {
        return Some(format!("{}:\n  ▸ env — api key via {}", provider, env_var));
    }

    if let Some(pi_store) = pi_store
        && let Some(summary) = render_provider_accounts(pi_store, provider)
    {
        return Some(format!("Using credentials from ~/.pi:\n{}", summary));
    }

    None
}

pub(crate) fn render_grouped_status_with_fallback(
    store: &crate::provider::auth::AuthStore,
    pi_store: Option<&crate::provider::auth::AuthStore>,
) -> String {
    let mut providers: BTreeSet<String> = store.configured_providers().into_iter().map(ToString::to_string).collect();
    if let Some(pi_store) = pi_store {
        providers.extend(pi_store.configured_providers().into_iter().map(ToString::to_string));
    }
    for provider in [
        "anthropic",
        "openai",
        "openrouter",
        "groq",
        "deepseek",
        "mistral",
        "together",
        "fireworks",
        "xai",
    ] {
        if let Some(env_var) = crate::provider::auth::env_var_for_provider(provider)
            && std::env::var(env_var).is_ok()
        {
            providers.insert(provider.to_string());
        }
    }

    if providers.is_empty() {
        return "No provider credentials configured.".to_string();
    }

    let mut sections = Vec::new();
    for provider in providers {
        if let Some(summary) = provider_status_summary(store, &provider, pi_store) {
            sections.push(summary);
        }
    }
    sections.join("\n\n")
}

fn handle_default_anthropic_status(ctx: &CommandContext) {
    let store = crate::provider::auth::AuthStore::load(&ctx.paths.global_auth);
    let accounts = store.list_anthropic_accounts();
    if !accounts.is_empty() {
        println!("Accounts:");
        for info in &accounts {
            let marker = if info.is_active { "▸" } else { " " };
            let status = if info.is_expired { "expired" } else { "valid" };
            let label = info.label.as_ref().map(|l| format!(" ({})", l)).unwrap_or_default();
            println!("  {} {}{} — {}", marker, info.name, label, status);
        }
    } else if std::env::var("ANTHROPIC_API_KEY").is_ok() {
        println!("Anthropic: API key set via ANTHROPIC_API_KEY");
    } else if let Some(ref pi_auth) = ctx.paths.pi_auth {
        let pi_store = crate::provider::auth::AuthStore::load(pi_auth);
        if !pi_store.list_anthropic_accounts().is_empty() {
            println!("Using credentials from ~/.pi:");
            for info in &pi_store.list_anthropic_accounts() {
                let status = if info.is_expired { "expired" } else { "valid" };
                println!("  {} — {}", info.name, status);
            }
        } else {
            println!("Anthropic: not authenticated");
        }
    } else {
        println!("Anthropic: not authenticated");
    }
}

fn handle_provider_status(ctx: &CommandContext, provider: &str) {
    let store = crate::provider::auth::AuthStore::load(&ctx.paths.global_auth);
    let pi_store = ctx.paths.pi_auth.as_ref().map(|path| crate::provider::auth::AuthStore::load(path));
    if let Some(summary) = provider_status_summary(&store, provider, pi_store.as_ref()) {
        println!("{}", summary);
        return;
    }

    println!("{}: not authenticated", provider);
}

/// Run the auth subcommand
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(catch_all_on_enum, reason = "default handler covers many variants uniformly")
)]
pub async fn run(ctx: &CommandContext, action: AuthAction) -> Result<()> {
    match action {
        AuthAction::Login {
            provider,
            account,
            code,
        } => handle_login(ctx, provider, account, code).await,
        AuthAction::Status { provider, all } => handle_status(ctx, provider, all),
        AuthAction::Logout { provider, account, all } => handle_logout(ctx, provider, account, all),
        AuthAction::Switch { provider, account } => handle_switch(ctx, provider, &account),
        AuthAction::Accounts => handle_accounts(ctx),
        AuthAction::Export { provider, account } => handle_export(ctx, &provider, account.as_deref()),
        AuthAction::Import { input } => handle_import(ctx, &input),
        _ => Err(crate::error::Error::ProviderAuth {
            message: "This auth command is not yet implemented.".to_string(),
        }),
    }
}

#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(no_unwrap, reason = "stdin read failure is unrecoverable in interactive login")
)]
async fn handle_login(
    ctx: &CommandContext,
    provider: Option<String>,
    account: Option<String>,
    code: Option<String>,
) -> Result<()> {
    let oauth_flow = crate::provider::auth::OAuthFlow::from_provider(provider.as_deref())?;
    let provider_name = oauth_flow.provider_name();
    let account_name = account.unwrap_or_else(|| "default".to_string());
    let pending_path =
        crate::provider::auth::pending_oauth_login_path(&ctx.paths.global_config_dir, provider_name, &account_name);
    let legacy_pending_path = crate::provider::auth::legacy_pending_oauth_login_path(&ctx.paths.global_config_dir);

    let input = if let Some(code_input) = code {
        code_input
    } else {
        let (url, verifier_val) = oauth_flow.build_auth_url()?;
        let pending = crate::provider::auth::PendingOAuthLogin::new(provider_name, account_name.clone(), verifier_val);
        pending.save(&pending_path).map_err(|e| crate::error::Error::ProviderAuth {
            message: format!("Failed to persist pending login: {e}"),
        })?;

        println!("Logging in to provider '{}' as account '{}'.", provider_name, account_name);

        if open::that_detached(&url).is_ok() {
            println!("Opening browser automatically...\n");
        } else {
            println!("Could not open browser automatically.\n");
        }

        println!("Ctrl+Click or open this URL in your browser:\n\n  \x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\\n", url, url);
        println!(
            "After authorizing, paste the code or callback URL.\n\
             Accepted formats:\n  \
             code#state\n  \
             https://...?code=CODE&state=STATE\n"
        );

        let mut buf = String::new();
        std::io::stdin().read_line(&mut buf).expect("failed to read input");
        buf.trim().to_string()
    };

    let (code_str, state_str) = parse_oauth_callback_input(&input)?;
    let pending = crate::provider::auth::PendingOAuthLogin::load(&pending_path)
        .or_else(|| crate::provider::auth::PendingOAuthLogin::load(&legacy_pending_path))
        .ok_or(crate::error::Error::ProviderAuth {
            message: "No login in progress. Run `clankers auth login` first to get the auth URL.".to_string(),
        })?;
    let oauth_flow = crate::provider::auth::OAuthFlow::from_provider(Some(&pending.provider))?;

    let creds = oauth_flow.exchange_code(&code_str, &state_str, &pending.verifier).await?;
    std::fs::remove_file(&pending_path).ok();
    std::fs::remove_file(&legacy_pending_path).ok();

    let mut store = crate::provider::auth::AuthStore::load(&ctx.paths.global_auth);
    store.set_provider_credentials(&pending.provider, &pending.account, creds);
    store.switch_provider_account(&pending.provider, &pending.account);
    store.save(&ctx.paths.global_auth)?;
    crate::provider::openai_codex::reset_entitlement(&pending.provider, None);
    println!(
        "Authentication successful! Credentials saved as '{}' for provider '{}'.",
        pending.account, pending.provider
    );
    Ok(())
}

#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(
        nested_conditionals,
        reason = "complex control flow — extracting helpers would obscure logic"
    )
)]
fn handle_status(ctx: &CommandContext, provider: Option<String>, all: bool) -> Result<()> {
    if all {
        let store = crate::provider::auth::AuthStore::load(&ctx.paths.global_auth);
        let pi_store = ctx.paths.pi_auth.as_ref().map(|path| crate::provider::auth::AuthStore::load(path));
        println!("{}", render_grouped_status_with_fallback(&store, pi_store.as_ref()));
        return Ok(());
    }

    if let Some(provider) = provider {
        handle_provider_status(ctx, &provider);
    } else {
        handle_default_anthropic_status(ctx);
    }
    Ok(())
}

fn handle_logout(ctx: &CommandContext, provider: Option<String>, account: Option<String>, all: bool) -> Result<()> {
    let provider_was_explicit = provider.is_some();
    let provider = provider.unwrap_or_else(|| crate::provider::auth::DEFAULT_OAUTH_PROVIDER.to_string());
    let mut store = crate::provider::auth::AuthStore::load(&ctx.paths.global_auth);
    if all {
        if let Some(prov) = store.providers.get_mut(&provider) {
            prov.accounts.clear();
            prov.active_account = None;
        }
        store.save(&ctx.paths.global_auth)?;
        crate::provider::openai_codex::reset_entitlement(&provider, None);
        if provider_was_explicit {
            println!("Removed all accounts for provider '{}'.", provider);
        } else {
            println!("Removed all accounts.");
        }
    } else {
        let name = account.unwrap_or_else(|| store.active_account_name_for(&provider).to_string());
        if store.remove_provider_account(&provider, &name) {
            store.save(&ctx.paths.global_auth)?;
            crate::provider::openai_codex::reset_entitlement(&provider, None);
            if provider_was_explicit {
                println!("Removed account '{}' from provider '{}'.", name, provider);
            } else {
                println!("Removed account '{}'.", name);
            }
        } else if provider_was_explicit {
            return Err(crate::error::Error::ProviderAuth {
                message: format!("No account '{}' found for provider '{}'.", name, provider),
            });
        } else {
            return Err(crate::error::Error::ProviderAuth {
                message: format!("No account '{}' found.", name),
            });
        }
    }
    Ok(())
}

fn handle_switch(ctx: &CommandContext, provider: Option<String>, account: &str) -> Result<()> {
    let provider_was_explicit = provider.is_some();
    let provider = provider.unwrap_or_else(|| crate::provider::auth::DEFAULT_OAUTH_PROVIDER.to_string());
    let mut store = crate::provider::auth::AuthStore::load(&ctx.paths.global_auth);
    if store.switch_provider_account(&provider, account) {
        store.save(&ctx.paths.global_auth)?;
        crate::provider::openai_codex::reset_entitlement(&provider, None);
        if provider_was_explicit {
            println!("Switched provider '{}' to account '{}'.", provider, account);
        } else {
            println!("Switched to account '{}'.", account);
        }
    } else {
        let names: Vec<String> = store.list_provider_accounts(&provider).into_iter().map(|info| info.name).collect();
        if provider_was_explicit {
            return Err(crate::error::Error::ProviderAuth {
                message: format!("No account '{}' for provider '{}'. Available: {:?}", account, provider, names),
            });
        }
        return Err(crate::error::Error::ProviderAuth {
            message: format!("No account '{}'. Available: {:?}", account, names),
        });
    }
    Ok(())
}

fn handle_accounts(ctx: &CommandContext) -> Result<()> {
    let store = crate::provider::auth::AuthStore::load(&ctx.paths.global_auth);
    let pi_store = ctx.paths.pi_auth.as_ref().map(|path| crate::provider::auth::AuthStore::load(path));
    println!("{}", render_grouped_status_with_fallback(&store, pi_store.as_ref()));
    Ok(())
}

fn handle_export(ctx: &CommandContext, provider: &str, account: Option<&str>) -> Result<()> {
    let account_name = account.unwrap_or("default");
    let store = crate::provider::auth::AuthStore::load(&ctx.paths.global_auth);
    let pi_store = ctx.paths.pi_auth.as_ref().map(|path| crate::provider::auth::AuthStore::load(path));

    let record = store
        .credential_for(provider, account_name)
        .cloned()
        .map(|credential| clanker_router::auth::ProviderAccountExport {
            version: 1,
            provider: provider.to_string(),
            account: account_name.to_string(),
            active: store.active_account_name_for(provider) == account_name,
            credential,
        })
        .or_else(|| {
            pi_store.as_ref().and_then(|fallback| {
                fallback.credential_for(provider, account_name).cloned().map(|credential| {
                    clanker_router::auth::ProviderAccountExport {
                        version: 1,
                        provider: provider.to_string(),
                        account: account_name.to_string(),
                        active: fallback.active_account_name_for(provider) == account_name,
                        credential,
                    }
                })
            })
        })
        .ok_or(crate::error::Error::ProviderAuth {
            message: format!("No account '{}' found for provider '{}'.", account_name, provider),
        })?;

    println!("{}", serde_json::to_string_pretty(&record).expect("auth export should serialize"));
    Ok(())
}

fn handle_import(ctx: &CommandContext, input: &str) -> Result<()> {
    let raw = if input == "-" {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf).map_err(|e| crate::error::Error::ProviderAuth {
            message: format!("Failed to read import record from stdin: {e}"),
        })?;
        buf
    } else {
        std::fs::read_to_string(input).map_err(|e| crate::error::Error::ProviderAuth {
            message: format!("Failed to read import record '{}': {e}", input),
        })?
    };

    let record: clanker_router::auth::ProviderAccountExport =
        serde_json::from_str(&raw).map_err(|e| crate::error::Error::ProviderAuth {
            message: format!("Failed to parse auth import record: {e}"),
        })?;

    let mut store = crate::provider::auth::AuthStore::load(&ctx.paths.global_auth);
    let had_active = store.active_credential(&record.provider).is_some();
    store.set_credential(&record.provider, &record.account, record.credential.clone());
    if record.active || !had_active {
        store.switch_provider_account(&record.provider, &record.account);
    }
    store.save(&ctx.paths.global_auth)?;

    println!(
        "Imported provider '{}' account '{}' into {}.",
        record.provider,
        record.account,
        ctx.paths.global_auth.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_provider_prefix_defaults_to_anthropic() {
        assert_eq!(
            split_provider_prefix("work"),
            (crate::provider::auth::DEFAULT_OAUTH_PROVIDER.to_string(), "work".to_string())
        );
    }

    #[test]
    fn split_provider_prefix_extracts_explicit_provider() {
        assert_eq!(split_provider_prefix("openai-codex work"), ("openai-codex".to_string(), "work".to_string()));
    }

    #[test]
    fn split_provider_prefix_keeps_omitted_provider_default_for_status_switch_logout() {
        assert_eq!(
            split_provider_prefix("status-account"),
            (crate::provider::auth::DEFAULT_OAUTH_PROVIDER.to_string(), "status-account".to_string())
        );
        assert_eq!(
            split_provider_prefix("logout-account"),
            (crate::provider::auth::DEFAULT_OAUTH_PROVIDER.to_string(), "logout-account".to_string())
        );
        assert_eq!(
            split_provider_prefix("switch-account"),
            (crate::provider::auth::DEFAULT_OAUTH_PROVIDER.to_string(), "switch-account".to_string())
        );
    }

    #[test]
    fn provider_status_summary_uses_pi_fallback_for_openai_codex() {
        let store = crate::provider::auth::AuthStore::default();
        let mut pi_store = crate::provider::auth::AuthStore::default();
        pi_store.set_provider_credentials("openai-codex", "work", crate::provider::auth::OAuthCredentials {
            access: "not-a-valid-jwt".into(),
            refresh: "refresh".into(),
            expires: chrono::Utc::now().timestamp_millis() + 3_600_000,
        });

        let rendered = provider_status_summary(&store, "openai-codex", Some(&pi_store)).expect("provider summary");
        assert!(rendered.contains("Using credentials from ~/.pi:"));
        assert!(rendered.contains("openai-codex:"));
    }

    #[test]
    fn render_grouped_status_with_fallback_includes_pi_only_codex() {
        let store = crate::provider::auth::AuthStore::default();
        let mut pi_store = crate::provider::auth::AuthStore::default();
        pi_store.set_provider_credentials("openai-codex", "work", crate::provider::auth::OAuthCredentials {
            access: "not-a-valid-jwt".into(),
            refresh: "refresh".into(),
            expires: chrono::Utc::now().timestamp_millis() + 3_600_000,
        });

        let rendered = render_grouped_status_with_fallback(&store, Some(&pi_store));
        assert!(rendered.contains("Using credentials from ~/.pi:"));
        assert!(rendered.contains("openai-codex:"));
    }

    #[test]
    fn render_grouped_status_includes_provider_scoped_details() {
        let mut store = crate::provider::auth::AuthStore::default();
        store.set_provider_credentials("anthropic", "work", crate::provider::auth::OAuthCredentials {
            access: "tok".into(),
            refresh: "ref".into(),
            expires: chrono::Utc::now().timestamp_millis() + 3_600_000,
        });
        store.set_provider_credentials("openai-codex", "codex", crate::provider::auth::OAuthCredentials {
            access: "codex-token".into(),
            refresh: "codex-refresh".into(),
            expires: chrono::Utc::now().timestamp_millis() + 7_200_000,
        });
        store.set_credential("openai", "default", crate::provider::auth::StoredCredential::ApiKey {
            api_key: "sk-test".into(),
            label: None,
        });

        let rendered = render_grouped_status_with_fallback(&store, None);
        assert!(rendered.contains("anthropic:"));
        assert!(rendered.contains("openai-codex:"));
        assert!(rendered.contains("oauth valid (expires in"));
        assert!(rendered.contains("openai:"));
        assert!(rendered.contains("api key"));
    }

    #[test]
    fn render_provider_accounts_reports_expired_oauth_separately() {
        let mut store = crate::provider::auth::AuthStore::default();
        store.set_credential("openai-codex", "expired", crate::provider::auth::StoredCredential::OAuth {
            access_token: "expired-token".into(),
            refresh_token: "refresh".into(),
            expires_at_ms: 0,
            label: None,
        });

        let rendered = render_provider_accounts(&store, "openai-codex").expect("provider summary");
        assert!(rendered.contains("openai-codex:"));
        assert!(rendered.contains("oauth expired"));
    }

    #[test]
    fn render_provider_accounts_surfaces_codex_probe_failures() {
        let mut store = crate::provider::auth::AuthStore::default();
        store.set_provider_credentials("openai-codex", "work", crate::provider::auth::OAuthCredentials {
            access: "not-a-valid-jwt".into(),
            refresh: "refresh".into(),
            expires: chrono::Utc::now().timestamp_millis() + 3_600_000,
        });

        let rendered = render_provider_accounts(&store, "openai-codex").expect("provider summary");
        assert!(rendered.contains("authenticated, entitlement check failed"));
    }

    #[test]
    fn provider_scoped_switch_and_logout_leave_other_providers_unchanged() {
        let mut store = crate::provider::auth::AuthStore::default();
        store.set_provider_credentials("anthropic", "personal", crate::provider::auth::OAuthCredentials {
            access: "anthropic-token".into(),
            refresh: "anthropic-refresh".into(),
            expires: chrono::Utc::now().timestamp_millis() + 3_600_000,
        });
        store.set_provider_credentials("openai-codex", "work", crate::provider::auth::OAuthCredentials {
            access: "codex-token".into(),
            refresh: "codex-refresh".into(),
            expires: chrono::Utc::now().timestamp_millis() + 3_600_000,
        });
        store.set_provider_credentials("openai-codex", "backup", crate::provider::auth::OAuthCredentials {
            access: "codex-backup".into(),
            refresh: "codex-refresh-2".into(),
            expires: chrono::Utc::now().timestamp_millis() + 3_600_000,
        });

        assert!(store.switch_provider_account("openai-codex", "backup"));
        assert_eq!(store.active_account_name_for("openai-codex"), "backup");
        assert_eq!(store.active_account_name_for("anthropic"), "personal");

        assert!(store.remove_provider_account("openai-codex", "work"));
        assert!(store.credential_for("openai-codex", "work").is_none());
        assert_eq!(store.active_account_name_for("anthropic"), "personal");
        assert_eq!(store.credential_for("anthropic", "personal").unwrap().token(), "anthropic-token");
    }
}
