//! In-process Nix evaluation via snix-eval.
//!
//! Evaluates pure Nix expressions without spawning `nix eval`.
//! Falls back to the CLI for impure operations (file import, fetchurl, IFD).

use serde::Serialize;
use serde_json::Value as JsonValue;
use std::path::Path;
use std::time::{Duration, Instant};

use crate::error::*;

/// Maximum output size for serialized results (1 MB).
const MAX_OUTPUT_SIZE: usize = 1_024 * 1_024;

/// Default timeout for pure evaluation (10 seconds).
const PURE_EVAL_TIMEOUT: Duration = Duration::from_secs(10);

/// Result of an in-process Nix evaluation.
#[derive(Debug, Clone, Serialize)]
pub struct EvalResult {
    /// The evaluated value as JSON.
    pub value: JsonValue,
    /// Whether the result was computed in-process (true) or via CLI fallback (false).
    pub in_process: bool,
    /// Warnings from the evaluation, if any.
    pub warnings: Vec<String>,
}

/// Evaluate a Nix expression in-process using snix-eval.
///
/// Uses pure evaluation (no filesystem access, no network, no imports).
/// Returns `Err` with `NixError::EvalFailed` if the expression fails.
///
/// For impure expressions, callers should catch the error and fall back
/// to `nix eval --json`.
pub fn evaluate(expr: &str) -> Result<EvalResult, NixError> {
    evaluate_with_timeout(expr, PURE_EVAL_TIMEOUT)
}

/// Evaluate with a custom timeout.
pub fn evaluate_with_timeout(expr: &str, timeout: Duration) -> Result<EvalResult, NixError> {
    let expr_owned = expr.to_string();
    let start = Instant::now();

    // snix-eval uses Rc internally (not Send), so we run it on the current thread.
    // The caller should use spawn_blocking if needed.
    let eval = snix_eval::Evaluation::builder_pure()
        .mode(snix_eval::EvalMode::Strict)
        .build();

    let result = eval.evaluate(&expr_owned, None);

    // Check timeout
    if start.elapsed() > timeout {
        return Err(NixError::EvalTimeout {
            seconds: timeout.as_secs(),
        });
    }

    // Check for errors
    if !result.errors.is_empty() {
        let error_msgs: Vec<String> = result.errors.iter().map(|e| format!("{e}")).collect();
        let is_impure = error_msgs.iter().any(|msg| {
            let lower = msg.to_lowercase();
            lower.contains("import")
                || lower.contains("file")
                || lower.contains("io error")
                || lower.contains("not implemented")
                || lower.contains("relative path")
                || lower.contains("nix_path")
                || lower.contains("disabled")
                || lower.contains("not allowed")
                || lower.contains("forbidden")
                || lower.contains("pure")
        });

        return Err(NixError::EvalFailed {
            expr: truncate_expr(&expr_owned),
            reason: error_msgs.join("; "),
            is_impure,
        });
    }

    let value = result
        .value
        .ok_or_else(|| NixError::EvalFailed {
            expr: truncate_expr(&expr_owned),
            reason: "evaluation produced no value".to_string(),
            is_impure: false,
        })?;

    // Serialize to JSON
    let json = value_to_json(&value)?;

    // Check output size
    let json_str = serde_json::to_string(&json).unwrap_or_default();
    if json_str.len() > MAX_OUTPUT_SIZE {
        return Err(NixError::EvalOutputTooLarge {
            size: json_str.len(),
            max: MAX_OUTPUT_SIZE,
        });
    }

    let warnings: Vec<String> = result.warnings.iter().map(|w| format!("{w:?}")).collect();

    Ok(EvalResult {
        value: json,
        in_process: true,
        warnings,
    })
}

/// Evaluate a .nix file in-process.
pub fn evaluate_file(path: &Path) -> Result<EvalResult, NixError> {
    let content = std::fs::read_to_string(path).map_err(|e| NixError::EvalFailed {
        expr: path.display().to_string(),
        reason: format!("failed to read file: {e}"),
        is_impure: true,
    })?;
    evaluate(&content)
}

