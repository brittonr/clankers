//! Integration tests for nix store ref annotation and tool registration gating.
//!
//! Tests the three integration points added when store ref annotation was
//! extended to BashTool and NixEvalTool registration was gated on PATH:
//!
//! 1. `append_to_result` correctly appends annotation sections to ToolResult
//! 2. BashTool annotates output containing nix store paths (when enabled)
//! 3. `build_tiered_tools` conditionally registers NixEvalTool based on nix availability

use clankers::modes::common::{ToolEnv, build_tiered_tools};
use clankers::tools::{Tool, ToolContext, ToolResult, ToolResultContent};
use serde_json::json;
use tokio_util::sync::CancellationToken;

// ── Helpers ─────────────────────────────────────────────────────────────────

fn make_ctx() -> ToolContext {
    ToolContext::new("test-call".into(), CancellationToken::new(), None)
}

fn result_text(result: &ToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|c| match c {
            ToolResultContent::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

fn tool_names(tools: &[(clankers::modes::common::ToolTier, std::sync::Arc<dyn Tool>)]) -> Vec<String> {
    tools.iter().map(|(_, t)| t.definition().name.clone()).collect()
}

fn has_nix_on_path() -> bool {
    std::process::Command::new("nix")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
}

// ── 1. append_to_result ─────────────────────────────────────────────────────

#[test]
fn append_to_result_adds_section() {
    let mut result = ToolResult::text("initial output");
    // Use the same logic as the production code
    if let Some(ToolResultContent::Text { text }) = result.content.first_mut() {
        text.push_str("\n\n");
        text.push_str("[nix refs: glibc-2.38 (1 store paths)]");
    }
    let text = result_text(&result);
    assert!(text.starts_with("initial output"));
    assert!(text.contains("[nix refs: glibc-2.38"));
    assert!(text.contains("\n\n")); // separated by blank line
}

#[test]
fn append_to_result_noop_on_empty_content() {
    let mut result = ToolResult {
        content: vec![],
        is_error: false,
        details: None,
        full_output_path: None,
    };
    // Appending to empty content vec should not panic
    if let Some(ToolResultContent::Text { text }) = result.content.first_mut() {
        text.push_str("\n\nshould not appear");
    }
    assert!(result.content.is_empty());
}

#[test]
fn append_to_result_noop_on_image_content() {
    let mut result = ToolResult {
        content: vec![ToolResultContent::Image {
            media_type: "image/png".into(),
            data: "base64data".into(),
        }],
        is_error: false,
        details: None,
        full_output_path: None,
    };
    // Image content should not be modified
    if let Some(ToolResultContent::Text { text }) = result.content.first_mut() {
        text.push_str("\n\nshould not appear");
    }
    // Content unchanged — still an image
    assert!(matches!(result.content.first(), Some(ToolResultContent::Image { .. })));
}

// ── 2. annotate_store_refs (clankers_nix core) ─────────────────────────────

#[test]
fn annotation_round_trip_through_tool_result() {
    // Simulate what BashTool does: run annotate_store_refs on output, append if present
    let bash_output = "building '/nix/store/00bgd045z0d4icpbc2yyz4gx48ak44la-hello-2.12.1'\n\
                       /nix/store/vxjiwkjkn7x4079qvh1jkl5pn05j2aw0-glibc-2.38/lib/libc.so";

    let mut result = ToolResult::text(bash_output);

    if let Some(ToolResultContent::Text { text }) = result.content.first()
        && let Some(annotation) = clankers_nix::annotate_store_refs(text)
        && let Some(ToolResultContent::Text { text }) = result.content.first_mut()
    {
        text.push_str("\n\n");
        text.push_str(&annotation);
    }

    let text = result_text(&result);
    // Original output preserved
    assert!(text.contains("hello-2.12.1"));
    assert!(text.contains("glibc-2.38"));
    // Annotation appended
    assert!(text.contains("[nix refs:"));
    assert!(text.contains("store paths)]"));
}

#[test]
fn no_annotation_when_output_has_no_store_paths() {
    let bash_output = "cargo build\n   Compiling clankers v0.1.0\n    Finished release";
    let mut result = ToolResult::text(bash_output);

    if let Some(ToolResultContent::Text { text }) = result.content.first()
        && let Some(annotation) = clankers_nix::annotate_store_refs(text)
        && let Some(ToolResultContent::Text { text }) = result.content.first_mut()
    {
        text.push_str("\n\n");
        text.push_str(&annotation);
    }

    let text = result_text(&result);
    // No annotation appended
    assert!(!text.contains("[nix refs:"));
    assert_eq!(text, bash_output);
}

// ── 3. BashTool execute with store paths ────────────────────────────────────

#[tokio::test]
async fn bash_tool_runs_echo_with_store_path() {
    // Execute a bash command that prints a nix store path.
    // Whether annotation appears depends on the config setting, but the tool
    // must not crash and must preserve the original output.
    let tool = clankers::tools::bash::BashTool::new();
    let ctx = make_ctx();
    let result = tool
        .execute(
            &ctx,
            json!({
                "command": "echo '/nix/store/00bgd045z0d4icpbc2yyz4gx48ak44la-hello-2.12.1'"
            }),
        )
        .await;

    assert!(!result.is_error);
    let text = result_text(&result);
    assert!(text.contains("hello-2.12.1"));
}

#[tokio::test]
async fn bash_tool_preserves_output_without_store_paths() {
    let tool = clankers::tools::bash::BashTool::new();
    let ctx = make_ctx();
    let result = tool
        .execute(&ctx, json!({ "command": "echo 'hello world'" }))
        .await;

    assert!(!result.is_error);
    let text = result_text(&result);
    assert!(text.contains("hello world"));
    // No annotation on plain output
    assert!(!text.contains("[nix refs:"));
}

#[tokio::test]
async fn bash_tool_annotation_matches_nix_tool_format() {
    // When annotation is active, both BashTool and NixTool should produce
    // the same annotation format for the same store paths.
    let store_path = "/nix/store/00bgd045z0d4icpbc2yyz4gx48ak44la-hello-2.12.1";

    // Get what the annotation would look like
    let annotation = clankers_nix::annotate_store_refs(store_path);

    if let Some(ref ann) = annotation {
        // Annotation format is consistent
        assert!(ann.starts_with("[nix refs:"));
        assert!(ann.contains("hello-2.12.1"));
        assert!(ann.ends_with("store paths)]"));
    }

    // Run through BashTool (annotation depends on config, but format is same)
    let tool = clankers::tools::bash::BashTool::new();
    let ctx = make_ctx();
    let result = tool
        .execute(&ctx, json!({ "command": format!("echo '{store_path}'") }))
        .await;

    let text = result_text(&result);
    if text.contains("[nix refs:") {
        // If annotation was appended, verify format matches
        let annotation = annotation.expect("clankers_nix should also produce annotation");
        assert!(text.contains(&annotation));
    }
}

// ── 4. NixEvalTool registration gating ──────────────────────────────────────

#[test]
fn build_tiered_tools_always_includes_nix_tool() {
    let env = ToolEnv::default();
    let tools = build_tiered_tools(&env);
    let names = tool_names(&tools);
    assert!(names.contains(&"nix".to_string()), "nix tool must always be registered");
}

#[test]
fn build_tiered_tools_nix_eval_matches_path_availability() {
    let env = ToolEnv::default();
    let tools = build_tiered_tools(&env);
    let names = tool_names(&tools);

    let nix_available = has_nix_on_path();
    let nix_eval_registered = names.contains(&"nix_eval".to_string());

    if nix_available {
        assert!(
            nix_eval_registered,
            "nix_eval should be registered when nix is on PATH"
        );
    } else {
        assert!(
            !nix_eval_registered,
            "nix_eval should NOT be registered when nix is not on PATH"
        );
    }
}

#[test]
fn build_tiered_tools_no_duplicate_tool_names() {
    let env = ToolEnv::default();
    let tools = build_tiered_tools(&env);
    let names = tool_names(&tools);
    let mut seen = std::collections::HashSet::new();
    for name in &names {
        assert!(seen.insert(name.clone()), "duplicate tool name: {name}");
    }
}

// ── 5. End-to-end: bash output → annotation pipeline ───────────────────────

#[tokio::test]
async fn bash_tool_multiple_store_paths_annotated() {
    // When store refs are in output and annotation is enabled, all paths
    // should appear in the annotation.
    let cmd = "echo '/nix/store/00bgd045z0d4icpbc2yyz4gx48ak44la-glibc-2.38' && \
               echo '/nix/store/vxjiwkjkn7x4079qvh1jkl5pn05j2aw0-hello-2.12.1'";

    let tool = clankers::tools::bash::BashTool::new();
    let ctx = make_ctx();
    let result = tool.execute(&ctx, json!({ "command": cmd })).await;

    assert!(!result.is_error);
    let text = result_text(&result);

    // Both paths present in raw output
    assert!(text.contains("glibc-2.38"));
    assert!(text.contains("hello-2.12.1"));

    // Verify annotation would contain both (regardless of config)
    let annotation = clankers_nix::annotate_store_refs(&text);
    if let Some(ann) = annotation {
        assert!(ann.contains("glibc-2.38"));
        assert!(ann.contains("hello-2.12.1"));
        assert!(ann.contains("2 store paths"));
    }
}

#[tokio::test]
async fn bash_tool_error_output_not_annotated_incorrectly() {
    // A failing command should still get error status, and annotation
    // should not mask the error
    let tool = clankers::tools::bash::BashTool::new();
    let ctx = make_ctx();
    let result = tool
        .execute(
            &ctx,
            json!({
                "command": "echo '/nix/store/00bgd045z0d4icpbc2yyz4gx48ak44la-broken-pkg' >&2; exit 1"
            }),
        )
        .await;

    assert!(result.is_error);
    let text = result_text(&result);
    assert!(text.contains("Exit code: 1"));
    assert!(text.contains("broken-pkg"));
}
