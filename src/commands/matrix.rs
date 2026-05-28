use std::io::Read;

use crate::cli::MatrixAction;
use crate::commands::CommandContext;
use crate::error::Error;
use crate::error::Result;

pub async fn run(ctx: &CommandContext, action: MatrixAction) -> Result<()> {
    match action {
        MatrixAction::Status => run_status(ctx),
        MatrixAction::ConfigureToken {
            homeserver,
            user_id,
            device_id,
            access_token_env,
            access_token_stdin,
            rooms,
            allowed_users,
            no_announce,
        } => run_configure_token(ctx, TokenConfigRequest {
            homeserver,
            user_id,
            device_id,
            access_token_env,
            access_token_stdin,
            rooms,
            allowed_users,
            no_announce,
        }),
        MatrixAction::Login {
            homeserver,
            username,
            password_env,
            password_stdin,
            device_name,
            rooms,
            allowed_users,
            no_announce,
        } => {
            run_login(ctx, LoginRequest {
                homeserver,
                username,
                password_env,
                password_stdin,
                device_name,
                rooms,
                allowed_users,
                no_announce,
            })
            .await
        }
    }
}

struct TokenConfigRequest {
    homeserver: String,
    user_id: String,
    device_id: String,
    access_token_env: String,
    access_token_stdin: bool,
    rooms: Vec<String>,
    allowed_users: Vec<String>,
    no_announce: bool,
}

struct LoginRequest {
    homeserver: String,
    username: String,
    password_env: String,
    password_stdin: bool,
    device_name: Option<String>,
    rooms: Vec<String>,
    allowed_users: Vec<String>,
    no_announce: bool,
}

fn matrix_config_path(ctx: &CommandContext) -> std::path::PathBuf {
    ctx.paths.global_config_dir.join("matrix.json")
}

fn run_status(ctx: &CommandContext) -> Result<()> {
    let path = matrix_config_path(ctx);
    println!("Matrix config: {}", path.display());
    let Some(config) = clankers_matrix::MatrixConfig::load(&path) else {
        println!("Status: missing");
        println!("Run `clankers matrix login ...` or `clankers matrix configure-token ...` first.");
        return Ok(());
    };

    println!("Status: configured");
    println!("Homeserver: {}", config.homeserver);
    println!("User ID: {}", empty_placeholder(&config.user_id));
    println!(
        "Device ID: {}",
        config.device_id.as_deref().map(redact_middle).unwrap_or_else(|| "missing".to_string())
    );
    println!(
        "Access token: {}",
        if config.access_token.is_some() {
            "present"
        } else {
            "missing"
        }
    );
    println!("Auto-join rooms: {}", config.auto_join_rooms.len());
    println!("Allowed users: {}", config.allowed_users.len());
    println!("Announce on join: {}", config.announce_on_join);
    Ok(())
}

fn run_configure_token(ctx: &CommandContext, request: TokenConfigRequest) -> Result<()> {
    let access_token = read_secret(&request.access_token_env, request.access_token_stdin, "access token")?;
    let path = matrix_config_path(ctx);
    let mut config = clankers_matrix::MatrixConfig::load(&path).unwrap_or_else(clankers_matrix::MatrixConfig::default);
    config.homeserver = request.homeserver;
    config.user_id = request.user_id;
    config.device_id = Some(request.device_id);
    config.access_token = Some(access_token);
    apply_bridge_options(&mut config, request.rooms, request.allowed_users, request.no_announce);
    save_config(&config, &path)?;
    println!("Saved Matrix config: {}", path.display());
    println!("Access token: present");
    Ok(())
}