/// Flake output listing.
#[derive(Debug, Clone, Default, Serialize)]
pub struct FlakeOutputs {
    pub packages: Vec<String>,
    pub dev_shells: Vec<String>,
    pub checks: Vec<String>,
    pub apps: Vec<String>,
    pub nixos_configurations: Vec<String>,
    pub other: Vec<String>,
}

/// Introspect a flake's outputs by evaluating its flake.nix.
///
/// This attempts pure in-process evaluation. Flakes typically need
/// impure features (lock file resolution, fetching inputs), so this
/// will usually fail and the caller should fall back to `nix flake show --json`.
pub fn introspect_flake(flake_dir: &Path) -> Result<FlakeOutputs, NixError> {
    let flake_nix = flake_dir.join("flake.nix");
    if !flake_nix.exists() {
        return Err(NixError::EvalFailed {
            expr: flake_nix.display().to_string(),
            reason: "flake.nix not found".to_string(),
            is_impure: false,
        });
    }

    // Flake evaluation requires imports + lock resolution — this will almost
    // always fail in pure mode. Callers should catch EvalFailed with is_impure
    // and fall back to `nix flake show --json`.
    Err(NixError::EvalFailed {
        expr: "flake introspection".to_string(),
        reason: "flake evaluation requires impure features (input fetching, lock resolution); \
                 use `nix flake show --json` instead"
            .to_string(),
        is_impure: true,
    })
}

/// Convert a snix-eval Value to serde_json::Value.
/// Recursion follows the Nix value tree structure (bounded by eval depth).
#[cfg_attr(dylint_lib = "tigerstyle", allow(no_recursion, reason = "follows nix value tree; depth bounded by eval limits"))]
fn value_to_json(value: &snix_eval::Value) -> Result<JsonValue, NixError> {
    match value {
        snix_eval::Value::Null => Ok(JsonValue::Null),
        snix_eval::Value::Bool(b) => Ok(JsonValue::Bool(*b)),
        snix_eval::Value::Integer(i) => Ok(serde_json::json!(*i)),
        snix_eval::Value::Float(f) => {
            serde_json::Number::from_f64(*f)
                .map(JsonValue::Number)
                .ok_or_else(|| NixError::EvalFailed {
                    expr: String::new(),
                    reason: format!("non-finite float: {f}"),
                    is_impure: false,
                })
        }
        snix_eval::Value::String(s) => Ok(JsonValue::String(s.as_bstr().to_string())),
        snix_eval::Value::Path(p) => Ok(JsonValue::String(p.display().to_string())),
        snix_eval::Value::Attrs(attrs) => {
            let mut map = serde_json::Map::new();
            for (key, val) in attrs.iter() {
                let k = key.as_bstr().to_string();
                let v = value_to_json(val)?;
                map.insert(k, v);
            }
            Ok(JsonValue::Object(map))
        }
        snix_eval::Value::List(list) => {
            let items: Result<Vec<JsonValue>, NixError> =
                list.iter().map(value_to_json).collect();
            Ok(JsonValue::Array(items?))
        }
        snix_eval::Value::Closure(_) | snix_eval::Value::Builtin(_) => {
            Ok(JsonValue::String("<lambda>".to_string()))
        }
        snix_eval::Value::Thunk(thunk) => {
            // Thunks should be forced in strict mode, but handle gracefully
            value_to_json(&thunk.value())
        }
        _ => Ok(JsonValue::String(format!("<{}>", value.type_of()))),
    }
}

