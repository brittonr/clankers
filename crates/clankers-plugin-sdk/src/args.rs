//! Argument extraction helpers for plugin tool handlers.
//!
//! Provides the [`Args`] trait on `serde_json::Value` for ergonomic
//! extraction of typed parameters from tool call arguments.
//!
//! # Example
//! ```ignore
//! use clankers_plugin_sdk::prelude::*;
//!
//! fn handle_my_tool(args: &Value) -> Result<String, String> {
//!     let text = args.require_str("text")?;
//!     let count = args.get_u64_or("count", 10);
//!     let verbose = args.get_bool_or("verbose", false);
//!     // ...
//! }
//! ```

use serde_json::Value;

/// Extension trait for extracting typed values from a `serde_json::Value`.
///
/// All methods operate on the assumption that `self` is a JSON object.
/// If `self` is not an object, `get_*` methods return `None` and
/// `require_*` methods return an error.
pub trait Args {
    /// Get a string value by key. Returns `None` if missing or not a string.
    fn get_str(&self, key: &str) -> Option<&str>;

    /// Get a string value, falling back to a default if missing.
    fn get_str_or<'a>(&'a self, key: &str, default: &'a str) -> &'a str;

    /// Require a string value. Returns an error if missing or not a string.
    fn require_str(&self, key: &str) -> Result<&str, String>;

    /// Get a u64 value by key. Returns `None` if missing or not a number.
    fn get_u64(&self, key: &str) -> Option<u64>;

    /// Get a u64 value, falling back to a default.
    fn get_u64_or(&self, key: &str, default: u64) -> u64;

    /// Get an i64 value by key.
    fn get_i64(&self, key: &str) -> Option<i64>;

    /// Get an f64 value by key.
    fn get_f64(&self, key: &str) -> Option<f64>;

    /// Get an f64 value, falling back to a default.
    fn get_f64_or(&self, key: &str, default: f64) -> f64;

    /// Get a boolean value by key.
    fn get_bool(&self, key: &str) -> Option<bool>;

    /// Get a boolean value, falling back to a default.
    fn get_bool_or(&self, key: &str, default: bool) -> bool;

    /// Get a JSON array by key.
    fn get_array(&self, key: &str) -> Option<&Vec<Value>>;

    /// Get an array of strings by key. Non-string elements are skipped.
    fn get_str_array(&self, key: &str) -> Vec<String>;

    /// Get a nested JSON object by key.
    fn get_object(&self, key: &str) -> Option<&serde_json::Map<String, Value>>;
}

impl Args for Value {
    fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(|v| v.as_str())
    }

    fn get_str_or<'a>(&'a self, key: &str, default: &'a str) -> &'a str {
        self.get_str(key).unwrap_or(default)
    }

    fn require_str(&self, key: &str) -> Result<&str, String> {
        self.get_str(key)
            .ok_or_else(|| format!("missing required parameter: {key}"))
    }

    fn get_u64(&self, key: &str) -> Option<u64> {
        self.get(key).and_then(|v| v.as_u64())
    }

    fn get_u64_or(&self, key: &str, default: u64) -> u64 {
        self.get_u64(key).unwrap_or(default)
    }

    fn get_i64(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(|v| v.as_i64())
    }

    fn get_f64(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(|v| v.as_f64())
    }

    fn get_f64_or(&self, key: &str, default: f64) -> f64 {
        self.get_f64(key).unwrap_or(default)
    }

    fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(|v| v.as_bool())
    }

    fn get_bool_or(&self, key: &str, default: bool) -> bool {
        self.get_bool(key).unwrap_or(default)
    }

    fn get_array(&self, key: &str) -> Option<&Vec<Value>> {
        self.get(key).and_then(|v| v.as_array())
    }

    fn get_str_array(&self, key: &str) -> Vec<String> {
        self.get_array(key)
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn get_object(&self, key: &str) -> Option<&serde_json::Map<String, Value>> {
        self.get(key).and_then(|v| v.as_object())
    }
}
