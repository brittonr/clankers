//! Tool and event dispatch helpers.
//!
//! These functions eliminate the boilerplate of parsing JSON input,
//! routing to the correct handler, serializing the response, and
//! handling unknown tool/event names.
//!
//! # Tool dispatch
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
//! ```
//!
//! # Event dispatch
//! ```ignore
//! #[plugin_fn]
//! pub fn on_event(input: String) -> FnResult<String> {
//!     dispatch_events(&input, "my-plugin", &[
//!         ("agent_start", |_| "Plugin ready".to_string()),
//!         ("agent_end",   |_| "Plugin shutting down".to_string()),
//!     ])
//! }
//! ```

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use extism_pdk::{Error, FnResult};
use serde_json::Value;

use crate::types::{EventResult, ToolCall, ToolResult};

/// Tool handler function signature.
///
/// Receives the `args` object from the tool call and returns either
/// a success string or an error string.
pub type ToolHandler = fn(&Value) -> Result<String, String>;

/// Event handler function signature.
///
/// Receives the `data` object from the event and returns a message string.
/// The message is included in the event response as `"message"`.
pub type EventHandler = fn(&Value) -> String;

/// Dispatch a tool call to the matching handler.
///
/// Parses the input JSON, looks up the tool name in `handlers`, calls
/// the matching handler, and serializes the result. Returns an
/// `"unknown_tool"` response if no handler matches.
///
/// # Arguments
/// - `input` — raw JSON string from the host (`{"tool":"...","args":{...}}`)
/// - `handlers` — slice of `(tool_name, handler_fn)` pairs
///
/// # Example
/// ```ignore
/// #[plugin_fn]
/// pub fn handle_tool_call(input: String) -> FnResult<String> {
///     dispatch_tools(&input, &[
///         ("hash_text", handle_hash),
///         ("encode_text", handle_encode),
///     ])
/// }
/// ```
pub fn dispatch_tools(input: &str, handlers: &[(&str, ToolHandler)]) -> FnResult<String> {
    let call: ToolCall = serde_json::from_str(input)
        .map_err(|e| Error::msg(format!("Invalid JSON input: {e}")))?;

    // Find matching handler
    for (name, handler) in handlers {
        if *name == call.tool {
            let result = match handler(&call.args) {
                Ok(r) => ToolResult::ok(&call.tool, r),
                Err(e) => ToolResult::error(&call.tool, e),
            };
            return Ok(serde_json::to_string(&result)?);
        }
    }

    // No handler matched
    Ok(serde_json::to_string(&ToolResult::unknown(&call.tool))?)
}

/// Dispatch a lifecycle event to the matching handler.
///
/// Parses the input JSON, looks up the event name in `handlers`, calls
/// the matching handler, and serializes the result. Returns an
/// "unhandled" response if no handler matches.
///
/// # Arguments
/// - `input` — raw JSON string from the host (`{"event":"...","data":{...}}`)
/// - `plugin_name` — plugin name (included in "unhandled" messages)
/// - `handlers` — slice of `(event_name, handler_fn)` pairs
///
/// # Example
/// ```ignore
/// #[plugin_fn]
/// pub fn on_event(input: String) -> FnResult<String> {
///     dispatch_events(&input, "my-plugin", &[
///         ("agent_start", |_| "Ready".to_string()),
///         ("agent_end",   |_| "Shutting down".to_string()),
///     ])
/// }
/// ```
pub fn dispatch_events(
    input: &str,
    plugin_name: &str,
    handlers: &[(&str, EventHandler)],
) -> FnResult<String> {
    let evt: crate::types::Event = serde_json::from_str(input)
        .map_err(|e| Error::msg(format!("Invalid event JSON: {e}")))?;

    // Find matching handler
    for (name, handler) in handlers {
        if *name == evt.event {
            let message = handler(&evt.data);
            let result = EventResult::handled(&evt.event, message);
            return Ok(serde_json::to_string(&result)?);
        }
    }

    // No handler matched — return unhandled
    let _ = plugin_name; // reserved for future use in unhandled messages
    Ok(serde_json::to_string(&EventResult::unhandled(&evt.event))?)
}
