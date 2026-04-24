//! Nix build/develop/run tool with structured output streaming
//!
//! Parses nix's `--log-format internal-json` to provide clean, meaningful
//! progress updates instead of raw terminal noise. Supports all common nix
//! subcommands: build, develop, run, shell, flake check/show/update.
//!
//! Uses `clankers-nix` for typed flake ref validation (pre-spawn) and
//! structured store path parsing of build outputs.

mod build;
pub mod eval_tool;
mod parser;

use async_trait::async_trait;
use build::format_and_truncate_result;
use build::spawn_nix_command;
use build::stream_nix_output;
use build::supports_structured_logging;
use serde_json::Value;
use serde_json::json;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;
use super::ToolResultContent;

// ── Tool implementation ─────────────────────────────────────────────────────

pub struct NixTool {
    definition: ToolDefinition,
}

impl NixTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "nix".to_string(),
                description: "Run nix commands with streaming build output. Supports build, develop, run, shell, flake, eval, and other nix subcommands. Parses nix's internal-json structured logging for clean progress display (builds, downloads, fetches, phases). Use this instead of bash for nix commands.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "subcommand": {
                            "type": "string",
                            "description": "Nix subcommand (build, develop, run, shell, flake, eval, store, etc.)"
                        },
                        "args": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Arguments to pass after the subcommand (e.g. [\".#myPackage\", \"--no-link\"])"
                        },
                        "timeout": {
                            "type": "number",
                            "description": "Timeout in seconds (default: 600 for builds, 0 = no timeout)"
                        }
                    },
                    "required": ["subcommand"]
                }),
            },
        }
    }
}

impl Default for NixTool {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if store path annotation is enabled in settings.
///
/// Reads the `annotateStoreRefs` setting from the global/project config.
/// Returns false if the setting is absent or the config can't be read.
pub(crate) fn should_annotate_store_refs() -> bool {
    let config_paths = clankers_config::paths::ClankersPaths::resolve();
    let cwd = std::env::current_dir().unwrap_or_default();
    let project_paths = clankers_config::paths::ProjectPaths::resolve(&cwd);
    let settings = clankers_config::settings::Settings::load(&config_paths.global_settings, &project_paths.settings);
    settings.annotate_store_refs
}

/// Nix subcommands that accept flake references as arguments.
fn accepts_flake_ref(subcommand: &str) -> bool {
    matches!(subcommand, "build" | "run" | "develop" | "shell" | "eval" | "flake")
}

/// Format parsed store paths into a structured "[build outputs]" section.
fn format_build_outputs(paths: &[clankers_nix::NixPath]) -> String {
    let mut lines = vec!["[build outputs]".to_string()];
    for p in paths {
        if !p.is_derivation {
            lines.push(format!("  {}  {}", p.name, p.path));
        }
    }
    lines.join("\n")
}

/// Format a derivation summary for build failure context.
fn format_derivation_summary(info: &clankers_nix::DerivationInfo) -> String {
    let inputs: Vec<&str> = info.input_drvs.iter().map(|d| d.name.strip_suffix(".drv").unwrap_or(&d.name)).collect();

    let input_list = if inputs.is_empty() {
        "none".to_string()
    } else {
        inputs.join(", ")
    };

    format!(
        "[derivation: {}]\n  builder: {}\n  system: {}\n  inputs: {}",
        info.name, info.builder, info.system, input_list
    )
}

/// Append a section to a ToolResult's text content.
pub(crate) fn append_to_result(result: &mut ToolResult, section: &str) {
    if let Some(ToolResultContent::Text { text }) = result.content.first_mut() {
        text.push_str("\n\n");
        text.push_str(section);
    }
}

#[async_trait]
impl Tool for NixTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        let subcommand = match params.get("subcommand").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => return ToolResult::error("Missing required parameter: subcommand"),
        };

        let args: Vec<String> = params
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();

        let timeout_secs = params.get("timeout").and_then(|v| v.as_u64()).unwrap_or(0);

        // Pre-validate flake references before spawning the CLI.
        // Catches malformed refs early with actionable errors.
        if accepts_flake_ref(&subcommand) {
            for arg in &args {
                if clankers_nix::looks_like_flake_ref(arg)
                    && let Err(e) = clankers_nix::parse_flake_ref(arg)
                {
                    return ToolResult::error(format!("Invalid flake reference '{arg}': {e}"));
                }
            }
        }

        // Decide whether to use structured logging
        let is_structured = supports_structured_logging(&subcommand);

        // Spawn the nix command
        let mut child = match spawn_nix_command(&subcommand, &args, is_structured) {
            Ok(c) => c,
            Err(e) => return ToolResult::error(e),
        };

        // Stream output and collect results
        let (exit_code, stdout_lines, build_log_lines, messages, errors) =
            match stream_nix_output(ctx, &mut child, is_structured, timeout_secs, &subcommand).await {
                Ok(result) => result,
                Err(e) => return e,
            };

        // Format and truncate the result, with structured store path metadata
        let mut result =
            format_and_truncate_result(&subcommand, exit_code, &stdout_lines, &build_log_lines, &messages, &errors);

        // Append structured build output metadata for successful builds
        if exit_code == 0 && subcommand == "build" && !stdout_lines.is_empty() {
            let parsed = clankers_nix::extract_store_paths(&stdout_lines.join("\n"));
            if !parsed.is_empty() {
                let section = format_build_outputs(&parsed);
                append_to_result(&mut result, &section);
            }
        }

        // Annotate store path references in all output (opt-in via config)
        if should_annotate_store_refs() {
            let all_output =
                format!("{}\n{}\n{}", stdout_lines.join("\n"), build_log_lines.join("\n"), errors.join("\n"));
            if let Some(annotation) = clankers_nix::annotate_store_refs(&all_output) {
                append_to_result(&mut result, &annotation);
            }
        }

        // On build failure, try to surface derivation info from the error log
        if exit_code != 0 && subcommand == "build" {
            let all_text = format!("{}\n{}", errors.join("\n"), build_log_lines.join("\n"));
            let drv_paths: Vec<_> =
                clankers_nix::extract_store_paths(&all_text).into_iter().filter(|p| p.is_derivation).collect();

            if let Some(drv) = drv_paths.first() {
                let drv_path = std::path::Path::new(&drv.path);
                if drv_path.exists()
                    && let Ok(info) = clankers_nix::read_derivation(drv_path)
                {
                    let summary = format_derivation_summary(&info);
                    append_to_result(&mut result, &summary);
                }
            }
        }

        result
    }
}
