//! Token command handlers for UCAN capability token management.

use crate::cli::TokenAction;
use crate::commands::CommandContext;
use crate::error::Result;
use crate::util::parsing::parse_duration;

/// Bundled capability scope for token creation (avoids excessive bool params).
struct TokenScope {
    tools: Option<String>,
    read_only: bool,
    bot_commands: Option<String>,
    session_manage: bool,
    model_switch: bool,
    delegate: bool,
    file_prefix: Option<String>,
    file_read_only: bool,
    shell: Option<String>,
    shell_wd: Option<String>,
    root: bool,
}

/// Run the token subcommand.
pub fn run(ctx: &CommandContext, action: TokenAction) -> Result<()> {
    let identity_path = crate::modes::rpc::iroh::identity_path(&ctx.paths);
    let identity = crate::modes::rpc::iroh::Identity::load_or_generate(&identity_path);

    let redb_db = open_auth_db(ctx)?;

    match action {
        TokenAction::Create {
            tools,
            read_only,
            expire,
            r#for: audience_key,
            from,
            bot_commands,
            session_manage,
            model_switch,
            delegate,
            file_prefix,
            file_read_only,
            shell,
            shell_wd,
            root,
        } => {
            let scope = TokenScope {
                tools,
                read_only,
                bot_commands,
                session_manage,
                model_switch,
                delegate,
                file_prefix,
                file_read_only,
                shell,
                shell_wd,
                root,
            };
            handle_create(&identity, &redb_db, &expire, audience_key, from, scope)
        }
        TokenAction::List => handle_list(&redb_db),
        TokenAction::Revoke { hash } => handle_revoke(&redb_db, &hash),
        TokenAction::Info { token: token_b64 } => handle_info(&token_b64),
    }
}

/// Open (or create) the redb database with auth tables.
fn open_auth_db(ctx: &CommandContext) -> Result<std::sync::Arc<redb::Database>> {
    let db_path = ctx.paths.global_config_dir.join("clankers.db");
    std::fs::create_dir_all(&ctx.paths.global_config_dir).ok();
    let redb_db = std::sync::Arc::new(redb::Database::create(&db_path).map_err(|e| crate::error::Error::Io {
        source: std::io::Error::other(e.to_string()),
    })?);
    {
        let tx = redb_db.begin_write().map_err(|e| crate::error::Error::Io {
            source: std::io::Error::other(e.to_string()),
        })?;
        tx.open_table(clankers_ucan::revocation::AUTH_TOKENS_TABLE).ok();
        tx.open_table(clankers_ucan::revocation::REVOKED_TOKENS_TABLE).ok();
        tx.commit().map_err(|e| crate::error::Error::Io {
            source: std::io::Error::other(e.to_string()),
        })?;
    }
    Ok(redb_db)
}

/// Create a new capability token.
fn handle_create(
    identity: &crate::modes::rpc::iroh::Identity,
    redb_db: &std::sync::Arc<redb::Database>,
    expire: &str,
    audience_key: Option<String>,
    from: Option<String>,
    scope: TokenScope,
) -> Result<()> {
    use clankers_ucan::Credential;
    use clankers_ucan::TokenBuilder;

    let lifetime = parse_duration(expire).ok_or_else(|| crate::error::Error::Config {
        message: format!("Invalid duration: '{}'. Examples: 1h, 24h, 7d, 30d, 365d", expire),
    })?;

    let parent_cred: Option<Credential> = from
        .as_ref()
        .map(|parent_b64| {
            Credential::from_base64(parent_b64).map_err(|e| crate::error::Error::Config {
                message: format!("Invalid parent credential: {}", e),
            })
        })
        .transpose()?;

    // Build the leaf token
    let mut builder = if let Some(ref parent) = parent_cred {
        TokenBuilder::new(identity.secret_key.clone()).delegated_from(parent.token.clone())
    } else {
        TokenBuilder::new(identity.secret_key.clone())
    };

    if let Some(ref pubkey_str) = audience_key {
        let pubkey: iroh::PublicKey = pubkey_str.parse().map_err(|e| crate::error::Error::Config {
            message: format!("Invalid public key: {}", e),
        })?;
        builder = builder.for_key(pubkey);
    }

    builder = if scope.root {
        build_root_capabilities(builder)
    } else {
        build_scoped_capabilities(builder, &scope)
    };

    builder = builder.with_lifetime(lifetime).with_random_nonce();

    let token = builder.build().map_err(|e| crate::error::Error::Config {
        message: format!("Failed to create token: {}", e),
    })?;

    // Wrap in a Credential with the parent chain
    let cred = if let Some(parent) = parent_cred {
        let mut proofs = Vec::with_capacity(parent.proofs.len().saturating_add(1));
        proofs.push(parent.token);
        proofs.extend(parent.proofs);
        Credential { token, proofs }
    } else {
        Credential::from_root(token)
    };

    let b64 = cred.to_base64().map_err(|e| crate::error::Error::Config {
        message: format!("Failed to encode credential: {}", e),
    })?;

    store_credential(redb_db, &cred);
    print_credential_summary(&cred);
    println!("{}", b64);
    Ok(())
}

