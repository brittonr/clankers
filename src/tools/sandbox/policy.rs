//! Environment sanitization for bash child processes.
//!
//! Path policy is now in `clankers-util::path_policy` — this module
//! only handles environment variable scrubbing.

/// Environment variables that should be stripped from bash child processes.
///
/// These contain or provide access to secrets. The heuristic suffix check
/// in `sanitized_env()` catches most custom ones; this list handles the
/// well-known variables that don't follow naming conventions.
static SCRUBBED_ENV_VARS: &[&str] = &[
    // Cloud
    "AWS_ACCESS_KEY_ID",
    "AWS_SECRET_ACCESS_KEY",
    "AWS_SESSION_TOKEN",
    "AWS_SECURITY_TOKEN",
    "AZURE_CLIENT_SECRET",
    "AZURE_TENANT_ID",
    "AZURE_CLIENT_ID",
    "GOOGLE_APPLICATION_CREDENTIALS",
    // CI/CD
    "GITHUB_TOKEN",
    "GH_TOKEN",
    "GITLAB_TOKEN",
    "GITLAB_PRIVATE_TOKEN",
    "CIRCLE_TOKEN",
    "CODECOV_TOKEN",
    // LLM keys (don't let bash leak our own keys)
    "ANTHROPIC_API_KEY",
    "OPENAI_API_KEY",
    "OPENROUTER_API_KEY",
    "GROQ_API_KEY",
    "DEEPSEEK_API_KEY",
    // Package registries
    "NPM_TOKEN",
    "NUGET_API_KEY",
    "PYPI_TOKEN",
    // Infra
    "VAULT_TOKEN",
    "CONSUL_HTTP_TOKEN",
    "DOCKER_PASSWORD",
    "DOCKER_AUTH_CONFIG",
    // SSH agent
    "SSH_AUTH_SOCK",
    "SSH_AGENT_PID",
    // Databases
    "DATABASE_URL",
    "REDIS_URL",
    "MONGODB_URI",
    "MONGO_URL",
];

