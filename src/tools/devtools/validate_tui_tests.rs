use super::*;
use crate::tools::ToolResultContent;

#[tokio::test]
async fn validate_tui_basic_smoke() {
    let tool = ValidateTuiTool::new();
    let params = serde_json::json!({
        "description": "Basic smoke test",
        "rows": 24,
        "cols": 100,
        "steps": [
            {
                "action": { "type": "wait", "ms": 300 },
                "assert_visible": "NORMAL",
                "capture": true
            }
        ]
    });
    let result = tool.execute(&ToolContext::new("test-1".to_string(), CancellationToken::new(), None), params).await;
    assert!(!result.is_error, "Should pass: {:?}", result);
    let text = match &result.content[0] {
        ToolResultContent::Text { text } => text,
        _ => panic!("Expected text"),
    };
    assert!(text.contains("PASS"), "Should contain PASS: {}", text);
}

#[tokio::test]
async fn validate_tui_slash_command_and_panel() {
    let tool = ValidateTuiTool::new();
    let params = serde_json::json!({
        "description": "Todo panel appears after /todo add",
        "rows": 24,
        "cols": 120,
        "steps": [
            {
                "action": { "type": "slash_command", "command": "/todo add Test item" },
                "wait_for": "Added todo #1",
                "timeout_ms": 3000
            },
            {
                "action": { "type": "wait", "ms": 300 },
                "assert_visible": "Todo (",
                "capture": true
            }
        ]
    });
    let result = tool.execute(&ToolContext::new("test-2".to_string(), CancellationToken::new(), None), params).await;
    assert!(!result.is_error, "Should pass: {:?}", result);
    let text = match &result.content[0] {
        ToolResultContent::Text { text } => text,
        _ => panic!("Expected text"),
    };
    assert!(text.contains("PASS"), "Should contain PASS: {}", text);
}

#[tokio::test]
async fn validate_tui_panel_focus_with_backtick() {
    let tool = ValidateTuiTool::new();
    let params = serde_json::json!({
        "description": "Panel focus via backtick and spatial h/l navigation",
        "rows": 24,
        "cols": 200,
        "steps": [
            {
                "action": { "type": "slash_command", "command": "/todo add Task one" },
                "wait_for": "Added todo #1"
            },
            {
                "action": { "type": "key", "name": "esc" },
                "wait_for": "NORMAL"
            },
            {
                "action": { "type": "key", "name": "backtick" },
                "assert_visible": "z:zoom",
                "capture": true
            },
            {
                // h from right panel -> focus chat (spatial: chat is to the left)
                "action": { "type": "type", "text": "h" },
                "assert_absent": "z:zoom",
                "assert_visible": "h/l:panels"
            },
            {
                // l from main -> focus right panel again (spatial: right panels are to the right)
                "action": { "type": "type", "text": "l" },
                "assert_visible": "z:zoom",
                "capture": true
            },
            {
                "action": { "type": "key", "name": "esc" },
                "assert_absent": "z:zoom"
            }
        ]
    });
    let result = tool.execute(&ToolContext::new("test-3".to_string(), CancellationToken::new(), None), params).await;
    assert!(!result.is_error, "Should pass: {:?}", result);
    let text = match &result.content[0] {
        ToolResultContent::Text { text } => text,
        _ => panic!("Expected text"),
    };
    assert!(text.contains("PASS"), "Should contain PASS:\n{}", text);
}

#[tokio::test]
async fn validate_tui_failing_assertion() {
    let tool = ValidateTuiTool::new();
    let params = serde_json::json!({
        "description": "Should fail -- looking for text that doesn't exist",
        "rows": 24,
        "cols": 100,
        "steps": [
            {
                "action": { "type": "wait", "ms": 200 },
                "assert_visible": "THIS TEXT DOES NOT EXIST",
                "capture": true
            }
        ]
    });
    let result = tool.execute(&ToolContext::new("test-4".to_string(), CancellationToken::new(), None), params).await;
    assert!(result.is_error, "Should fail");
    let text = match &result.content[0] {
        ToolResultContent::Text { text } => text,
        _ => panic!("Expected text"),
    };
    assert!(text.contains("FAIL"), "Should contain FAIL: {}", text);
}