/// Build capabilities for a full-access root token.
fn build_root_capabilities(builder: clankers_ucan::TokenBuilder) -> clankers_ucan::TokenBuilder {
    use clankers_ucan::Capability;
    builder
        .with_capability(Capability::Prompt)
        .with_capability(Capability::ToolUse {
            tool_pattern: "*".into(),
        })
        .with_capability(Capability::ShellExecute {
            command_pattern: "*".into(),
            working_dir: None,
        })
        .with_capability(Capability::FileAccess {
            prefix: "/".into(),
            read_only: false,
        })
        .with_capability(Capability::BotCommand {
            command_pattern: "*".into(),
        })
        .with_capability(Capability::SessionManage)
        .with_capability(Capability::ModelSwitch)
        .with_capability(Capability::Delegate)
}

/// Build capabilities for a scoped (non-root) token.
fn build_scoped_capabilities(
    mut builder: clankers_ucan::TokenBuilder,
    scope: &TokenScope,
) -> clankers_ucan::TokenBuilder {
    use clankers_ucan::Capability;

    builder = builder.with_capability(Capability::Prompt);

    if scope.read_only {
        builder = builder.with_capability(Capability::ToolUse {
            tool_pattern: "read,grep,find,ls".into(),
        });
    } else if let Some(ref tool_list) = scope.tools {
        builder = builder.with_capability(Capability::ToolUse {
            tool_pattern: tool_list.clone(),
        });
    }

    if let Some(ref pattern) = scope.shell {
        builder = builder.with_capability(Capability::ShellExecute {
            command_pattern: pattern.clone(),
            working_dir: scope.shell_wd.clone(),
        });
    }
    if let Some(ref prefix) = scope.file_prefix {
        builder = builder.with_capability(Capability::FileAccess {
            prefix: prefix.clone(),
            read_only: scope.file_read_only,
        });
    }
    if let Some(ref cmds) = scope.bot_commands {
        builder = builder.with_capability(Capability::BotCommand {
            command_pattern: cmds.clone(),
        });
    }
    if scope.session_manage {
        builder = builder.with_capability(Capability::SessionManage);
    }
    if scope.model_switch {
        builder = builder.with_capability(Capability::ModelSwitch);
    }
    if scope.delegate {
        builder = builder.with_capability(Capability::Delegate);
    }
    builder
}

/// Store a credential in redb for tracking.
fn store_credential(redb_db: &std::sync::Arc<redb::Database>, cred: &clankers_ucan::Credential) {
    let hash_hex = hex::encode(cred.token.hash().unwrap_or([0u8; 32]));
    let encoded = cred.encode().unwrap_or_default();
    if let Ok(tx) = redb_db.begin_write() {
        if let Ok(mut table) = tx.open_table(clankers_ucan::revocation::AUTH_TOKENS_TABLE) {
            table.insert(hash_hex.as_str(), encoded.as_slice()).ok();
        }
        if let Err(e) = tx.commit() {
            eprintln!("Warning: failed to store credential in database: {}", e);
        }
    }
}