async fn run_login(ctx: &CommandContext, request: LoginRequest) -> Result<()> {
    let password = read_secret(&request.password_env, request.password_stdin, "password")?;
    let path = matrix_config_path(ctx);
    let mut config = clankers_matrix::MatrixConfig::load(&path).unwrap_or_else(clankers_matrix::MatrixConfig::default);
    config.homeserver = request.homeserver;
    if let Some(device_name) = request.device_name {
        config.device_name = device_name;
    }
    apply_bridge_options(&mut config, request.rooms, request.allowed_users, request.no_announce);

    let store_path = config.resolve_store_path(&ctx.paths.global_config_dir);
    let mut client = clankers_matrix::MatrixClient::new(config, "clankers-daemon");
    client.login(&request.username, &password, &store_path).await.map_err(|error| Error::Config {
        message: format!("Matrix login failed: {error}"),
    })?;
    save_config(client.config(), &path)?;
    println!("Matrix login saved: {}", path.display());
    println!("User ID: {}", client.config().user_id);
    println!("Access token: present");
    Ok(())
}

fn apply_bridge_options(
    config: &mut clankers_matrix::MatrixConfig,
    rooms: Vec<String>,
    allowed_users: Vec<String>,
    no_announce: bool,
) {
    if !rooms.is_empty() {
        config.auto_join_rooms = rooms;
    }
    if !allowed_users.is_empty() {
        config.allowed_users = allowed_users;
    }
    if no_announce {
        config.announce_on_join = false;
    }
}

fn read_secret(env_name: &str, from_stdin: bool, label: &str) -> Result<String> {
    if from_stdin {
        let mut secret = String::new();
        std::io::stdin().read_to_string(&mut secret).map_err(|source| Error::Io { source })?;
        let secret = secret.trim().to_string();
        if secret.is_empty() {
            return Err(Error::Config {
                message: format!("Matrix {label} read from stdin was empty"),
            });
        }
        return Ok(secret);
    }

    let secret = std::env::var(env_name).map_err(|_| Error::Config {
        message: format!("Matrix {label} missing; set {env_name} or pass --{label}-stdin"),
    })?;
    let secret = secret.trim().to_string();
    if secret.is_empty() {
        return Err(Error::Config {
            message: format!("Matrix {label} in {env_name} was empty"),
        });
    }
    Ok(secret)
}

fn save_config(config: &clankers_matrix::MatrixConfig, path: &std::path::Path) -> Result<()> {
    config.save(path).map_err(|error| Error::Config {
        message: format!("failed to save Matrix config at {}: {error}", path.display()),
    })
}

fn empty_placeholder(value: &str) -> &str {
    if value.is_empty() { "missing" } else { value }
}

fn redact_middle(value: &str) -> String {
    let char_count = value.chars().count();
    if char_count <= 8 {
        return "<redacted>".to_string();
    }
    let prefix: String = value.chars().take(4).collect();
    let suffix: String = value.chars().skip(char_count.saturating_sub(4)).collect();
    format!("{prefix}…{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_middle_hides_short_values_and_keeps_edges_for_long_values() {
        assert_eq!(redact_middle("short"), "<redacted>");
        assert_eq!(redact_middle("DEVICE123456"), "DEVI…3456");
    }

    #[test]
    fn apply_bridge_options_only_overwrites_explicit_lists() {
        let mut config = clankers_matrix::MatrixConfig {
            auto_join_rooms: vec!["!old:example.org".to_string()],
            allowed_users: vec!["@old:example.org".to_string()],
            announce_on_join: true,
            ..Default::default()
        };

        apply_bridge_options(&mut config, Vec::new(), Vec::new(), false);
        assert_eq!(config.auto_join_rooms, vec!["!old:example.org".to_string()]);
        assert_eq!(config.allowed_users, vec!["@old:example.org".to_string()]);
        assert!(config.announce_on_join);

        apply_bridge_options(
            &mut config,
            vec!["!new:example.org".to_string()],
            vec!["@new:example.org".to_string()],
            true,
        );
        assert_eq!(config.auto_join_rooms, vec!["!new:example.org".to_string()]);
        assert_eq!(config.allowed_users, vec!["@new:example.org".to_string()]);
        assert!(!config.announce_on_join);
    }
}
