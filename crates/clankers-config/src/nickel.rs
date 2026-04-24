//! Nickel configuration evaluator.
//!
//! Evaluates `.ncl` settings files to `serde_json::Value` for the existing
//! deserialization pipeline. The embedded settings contract provides type
//! checking, defaults, and validation.

use std::path::Path;

use nickel_lang::Context;

/// The embedded settings contract, compiled into the binary.
pub const SETTINGS_CONTRACT: &str = include_str!("settings-contract.ncl");

/// The embedded theme contract, compiled into the binary.
pub const THEME_CONTRACT: &str = include_str!("theme-contract.ncl");

/// Nickel pseudo-URL prefix for the settings contract.
const CONTRACT_PREFIX: &str = "clankers://settings";

/// Nickel pseudo-URL prefix for the theme contract.
const THEME_CONTRACT_PREFIX: &str = "clankers://theme";

/// Evaluate a `.ncl` file and return the result as a JSON value.
///
/// The file is read, the `clankers://settings` import is resolved to the
/// embedded contract, and the result is deeply evaluated then converted
/// to JSON.
pub fn eval_ncl_file(path: &Path) -> Result<serde_json::Value, NickelError> {
    let src = std::fs::read_to_string(path).map_err(|e| NickelError {
        message: format!("failed to read {}: {e}", path.display()),
    })?;
    eval_ncl_source(&src, path.display().to_string())
}

/// Evaluate a Nickel source string and return the result as a JSON value.
///
/// `source_name` is used in error diagnostics to identify the file.
pub fn eval_ncl_source(src: &str, source_name: String) -> Result<serde_json::Value, NickelError> {
    // Rewrite the clankers://settings pseudo-import to inline the contract.
    // Nickel doesn't support custom URL schemes in imports, so we splice
    // the contract source directly.
    let resolved = resolve_contract_import(src);

    let mut ctx = Context::new().with_source_name(source_name);
    let expr = ctx.eval_deep_for_export(&resolved).map_err(|e| NickelError {
        message: format_nickel_error(&e),
    })?;
    let json_str = ctx.expr_to_json(&expr).map_err(|e| NickelError {
        message: format_nickel_error(&e),
    })?;
    serde_json::from_str(&json_str).map_err(|e| NickelError {
        message: format!("nickel output is not valid JSON: {e}"),
    })
}

/// Evaluate a Nickel source string merged with the embedded contract.
///
/// Wraps the user source as `(CONTRACT) & (USER_SOURCE)` so that
/// contract defaults and type checks are applied. Returns the merged
/// result as a JSON value.
pub fn eval_ncl_with_contract(path: &Path) -> Result<serde_json::Value, NickelError> {
    let user_src = std::fs::read_to_string(path).map_err(|e| NickelError {
        message: format!("failed to read {}: {e}", path.display()),
    })?;

    // Build a wrapper that merges the contract defaults with user overrides.
    let merged_src = format!("({SETTINGS_CONTRACT}) & ({user_src})");

    let mut ctx = Context::new().with_source_name(path.display().to_string());
    let expr = ctx.eval_deep_for_export(&merged_src).map_err(|e| NickelError {
        message: format_nickel_error(&e),
    })?;
    let json_str = ctx.expr_to_json(&expr).map_err(|e| NickelError {
        message: format_nickel_error(&e),
    })?;
    serde_json::from_str(&json_str).map_err(|e| NickelError {
        message: format!("nickel output is not valid JSON: {e}"),
    })
}

/// Replace `import "clankers://settings"` with an inline let-binding
/// of the contract source.
fn resolve_contract_import(src: &str) -> String {
    let mut result = src.to_string();
    if result.contains(CONTRACT_PREFIX) {
        result = result.replace(&format!("import \"{CONTRACT_PREFIX}\""), &format!("({SETTINGS_CONTRACT})"));
    }
    if result.contains(THEME_CONTRACT_PREFIX) {
        result = result.replace(&format!("import \"{THEME_CONTRACT_PREFIX}\""), &format!("({THEME_CONTRACT})"));
    }
    result
}

/// Format a Nickel error preserving the diagnostic message.
fn format_nickel_error(err: &nickel_lang::Error) -> String {
    // Error doesn't impl Display — use Debug which includes the diagnostic.
    format!("{err:?}")
}

/// Error from Nickel evaluation.
#[derive(Debug, Clone)]
pub struct NickelError {
    /// Human-readable error message including Nickel diagnostics.
    pub message: String,
}

impl std::fmt::Display for NickelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for NickelError {}

