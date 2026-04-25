//! Ask tool — structured multi-choice/multi-select questions to the user
//!
//! Allows the agent to ask the user structured questions with:
//! - Single-choice selection (radio buttons)
//! - Multi-choice selection (checkboxes)
//! - Free text input
//! - Yes/no confirmation
//!
//! The tool renders the question in the TUI and waits for user input.

use async_trait::async_trait;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;

/// Channel types for ask tool communication
pub type AskRequest = tokio::sync::oneshot::Sender<AskResponse>;
pub type AskTx = tokio::sync::mpsc::UnboundedSender<(AskQuestion, AskRequest)>;
pub type AskRx = tokio::sync::mpsc::UnboundedReceiver<(AskQuestion, AskRequest)>;

/// A structured question
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AskQuestion {
    /// The question text
    pub text: String,
    /// Type of question
    pub question_type: QuestionType,
    /// Available options (for choice/multi types)
    pub options: Vec<AskOption>,
    /// Default value or selection
    pub default: Option<String>,
    /// Whether to allow skipping (empty answer)
    pub optional: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestionType {
    /// Single choice (radio buttons)
    Choice,
    /// Multi-select (checkboxes)
    Multi,
    /// Free text input
    Text,
    /// Yes/no confirmation
    Confirm,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AskOption {
    /// Short label for the option
    pub label: String,
    /// Longer description (optional)
    pub description: Option<String>,
    /// Value returned when selected (defaults to label if empty)
    pub value: Option<String>,
}

/// The user's response to a question
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AskResponse {
    /// The selected value(s) or text input
    pub values: Vec<String>,
    /// Whether the user skipped/cancelled
    pub skipped: bool,
}

impl AskResponse {
    pub fn single(value: String) -> Self {
        Self {
            values: vec![value],
            skipped: false,
        }
    }

    pub fn multi(values: Vec<String>) -> Self {
        Self { values, skipped: false }
    }

    pub fn skipped() -> Self {
        Self {
            values: Vec::new(),
            skipped: true,
        }
    }
}

pub struct AskTool {
    definition: ToolDefinition,
    tx: Option<AskTx>,
}

impl Default for AskTool {
    fn default() -> Self {
        Self::new()
    }
}

impl AskTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "ask".to_string(),
                description: "Ask the user a structured question. Supports single-choice, \
                    multi-select, free text, and yes/no confirmation. Use this when you need \
                    specific input from the user to proceed."
                    .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "question": {
                            "type": "string",
                            "description": "The question to ask the user"
                        },
                        "type": {
                            "type": "string",
                            "enum": ["choice", "multi", "text", "confirm"],
                            "description": "Type of question: 'choice' for single selection, 'multi' for multiple selection, 'text' for free input, 'confirm' for yes/no",
                            "default": "text"
                        },
                        "options": {
                            "type": "array",
                            "description": "Options for choice/multi questions",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "label": {"type": "string", "description": "Short label"},
                                    "description": {"type": "string", "description": "Longer description"},
                                    "value": {"type": "string", "description": "Return value (defaults to label)"}
                                },
                                "required": ["label"]
                            }
                        },
                        "default": {
                            "type": "string",
                            "description": "Default value or selection"
                        },
                        "optional": {
                            "type": "boolean",
                            "description": "Whether the user can skip this question",
                            "default": false
                        }
                    },
                    "required": ["question"]
                }),
            },
            tx: None,
        }
    }

    pub fn with_tx(mut self, tx: AskTx) -> Self {
        self.tx = Some(tx);
        self
    }
}

#[async_trait]
impl Tool for AskTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        let question_text = match params["question"].as_str() {
            Some(q) => q.to_string(),
            None => return ToolResult::error("Missing required parameter: question"),
        };

        let question_type = match params["type"].as_str().unwrap_or("text") {
            "choice" => QuestionType::Choice,
            "multi" => QuestionType::Multi,
            "confirm" => QuestionType::Confirm,
            _ => QuestionType::Text,
        };

        let options: Vec<AskOption> = params["options"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|o| {
                        Some(AskOption {
                            label: o["label"].as_str()?.to_string(),
                            description: o["description"].as_str().map(String::from),
                            value: o["value"].as_str().map(String::from),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let default = params["default"].as_str().map(String::from);
        let is_optional = params["optional"].as_bool().unwrap_or(false);

        let question = AskQuestion {
            text: question_text.clone(),
            question_type: question_type.clone(),
            options: options.clone(),
            default: default.clone(),
            optional: is_optional,
        };

        // If we have a channel to the TUI, send the question and wait for response
        if let Some(ref tx) = self.tx {
            let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
            if tx.send((question, resp_tx)).is_err() {
                return ToolResult::error("Ask channel closed — cannot communicate with TUI");
            }

            tokio::select! {
                result = resp_rx => {
                    match result {
                        Ok(response) => {
                            if response.skipped {
                                ToolResult::text("User skipped this question.")
                            } else if response.values.is_empty() {
                                ToolResult::text("No response provided.")
                            } else {
                                ToolResult::text(format!("User response: {}", response.values.join(", ")))
                            }
                        }
                        Err(_) => ToolResult::error("Failed to receive user response"),
                    }
                }
                () = ctx.signal.cancelled() => {
                    ToolResult::error("Ask cancelled")
                }
            }
        } else {
            // No TUI channel — format the question as text for non-interactive modes
            use std::fmt::Write;

            let mut out = format!("**Question:** {}\n\n", question_text);
            match question_type {
                QuestionType::Choice | QuestionType::Multi => {
                    let tag = if matches!(question_type, QuestionType::Multi) {
                        "Select one or more"
                    } else {
                        "Select one"
                    };
                    writeln!(out, "*{}:*", tag).ok();
                    for (i, opt) in options.iter().enumerate() {
                        write!(out, "  {}. {}", i + 1, opt.label).ok();
                        if let Some(ref desc) = opt.description {
                            write!(out, " — {}", desc).ok();
                        }
                        out.push('\n');
                    }
                }
                QuestionType::Confirm => {
                    let def = default.as_deref().unwrap_or("y");
                    writeln!(out, "*Confirm (y/n, default: {})*", def).ok();
                }
                QuestionType::Text => {
                    if let Some(ref def) = default {
                        writeln!(out, "*Default: {}*", def).ok();
                    }
                }
            }

            ToolResult::text(format!(
                "{}\n\n[Note: Running in non-interactive mode. \
                 Please provide the answer in your next message.]",
                out
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ask_tool_definition() {
        let tool = AskTool::new();
        assert_eq!(tool.definition().name, "ask");
    }

    #[test]
    fn test_ask_response_single() {
        let resp = AskResponse::single("option1".to_string());
        assert_eq!(resp.values, vec!["option1"]);
        assert!(!resp.skipped);
    }

    #[test]
    fn test_ask_response_skipped() {
        let resp = AskResponse::skipped();
        assert!(resp.skipped);
        assert!(resp.values.is_empty());
    }
}
