//! Plugin hook handler — wraps plugin dispatch as a HookHandler.

use std::sync::Arc;
use std::sync::Mutex;

use async_trait::async_trait;
use clankers_hooks::dispatcher::HookHandler;
use clankers_hooks::dispatcher::PRIORITY_PLUGIN_HOOKS;
use clankers_hooks::payload::HookPayload;
use clankers_hooks::point::HookPoint;
use clankers_hooks::verdict::HookVerdict;

use crate::PluginHostFacade;
use crate::PluginManager;

/// Wraps `PluginManager` as a `HookHandler` so plugins participate in
/// the hook pipeline alongside script hooks and git hooks.
pub struct PluginHookHandler {
    plugin_host: PluginHostFacade,
}

impl PluginHookHandler {
    pub fn new(plugin_manager: Arc<Mutex<PluginManager>>) -> Self {
        Self {
            plugin_host: PluginHostFacade::new(plugin_manager),
        }
    }

    /// Map a HookPoint to the plugin event name string.
    fn hook_to_event_kind(point: HookPoint) -> Option<&'static str> {
        point.plugin_event_kind()
    }

    fn hook_event_input(point: HookPoint, payload: &HookPayload) -> Option<(&'static str, String)> {
        let kind = Self::hook_to_event_kind(point)?;
        let event_json = serde_json::json!({
            "event": kind,
            "data": payload,
        });
        Some((kind, event_json.to_string()))
    }
}

#[async_trait]
impl HookHandler for PluginHookHandler {
    fn name(&self) -> &str {
        "plugin"
    }

    fn priority(&self) -> u32 {
        PRIORITY_PLUGIN_HOOKS
    }

    fn subscribes_to(&self, point: HookPoint) -> bool {
        Self::hook_to_event_kind(point).is_some_and(|kind| self.plugin_host.has_event_subscriber(kind))
    }

    async fn handle(&self, point: HookPoint, payload: &HookPayload) -> HookVerdict {
        let Some((kind, input)) = Self::hook_event_input(point, payload) else {
            return HookVerdict::Continue;
        };

        // Dispatch to all subscribed plugins (blocking — Extism is sync)
        let result = tokio::task::spawn_blocking({
            let plugin_host = self.plugin_host.clone();
            let kind = kind.to_string();
            move || {
                let mut deny_reason = None;

                for info in plugin_host.event_subscribers(&kind) {
                    match plugin_host.call_plugin(&info.name, "on_event", &input) {
                        Ok(response) => {
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&response)
                                && val.get("deny").and_then(|v| v.as_bool()).unwrap_or(false)
                            {
                                let reason = val
                                    .get("reason")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("denied by plugin")
                                    .to_string();
                                deny_reason = Some(reason);
                                break;
                            }
                        }
                        Err(e) => {
                            tracing::warn!(plugin = %info.name, error = %e, "plugin hook dispatch failed");
                        }
                    }
                }

                deny_reason
            }
        })
        .await;

        match result {
            Ok(Some(reason)) if point.is_pre_hook() => HookVerdict::Deny { reason },
            _ => HookVerdict::Continue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_event_payloads_preserve_correlation_and_redact_safe_turn_fields() {
        let prompt_text = "deploy with token=super-secret-value";
        let tool_output_secret = "tool output contained sk-live-secret";
        let prompt_id = "prompt-correlation-1";
        let pre_prompt = HookPayload::prompt_with_metadata(
            "pre-prompt",
            "session-redaction",
            prompt_id,
            prompt_text,
            Some("system prompt must not appear in turn payload"),
            clankers_hooks::payload::HookStatus::Pending,
            None,
        );
        let post_turn = HookPayload::turn(
            "post-turn",
            "session-redaction",
            prompt_id,
            "test-model",
            prompt_text,
            4,
            1,
            clankers_hooks::payload::HookStatus::Success,
            Some(clankers_hooks::payload::HookSafeError::new(tool_output_secret, Some("tool_output"))),
            Some(clankers_hooks::payload::HookUsage {
                input_tokens: 3,
                output_tokens: 5,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            }),
        );

        let (pre_kind, pre_input) = PluginHookHandler::hook_event_input(HookPoint::PrePrompt, &pre_prompt)
            .expect("pre-prompt maps to plugin event");
        let (post_kind, post_input) = PluginHookHandler::hook_event_input(HookPoint::PostTurn, &post_turn)
            .expect("post-turn maps to plugin event");
        let pre_json: serde_json::Value = serde_json::from_str(&pre_input).expect("pre input is JSON");
        let post_json: serde_json::Value = serde_json::from_str(&post_input).expect("post input is JSON");

        assert_eq!(pre_kind, "user_input");
        assert_eq!(post_kind, "post_turn");
        assert_eq!(pre_json["event"], "user_input");
        assert_eq!(post_json["event"], "post_turn");
        assert_eq!(pre_json["data"]["prompt_id"], prompt_id);
        assert_eq!(post_json["data"]["prompt_id"], prompt_id);
        assert_eq!(post_json["data"]["kind"], "turn");
        assert_eq!(post_json["data"]["model"], "test-model");
        assert_eq!(post_json["data"]["tool_call_count"], 1);
        assert_eq!(post_json["data"]["prompt_digest"], pre_json["data"]["prompt_digest"]);
        assert_eq!(post_json["data"]["prompt_preview"], "[redacted secret-like text]");
        assert_eq!(post_json["data"]["error"]["message"], "[redacted secret-like text]");
        assert!(post_json["data"].get("text").is_none());
        assert!(post_json["data"].get("system_prompt").is_none());
        assert!(!post_input.contains("super-secret-value"));
        assert!(!post_input.contains("sk-live-secret"));
    }
}
