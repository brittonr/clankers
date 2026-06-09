//! Public UCAN credential command handlers.

use base64::Engine as _;
use clankers_util::parsing::parse_duration;
use redb::ReadableTable;

use crate::cli::TokenAction;
use crate::commands::CommandContext;
use crate::error::Result;

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
    let event_time_seconds = current_unix_time_seconds()?;
    let redb_db = open_auth_db(ctx, event_time_seconds)?;

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
            handle_create(&identity, &redb_db, &expire, audience_key, from, scope, event_time_seconds)
        }
        TokenAction::List => handle_list(&redb_db),
        TokenAction::Revoke { hash } => handle_revoke(&redb_db, &hash, event_time_seconds),
        TokenAction::Info { token } => handle_info(&token),
    }
}

fn open_auth_db(ctx: &CommandContext, event_time_seconds: u64) -> Result<std::sync::Arc<redb::Database>> {
    let db_path = ctx.paths.global_config_dir.join("clankers.db");
    std::fs::create_dir_all(&ctx.paths.global_config_dir).ok();
    let redb_db = std::sync::Arc::new(redb::Database::create(&db_path).map_err(|e| crate::error::Error::Io {
        source: std::io::Error::other(e.to_string()),
    })?);
    clankers_ucan::RedbPublicCredentialStore::new(std::sync::Arc::clone(&redb_db), event_time_seconds).map_err(|e| {
        crate::error::Error::Io {
            source: std::io::Error::other(e.to_string()),
        }
    })?;
    Ok(redb_db)
}

fn current_unix_time_seconds() -> Result<u64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|source| crate::error::Error::Config {
            message: format!("System clock is before Unix epoch: {source}"),
        })
}

fn handle_create(
    identity: &crate::modes::rpc::iroh::Identity,
    redb_db: &std::sync::Arc<redb::Database>,
    expire: &str,
    audience_key: Option<String>,
    from: Option<String>,
    scope: TokenScope,
    event_time_seconds: u64,
) -> Result<()> {
    let lifetime = parse_duration(expire).ok_or_else(|| crate::error::Error::Config {
        message: format!("Invalid duration: '{}'. Examples: 1h, 24h, 7d, 30d, 365d", expire),
    })?;
    let issuer = clankers_ucan::PublicUcanIssuer::from_iroh_secret_key(&identity.secret_key);
    let audience = match audience_key {
        Some(pubkey) => audience_from_iroh_public_key(&pubkey)?,
        None => issuer.audience().map_err(|e| crate::error::Error::Config { message: e.to_string() })?,
    };
    let capabilities = build_capabilities(&scope)?;

    let credential = if let Some(parent_b64) = from {
        let parent = clankers_ucan::PublicCredentialEnvelope::from_base64(parent_b64.as_str()).map_err(|e| {
            crate::error::Error::Config {
                message: format!("Invalid parent public UCAN credential: {e}"),
            }
        })?;
        issuer
            .issue_child_from_parent_at(&parent, audience, capabilities, lifetime, event_time_seconds)
            .map_err(|e| crate::error::Error::Config { message: e.to_string() })?
    } else {
        issuer
            .issue_root_credential_at(audience, capabilities, lifetime, event_time_seconds)
            .map_err(|e| crate::error::Error::Config { message: e.to_string() })?
    };

    let b64 = credential.to_base64().map_err(|e| crate::error::Error::Config {
        message: format!("Failed to encode credential: {e}"),
    })?;

    store_credential(redb_db, &credential, event_time_seconds)?;
    print_credential_summary(&credential);
    println!("{}", b64);
    Ok(())
}

fn audience_from_iroh_public_key(pubkey_str: &str) -> Result<ucan::AudienceDid> {
    let pubkey: iroh::PublicKey = pubkey_str.parse().map_err(|e| crate::error::Error::Config {
        message: format!("Invalid public key: {e}"),
    })?;
    let did_bytes =
        ucan::verified::encode_ed25519_did_key(pubkey.as_bytes()).map_err(|e| crate::error::Error::Config {
            message: format!("Failed to encode audience DID: {e}"),
        })?;
    let did = String::from_utf8(did_bytes).map_err(|e| crate::error::Error::Config {
        message: format!("Failed to encode audience DID as UTF-8: {e}"),
    })?;
    ucan::AudienceDid::new(did).map_err(|e| crate::error::Error::Config { message: e.to_string() })
}

fn build_capabilities(scope: &TokenScope) -> Result<ucan::CapabilitySet> {
    let documents = if scope.root {
        root_capabilities()
    } else {
        scoped_capabilities(scope)
    };
    ucan::CapabilitySet::new(documents).map_err(|e| crate::error::Error::Config {
        message: format!("Invalid public UCAN capability set: {e}"),
    })
}

