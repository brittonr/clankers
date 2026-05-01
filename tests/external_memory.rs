use clankers::db::Db;
use clankers::db::memory::MemoryEntry;
use clankers::db::memory::MemoryScope;
use clankers::modes::common::ToolEnv;
use clankers::modes::common::ToolSet;
use clankers::modes::common::ToolTier;
use clankers::tools::ToolContext;
use clankers::tools::ToolResult;
use clankers::tools::ToolResultContent;
use serde_json::Value;
use serde_json::json;
use tokio_util::sync::CancellationToken;

fn external_memory_env(enabled: bool) -> ToolEnv {
    let mut settings = clankers::config::settings::Settings::default();
    settings.external_memory.enabled = enabled;
    settings.external_memory.name = Some("integration-memory".to_string());
    settings.external_memory.max_results = 2;
    ToolEnv {
        settings: Some(settings),
        ..Default::default()
    }
}

fn external_memory_tool(env: &ToolEnv) -> std::sync::Arc<dyn clankers::tools::Tool> {
    let tiered = clankers::modes::common::build_all_tiered_tools(env, None);
    let tool_set = ToolSet::new(tiered, [ToolTier::Specialty]);
    tool_set
        .active_tools()
        .into_iter()
        .find(|tool| tool.definition().name == "external_memory")
        .expect("external_memory tool should be published")
}

fn result_text(result: &ToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|content| match content {
            ToolResultContent::Text { text } => Some(text.as_str()),
            ToolResultContent::Image { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

fn tool_context(db: Option<Db>) -> ToolContext {
    let ctx = ToolContext::new("external-memory-call".to_string(), CancellationToken::new(), None);
    match db {
        Some(db) => ctx.with_db(db),
        None => ctx,
    }
}

#[tokio::test]
async fn configured_external_memory_searches_local_provider() {
    let db = Db::in_memory().unwrap();
    db.memory()
        .save(&MemoryEntry::new("Rust automation uses cargo-script", MemoryScope::Global))
        .unwrap();
    db.memory()
        .save(&MemoryEntry::new("Rust project memory is scoped", MemoryScope::Project {
            path: "/tmp/project".to_string(),
        }))
        .unwrap();
    db.memory().save(&MemoryEntry::new("unrelated note", MemoryScope::Global)).unwrap();

    let env = external_memory_env(true);
    let tool = external_memory_tool(&env);
    let result = tool
        .execute(&tool_context(Some(db)), json!({"action": "search", "query": "Rust", "limit": 5}))
        .await;

    assert!(!result.is_error, "expected search to succeed: {}", result_text(&result));
    let text = result_text(&result);
    assert!(text.contains("Found 2 external memories"));
    assert!(text.contains("Rust automation uses cargo-script"));
    assert!(text.contains("Rust project memory is scoped"));
    assert!(!text.contains("unrelated note"));

    let details = result.details.as_ref().expect("external memory attaches details");
    assert_eq!(details.get("source").and_then(Value::as_str), Some("external_memory_provider"));
    assert_eq!(details.get("providerKind").and_then(Value::as_str), Some("local"));
    assert_eq!(details.get("providerName").and_then(Value::as_str), Some("integration-memory"));
    assert_eq!(details.get("action").and_then(Value::as_str), Some("search"));
    assert_eq!(details.get("status").and_then(Value::as_str), Some("ok"));
    assert_eq!(details.get("resultCount").and_then(Value::as_u64), Some(2));
    assert!(details.get("query").is_none(), "debug metadata must not persist raw queries");
    assert!(details.get("results").is_none(), "debug metadata must not persist memory text");
}

#[tokio::test]
async fn configured_external_memory_requires_runtime_database() {
    let env = external_memory_env(true);
    let tool = external_memory_tool(&env);
    let result = tool.execute(&tool_context(None), json!({"action": "search", "query": "Rust"})).await;

    assert!(result.is_error);
    assert!(result_text(&result).contains("requires a database connection"));
    let details = result.details.as_ref().expect("failure includes safe details");
    assert_eq!(details.get("source").and_then(Value::as_str), Some("external_memory_provider"));
    assert_eq!(details.get("status").and_then(Value::as_str), Some("error"));
    assert_eq!(details.get("errorKind").and_then(Value::as_str), Some("missing_database"));
    assert!(details.get("query").is_none(), "failure metadata must not persist raw queries");
}

#[test]
fn external_memory_tool_is_not_published_when_disabled() {
    let env = external_memory_env(false);
    let tiered = clankers::modes::common::build_all_tiered_tools(&env, None);
    let tool_set = ToolSet::new(tiered, [ToolTier::Specialty]);
    assert!(
        tool_set.active_tools().iter().all(|tool| tool.definition().name != "external_memory"),
        "disabled externalMemory config must not publish the external_memory tool"
    );
}
