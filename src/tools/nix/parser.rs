//! Nix internal-json protocol parsing
//!
//! Parses nix's `--log-format internal-json` output to provide structured
//! progress information and build logs.

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

use super::super::ToolContext;
use crate::util::ansi::strip_ansi;

// ── Nix internal-json protocol types ────────────────────────────────────────

/// Activity types from nix's internal-json format.
/// See: https://github.com/NixOS/nix/blob/master/src/libutil/logging.hh
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum ActivityType {
    Unknown = 0,
    CopyPath = 100,
    FileTransfer = 101,
    Realise = 102,
    CopyPaths = 103,
    Builds = 104,
    Build = 105,
    OptimiseStore = 106,
    VerifyPaths = 107,
    Substitute = 108,
    QueryPathInfo = 109,
    PostBuildHook = 110,
    BuildWaiting = 111,
    FetchTree = 112,
}

impl ActivityType {
    pub fn from_u64(v: u64) -> Self {
        match v {
            100 => Self::CopyPath,
            101 => Self::FileTransfer,
            102 => Self::Realise,
            103 => Self::CopyPaths,
            104 => Self::Builds,
            105 => Self::Build,
            106 => Self::OptimiseStore,
            107 => Self::VerifyPaths,
            108 => Self::Substitute,
            109 => Self::QueryPathInfo,
            110 => Self::PostBuildHook,
            111 => Self::BuildWaiting,
            112 => Self::FetchTree,
            _ => Self::Unknown,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Unknown => "working",
            Self::CopyPath => "copying",
            Self::FileTransfer => "downloading",
            Self::Realise => "realising",
            Self::CopyPaths => "copying paths",
            Self::Builds => "building",
            Self::Build => "building",
            Self::OptimiseStore => "optimising store",
            Self::VerifyPaths => "verifying",
            Self::Substitute => "fetching",
            Self::QueryPathInfo => "querying",
            Self::PostBuildHook => "post-build hook",
            Self::BuildWaiting => "waiting for build",
            Self::FetchTree => "fetching tree",
        }
    }
}

/// Result types from nix's internal-json format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum ResultType {
    FileLinked = 100,
    BuildLogLine = 101,
    UntrustedPath = 102,
    CorruptedPath = 103,
    SetPhase = 104,
    Progress = 105,
}

impl ResultType {
    pub fn from_u64(v: u64) -> Option<Self> {
        match v {
            100 => Some(Self::FileLinked),
            101 => Some(Self::BuildLogLine),
            102 => Some(Self::UntrustedPath),
            103 => Some(Self::CorruptedPath),
            104 => Some(Self::SetPhase),
            105 => Some(Self::Progress),
            _ => None,
        }
    }
}

/// Nix log level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u64)]
pub enum NixLogLevel {
    Error = 0,
    Warn = 1,
    Notice = 2,
    Info = 3,
    Talkative = 4,
    Chatty = 5,
    Debug = 6,
    Vomit = 7,
}

impl NixLogLevel {
    pub fn from_u64(v: u64) -> Self {
        match v {
            0 => Self::Error,
            1 => Self::Warn,
            2 => Self::Notice,
            3 => Self::Info,
            4 => Self::Talkative,
            5 => Self::Chatty,
            6 => Self::Debug,
            _ => Self::Vomit,
        }
    }
}

/// Parsed nix internal-json event
#[derive(Debug, Deserialize)]
pub struct NixEvent {
    pub action: String,
    #[serde(default)]
    pub id: u64,
    #[serde(default)]
    pub level: u64,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub msg: String,
    #[serde(default)]
    pub raw_msg: String,
    #[serde(rename = "type", default)]
    pub activity_type: u64,
    #[serde(default)]
    pub fields: Vec<Value>,
    #[serde(default)]
    pub _parent: u64,
}

/// Tracks active nix activities for progress display
pub struct ActivityTracker {
    pub activities: HashMap<u64, TrackedActivity>,
}

pub struct TrackedActivity {
    pub text: String,
    pub activity_type: ActivityType,
    pub _phase: Option<String>,
}