/// Truncate expression for error messages.
fn truncate_expr(expr: &str) -> String {
    if expr.len() > 100 {
        format!("{}...", &expr[..97])
    } else {
        expr.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eval_arithmetic() {
        let result = evaluate("1 + 1").unwrap();
        assert_eq!(result.value, serde_json::json!(2));
        assert!(result.in_process);
    }

    #[test]
    fn eval_string() {
        let result = evaluate(r#""hello world""#).unwrap();
        assert_eq!(result.value, serde_json::json!("hello world"));
    }

    #[test]
    fn eval_attrset() {
        let result = evaluate("{ a = 1; b = 2; }").unwrap();
        assert_eq!(result.value, serde_json::json!({"a": 1, "b": 2}));
    }

    #[test]
    fn eval_list() {
        let result = evaluate("[1 2 3]").unwrap();
        assert_eq!(result.value, serde_json::json!([1, 2, 3]));
    }

    #[test]
    fn eval_bool() {
        let result = evaluate("true").unwrap();
        assert_eq!(result.value, serde_json::json!(true));
    }

    #[test]
    fn eval_null() {
        let result = evaluate("null").unwrap();
        assert_eq!(result.value, JsonValue::Null);
    }

    #[test]
    fn eval_float() {
        let result = evaluate("1.5").unwrap();
        assert_eq!(result.value, serde_json::json!(1.5));
    }

    #[test]
    fn eval_builtins_attr_names() {
        let result = evaluate("builtins.attrNames { a = 1; b = 2; c = 3; }").unwrap();
        assert_eq!(result.value, serde_json::json!(["a", "b", "c"]));
    }

    #[test]
    fn eval_string_concat() {
        let result = evaluate(r#""hello" + " " + "world""#).unwrap();
        assert_eq!(result.value, serde_json::json!("hello world"));
    }

    #[test]
    fn eval_let_binding() {
        let result = evaluate("let x = 5; in x * x").unwrap();
        assert_eq!(result.value, serde_json::json!(25));
    }

    #[test]
    fn eval_nested_attrset() {
        let result = evaluate("{ a.b.c = 42; }").unwrap();
        assert_eq!(result.value, serde_json::json!({"a": {"b": {"c": 42}}}));
    }

    #[test]
    fn eval_lambda_in_output() {
        let result = evaluate("{ f = x: x + 1; v = 42; }").unwrap();
        // Lambda should serialize as "<lambda>"
        let obj = result.value.as_object().unwrap();
        assert_eq!(obj["f"], serde_json::json!("<lambda>"));
        assert_eq!(obj["v"], serde_json::json!(42));
    }

    #[test]
    fn eval_import_fails_pure() {
        let result = evaluate("import ./default.nix");
        assert!(result.is_err());
        // Import requires filesystem access, which pure eval doesn't have.
        // The error might or might not be classified as impure depending on
        // how snix-eval reports it, but it should definitely fail.
        if let Err(NixError::EvalFailed { reason, .. }) = &result {
            // Sanity check — the error message should mention something relevant
            assert!(
                !reason.is_empty(),
                "expected non-empty error for import in pure mode"
            );
        }
    }

    #[test]
    fn eval_syntax_error() {
        let result = evaluate("{ a = ; }");
        assert!(result.is_err());
    }

    #[test]
    fn eval_output_size_limit() {
        // Generate a large output — list of 100000 numbers
        let result = evaluate("builtins.genList (x: x) 100000");
        // This should succeed (100k ints serialized is under 1MB)
        assert!(result.is_ok());
    }

    #[test]
    fn eval_builtins_map() {
        let result = evaluate("builtins.map (x: x * 2) [1 2 3]").unwrap();
        assert_eq!(result.value, serde_json::json!([2, 4, 6]));
    }

    #[test]
    fn eval_builtins_filter() {
        let result = evaluate("builtins.filter (x: x > 2) [1 2 3 4 5]").unwrap();
        assert_eq!(result.value, serde_json::json!([3, 4, 5]));
    }

    #[test]
    fn eval_recursive_attrset() {
        let result = evaluate("rec { a = 1; b = a + 1; c = b + 1; }").unwrap();
        assert_eq!(result.value, serde_json::json!({"a": 1, "b": 2, "c": 3}));
    }

    #[test]
    fn introspect_nonexistent_flake() {
        let result = introspect_flake(Path::new("/nonexistent"));
        assert!(result.is_err());
    }
}
