use serde_json::Value;
use serde_json::json;

use super::OPENAI_CODEX_BETA_HEADER;
use super::OPENAI_CODEX_NOT_ENTITLED_CODE;
use super::OPENAI_CODEX_PROVIDER;
use super::common;
use super::responses_url;
use crate::auth::StoredCredential;
use crate::auth::openai_codex_account_id_from_credential;
use crate::error::Error;
use crate::error::Result;
use crate::provider::CompletionRequest;

pub(crate) fn build_codex_request(
    client: &reqwest::Client,
    credential: &StoredCredential,
    request: &CompletionRequest,
) -> Result<reqwest::Request> {
    let token = credential.token().to_string();
    let account_id = openai_codex_account_id_from_credential(credential)?;
    let session_id = request.extra_params.get("_session_id").and_then(|value| value.as_str());
    let body = build_codex_request_body(request, session_id)?;

    let mut builder = client
        .post(responses_url())
        .header("authorization", format!("Bearer {token}"))
        .header("chatgpt-account-id", account_id)
        .header("OpenAI-Beta", OPENAI_CODEX_BETA_HEADER)
        .header("originator", "pi")
        .header("accept", "text/event-stream")
        .header("content-type", "application/json");

    if let Some(session_id) = session_id {
        builder = builder.header("session_id", session_id);
    }

    builder.json(&body).build().map_err(Into::into)
}

pub(crate) fn map_codex_error(status: u16, body_text: &str) -> Error {
    let friendly = serde_json::from_str::<serde_json::Value>(body_text)
        .ok()
        .and_then(|value| value.get("error").cloned())
        .and_then(|error| {
            let code = error.get("code").and_then(|value| value.as_str()).unwrap_or_default();
            let plan = error.get("plan_type").and_then(|value| value.as_str());
            if code.eq_ignore_ascii_case("usage_not_included") {
                let plan_suffix = plan.map(|value| format!(" ({value})")).unwrap_or_default();
                Some(format!("ChatGPT usage limit or entitlement block{plan_suffix}"))
            } else {
                error.get("message").and_then(|value| value.as_str()).map(str::to_string)
            }
        })
        .unwrap_or_else(|| body_text.to_string());

    if status == 401 {
        Error::Auth {
            message: if friendly.is_empty() {
                "OpenAI Codex account is unauthenticated".to_string()
            } else {
                friendly
            },
        }
    } else if status == 403 || body_text.contains(OPENAI_CODEX_NOT_ENTITLED_CODE) {
        Error::Auth {
            message: "authenticated but not entitled for Codex use. ChatGPT Plus or Pro is required for openai-codex"
                .to_string(),
        }
    } else {
        Error::provider_with_status(status, common::truncate(&friendly, 500))
    }
}

pub(crate) fn codex_model_id(model: &str) -> &str {
    model.strip_prefix(&format!("{OPENAI_CODEX_PROVIDER}/")).unwrap_or(model)
}

pub(crate) fn build_codex_request_body(request: &CompletionRequest, session_id: Option<&str>) -> Result<Value> {
    let mut extra = request.extra_params.clone();
    let text_override = extra.remove("text");
    let reasoning_override = extra.remove("reasoning");
    let verbosity_override = extra.remove("verbosity");
    extra.remove("_session_id");

    let mut body = json!({
        "model": codex_model_id(&request.model),
        "store": false,
        "stream": true,
        "input": build_codex_input(&request.messages)?,
        "text": {"verbosity": "medium"},
        "include": ["reasoning.encrypted_content"],
        "tool_choice": "auto",
        "parallel_tool_calls": true,
    });

    if let Some(system_prompt) = &request.system_prompt {
        body["instructions"] = json!(system_prompt);
    }

    if let Some(session_id) = session_id {
        body["prompt_cache_key"] = json!(session_id);
    }

    if !request.tools.is_empty() {
        body["tools"] = json!(
            request
                .tools
                .iter()
                .map(|tool| json!({
                    "type": "function",
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.input_schema,
                    "strict": null,
                }))
                .collect::<Vec<_>>()
        );
    }

    if let Some(temperature) = request.temperature {
        body["temperature"] = json!(temperature);
    }

    if let Some(thinking) = &request.thinking
        && thinking.enabled
    {
        body["reasoning"] = json!({
            "effort": "medium",
            "summary": "auto",
        });
    }

    if let Some(override_value) = verbosity_override
        && let Some(verbosity) = override_value.as_str()
    {
        body["text"] = json!({"verbosity": verbosity});
    }

    if let Some(override_value) = text_override {
        body["text"] = override_value;
    }

    if let Some(override_value) = reasoning_override {
        body["reasoning"] = override_value;
    }

    if let Some(map) = body.as_object_mut() {
        for (key, value) in extra {
            map.insert(key, value);
        }
    }

    Ok(body)
}