impl ActivityTracker {
    pub fn new() -> Self {
        Self {
            activities: HashMap::new(),
        }
    }

    pub fn start(&mut self, id: u64, text: String, activity_type: ActivityType) {
        self.activities.insert(id, TrackedActivity {
            text,
            activity_type,
            _phase: None,
        });
    }

    pub fn stop(&mut self, id: u64) {
        self.activities.remove(&id);
    }

    pub fn set_phase(&mut self, id: u64, phase: String) {
        if let Some(a) = self.activities.get_mut(&id) {
            a._phase = Some(phase);
        }
    }

    pub fn get(&self, id: u64) -> Option<&TrackedActivity> {
        self.activities.get(&id)
    }
}

/// Collects mutable output state for nix line processing.
///
/// Groups the 5 mutable parameters that `process_nix_line` previously took
/// individually. Each field has a clear role:
pub struct NixOutputState {
    pub tracker: ActivityTracker,
    pub build_log_lines: Vec<String>,
    pub messages: Vec<String>,
    pub errors: Vec<String>,
    pub last_progress_text: Option<String>,
}

impl NixOutputState {
    pub fn new() -> Self {
        Self {
            tracker: ActivityTracker::new(),
            build_log_lines: Vec::new(),
            messages: Vec::new(),
            errors: Vec::new(),
            last_progress_text: None,
        }
    }
}

/// Maximum build log lines retained (prevents unbounded growth on long builds).
const MAX_BUILD_LOG_LINES: usize = 10_000;
/// Maximum message lines retained.
const MAX_MESSAGES: usize = 5_000;
/// Maximum error lines retained.
const MAX_ERRORS: usize = 1_000;

// Tiger Style: compile-time bounds validation
const _: () = assert!(MAX_BUILD_LOG_LINES > 0);
const _: () = assert!(MAX_MESSAGES > 0);
const _: () = assert!(MAX_ERRORS > 0);
const _: () = assert!(MAX_BUILD_LOG_LINES >= MAX_ERRORS);

/// Parse a single stderr line from nix's internal-json output.
///
/// Dispatches to focused per-action handlers. Each handler is under 40 lines.
pub fn process_nix_line(line: &str, ctx: &ToolContext, state: &mut NixOutputState) {
    // Internal-json lines start with "@nix "
    let json_str = match line.strip_prefix("@nix ") {
        Some(s) => s,
        None => {
            // Not a nix structured line — treat as raw output
            if !line.is_empty() {
                ctx.emit_progress(line);
                push_bounded(&mut state.messages, line.to_string(), MAX_MESSAGES);
            }
            return;
        }
    };

    let event: NixEvent = match serde_json::from_str(json_str) {
        Ok(e) => e,
        Err(_) => return,
    };

    match event.action.as_str() {
        "start" => handle_start_event(&event, ctx, state),
        "stop" => state.tracker.stop(event.id),
        "result" => handle_result_event(&event, ctx, state),
        "msg" => handle_msg_event(&event, ctx, state),
        _ => {}
    }
}

/// Handle a nix "start" event — registers the activity and emits progress.
fn handle_start_event(event: &NixEvent, ctx: &ToolContext, state: &mut NixOutputState) {
    let activity_type = ActivityType::from_u64(event.activity_type);
    let level = NixLogLevel::from_u64(event.level);
    let text = event.text.clone();

    state.tracker.start(event.id, text.clone(), activity_type);

    match activity_type {
        ActivityType::Build | ActivityType::Substitute => {
            let display = shorten_drv_path(&text);
            let msg = format!("⚙ {} {}", activity_type.label(), display);
            ctx.emit_progress(&msg);
            push_bounded(&mut state.messages, msg, MAX_MESSAGES);
        }
        ActivityType::FileTransfer => {
            if level <= NixLogLevel::Talkative {
                let msg = format!("↓ {}", shorten_url(&text));
                ctx.emit_progress(&msg);
            }
        }
        ActivityType::FetchTree => {
            let msg = format!("🌲 fetching {}", shorten_url(&text));
            ctx.emit_progress(&msg);
            push_bounded(&mut state.messages, msg, MAX_MESSAGES);
        }
        ActivityType::CopyPath | ActivityType::CopyPaths => {
            if level <= NixLogLevel::Info && !text.is_empty() {
                let msg = format!("📦 {}", shorten_store_path(&text));
                emit_deduped(ctx, &mut state.last_progress_text, msg);
            }
        }
        ActivityType::PostBuildHook => {
            let msg = format!("🪝 post-build hook: {}", text);
            ctx.emit_progress(&msg);
            push_bounded(&mut state.messages, msg, MAX_MESSAGES);
        }
        _ => {
            if level <= NixLogLevel::Info && !text.is_empty() {
                ctx.emit_progress(&text);
            }
        }
    }
}