/// Print credential metadata to stderr.
fn print_credential_summary(cred: &clankers_ucan::Credential) {
    let hash_hex = hex::encode(cred.token.hash().unwrap_or([0u8; 32]));
    eprintln!("Credential created:");
    eprintln!("  Issuer:  {}", cred.token.issuer.fmt_short());
    eprintln!("  Hash:    {}", &hash_hex[..16]);
    eprintln!(
        "  Expires: {}",
        chrono::DateTime::from_timestamp(cred.token.expires_at as i64, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string())
            .unwrap_or_else(|| "unknown".to_string())
    );
    let cap_strs: Vec<String> = cred.token.capabilities.iter().map(|c| format!("{:?}", c)).collect();
    eprintln!("  Caps:    {}", cap_strs.join(", "));
    if cred.token.delegation_depth > 0 {
        eprintln!("  Depth:   {}", cred.token.delegation_depth);
    }
    if !cred.proofs.is_empty() {
        eprintln!("  Chain:   {} proof(s)", cred.proofs.len());
    }
    eprintln!();
}

/// List all issued tokens.
fn handle_list(redb_db: &std::sync::Arc<redb::Database>) -> Result<()> {
    use redb::ReadableTable;

    let read_tx = redb_db.begin_read().map_err(|e| crate::error::Error::Io {
        source: std::io::Error::other(e.to_string()),
    })?;
    match read_tx.open_table(clankers_ucan::revocation::AUTH_TOKENS_TABLE) {
        Ok(table) => {
            let mut count = 0;
            let iter = table.iter().map_err(|e| crate::error::Error::Io {
                source: std::io::Error::other(e.to_string()),
            })?;
            for entry in iter {
                let (key, value) = match entry {
                    Ok(kv) => kv,
                    Err(e) => {
                        eprintln!("  (read error: {})", e);
                        continue;
                    }
                };
                let hash_hex = key.value().to_string();
                let encoded = value.value().to_vec();
                count += 1;
                print_credential_list_entry(&hash_hex, &encoded);
            }
            if count == 0 {
                println!("No tokens issued. Create one with: clankers token create");
            } else {
                println!("\n{} token(s) total.", count);
            }
        }
        Err(_) => {
            println!("No tokens issued. Create one with: clankers token create");
        }
    }
    Ok(())
}

/// Print a single credential entry for the list command.
fn print_credential_list_entry(hash_hex: &str, encoded: &[u8]) {
    match clankers_ucan::Credential::decode(encoded) {
        Ok(cred) => {
            let now =
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            let status = if cred.token.expires_at < now { "expired" } else { "valid" };
            let expires = chrono::DateTime::from_timestamp(cred.token.expires_at as i64, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| "?".to_string());
            let caps: Vec<&str> = cred.token.capabilities.iter().map(cap_short_name).collect();
            let chain = if cred.proofs.is_empty() {
                String::new()
            } else {
                format!(" chain={}", cred.proofs.len())
            };
            println!(
                "  {}  {} | {} | {} | depth={}{}",
                &hash_hex[..16],
                status,
                expires,
                caps.join(","),
                cred.token.delegation_depth,
                chain,
            );
        }
        Err(e) => {
            println!("  {}  (decode error: {})", &hash_hex[..16], e);
        }
    }
}

/// Short display name for a capability.
fn cap_short_name(cap: &clankers_ucan::Capability) -> &'static str {
    match cap {
        clankers_ucan::Capability::Prompt => "prompt",
        clankers_ucan::Capability::ToolUse { .. } => "tools",
        clankers_ucan::Capability::ShellExecute { .. } => "shell",
        clankers_ucan::Capability::FileAccess { .. } => "files",
        clankers_ucan::Capability::BotCommand { .. } => "bot-cmd",
        clankers_ucan::Capability::SessionManage => "session",
        clankers_ucan::Capability::ModelSwitch => "model",
        clankers_ucan::Capability::Delegate => "delegate",
    }
}

