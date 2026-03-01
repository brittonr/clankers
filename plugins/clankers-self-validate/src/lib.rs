//! clankers self-validate plugin — WASM module for self-validating development.
//!
//! This plugin provides tools that spawn a separate clankers validator instance
//! to verify code changes, run tests, and review work.
//!
//! ## Architecture
//!
//! The WASM plugin handles JSON dispatch and prompt construction. The actual
//! validation is performed by the host (clankers) which spawns a subprocess.
//! Since WASM plugins can't spawn processes directly, the heavy lifting
//! happens in two phases:
//! 1. Plugin builds a structured validation request (this WASM)
//! 2. Host-side tool adapter executes the validation subprocess

use clankers_plugin_sdk::prelude::*;

// ── Extended tool result with validator metadata ────────────────────

#[derive(Serialize)]
struct ValidateResult {
    tool: String,
    result: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    meta: Option<ValidatorMeta>,
}

#[derive(Serialize)]
struct ValidatorMeta {
    prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    agent: Option<String>,
    severity: String,
}

impl ValidateResult {
    fn ok(tool: &str, prompt: String, cwd: Option<String>, agent: Option<String>, severity: String) -> Self {
        Self {
            tool: tool.to_string(),
            result: prompt.clone(),
            status: "ok".to_string(),
            meta: Some(ValidatorMeta { prompt, cwd, agent, severity }),
        }
    }

    fn error(tool: &str, msg: &str) -> Self {
        Self {
            tool: tool.to_string(),
            result: msg.to_string(),
            status: "error".to_string(),
            meta: None,
        }
    }

