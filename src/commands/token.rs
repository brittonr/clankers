//! Token command handlers for UCAN capability token management.

use crate::cli::TokenAction;
use crate::commands::CommandContext;
use crate::error::Result;
use crate::util::parsing::parse_duration;

/// Run the token subcommand.
pub async fn run(ctx: &CommandContext, action: TokenAction) -> Result<()> {
    use clankers_auth::RevocationStore;
    use redb::ReadableTable;

    let identity_path = crate::modes::rpc::iroh::identity_path(&ctx.paths);
    let identity = crate::modes::rpc::iroh::Identity::load_or_generate(&identity_path);

    // Open redb directly for token storage (bypassing Db wrapper)
    let db_path = ctx.paths.global_config_dir.join("clankers.db");
    std::fs::create_dir_all(&ctx.paths.global_config_dir).ok();
    let redb_db = std::sync::Arc::new(
        redb::Database::create(&db_path).map_err(|e| crate::error::Error::Io {
            source: std::io::Error::new(std::io::ErrorKind::Other, e.to_string()),
        })?,
    );
    // Ensure auth tables exist
    {
        let tx = redb_db.begin_write().map_err(|e| crate::error::Error::Io {
            source: std::io::Error::new(std::io::ErrorKind::Other, e.to_string()),
        })?;
        let _ = tx.open_table(clankers_auth::revocation::AUTH_TOKENS_TABLE);
        let _ = tx.open_table(clankers_auth::revocation::REVOKED_TOKENS_TABLE);
        tx.commit().map_err(|e| crate::error::Error::Io {
            source: std::io::Error::new(std::io::ErrorKind::Other, e.to_string()),
        })?;
    }

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
            use clankers_auth::{Capability, CapabilityToken, TokenBuilder};

            // Parse duration string
            let lifetime = parse_duration(&expire).ok_or_else(|| crate::error::Error::Config {
                message: format!(
                    "Invalid duration: '{}'. Examples: 1h, 24h, 7d, 30d, 365d",
                    expire
                ),
            })?;

            // If delegating from a parent token
            let parent: Option<CapabilityToken> = from
                .as_ref()
                .map(|parent_b64| {
                    CapabilityToken::from_base64(parent_b64).map_err(|e| crate::error::Error::Config {
                        message: format!("Invalid parent token: {}", e),
                    })
                })
                .transpose()?;

            let mut builder = if let Some(parent) = parent {
                // Delegated token — use daemon's key as issuer
                TokenBuilder::new(identity.secret_key.clone()).delegated_from(parent)
            } else {
                TokenBuilder::new(identity.secret_key.clone())
            };

            // Set audience if specified
            if let Some(ref pubkey_str) = audience_key {
                let pubkey: iroh::PublicKey = pubkey_str.parse().map_err(|e| crate::error::Error::Config {
                    message: format!("Invalid public key: {}", e),
                })?;
                builder = builder.for_key(pubkey);
            }

            // Build capabilities
            if root {
                // Full-access root token
                builder = builder
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
                    .with_capability(Capability::Delegate);
            } else {
                // Always include Prompt (base capability)
                builder = builder.with_capability(Capability::Prompt);

                // Tool access
                if read_only {
                    builder = builder.with_capability(Capability::ToolUse {
                        tool_pattern: "read,grep,find,ls".into(),
                    });
                } else if let Some(ref tool_list) = tools {
                    builder = builder.with_capability(Capability::ToolUse {
                        tool_pattern: tool_list.clone(),
                    });
                }

                // Shell access
                if let Some(ref pattern) = shell {
                    builder = builder.with_capability(Capability::ShellExecute {
                        command_pattern: pattern.clone(),
                        working_dir: shell_wd.clone(),
                    });
                }

                // File access
                if let Some(ref prefix) = file_prefix {
                    builder = builder.with_capability(Capability::FileAccess {
                        prefix: prefix.clone(),
                        read_only: file_read_only,
                    });
                }

                // Bot commands
                if let Some(ref cmds) = bot_commands {
                    builder = builder.with_capability(Capability::BotCommand {
                        command_pattern: cmds.clone(),
                    });
                }

                if session_manage {
                    builder = builder.with_capability(Capability::SessionManage);
                }
                if model_switch {
                    builder = builder.with_capability(Capability::ModelSwitch);
                }
                if delegate {
                    builder = builder.with_capability(Capability::Delegate);
                }
            }

            builder = builder.with_lifetime(lifetime).with_random_nonce();

            let token = builder.build().map_err(|e| crate::error::Error::Config {
                message: format!("Failed to create token: {}", e),
            })?;

            let b64 = token.to_base64().map_err(|e| crate::error::Error::Config {
                message: format!("Failed to encode token: {}", e),
            })?;

            // Store the token in redb for tracking
            let hash = token.hash();
            let hash_hex = hex::encode(hash);
            let encoded = token.encode().unwrap_or_default();
            if let Ok(tx) = redb_db.begin_write() {
                {
                    if let Ok(mut table) = tx.open_table(clankers_auth::revocation::AUTH_TOKENS_TABLE) {
                        let _ = table.insert(hash_hex.as_str(), encoded.as_slice());
                    }
                }
                if let Err(e) = tx.commit() {
                    eprintln!("Warning: failed to store token in database: {}", e);
                }
            }

            // Print token info
            eprintln!("Token created:");
            eprintln!("  Issuer:  {}", token.issuer.fmt_short());
            eprintln!("  Hash:    {}", &hash_hex[..16]);
            eprintln!(
                "  Expires: {}",
                chrono::DateTime::from_timestamp(token.expires_at as i64, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            );
            let cap_strs: Vec<String> = token.capabilities.iter().map(|c| format!("{:?}", c)).collect();
            eprintln!("  Caps:    {}", cap_strs.join(", "));
            if token.delegation_depth > 0 {
                eprintln!("  Depth:   {}", token.delegation_depth);
            }
            eprintln!();
            // Print the raw token to stdout (for piping)
            println!("{}", b64);
        }
        TokenAction::List => {
            let read_tx = redb_db.begin_read().map_err(|e| crate::error::Error::Io {
                source: std::io::Error::new(std::io::ErrorKind::Other, e.to_string()),
            })?;
            match read_tx.open_table(clankers_auth::revocation::AUTH_TOKENS_TABLE) {
                Ok(table) => {
                    let mut count = 0;
                    let iter = table.iter().map_err(|e| crate::error::Error::Io {
                        source: std::io::Error::new(std::io::ErrorKind::Other, e.to_string()),
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
                        match clankers_auth::CapabilityToken::decode(&encoded) {
                            Ok(token) => {
                                let expired = {
                                    let now = std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .map(|d| d.as_secs())
                                        .unwrap_or(0);
                                    token.expires_at < now
                                };
                                let status = if expired { "expired" } else { "valid" };
                                let expires = chrono::DateTime::from_timestamp(token.expires_at as i64, 0)
                                    .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                                    .unwrap_or_else(|| "?".to_string());
                                let caps: Vec<&str> = token
                                    .capabilities
                                    .iter()
                                    .map(|c| match c {
                                        clankers_auth::Capability::Prompt => "prompt",
                                        clankers_auth::Capability::ToolUse { .. } => "tools",
                                        clankers_auth::Capability::ShellExecute { .. } => "shell",
                                        clankers_auth::Capability::FileAccess { .. } => "files",
                                        clankers_auth::Capability::BotCommand { .. } => "bot-cmd",
                                        clankers_auth::Capability::SessionManage => "session",
                                        clankers_auth::Capability::ModelSwitch => "model",
                                        clankers_auth::Capability::Delegate => "delegate",
                                    })
                                    .collect();
                                println!(
                                    "  {}  {} | {} | {} | depth={}",
                                    &hash_hex[..16],
                                    status,
                                    expires,
                                    caps.join(","),
                                    token.delegation_depth,
                                );
                            }
                            Err(e) => {
                                println!("  {}  (decode error: {})", &hash_hex[..16], e);
                            }
                        }
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
        }
        TokenAction::Revoke { hash } => {
            // Determine if this is a hex hash or a base64 token
            let token_hash: [u8; 32] = if hash.len() == 64 {
                // Looks like hex-encoded hash
                let bytes = hex::decode(&hash).map_err(|e| crate::error::Error::Config {
                    message: format!("Invalid hex hash: {}", e),
                })?;
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&bytes);
                arr
            } else {
                // Try to decode as base64 token
                match clankers_auth::CapabilityToken::from_base64(&hash) {
                    Ok(token) => token.hash(),
                    Err(_) => {
                        return Err(crate::error::Error::Config {
                            message: "Invalid input: expected 64-char hex hash or base64 token".to_string(),
                        });
                    }
                }
            };

            let hash_hex = hex::encode(token_hash);
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);

            // Add to revoked_tokens table
            let store = clankers_auth::RedbRevocationStore::new(redb_db.clone()).map_err(|e| {
                crate::error::Error::Config {
                    message: format!("Failed to initialize revocation store: {}", e),
                }
            })?;
            store.revoke(token_hash, now);

            println!("Token revoked: {}", &hash_hex[..16]);
            println!("The token will be rejected on all future verification checks.");
        }
        TokenAction::Info { token: token_b64 } => {
            let token = clankers_auth::CapabilityToken::from_base64(&token_b64).map_err(|e| {
                crate::error::Error::Config {
                    message: format!("Failed to decode token: {}", e),
                }
            })?;

            let hash = hex::encode(token.hash());
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let expired = token.expires_at < now;

            println!("Token Info:");
            println!("  Version:    {}", token.version);
            println!("  Issuer:     {}", token.issuer);
            println!("  Audience:   {:?}", token.audience);
            println!("  Hash:       {}", hash);
            println!(
                "  Issued:     {}",
                chrono::DateTime::from_timestamp(token.issued_at as i64, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                    .unwrap_or_else(|| "?".to_string())
            );
            println!(
                "  Expires:    {} {}",
                chrono::DateTime::from_timestamp(token.expires_at as i64, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                    .unwrap_or_else(|| "?".to_string()),
                if expired { "(EXPIRED)" } else { "" }
            );
            println!("  Depth:      {}", token.delegation_depth);
            if let Some(proof) = token.proof {
                println!("  Parent:     {}", hex::encode(proof));
            }
            if let Some(nonce) = token.nonce {
                println!("  Nonce:      {}", hex::encode(nonce));
            }
            println!("  Capabilities:");
            for cap in &token.capabilities {
                match cap {
                    clankers_auth::Capability::Prompt => println!("    - Prompt"),
                    clankers_auth::Capability::ToolUse { tool_pattern } => {
                        println!("    - ToolUse: {}", tool_pattern);
                    }
                    clankers_auth::Capability::ShellExecute {
                        command_pattern,
                        working_dir,
                    } => {
                        println!(
                            "    - ShellExecute: {} (wd: {})",
                            command_pattern,
                            working_dir.as_deref().unwrap_or("any")
                        );
                    }
                    clankers_auth::Capability::FileAccess { prefix, read_only } => {
                        let mode = if *read_only { "read-only" } else { "read-write" };
                        println!("    - FileAccess: {} ({})", prefix, mode);
                    }
                    clankers_auth::Capability::BotCommand { command_pattern } => {
                        println!("    - BotCommand: {}", command_pattern);
                    }
                    clankers_auth::Capability::SessionManage => println!("    - SessionManage"),
                    clankers_auth::Capability::ModelSwitch => println!("    - ModelSwitch"),
                    clankers_auth::Capability::Delegate => println!("    - Delegate"),
                }
            }
        }
    }

    Ok(())
}
