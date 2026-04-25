//! SDK for building [clankers](https://github.com/brittonr/clankers) WASM plugins.
//!
//! # clanker-plugin-sdk
//!
//! Provides the protocol types, dispatch helpers, and argument extraction
//! utilities that every clankers plugin needs. Eliminates the boilerplate of
//! hand-rolling JSON serialization and tool routing.
//!
//! ## Quick start
//!
//! Add to your plugin's `Cargo.toml`:
//!
//! ```toml
//! [package]
//! name = "my-plugin"
//! version = "0.1.0"
//! edition = "2021"
//!
//! [lib]
//! crate-type = ["cdylib"]
//!
//! [dependencies]
//! clanker-plugin-sdk = { path = "../../crates/clanker-plugin-sdk" }
//!
//! [workspace]
//! ```
//!
//! Then write your plugin:
//!
//! ```ignore
//! use clanker_plugin_sdk::prelude::*;
//!
//! fn handle_greet(args: &Value) -> Result<String, String> {
//!     let name = args.require_str("name")?;
//!     Ok(format!("Hello, {name}!"))
//! }
//!
//! #[plugin_fn]
//! pub fn handle_tool_call(input: String) -> FnResult<String> {
//!     dispatch_tools(&input, &[
//!         ("greet", handle_greet),
//!     ])
//! }
//!
//! #[plugin_fn]
//! pub fn on_event(input: String) -> FnResult<String> {
//!     dispatch_events(&input, "my-plugin", &[
//!         ("agent_start", |_| "Plugin ready".to_string()),
//!         ("agent_end",   |_| "Shutting down".to_string()),
//!     ])
//! }
//!
//! #[plugin_fn]
//! pub fn describe(Json(_): Json<()>) -> FnResult<Json<PluginMeta>> {
//!     Ok(Json(PluginMeta::new("my-plugin", "0.1.0", &[
//!         ("greet", "Greet someone by name"),
//!     ], &[])))
//! }
//! ```
//!
//! ## Modules
//!
//! - [`types`] — Protocol types: [`ToolCall`], [`ToolResult`], [`Event`],
//!   [`EventResult`], [`PluginMeta`], [`ToolMeta`]
//! - [`args`] — Argument extraction trait [`Args`] on `serde_json::Value`
//! - [`dispatch`] — [`dispatch_tools`] and [`dispatch_events`] helpers
//! - [`prelude`] — Convenient re-exports of everything you need

pub mod args;
pub mod dispatch;
pub mod types;

/// HTTP client for plugins with "net" permission.
#[cfg(feature = "http")]
pub mod http;

/// Convenient re-exports for plugin authors.
///
/// ```ignore
/// use clanker_plugin_sdk::prelude::*;
/// ```
///
/// This gives you:
/// - All protocol types (`ToolCall`, `ToolResult`, `Event`, `EventResult`, `PluginMeta`, `ToolMeta`)
/// - Arg extraction trait (`Args`)
/// - Dispatch functions (`dispatch_tools`, `dispatch_events`)
/// - Extism PDK essentials (`plugin_fn`, `FnResult`, `Error`, `Json`)
/// - Serde traits (`Serialize`, `Deserialize`)
/// - `serde_json::Value`
pub mod prelude {
    // Protocol types
    pub use crate::types::{Event, EventResult, PluginMeta, ToolCall, ToolMeta, ToolResult};

    // Arg extraction
    pub use crate::args::Args;

    // Dispatch helpers
    pub use crate::dispatch::{dispatch_events, dispatch_tools};

    // Extism PDK essentials
    pub use extism_pdk::{plugin_fn, Error, FnResult, Json};

    // Serde (plugins almost always need these)
    pub use serde::{Deserialize, Serialize};
    pub use serde_json::Value;
}

// Re-export extism_pdk so plugins can use `clanker_plugin_sdk::extism_pdk`
// without adding it as a separate dependency.
pub use extism_pdk;
pub use serde;
pub use serde_json;