fn root_capabilities() -> Vec<ucan::CapabilityDocument> {
    vec![
        cap("clankers:daemon/", "session/create"),
        cap("clankers:session/", "session/attach"),
        cap("clankers:session/", "session/prompt"),
        cap("clankers:session/", "session/manage"),
        cap("clankers:tool/", "tool/use"),
        cap("clankers:file:", "file/read"),
        cap("clankers:file:", "file/write"),
        cap("clankers:shell:", "shell/execute"),
        cap("clankers:process/", "process/observe"),
        cap("clankers:process/", "process/start"),
        cap("clankers:process/", "process/mutate"),
        cap("clankers:process/", "process/stdin"),
        cap("clankers:process/", "process/logs"),
        cap("clankers:model/", "model/use"),
    ]
}

fn scoped_capabilities(scope: &TokenScope) -> Vec<ucan::CapabilityDocument> {
    let mut capabilities = vec![cap("clankers:session/", "session/prompt")];
    if scope.session_manage {
        capabilities.push(cap("clankers:daemon/", "session/create"));
        capabilities.push(cap("clankers:session/", "session/attach"));
        capabilities.push(cap("clankers:session/", "session/manage"));
    }
    if scope.model_switch {
        capabilities.push(cap("clankers:tool/switch_model", "tool/use"));
        capabilities.push(cap("clankers:model/", "model/use"));
    }
    if scope.read_only {
        add_tool_caps(&mut capabilities, "read,grep,find,ls");
        capabilities.push(cap(file_resource_prefix(scope.file_prefix.as_deref()).as_str(), "file/read"));
    } else if let Some(tools) = scope.tools.as_deref() {
        add_tool_caps(&mut capabilities, tools);
    }
    if let Some(prefix) = scope.file_prefix.as_deref() {
        capabilities.push(cap(file_resource_prefix(Some(prefix)).as_str(), "file/read"));
        if !scope.file_read_only {
            capabilities.push(cap(file_resource_prefix(Some(prefix)).as_str(), "file/write"));
        }
    }
    if scope.shell.is_some() {
        capabilities.push(cap("clankers:tool/bash", "tool/use"));
        capabilities.push(cap(shell_resource_prefix(scope.shell_wd.as_deref()).as_str(), "shell/execute"));
    }
    if scope.bot_commands.is_some() {
        capabilities.push(cap("clankers:tool/matrix-bot-command", "tool/use"));
    }
    if scope.delegate {
        capabilities.push(cap("clankers:session/", "session/attach"));
    }
    capabilities
}

fn add_tool_caps(capabilities: &mut Vec<ucan::CapabilityDocument>, tools: &str) {
    if tools.trim() == "*" {
        capabilities.push(cap("clankers:tool/", "tool/use"));
        return;
    }
    for tool in tools.split(',').map(str::trim).filter(|tool| !tool.is_empty()) {
        capabilities.push(cap(format!("clankers:tool/{}", encode_resource_segment(tool)).as_str(), "tool/use"));
    }
}

fn file_resource_prefix(prefix: Option<&str>) -> String {
    let Some(prefix) = prefix else {
        return "clankers:file:".to_owned();
    };
    match clankers_ucan::EffectCapability::new(clankers_ucan::EffectKind::FileRead, prefix) {
        Ok(capability) => ensure_trailing_slash_or_colon(capability.resource()),
        Err(_) => "clankers:file:".to_owned(),
    }
}

fn shell_resource_prefix(cwd: Option<&str>) -> String {
    match cwd {
        Some(cwd) => format!("clankers:shell:{}", encode_resource_segment(cwd)),
        None => "clankers:shell:".to_owned(),
    }
}

fn ensure_trailing_slash_or_colon(input: &str) -> String {
    if input.ends_with('/') || input.ends_with(':') {
        input.to_owned()
    } else {
        format!("{input}/")
    }
}

fn cap(resource: &str, ability: &str) -> ucan::CapabilityDocument {
    ucan::CapabilityDocument::new(resource.to_owned(), ability.to_owned()).expect("static capability is valid")
}