/// Build a sanitized copy of the environment for bash child processes.
///
/// Removes known secret variables and anything matching heuristic
/// patterns (*_SECRET, *_TOKEN, *_PASSWORD, *_API_KEY, etc.).
/// Sets `CLANKERS_SANDBOX=1` so scripts can detect sandboxed execution.
pub fn sanitized_env() -> Vec<(String, String)> {
    let scrubbed: std::collections::HashSet<&str> = SCRUBBED_ENV_VARS.iter().copied().collect();

    let mut env: Vec<(String, String)> = std::env::vars()
        .filter(|(key, _)| {
            if scrubbed.contains(key.as_str()) {
                return false;
            }
            let upper = key.to_uppercase();
            !(upper.ends_with("_SECRET")
                || upper.ends_with("_TOKEN")
                || upper.ends_with("_PASSWORD")
                || upper.ends_with("_CREDENTIALS")
                || upper.ends_with("_API_KEY")
                || upper.ends_with("_APIKEY")
                || upper.ends_with("_PRIVATE_KEY"))
        })
        .collect();

    env.push(("CLANKERS_SANDBOX".to_string(), "1".to_string()));
    env
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Env scrubbing ───────────────────────────────────────────────

    #[test]
    fn scrubs_known_secrets() {
        unsafe {
            std::env::set_var("TEST_CLANKERS_API_KEY", "secret");
        }
        let env = sanitized_env();
        assert!(!env.iter().any(|(k, _)| k == "TEST_CLANKERS_API_KEY"));
        unsafe {
            std::env::remove_var("TEST_CLANKERS_API_KEY");
        }
    }

    #[test]
    fn scrubs_heuristic_patterns() {
        unsafe {
            std::env::set_var("MY_APP_SECRET", "x");
        }
        let env = sanitized_env();
        assert!(!env.iter().any(|(k, _)| k == "MY_APP_SECRET"));
        unsafe {
            std::env::remove_var("MY_APP_SECRET");
        }
    }

    #[test]
    fn preserves_normal_vars() {
        let env = sanitized_env();
        assert!(env.iter().any(|(k, _)| k == "PATH"));
    }

    #[test]
    fn sets_sandbox_marker() {
        let env = sanitized_env();
        assert!(env.iter().any(|(k, v)| k == "CLANKERS_SANDBOX" && v == "1"));
    }

    #[test]
    fn scrubs_token_suffix() {
        unsafe {
            std::env::set_var("CLANKERS_TEST_MY_TOKEN", "t");
        }
        let env = sanitized_env();
        assert!(!env.iter().any(|(k, _)| k == "CLANKERS_TEST_MY_TOKEN"));
        unsafe {
            std::env::remove_var("CLANKERS_TEST_MY_TOKEN");
        }
    }

    #[test]
    fn scrubs_password_suffix() {
        unsafe {
            std::env::set_var("CLANKERS_TEST_DB_PASSWORD", "p");
        }
        let env = sanitized_env();
        assert!(!env.iter().any(|(k, _)| k == "CLANKERS_TEST_DB_PASSWORD"));
        unsafe {
            std::env::remove_var("CLANKERS_TEST_DB_PASSWORD");
        }
    }

    #[test]
    fn scrubs_credentials_suffix() {
        unsafe {
            std::env::set_var("CLANKERS_TEST_CLOUD_CREDENTIALS", "c");
        }
        let env = sanitized_env();
        assert!(!env.iter().any(|(k, _)| k == "CLANKERS_TEST_CLOUD_CREDENTIALS"));
        unsafe {
            std::env::remove_var("CLANKERS_TEST_CLOUD_CREDENTIALS");
        }
    }

    #[test]
    fn scrubs_private_key_suffix() {
        unsafe {
            std::env::set_var("CLANKERS_TEST_SIGNING_PRIVATE_KEY", "k");
        }
        let env = sanitized_env();
        assert!(!env.iter().any(|(k, _)| k == "CLANKERS_TEST_SIGNING_PRIVATE_KEY"));
        unsafe {
            std::env::remove_var("CLANKERS_TEST_SIGNING_PRIVATE_KEY");
        }
    }

    #[test]
    fn scrubs_explicit_anthropic_key() {
        unsafe {
            std::env::set_var("ANTHROPIC_API_KEY", "sk-test");
        }
        let env = sanitized_env();
        assert!(!env.iter().any(|(k, _)| k == "ANTHROPIC_API_KEY"));
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
        }
    }

    #[test]
    fn scrubs_explicit_github_token() {
        unsafe {
            std::env::set_var("GITHUB_TOKEN", "ghp_test");
        }
        let env = sanitized_env();
        assert!(!env.iter().any(|(k, _)| k == "GITHUB_TOKEN"));
        unsafe {
            std::env::remove_var("GITHUB_TOKEN");
        }
    }

    #[test]
    fn scrubs_ssh_auth_sock() {
        unsafe {
            std::env::set_var("SSH_AUTH_SOCK", "/tmp/agent.123");
        }
        let env = sanitized_env();
        assert!(!env.iter().any(|(k, _)| k == "SSH_AUTH_SOCK"));
        unsafe {
            std::env::remove_var("SSH_AUTH_SOCK");
        }
    }

    #[test]
    fn scrubs_mixed_case_suffix() {
        unsafe {
            std::env::set_var("clankers_test_My_Secret", "s");
        }
        let env = sanitized_env();
        assert!(!env.iter().any(|(k, _)| k == "clankers_test_My_Secret"));
        unsafe {
            std::env::remove_var("clankers_test_My_Secret");
        }
    }

    #[test]
    fn preserves_home() {
        let env = sanitized_env();
        assert!(env.iter().any(|(k, _)| k == "HOME"));
    }

    #[test]
    fn preserves_user() {
        if std::env::var("USER").is_ok() {
            let env = sanitized_env();
            assert!(env.iter().any(|(k, _)| k == "USER"));
        }
    }
}
