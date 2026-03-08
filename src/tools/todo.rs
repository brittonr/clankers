//! Todo tool — lets the agent manage a visible todo list in the TUI panel
//!
//! Supports: add, done, update (status/note), remove, list

use async_trait::async_trait;
use serde_json::Value;
use serde_json::json;
use tokio::sync::mpsc;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;

/// Messages sent from the tool to the TUI to mutate todo state
#[derive(Debug, Clone)]
pub enum TodoAction {
    Add { text: String },
    SetStatus { id: usize, status: String },
    SetStatusByText { query: String, status: String },
    SetNote { id: usize, note: String },
    Remove { id: usize },
    ClearDone,
    List,
}

/// Response sent back from the TUI after processing a TodoAction
#[derive(Debug, Clone)]
pub enum TodoResponse {
    Added { id: usize },
    Updated { id: usize },
    NotFound,
    Cleared,
    Listed { summary: String },
}

pub type TodoTx = mpsc::UnboundedSender<(TodoAction, oneshot::Sender<TodoResponse>)>;

/// Oneshot channel for synchronous request/response with the TUI
pub mod oneshot {
    pub use tokio::sync::oneshot::*;
}

pub struct TodoTool {
    definition: ToolDefinition,
    tx: Option<TodoTx>,
}

impl TodoTool {
    pub fn new() -> Self {
        Self {
            tx: None,
            definition: ToolDefinition {
                name: "todo".to_string(),
                description: "Manage a todo list visible in the TUI panel. Use this to track planned work, mark progress, and keep the user informed of your plan.\n\nActions:\n- add: Add a new item\n- done: Mark item as done (by id or text match)\n- status: Set status (pending/in-progress/done/blocked) by id or text\n- note: Add a note to an item\n- remove: Remove an item\n- list: Show all items".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["add", "done", "status", "note", "remove", "list", "clear_done"],
                            "description": "Action to perform"
                        },
                        "text": {
                            "type": "string",
                            "description": "Text for add, or search query for done/status"
                        },
                        "id": {
                            "type": "number",
                            "description": "Item ID (for done/status/note/remove)"
                        },
                        "status": {
                            "type": "string",
                            "description": "New status: pending, in-progress, done, blocked"
                        },
                        "note": {
                            "type": "string",
                            "description": "Note text to attach to an item"
                        }
                    },
                    "required": ["action"]
                }),
            },
        }
    }

    pub fn with_tx(mut self, tx: TodoTx) -> Self {
        self.tx = Some(tx);
        self
    }

    async fn send(&self, action: TodoAction) -> Result<TodoResponse, String> {
        let tx = self.tx.as_ref().ok_or("Todo panel not connected")?;
        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
        tx.send((action, resp_tx)).map_err(|_| "Todo panel channel closed")?;
        resp_rx.await.map_err(|_| "Todo panel did not respond".to_string())
    }
}

impl Default for TodoTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for TodoTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, _ctx: &ToolContext, params: Value) -> ToolResult {
        let action = match params.get("action").and_then(|v| v.as_str()) {
            Some(a) => a,
            None => return ToolResult::error("Missing required parameter: action"),
        };

        match action {
            "add" => self.handle_add(&params).await,
            "done" => self.handle_done(&params).await,
            "status" => self.handle_status(&params).await,
            "note" => self.handle_note(&params).await,
            "remove" => self.handle_remove(&params).await,
            "clear_done" => self.handle_clear_done().await,
            "list" => self.handle_list().await,
            _ => ToolResult::error(format!(
                "Unknown action '{}'. Use: add, done, status, note, remove, list, clear_done",
                action
            )),
        }
    }
}

impl TodoTool {
    async fn handle_add(&self, params: &Value) -> ToolResult {
        let text = match params.get("text").and_then(|v| v.as_str()) {
            Some(t) => t.to_string(),
            None => return ToolResult::error("Missing 'text' for add action"),
        };
        match self.send(TodoAction::Add { text: text.clone() }).await {
            Ok(TodoResponse::Added { id }) => ToolResult::text(format!("Added todo #{}: {}", id, text)),
            Ok(_) => ToolResult::text("Added.".to_string()),
            Err(e) => ToolResult::error(e),
        }
    }