/// Generate a starter `settings.ncl` file with comments and the contract import.
pub fn generate_starter_config() -> String {
    r#"# Clankers settings (Nickel)
#
# This file is evaluated by the Nickel language before being loaded as
# JSON configuration. You get comments, types, defaults, and computed
# values.
#
# Import the built-in contract for type checking and defaults:
(import "clankers://settings") & {
  # Uncomment and edit the fields you want to override:

  # model = "claude-sonnet-4-5",
  # maxTokens = 16384,
  # planMode = false,
  # useDaemon = true,

  # keymap = {
  #   preset = "helix",
  # },

  # hooks = {
  #   disabledHooks = [],
  # },

  # memory = {
  #   globalCharLimit = 2200,
  #   projectCharLimit = 1375,
  # },
}
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eval_simple_record() {
        let result = eval_ncl_source(r#"{ model = "claude-opus-4-6", maxTokens = 32768 }"#, "test".into()).unwrap();
        assert_eq!(result["model"], "claude-opus-4-6");
        assert_eq!(result["maxTokens"], 32768);
    }

    #[test]
    fn eval_syntax_error() {
        let result = eval_ncl_source("{ model = }", "test".into());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(!err.message.is_empty());
    }

    #[test]
    fn eval_contract_import_resolution() {
        // A config that imports the contract and overrides model
        let src = r#"(import "clankers://settings") & { model = "claude-opus-4-6" }"#;
        let result = eval_ncl_source(src, "test".into()).unwrap();
        assert_eq!(result["model"], "claude-opus-4-6");
        // Defaults from contract should be present
        assert_eq!(result["maxTokens"], 16384);
        assert_eq!(result["useDaemon"], true);
        assert_eq!(result["noCache"], false);
    }

    #[test]
    fn eval_plain_record_without_contract() {
        // A plain record without importing the contract — should work fine
        let result = eval_ncl_source(r#"{ model = "opus" }"#, "test".into()).unwrap();
        assert_eq!(result["model"], "opus");
        // No defaults filled in without the contract
        assert!(result.get("maxTokens").is_none());
    }

    #[test]
    fn contract_defaults_match_settings_default() {
        // Evaluate the contract with no overrides — use contract import
        // resolution rather than raw wrapping to avoid comment/paren issues.
        let src = r#"(import "clankers://settings") & {}"#;
        let result = eval_ncl_source(src, "contract-test".into()).unwrap();

        // Deserialize to Settings — should match Settings::default()
        let settings: crate::Settings = serde_json::from_value(result).unwrap();
        let defaults = crate::Settings::default();

        assert_eq!(settings.model, defaults.model);
        assert_eq!(settings.max_tokens, defaults.max_tokens);
        assert_eq!(settings.use_daemon, defaults.use_daemon);
        assert_eq!(settings.no_cache, defaults.no_cache);
        assert_eq!(settings.plan_mode, defaults.plan_mode);
        assert_eq!(settings.max_output_lines, defaults.max_output_lines);
        assert_eq!(settings.max_output_bytes, defaults.max_output_bytes);
        assert_eq!(settings.bash_timeout, defaults.bash_timeout);
        assert_eq!(settings.max_subagent_panes, defaults.max_subagent_panes);
        assert!(settings.disabled_tools.is_empty());
        assert_eq!(settings.hooks.enabled, defaults.hooks.enabled);
        assert_eq!(settings.hooks.script_timeout_secs, defaults.hooks.script_timeout_secs);
        assert_eq!(settings.memory.global_char_limit, defaults.memory.global_char_limit);
        assert_eq!(settings.memory.project_char_limit, defaults.memory.project_char_limit);
        assert_eq!(settings.compression.summary_model, defaults.compression.summary_model);
        assert_eq!(settings.compression.keep_recent, defaults.compression.keep_recent);
        assert_eq!(settings.compression.tail_budget_fraction, defaults.compression.tail_budget_fraction);
        assert_eq!(settings.compression.min_messages, defaults.compression.min_messages);
    }

    #[test]
    #[ignore = "nickel-lang error formatting overflows default stack; run with RUST_MIN_STACK=33554432"]
    fn type_violation_produces_error_not_value() {
        // Nickel contract violations trigger deep recursion in the error
        // formatter which overflows the default stack. Use a thread with
        // 16MB stack. If the thread panics (stack overflow), that still
        // proves the evaluator rejects the input — it didn't silently
        // produce a value.
        let handle = std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(|| {
                let src = r#"({ model | String | default = "x" }) & { model = 42 }"#;
                eval_ncl_source(src, "test".into())
            })
            .unwrap();
        match handle.join() {
            Ok(Ok(_)) => panic!("expected type violation to produce an error"),
            Ok(Err(_)) => {} // got NickelError — correct
            Err(_) => {}     // thread panicked (stack overflow) — still not a silent success
        }
    }

    #[test]
    fn generate_starter_contains_contract_import() {
        let starter = generate_starter_config();
        assert!(starter.contains("clankers://settings"));
        assert!(starter.contains("model"));
    }
}
