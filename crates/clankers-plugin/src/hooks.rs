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
    fn hook_to_event_kind(point: HookPoint) -> &'static str {
        match point {
            HookPoint::PreTool => "tool_call",
            HookPoint::PostTool => "tool_result",
            HookPoint::PrePrompt | HookPoint::PostPrompt => "user_input",
            HookPoint::SessionStart => "session_start",
            HookPoint::SessionEnd => "session_end",
            HookPoint::TurnStart => "turn_start",
            HookPoint::TurnEnd => "turn_end",
            HookPoint::ModelChange => "model_change",
            HookPoint::PreCommit | HookPoint::PostCommit => "",
            HookPoint::OnError => "",
        }
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
        let kind = Self::hook_to_event_kind(point);
        if kind.is_empty() {
            return false;
        }

        self.plugin_host.has_event_subscriber(kind)
    }

    async fn handle(&self, point: HookPoint, payload: &HookPayload) -> HookVerdict {
        let kind = Self::hook_to_event_kind(point);
        if kind.is_empty() {
            return HookVerdict::Continue;
        }

        // Build JSON payload matching plugin event protocol
        let event_json = serde_json::json!({
            "event": kind,
            "data": payload,
        });
        let input = event_json.to_string();

        // Dispatch to all subscribed plugins (blocking — Extism is sync)
        let result = tokio::task::spawn_blocking({
            let plugin_host = self.plugin_host.clone();
            let kind = kind.to_string();
            let input = input.clone();
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