/// Revoke a token by hash or base64.
fn handle_revoke(redb_db: &std::sync::Arc<redb::Database>, hash: &str) -> Result<()> {
    use clankers_ucan::RevocationStore;

    let token_hash: [u8; 32] = if hash.len() == 64 {
        let bytes = hex::decode(hash).map_err(|e| crate::error::Error::Config {
            message: format!("Invalid hex hash: {}", e),
        })?;
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        arr
    } else {
        match clankers_ucan::Credential::from_base64(hash) {
            Ok(cred) => cred.token.hash().map_err(|e| crate::error::Error::Config {
                message: format!("Failed to hash token: {}", e),
            })?,
            Err(_) => {
                return Err(crate::error::Error::Config {
                    message: "Invalid input: expected 64-char hex hash or base64 token".to_string(),
                });
            }
        }
    };

    let hash_hex = hex::encode(token_hash);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);

    let store = clankers_ucan::RedbRevocationStore::new(redb_db.clone()).map_err(|e| crate::error::Error::Config {
        message: format!("Failed to initialize revocation store: {}", e),
    })?;
    store.revoke(token_hash, now);

    println!("Token revoked: {}", &hash_hex[..16]);
    println!("The token will be rejected on all future verification checks.");
    Ok(())
}

/// Display detailed credential info.
fn handle_info(token_b64: &str) -> Result<()> {
    let cred = clankers_ucan::Credential::from_base64(token_b64).map_err(|e| crate::error::Error::Config {
        message: format!("Failed to decode credential: {}", e),
    })?;

    let hash = hex::encode(cred.token.hash().unwrap_or([0u8; 32]));
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let expired = cred.token.expires_at < now;

    println!("Credential Info:");
    println!("  Version:    {}", cred.token.version);
    println!("  Issuer:     {}", cred.token.issuer);
    println!("  Audience:   {:?}", cred.token.audience);
    println!("  Hash:       {}", hash);
    println!("  Issued:     {}", format_timestamp(cred.token.issued_at, "%Y-%m-%d %H:%M:%S UTC"),);
    println!(
        "  Expires:    {} {}",
        format_timestamp(cred.token.expires_at, "%Y-%m-%d %H:%M:%S UTC"),
        if expired { "(EXPIRED)" } else { "" }
    );
    println!("  Depth:      {}", cred.token.delegation_depth);
    if let Some(proof) = cred.token.proof {
        println!("  Parent:     {}", hex::encode(proof));
    }
    if let Some(nonce) = cred.token.nonce {
        println!("  Nonce:      {}", hex::encode(nonce));
    }
    println!("  Capabilities:");
    for cap in &cred.token.capabilities {
        print_capability_detail(cap);
    }
    if !cred.proofs.is_empty() {
        println!("  Proof chain ({} token(s)):", cred.proofs.len());
        for (i, proof_token) in cred.proofs.iter().enumerate() {
            let proof_hash = hex::encode(proof_token.hash().unwrap_or([0u8; 32]));
            println!(
                "    [{}] issuer={} depth={} hash={}",
                i,
                proof_token.issuer.fmt_short(),
                proof_token.delegation_depth,
                &proof_hash[..16],
            );
        }
    }
    Ok(())
}

/// Format a unix timestamp for display.
fn format_timestamp(ts: u64, fmt: &str) -> String {
    chrono::DateTime::from_timestamp(ts as i64, 0)
        .map(|dt| dt.format(fmt).to_string())
        .unwrap_or_else(|| "?".to_string())
}

/// Print a single capability with detail.
fn print_capability_detail(cap: &clankers_ucan::Capability) {
    match cap {
        clankers_ucan::Capability::Prompt => println!("    - Prompt"),
        clankers_ucan::Capability::ToolUse { tool_pattern } => {
            println!("    - ToolUse: {}", tool_pattern);
        }
        clankers_ucan::Capability::ShellExecute {
            command_pattern,
            working_dir,
        } => {
            println!("    - ShellExecute: {} (wd: {})", command_pattern, working_dir.as_deref().unwrap_or("any"));
        }
        clankers_ucan::Capability::FileAccess { prefix, read_only } => {
            let mode = if *read_only { "read-only" } else { "read-write" };
            println!("    - FileAccess: {} ({})", prefix, mode);
        }
        clankers_ucan::Capability::BotCommand { command_pattern } => {
            println!("    - BotCommand: {}", command_pattern);
        }
        clankers_ucan::Capability::SessionManage => println!("    - SessionManage"),
        clankers_ucan::Capability::ModelSwitch => println!("    - ModelSwitch"),
        clankers_ucan::Capability::Delegate => println!("    - Delegate"),
    }
}