pub(crate) fn build_codex_input(messages: &[Value]) -> Result<Vec<Value>> {
    let mut input = Vec::new();

    for message in messages {
        let Some(role) = message.get("role").and_then(|value| value.as_str()) else {
            continue;
        };

        if role == "user" {
            if let Some(tool_results) = message.get("content").and_then(|value| value.as_array()).filter(|blocks| {
                blocks.iter().any(|block| block.get("type").and_then(|value| value.as_str()) == Some("tool_result"))
            }) {
                for block in tool_results {
                    if block.get("type").and_then(|value| value.as_str()) != Some("tool_result") {
                        continue;
                    }
                    let Some(call_id) =
                        block.get("tool_use_id").or_else(|| block.get("call_id")).and_then(|value| value.as_str())
                    else {
                        continue;
                    };
                    let output = extract_tool_result_text(block);
                    input.push(json!({
                        "type": "function_call_output",
                        "call_id": split_tool_call_id(call_id).0,
                        "output": output,
                    }));
                }
                continue;
            }

            let parts = build_user_parts(message.get("content"));
            if !parts.is_empty() {
                input.push(json!({
                    "type": "message",
                    "role": "user",
                    "content": parts,
                }));
            }
            continue;
        }

        if role != "assistant" {
            continue;
        }

        let Some(content) = message.get("content") else {
            continue;
        };
        if let Some(text) = content.as_str() {
            input.push(json!({
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": text, "annotations": []}],
            }));
            continue;
        }

        let Some(blocks) = content.as_array() else {
            continue;
        };

        let mut assistant_parts = Vec::new();
        for block in blocks {
            match block.get("type").and_then(|value| value.as_str()) {
                Some("thinking") => {
                    if let Some(signature) =
                        block.get("signature").and_then(|value| value.as_str()).filter(|value| !value.is_empty())
                    {
                        if let Ok(reasoning) = serde_json::from_str::<Value>(signature) {
                            input.push(reasoning);
                        }
                    }
                }
                Some("text") => {
                    if let Some(text) = block.get("text").and_then(|value| value.as_str()) {
                        assistant_parts.push(json!({"type": "output_text", "text": text, "annotations": []}));
                    }
                }
                Some("refusal") => {
                    if let Some(text) = block.get("text").and_then(|value| value.as_str()) {
                        assistant_parts.push(json!({"type": "refusal", "refusal": text}));
                    }
                }
                Some("tool_use") => {
                    if !assistant_parts.is_empty() {
                        input.push(json!({
                            "type": "message",
                            "role": "assistant",
                            "content": assistant_parts,
                            "status": "completed",
                        }));
                        assistant_parts = Vec::new();
                    }

                    let Some(id) = block.get("id").and_then(|value| value.as_str()) else {
                        continue;
                    };
                    let Some(name) = block.get("name").and_then(|value| value.as_str()) else {
                        continue;
                    };
                    let (call_id, item_id) = split_tool_call_id(id);
                    let arguments = serde_json::to_string(block.get("input").unwrap_or(&json!({})))
                        .unwrap_or_else(|_| "{}".to_string());
                    let mut item = json!({
                        "type": "function_call",
                        "call_id": call_id,
                        "name": name,
                        "arguments": arguments,
                    });
                    if let Some(item_id) = item_id {
                        item["id"] = json!(item_id);
                    }
                    input.push(item);
                }
                _ => {}
            }
        }

        if !assistant_parts.is_empty() {
            input.push(json!({
                "type": "message",
                "role": "assistant",
                "content": assistant_parts,
                "status": "completed",
            }));
        }
    }

    Ok(input)
}

pub(crate) fn build_user_parts(content: Option<&Value>) -> Vec<Value> {
    let Some(content) = content else {
        return Vec::new();
    };
    if let Some(text) = content.as_str() {
        return vec![json!({"type": "input_text", "text": text})];
    }

    let mut parts = Vec::new();
    let Some(blocks) = content.as_array() else {
        return parts;
    };

    for block in blocks {
        match block.get("type").and_then(|value| value.as_str()) {
            Some("text") => {
                if let Some(text) = block.get("text").and_then(|value| value.as_str()) {
                    parts.push(json!({"type": "input_text", "text": text}));
                }
            }
            Some("input_text") => parts.push(block.clone()),
            Some("image") => {
                if let Some(source) = block.get("source") {
                    parts.push(json!({"type": "input_image", "source": source}));
                } else if let (Some(media_type), Some(data)) = (
                    block.get("media_type").and_then(|value| value.as_str()),
                    block.get("data").and_then(|value| value.as_str()),
                ) {
                    parts.push(json!({
                        "type": "input_image",
                        "source": {
                            "type": "base64",
                            "media_type": media_type,
                            "data": data,
                        }
                    }));
                }
            }
            Some("input_image") => parts.push(block.clone()),
            _ => {}
        }
    }

    parts
}

pub(crate) fn extract_tool_result_text(block: &Value) -> String {
    if let Some(text) = block.get("output").and_then(|value| value.as_str()) {
        return text.to_string();
    }
    if let Some(content) = block.get("content").and_then(|value| value.as_array()) {
        let text = content
            .iter()
            .filter_map(|item| item.get("text").and_then(|value| value.as_str()))
            .collect::<Vec<_>>()
            .join("\n");
        if !text.is_empty() {
            return text;
        }
    }
    "(tool result)".to_string()
}

pub(crate) fn split_tool_call_id(id: &str) -> (&str, Option<&str>) {
    if let Some((call_id, item_id)) = id.split_once('|') {
        (call_id, Some(item_id))
    } else {
        (id, None)
    }
}