    async fn handle_done(&self, params: &Value) -> ToolResult {
        let id = params.get("id").and_then(|v| v.as_u64()).map(|v| v as usize);
        let text = params.get("text").and_then(|v| v.as_str());

        let action = if let Some(id) = id {
            TodoAction::SetStatus {
                id,
                status: "done".into(),
            }
        } else if let Some(query) = text {
            TodoAction::SetStatusByText {
                query: query.into(),
                status: "done".into(),
            }
        } else {
            return ToolResult::error("Provide 'id' or 'text' to identify the item");
        };

        match self.send(action).await {
            Ok(TodoResponse::Updated { id }) => ToolResult::text(format!("Marked #{} as done.", id)),
            Ok(TodoResponse::NotFound) => ToolResult::error("No matching todo item found."),
            Ok(_) => ToolResult::text("Done.".to_string()),
            Err(e) => ToolResult::error(e),
        }
    }

    async fn handle_status(&self, params: &Value) -> ToolResult {
        let status = match params.get("status").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => return ToolResult::error("Missing 'status' parameter"),
        };
        let id = params.get("id").and_then(|v| v.as_u64()).map(|v| v as usize);
        let text = params.get("text").and_then(|v| v.as_str());

        let action = if let Some(id) = id {
            TodoAction::SetStatus { id, status }
        } else if let Some(query) = text {
            TodoAction::SetStatusByText {
                query: query.into(),
                status,
            }
        } else {
            return ToolResult::error("Provide 'id' or 'text' to identify the item");
        };

        match self.send(action).await {
            Ok(TodoResponse::Updated { id }) => ToolResult::text(format!("Updated #{} status.", id)),
            Ok(TodoResponse::NotFound) => ToolResult::error("No matching todo item found."),
            Ok(_) => ToolResult::text("Updated.".to_string()),
            Err(e) => ToolResult::error(e),
        }
    }

    async fn handle_note(&self, params: &Value) -> ToolResult {
        let id = match params.get("id").and_then(|v| v.as_u64()) {
            Some(id) => id as usize,
            None => return ToolResult::error("Missing 'id' for note action"),
        };
        let note = match params.get("note").and_then(|v| v.as_str()) {
            Some(n) => n.to_string(),
            None => return ToolResult::error("Missing 'note' text"),
        };
        match self.send(TodoAction::SetNote { id, note }).await {
            Ok(TodoResponse::Updated { id }) => ToolResult::text(format!("Added note to #{}.", id)),
            Ok(TodoResponse::NotFound) => ToolResult::error("No todo item with that ID."),
            Ok(_) => ToolResult::text("Updated.".to_string()),
            Err(e) => ToolResult::error(e),
        }
    }

    async fn handle_remove(&self, params: &Value) -> ToolResult {
        let id = match params.get("id").and_then(|v| v.as_u64()) {
            Some(id) => id as usize,
            None => return ToolResult::error("Missing 'id' for remove action"),
        };
        match self.send(TodoAction::Remove { id }).await {
            Ok(TodoResponse::Updated { .. }) => ToolResult::text(format!("Removed #{}.", id)),
            Ok(TodoResponse::NotFound) => ToolResult::error("No todo item with that ID."),
            Ok(_) => ToolResult::text("Removed.".to_string()),
            Err(e) => ToolResult::error(e),
        }
    }

    async fn handle_clear_done(&self) -> ToolResult {
        match self.send(TodoAction::ClearDone).await {
            Ok(_) => ToolResult::text("Cleared completed items."),
            Err(e) => ToolResult::error(e),
        }
    }

    async fn handle_list(&self) -> ToolResult {
        match self.send(TodoAction::List).await {
            Ok(TodoResponse::Listed { summary }) => ToolResult::text(summary),
            Ok(_) => ToolResult::text("No items.".to_string()),
            Err(e) => ToolResult::error(e),
        }
    }
}