/// Handle a nix "result" event — build log lines, phase changes, and progress.
fn handle_result_event(event: &NixEvent, ctx: &ToolContext, state: &mut NixOutputState) {
    let result_type = match ResultType::from_u64(event.activity_type) {
        Some(rt) => rt,
        None => return,
    };

    match result_type {
        ResultType::BuildLogLine => {
            if let Some(log_line) = event.fields.first().and_then(|v| v.as_str()) {
                let clean = strip_ansi(log_line);
                ctx.emit_progress(&format!("  │ {}", clean));
                push_bounded(&mut state.build_log_lines, clean, MAX_BUILD_LOG_LINES);
            }
        }
        ResultType::SetPhase => {
            if let Some(phase) = event.fields.first().and_then(|v| v.as_str()) {
                state.tracker.set_phase(event.id, phase.to_string());
                let activity_name = state.tracker.get(event.id).map(|a| shorten_drv_path(&a.text)).unwrap_or_default();
                let msg = format!("  ▸ phase: {} ({})", phase, activity_name);
                ctx.emit_progress(&msg);
                push_bounded(&mut state.messages, msg, MAX_MESSAGES);
            }
        }
        ResultType::Progress => {
            if event.fields.len() >= 2 {
                let done = event.fields[0].as_u64().unwrap_or(0);
                let expected = event.fields[1].as_u64().unwrap_or(0);
                if expected > 0 && done > 0 {
                    let label = state.tracker.get(event.id).map(|a| a.activity_type.label()).unwrap_or("progress");
                    let msg = format!("  {} {}/{}", label, done, expected);
                    emit_deduped(ctx, &mut state.last_progress_text, msg);
                }
            }
        }
        _ => {}
    }
}

/// Handle a nix "msg" event — error, warning, or info messages.
fn handle_msg_event(event: &NixEvent, ctx: &ToolContext, state: &mut NixOutputState) {
    let level = NixLogLevel::from_u64(event.level);
    let text = if !event.raw_msg.is_empty() {
        strip_ansi(&event.raw_msg)
    } else {
        strip_ansi(&event.msg)
    };

    if text.is_empty() {
        return;
    }

    match level {
        NixLogLevel::Error => {
            let msg = format!("✗ {}", text);
            ctx.emit_progress(&msg);
            push_bounded(&mut state.errors, text, MAX_ERRORS);
        }
        NixLogLevel::Warn => {
            let msg = format!("⚠ {}", text);
            ctx.emit_progress(&msg);
            push_bounded(&mut state.messages, msg, MAX_MESSAGES);
        }
        _ => {
            ctx.emit_progress(&text);
            push_bounded(&mut state.messages, text, MAX_MESSAGES);
        }
    }
}

/// Push to a Vec with an upper bound — drops oldest when full.
fn push_bounded(vec: &mut Vec<String>, item: String, max: usize) {
    debug_assert!(max > 0, "bound must be positive");
    if vec.len() >= max {
        // Drop the first 10% to amortize shifts
        let drop_count = max / 10;
        debug_assert!(drop_count > 0);
        vec.drain(..drop_count);
    }
    vec.push(item);
}

