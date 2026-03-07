//! Authentication command handler

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

/// Run the auth subcommand
pub async fn run(ctx: &CommandContext, action: AuthAction) -> Result<()> {
    match action {
        AuthAction::Login { provider: _, account, code } => handle_login(ctx, account, code).await,
        AuthAction::Status { .. } => handle_status(ctx),
        AuthAction::Logout { account, all, .. } => handle_logout(ctx, account, all),
        AuthAction::Switch { account } => handle_switch(ctx, &account),
        AuthAction::Accounts => handle_accounts(ctx),
        _ => Err(crate::error::Error::ProviderAuth {
            message: "This auth command is not yet implemented.".to_string(),
        }),
    }
}

async fn handle_login(ctx: &CommandContext, account: Option<String>, code: Option<String>) -> Result<()> {
    let account_name = account.as_deref().unwrap_or("default");

    let input = if let Some(code_input) = code {
        code_input
    } else {
        let (url, verifier_val) = crate::provider::anthropic::oauth::build_auth_url();
        println!("Logging in as account: {}", account_name);

        if open::that_detached(&url).is_ok() {
            println!("Opening browser automatically...\n");
        } else {
            println!("Could not open browser automatically.\n");
        }

        println!(
            "Ctrl+Click or open this URL in your browser:\n\n  \x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\\n",
            url, url
        );
        println!(
            "After authorizing, paste the code or callback URL.\n\
             Accepted formats:\n  \
             code#state\n  \
             https://...?code=CODE&state=STATE\n"
        );

        let verifier_path = ctx.paths.global_config_dir.join(".login_verifier");
        std::fs::create_dir_all(&ctx.paths.global_config_dir).ok();
        std::fs::write(&verifier_path, &verifier_val).ok();

        let mut buf = String::new();
        std::io::stdin().read_line(&mut buf).expect("failed to read input");
        buf.trim().to_string()
    };

    let (code_str, state_str) = parse_oauth_callback_input(&input)?;

    let verifier_path = ctx.paths.global_config_dir.join(".login_verifier");
    let verifier = std::fs::read_to_string(&verifier_path).map_err(|_| crate::error::Error::ProviderAuth {
        message: "No login in progress. Run `clankers auth login` first to get the auth URL.".to_string(),
    })?;

    let creds = crate::provider::anthropic::oauth::exchange_code(&code_str, &state_str, &verifier).await?;
    std::fs::remove_file(&verifier_path).ok();

    let mut store = crate::provider::auth::AuthStore::load(&ctx.paths.global_auth);
    store.set_credentials(account_name, creds);
    store.switch_anthropic_account(account_name);
    store.save(&ctx.paths.global_auth)?;
    println!("Authentication successful! Credentials saved as '{}'.", account_name);
    Ok(())
}

fn handle_status(ctx: &CommandContext) -> Result<()> {
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
    Ok(())
}

fn handle_logout(ctx: &CommandContext, account: Option<String>, all: bool) -> Result<()> {
    let mut store = crate::provider::auth::AuthStore::load(&ctx.paths.global_auth);
    if all {
        if let Some(prov) = store.providers.get_mut("anthropic") {
            prov.accounts.clear();
            prov.active_account = None;
        }
        store.save(&ctx.paths.global_auth)?;
        println!("Removed all accounts.");
    } else {
        let name = account.as_deref().unwrap_or(store.active_account_name()).to_string();
        if store.remove_anthropic_account(&name) {
            store.save(&ctx.paths.global_auth)?;
            println!("Removed account '{}'.", name);
        } else {
            return Err(crate::error::Error::ProviderAuth {
                message: format!("No account '{}' found.", name),
            });
        }
    }
    Ok(())
}

fn handle_switch(ctx: &CommandContext, account: &str) -> Result<()> {
    let mut store = crate::provider::auth::AuthStore::load(&ctx.paths.global_auth);
    if store.switch_anthropic_account(account) {
        store.save(&ctx.paths.global_auth)?;
        println!("Switched to account '{}'.", account);
    } else {
        let names: Vec<_> = store
            .providers
            .get("anthropic")
            .map(|p| p.accounts.keys().collect::<Vec<_>>())
            .unwrap_or_default();
        return Err(crate::error::Error::ProviderAuth {
            message: format!("No account '{}'. Available: {:?}", account, names),
        });
    }
    Ok(())
}

fn handle_accounts(ctx: &CommandContext) -> Result<()> {
    let store = crate::provider::auth::AuthStore::load(&ctx.paths.global_auth);
    print!("{}", store.account_summary());
    Ok(())
}
