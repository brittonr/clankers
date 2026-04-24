//! Tool handlers for the OpenSpec plugin.
//!
//! Each handler receives a `&serde_json::Value` (the tool call args) and
//! returns `Result<String, String>`. The host handles filesystem access —
//! handlers receive file contents as JSON fields.

use std::collections::HashSet;

use clanker_plugin_sdk::prelude::*;
use openspec::core::{
    artifact::ArtifactGraph,
    change::{parse_change_meta, parse_task_progress_content},
    schema::SchemaArtifact,
    spec::parse_spec_content,
    verify::verify_from_content,
};

// ── spec_list ───────────────────────────────────────────────────────

/// List specs from parsed file contents.
///
/// Input:
/// ```json
/// {"entries": [{"domain": "auth", "content": "# Auth spec\n..."}]}
/// ```
///
/// Output: JSON array of spec summaries.
pub fn handle_spec_list(args: &Value) -> Result<String, String> {
    let entries = args
        .get("entries")
        .and_then(|v| v.as_array())
        .ok_or("missing 'entries' array")?;

    let mut specs = Vec::new();
    for entry in entries {
        let domain = entry
            .get("domain")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let content = entry
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if let Some(spec) = parse_spec_content(content, domain) {
            let summary = serde_json::json!({
                "domain": spec.domain,
                "purpose": spec.purpose,
                "requirement_count": spec.requirements.len(),
                "requirements": spec.requirements.iter().map(|r| {
                    serde_json::json!({
                        "heading": r.heading,
                        "strength": format!("{:?}", r.strength),
                        "scenario_count": r.scenarios.len(),
                    })
                }).collect::<Vec<_>>(),
            });
            specs.push(summary);
        }
    }

    serde_json::to_string_pretty(&specs).map_err(|e| e.to_string())
}

// ── spec_parse ──────────────────────────────────────────────────────

/// Parse a single spec markdown file into structured requirements.
///
/// Input:
/// ```json
/// {"content": "# Spec\n## Purpose\n...", "domain": "auth"}
/// ```
pub fn handle_spec_parse(args: &Value) -> Result<String, String> {
    let content = args
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or("missing 'content' string")?;
    let domain = args
        .get("domain")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    let spec =
        parse_spec_content(content, domain).ok_or("failed to parse spec content")?;

    let result = serde_json::json!({
        "domain": spec.domain,
        "purpose": spec.purpose,
        "requirements": spec.requirements.iter().map(|r| {
            serde_json::json!({
                "heading": r.heading,
                "body": r.body,
                "strength": format!("{:?}", r.strength),
                "scenarios": r.scenarios.iter().map(|s| {
                    serde_json::json!({
                        "name": s.name,
                        "given": s.given,
                        "when": s.when,
                        "then": s.then,
                    })
                }).collect::<Vec<_>>(),
            })
        }).collect::<Vec<_>>(),
    });

    serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
}

// ── change_list ─────────────────────────────────────────────────────

/// List active changes with task progress.
///
/// Input:
/// ```json
/// {"entries": [{"name": "my-change", "meta_content": "schema: spec-driven\ncreated: ...", "tasks_content": "- [x] Done\n- [ ] Todo"}]}
/// ```
pub fn handle_change_list(args: &Value) -> Result<String, String> {
    let entries = args
        .get("entries")
        .and_then(|v| v.as_array())
        .ok_or("missing 'entries' array")?;

    let mut changes = Vec::new();
    for entry in entries {
        let name = entry
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let meta_content = entry
            .get("meta_content")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let tasks_content = entry
            .get("tasks_content")
            .and_then(|v| v.as_str());

        let (schema, created) = parse_change_meta(meta_content);

        let task_progress = tasks_content.and_then(parse_task_progress_content);

        let mut change = serde_json::json!({
            "name": name,
            "schema": schema,
            "created_at": created,
        });

        if let Some(progress) = task_progress {
            change["task_progress"] = serde_json::json!({
                "done": progress.done,
                "in_progress": progress.in_progress,
                "todo": progress.todo,
                "total": progress.total,
            });
        }

        changes.push(change);
    }

    serde_json::to_string_pretty(&changes).map_err(|e| e.to_string())
}

// ── change_verify ───────────────────────────────────────────────────

/// Verify a change by checking task completion and spec presence.
///
/// Input:
/// ```json
/// {"tasks_content": "- [x] Done\n- [ ] Todo", "has_specs_dir": true}
/// ```
pub fn handle_change_verify(args: &Value) -> Result<String, String> {
    let tasks_content = args
        .get("tasks_content")
        .and_then(|v| v.as_str());
    let has_specs_dir = args
        .get("has_specs_dir")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let report = verify_from_content(tasks_content, has_specs_dir);

    let result = serde_json::json!({
        "summary": report.summary(),
        "has_critical": report.has_critical(),
        "items": report.items.iter().map(|item| {
            serde_json::json!({
                "severity": format!("{:?}", item.severity),
                "message": item.message,
                "context": item.context,
            })
        }).collect::<Vec<_>>(),
    });

    serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
}

// ── artifact_status ─────────────────────────────────────────────────

/// Show artifact dependency graph state.
///
/// Input:
/// ```json
/// {
///   "schema_artifacts": [
///     {"id": "proposal", "generates": "proposal.md", "requires": []},
///     {"id": "specs", "generates": "specs/**/*.md", "requires": ["proposal"]}
///   ],
///   "existing_files": ["proposal.md"]
/// }
/// ```
pub fn handle_artifact_status(args: &Value) -> Result<String, String> {
    let raw_artifacts = args
        .get("schema_artifacts")
        .and_then(|v| v.as_array())
        .ok_or("missing 'schema_artifacts' array")?;

    let schema_artifacts: Vec<SchemaArtifact> = raw_artifacts
        .iter()
        .filter_map(|v| {
            Some(SchemaArtifact {
                id: v.get("id")?.as_str()?.to_string(),
                generates: v.get("generates")?.as_str()?.to_string(),
                requires: v
                    .get("requires")
                    .and_then(|r| r.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|s| s.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
            })
        })
        .collect();

    let existing: HashSet<String> = args
        .get("existing_files")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|s| s.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let graph = ArtifactGraph::from_state(&schema_artifacts, &existing);

    let next_ready = graph.next_ready().map(|a| a.id.clone());

    let result = serde_json::json!({
        "artifacts": graph.artifacts.iter().map(|a| {
            serde_json::json!({
                "id": a.id,
                "generates": a.generates,
                "requires": a.requires,
                "state": format!("{:?}", a.state),
            })
        }).collect::<Vec<_>>(),
        "next_ready": next_ready,
        "is_complete": graph.is_complete(),
    });

    serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
}