/// Emit progress only if the message differs from the last (deduplication).
fn emit_deduped(ctx: &ToolContext, last: &mut Option<String>, msg: String) {
    if last.as_deref() != Some(&msg) {
        ctx.emit_progress(&msg);
        *last = Some(msg);
    }
}

/// Format the final result for the LLM
pub fn format_nix_result(
    subcommand: &str,
    exit_code: i32,
    stdout_lines: &[String],
    build_log_lines: &[String],
    messages: &[String],
    errors: &[String],
) -> String {
    let mut parts = Vec::new();

    if exit_code != 0 {
        parts.push(format!("nix {} failed (exit code {})", subcommand, exit_code));
    }

    // Stdout (command output)
    if !stdout_lines.is_empty() {
        parts.push(stdout_lines.join("\n"));
    }

    // Build log (if any)
    if !build_log_lines.is_empty() {
        parts.push(format!("Build log:\n{}", build_log_lines.join("\n")));
    }

    // Errors
    if !errors.is_empty() {
        parts.push(format!("Errors:\n{}", errors.join("\n")));
    }

    // Info messages (only if there's no other output)
    if stdout_lines.is_empty() && build_log_lines.is_empty() && errors.is_empty() && !messages.is_empty() {
        parts.push(messages.join("\n"));
    }

    if parts.is_empty() {
        if exit_code == 0 {
            format!("nix {} completed successfully", subcommand)
        } else {
            format!("nix {} failed with exit code {}", subcommand, exit_code)
        }
    } else {
        parts.join("\n\n")
    }
}

/// Hash prefix length in nix store paths (32-char hash + 1-char dash)
const HASH_PREFIX_LEN: usize = 33;
/// Maximum URL length before truncation
const URL_MAX_LEN: usize = 80;
/// URL truncation point (leaves room for "...")
const URL_TRUNCATE_AT: usize = 77;
/// GitHub URL prefix for shortened display
const GITHUB_URL_DISPLAY_LEN: usize = 60;
/// GitHub URL path truncation point
const GITHUB_URL_TRUNCATE_AT: usize = 57;

/// Shorten a nix store path for display
/// "/nix/store/abc123-foo-1.0" -> "foo-1.0"
pub fn shorten_store_path(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("/nix/store/") {
        // Skip the 32-char hash + dash
        if rest.len() > HASH_PREFIX_LEN {
            return rest[HASH_PREFIX_LEN..].to_string();
        }
    }
    // Try extracting from longer text like "copying '/nix/store/...'"
    if let Some(start) = path.find("/nix/store/") {
        let from = start + "/nix/store/".len();
        let rest = &path[from..];
        // Find end of path (quote, space, or end)
        let end = rest.find(['\'', '"', ' ']).unwrap_or(rest.len());
        let store_suffix = &rest[..end];
        if store_suffix.len() > HASH_PREFIX_LEN {
            return store_suffix[HASH_PREFIX_LEN..].to_string();
        }
    }
    path.to_string()
}

/// Shorten a derivation path for display
/// "building '/nix/store/abc...-foo.drv'" -> "foo"
pub fn shorten_drv_path(text: &str) -> String {
    if let Some(start) = text.find("/nix/store/") {
        let from = start + "/nix/store/".len();
        let rest = &text[from..];
        let end = rest.find(['\'', '"', ' ']).unwrap_or(rest.len());
        let name = &rest[..end];
        // Strip hash prefix
        if name.len() > HASH_PREFIX_LEN {
            let short = &name[HASH_PREFIX_LEN..];
            // Strip .drv extension
            return short.strip_suffix(".drv").unwrap_or(short).to_string();
        }
    }
    text.to_string()
}