    fn passthrough(tool: &str, result: String) -> Self {
        Self {
            tool: tool.to_string(),
            result,
            status: "ok".to_string(),
            meta: None,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  Extism guest functions
// ═══════════════════════════════════════════════════════════════════════

#[plugin_fn]
pub fn handle_tool_call(input: String) -> FnResult<String> {
    let call: ToolCall = clankers_plugin_sdk::serde_json::from_str(&input)
        .map_err(|e| Error::msg(format!("Invalid JSON input: {e}")))?;

    let result = match call.tool.as_str() {
        "validate" => handle_validate(&call.args),
        "validate_review" => handle_validate_review(&call.args),
        "validate_test" => handle_validate_test(&call.args),
        "validate_check" => handle_validate_check(&call.args),
        "validate_visual" => handle_validate_visual(&call.args),
        "validate_tui" => handle_validate_tui(&call.args),
        other => ValidateResult {
            tool: other.to_string(),
            result: format!("Unknown validation tool: {other}"),
            status: "unknown_tool".to_string(),
            meta: None,
        },
    };

    Ok(clankers_plugin_sdk::serde_json::to_string(&result)?)
}

#[plugin_fn]
pub fn on_event(input: String) -> FnResult<String> {
    dispatch_events(&input, "clankers-self-validate", &[
        ("agent_start", |_| "Self-validate plugin ready".to_string()),
        ("agent_end", |_| "Self-validate plugin shutting down".to_string()),
        ("tool_call", |data| {
            let tool = data.get_str("tool").unwrap_or("unknown");
            format!("Observed tool call: {tool}")
        }),
        ("tool_result", |data| {
            let tool = data.get_str("tool").unwrap_or("unknown");
            let is_error = data.get_bool_or("is_error", false);
            if is_error {
                format!("Tool '{tool}' returned an error — consider validation")
            } else {
                format!("Tool '{tool}' completed successfully")
            }
        }),
    ])
}

#[plugin_fn]
pub fn describe(Json(_): Json<()>) -> FnResult<Json<PluginMeta>> {
    Ok(Json(PluginMeta::new("clankers-self-validate", "0.1.0", &[
        ("validate", "Custom validation task"),
        ("validate_review", "Code review validation"),
        ("validate_test", "Run tests and report"),
        ("validate_check", "Quick compile/lint/format checks"),
        ("validate_visual", "Screenshot-based visual validation"),
        ("validate_tui", "PTY-based TUI interaction testing"),
    ], &["validate"])))
}

// ═══════════════════════════════════════════════════════════════════════
//  Tool handlers
// ═══════════════════════════════════════════════════════════════════════

fn handle_validate(args: &Value) -> ValidateResult {
    let task = args.get_str("task").unwrap_or("");
    let context = args.get_str("context").unwrap_or("");
    let cwd = args.get_str("cwd").map(String::from);
    let agent = args.get_str("agent").map(String::from);
    let severity = args.get_str_or("severity", "normal").to_string();

    if task.is_empty() {
        return ValidateResult::error("validate", "Missing required 'task' parameter");
    }

    let mut prompt = format!(
        "You are a code validator. Your job is to validate the following task thoroughly and report your findings.\n\n\
         ## Validation Task\n{task}\n"
    );
    if !context.is_empty() {
        prompt.push_str(&format!("\n## Context\n{context}\n"));
    }
    prompt.push_str(&format!(
        "\n## Severity Level: {severity}\n{}\n\n\
         ## Instructions\n\
         1. Use the available tools to inspect files, run commands, and gather information\n\
         2. Be thorough but focused on the specific validation task\n\
         3. Report your findings clearly with:\n\
            - PASS/FAIL status\n\
            - Summary of what was checked\n\
            - Any issues found (with file paths and line numbers)\n\
            - Suggestions for fixes if applicable\n\
         4. Format your response as a structured validation report\n",
        severity_description(&severity)
    ));

    ValidateResult::ok("validate", prompt, cwd, agent, severity)
}

fn handle_validate_review(args: &Value) -> ValidateResult {
    let files = args.get_str_array("files");
    let focus = args.get_str("focus").unwrap_or("");
    let cwd = args.get_str("cwd").map(String::from);

    let files_section = if files.is_empty() {
        "Review all uncommitted changes (use `bash` to run `git diff` and `git diff --staged`).".to_string()
    } else {
        format!("Review the following specific files:\n{}", files.iter().map(|f| format!("- {f}")).collect::<Vec<_>>().join("\n"))
    };

    let focus_section = if focus.is_empty() {
        "General code review — check for bugs, style issues, correctness, and potential improvements.".to_string()
    } else {
        format!("Focus your review on: **{focus}**")
    };

    let prompt = format!(
        "You are a senior code reviewer. Perform a thorough code review and report your findings.\n\n\
         ## Files to Review\n{files_section}\n\n\
         ## Review Focus\n{focus_section}\n\n\
         ## Instructions\n\
         1. Read each file/diff carefully\n\
         2. Check for: bugs, error handling gaps, performance, security, API compat, style, missing tests\n\
         3. Report: overall assessment (APPROVE/REQUEST_CHANGES/COMMENT), findings with severity, summary\n"
    );

    ValidateResult::ok("validate_review", prompt, cwd, None, "normal".to_string())
}

fn handle_validate_test(args: &Value) -> ValidateResult {
    let command = args.get_str("command").unwrap_or("");
    let filter = args.get_str("filter").unwrap_or("");
    let cwd = args.get_str("cwd").map(String::from);
    let fix = args.get_bool_or("fix", false);

    let test_cmd = if !command.is_empty() {
        format!("Run this test command: `{command}`")
    } else {
        "Auto-detect the test runner (Cargo.toml→cargo test, package.json→npm test, etc).".to_string()
    };

    let filter_section = if !filter.is_empty() {
        format!("Apply test filter: `{filter}`")
    } else {
        "Run all tests.".to_string()
    };

    let fix_section = if fix {
        "\n## Fix Mode\nIf tests fail, analyze the failures and suggest specific fixes with code snippets.\n"
    } else {
        ""
    };

    let prompt = format!(
        "You are a test runner and analyzer. Run tests and report the results.\n\n\
         ## Test Command\n{test_cmd}\n\n## Scope\n{filter_section}\n\n\
         ## Instructions\n\
         1. Run the test command using `bash`\n\
         2. Report: total passed/failed/skipped, each failure with name+error+file, execution time\n\
         {fix_section}\n"
    );

    ValidateResult::ok("validate_test", prompt, cwd, None, "normal".to_string())
}

fn handle_validate_check(args: &Value) -> ValidateResult {
    let checks = args.get_str_array("checks");
    let cwd = args.get_str("cwd").map(String::from);

    let effective = if checks.is_empty() {
        vec!["compile".to_string()]
    } else if checks.iter().any(|c| c == "all") {
        vec!["compile".to_string(), "lint".to_string(), "format".to_string(), "types".to_string()]
    } else {
        checks
    };

    let checks_section = effective.iter().map(|check| match check.as_str() {
        "compile" => "### Compile Check\nRust: `cargo check`, TypeScript: `tsc --noEmit`, Go: `go build ./...`",
        "lint" => "### Lint Check\nRust: `cargo clippy -- -D warnings`, JS/TS: `npx eslint .`, Python: `ruff check .`",
        "format" => "### Format Check\nRust: `cargo fmt -- --check`, JS/TS: `npx prettier --check .`, Python: `ruff format --check .`",
        "types" => "### Type Check\nTypeScript: `tsc --noEmit`, Python: `mypy .` or `pyright`",
        _ => "### Unknown check — skip",
    }).collect::<Vec<_>>().join("\n\n");

    let prompt = format!(
        "You are a quick validation checker. Run the requested checks and report results concisely.\n\n\
         ## Checks to Run\n\n{checks_section}\n\n\
         ## Instructions\n\
         1. Auto-detect the project type\n\
         2. Run each applicable check\n\
         3. Report: ✓ PASS or ✗ FAIL per check, error/warning count, summary\n"
    );

    ValidateResult::ok("validate_check", prompt, cwd, None, "normal".to_string())
}

fn handle_validate_visual(args: &Value) -> ValidateResult {
    let description = args.get_str("description").unwrap_or("");
    let target = args.get_str_or("target", "screen");
    let checks = args.get_str_array("checks");
    let delay = args.get_f64_or("delay", 1.0);
    let cwd = args.get_str("cwd").map(String::from);

    if description.is_empty() {
        return ValidateResult::error("validate_visual", "Missing required 'description' parameter");
    }

    let checks_section = if checks.is_empty() {
        "Check for any visual issues: rendering corruption, layout problems, missing elements, incorrect styling.".to_string()
    } else {
        format!("Specifically verify:\n{}", checks.iter().map(|c| format!("- {c}")).collect::<Vec<_>>().join("\n"))
    };

    let prompt = format!(
        "You are a visual validator. Take a screenshot and verify the visual state.\n\n\
         ## Expected Visual State\n{description}\n\n\
         ## Visual Checks\n{checks_section}\n\n\
         ## Instructions\n\
         1. Use `screenshot(target=\"{target}\", delay={delay})`\n\
         2. Compare against the expected description\n\
         3. Report PASS/FAIL with specifics\n"
    );

    ValidateResult::ok("validate_visual", prompt, cwd, None, "normal".to_string())
}

fn handle_validate_tui(args: &Value) -> ValidateResult {
    let description = args.get_str("description").unwrap_or("TUI validation");
    let steps = args.get_array("steps");

    let step_count = steps.map(|a| a.len()).unwrap_or(0);
    if step_count == 0 {
        return ValidateResult::error("validate_tui", "Missing or empty 'steps' array");
    }

    // For validate_tui, the host tool handles execution directly.
    ValidateResult::passthrough(
        "validate_tui",
        format!("TUI validation: {} ({} steps)", description, step_count),
    )
}

// ── Helpers ─────────────────────────────────────────────────────────

fn severity_description(severity: &str) -> &str {
    match severity {
        "strict" => "STRICT: Fail on any warnings or potential issues. Be very thorough.",
        "lenient" => "LENIENT: Only fail on actual errors. Ignore style and minor issues.",
        _ => "NORMAL: Fail on errors and significant warnings. Flag minor issues as notes.",
    }
}