fn encode_resource_segment(input: &str) -> String {
    use std::fmt::Write;

    let mut encoded = String::new();
    for byte in input.as_bytes() {
        if matches!(*byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_') {
            encoded.push(char::from(*byte));
        } else {
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

fn store_credential(
    redb_db: &std::sync::Arc<redb::Database>,
    cred: &clankers_ucan::PublicCredentialEnvelope,
    event_time_seconds: u64,
) -> Result<()> {
    let store = clankers_ucan::RedbPublicCredentialStore::new(std::sync::Arc::clone(redb_db), event_time_seconds).map_err(|e| {
        crate::error::Error::Io {
            source: std::io::Error::other(e.to_string()),
        }
    })?;
    store
        .store_credential(cred.token_reference().to_string().as_str(), cred)
        .map_err(|e| crate::error::Error::Io {
            source: std::io::Error::other(e.to_string()),
        })
}

fn print_credential_summary(cred: &clankers_ucan::PublicCredentialEnvelope) {
    eprintln!("Public UCAN credential created:");
    eprintln!("  Token ref: {}", cred.token_reference());
    eprintln!("  Audience:  {}", cred.audience());
    eprintln!(
        "  Roots:     {}",
        cred.trusted_roots().iter().map(ToString::to_string).collect::<Vec<_>>().join(", ")
    );
    eprintln!("  Proofs:    {}", cred.proofs().len());
    eprintln!();
}

fn handle_list(redb_db: &std::sync::Arc<redb::Database>) -> Result<()> {
    let read_tx = redb_db.begin_read().map_err(|e| crate::error::Error::Io {
        source: std::io::Error::other(e.to_string()),
    })?;
    match read_tx.open_table(clankers_ucan::public_store::PUBLIC_AUTH_TOKENS_TABLE) {
        Ok(table) => {
            let iter = table.iter().map_err(|e| crate::error::Error::Io {
                source: std::io::Error::other(e.to_string()),
            })?;
            let mut count = 0;
            for entry in iter {
                let (key, value) = match entry {
                    Ok(kv) => kv,
                    Err(e) => {
                        eprintln!("  (read error: {e})");
                        continue;
                    }
                };
                count += 1;
                print_credential_list_entry(key.value(), value.value());
            }
            if count == 0 {
                println!("No public UCAN credentials issued. Create one with: clankers token create");
            } else {
                println!("\n{count} public UCAN credential(s) total.");
            }
        }
        Err(_) => println!("No public UCAN credentials issued. Create one with: clankers token create"),
    }
    Ok(())
}

fn print_credential_list_entry(key: &str, encoded: &[u8]) {
    match clankers_ucan::PublicCredentialEnvelope::decode(encoded) {
        Ok(cred) => {
            println!(
                "  {} | audience={} | proofs={} | replay={}",
                key,
                cred.audience(),
                cred.proofs().len(),
                cred.replay_id().unwrap_or("-")
            );
        }
        Err(error) => println!("  {key} (decode error: {error})"),
    }
}

fn handle_revoke(redb_db: &std::sync::Arc<redb::Database>, input: &str, event_time_seconds: u64) -> Result<()> {
    let reference = match clankers_ucan::PublicCredentialEnvelope::from_base64(input) {
        Ok(credential) => credential.token_reference(),
        Err(_) => proof_reference_from_input(input)?,
    };
    let store = clankers_ucan::RedbPublicCredentialStore::new(std::sync::Arc::clone(redb_db), event_time_seconds).map_err(|e| {
        crate::error::Error::Io {
            source: std::io::Error::other(e.to_string()),
        }
    })?;
    store.revoke_reference(&reference).map_err(|e| crate::error::Error::Io {
        source: std::io::Error::other(e.to_string()),
    })?;
    println!("Public UCAN reference revoked: {reference}");
    Ok(())
}

fn proof_reference_from_input(input: &str) -> Result<ucan::ProofReference> {
    let bytes = if input.len() == 64 && input.chars().all(|c| c.is_ascii_hexdigit()) {
        hex::decode(input).map_err(|e| crate::error::Error::Config {
            message: format!("Invalid hex reference: {e}"),
        })?
    } else {
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(input)
            .map_err(|e| crate::error::Error::Config {
                message: format!("Invalid proof reference or credential: {e}"),
            })?
    };
    Ok(ucan::ProofReference::new(bytes))
}

fn handle_info(token_b64: &str) -> Result<()> {
    let cred =
        clankers_ucan::PublicCredentialEnvelope::from_base64(token_b64).map_err(|e| crate::error::Error::Config {
            message: format!("Failed to decode public UCAN credential: {e}"),
        })?;
    println!("Public UCAN Credential Info:");
    println!("  Schema:      {}", cred.schema());
    println!("  Token ref:   {}", cred.token_reference());
    println!("  Audience:    {}", cred.audience());
    println!("  Proofs:      {}", cred.proofs().len());
    println!("  Replay ID:   {}", cred.replay_id().unwrap_or("-"));
    println!("  Trusted roots:");
    for root in cred.trusted_roots() {
        println!("    {root}");
    }
    Ok(())
}