/// Shorten a URL for display
pub fn shorten_url(url: &str) -> String {
    // For github URLs, show just the relevant part
    if let Some(rest) = url.strip_prefix("https://github.com/")
        && rest.len() > GITHUB_URL_DISPLAY_LEN
    {
        return format!(
            "github:{}",
            &rest[..GITHUB_URL_TRUNCATE_AT]
                .rsplit_once('/')
                .map(|(l, _)| l)
                .unwrap_or(&rest[..GITHUB_URL_TRUNCATE_AT])
        );
    }
    // Trim long URLs
    if url.len() > URL_MAX_LEN {
        format!("{}...", &url[..URL_TRUNCATE_AT])
    } else {
        url.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shorten_store_path_strips_hash() {
        assert_eq!(shorten_store_path("/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1-hello-2.12"), "hello-2.12");
    }

    #[test]
    fn shorten_store_path_from_text() {
        assert_eq!(
            shorten_store_path("copying '/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1-glibc-2.39'"),
            "glibc-2.39"
        );
    }

    #[test]
    fn shorten_store_path_passthrough() {
        assert_eq!(shorten_store_path("something else"), "something else");
    }

    #[test]
    fn shorten_drv_path_extracts_name() {
        assert_eq!(
            shorten_drv_path("building '/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1-hello-2.12.drv'"),
            "hello-2.12"
        );
    }

    #[test]
    fn shorten_url_github() {
        let url =
            "https://github.com/NixOS/nixpkgs/archive/abc123def456abc123def456abc123def456abc123def456abc123.tar.gz";
        let short = shorten_url(url);
        assert!(short.len() < url.len());
        assert!(short.starts_with("github:"));
    }

    #[test]
    fn shorten_url_passthrough_short() {
        assert_eq!(shorten_url("https://example.com/short"), "https://example.com/short");
    }

    #[test]
    fn activity_type_labels() {
        assert_eq!(ActivityType::Build.label(), "building");
        assert_eq!(ActivityType::FileTransfer.label(), "downloading");
        assert_eq!(ActivityType::Substitute.label(), "fetching");
    }

    #[test]
    fn format_result_success_empty() {
        let result = format_nix_result("build", 0, &[], &[], &[], &[]);
        assert_eq!(result, "nix build completed successfully");
    }

    #[test]
    fn format_result_with_stdout() {
        let result = format_nix_result("build", 0, &["/nix/store/abc-hello".into()], &[], &[], &[]);
        assert_eq!(result, "/nix/store/abc-hello");
    }

    #[test]
    fn format_result_failure_with_errors() {
        let result = format_nix_result("build", 1, &[], &["make: error".into()], &[], &["builder failed".into()]);
        assert!(result.contains("failed (exit code 1)"));
        assert!(result.contains("Build log:"));
        assert!(result.contains("make: error"));
        assert!(result.contains("Errors:"));
        assert!(result.contains("builder failed"));
    }

    #[test]
    fn parse_nix_start_event() {
        let json = r#"{"action":"start","fields":["/nix/store/abc-test.drv","",1,1],"id":123,"level":3,"parent":0,"text":"building '/nix/store/abc-test.drv'","type":105}"#;
        let event: NixEvent = serde_json::from_str(json).expect("should parse start event");
        assert_eq!(event.action, "start");
        assert_eq!(event.id, 123);
        assert_eq!(ActivityType::from_u64(event.activity_type), ActivityType::Build);
    }

    #[test]
    fn parse_nix_msg_event() {
        let json = r#"{"action":"msg","level":0,"msg":"error: build failed","raw_msg":"build failed"}"#;
        let event: NixEvent = serde_json::from_str(json).expect("should parse msg event");
        assert_eq!(event.action, "msg");
        assert_eq!(NixLogLevel::from_u64(event.level), NixLogLevel::Error);
        assert_eq!(event.raw_msg, "build failed");
    }

    #[test]
    fn parse_nix_result_build_log() {
        let json = r#"{"action":"result","fields":["compiling main.rs"],"id":123,"type":101}"#;
        let event: NixEvent = serde_json::from_str(json).expect("should parse result event");
        assert_eq!(event.action, "result");
        assert_eq!(ResultType::from_u64(event.activity_type), Some(ResultType::BuildLogLine));
        assert_eq!(event.fields[0].as_str().expect("should have log line field"), "compiling main.rs");
    }
}
