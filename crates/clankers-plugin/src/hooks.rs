//! Plugin hook handler — wraps plugin dispatch as a HookHandler.

use std::sync::Arc;
use std::sync::Mutex;

use async_trait::async_trait;
use clankers_hooks::dispatcher::HookHandler;
use clankers_hooks::dispatcher::PRIORITY_PLUGIN_HOOKS;
use clankers_hooks::payload::HookPayload;
use clankers_hooks::point::HookPoint;
use clankers_hooks::verdict::HookVerdict;

use crate::PluginManager;

/// Wraps `PluginManager` as a `HookHandler` so plugins participate in
/// the hook pipeline alongside script hooks and git hooks.
pub struct PluginHookHandler {
    plugin_manager: Arc<Mutex<PluginManager>>,
}

impl PluginHookHandler {
    pub fn new(plugin_manager: Arc<Mutex<PluginManager>>) -> Self {
        Self { plugin_manager }
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

        let pm = self.plugin_manager.lock().unwrap_or_else(|p| p.into_inner());
        pm.active_plugin_infos().any(|info| {
            info.manifest
                .events
                .iter()
                .any(|e| crate::bridge::PluginEvent::parse(e).is_some_and(|pe| pe.matches_event_kind(kind)))
        })
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
            let pm = self.plugin_manager.clone();
            let kind = kind.to_string();
            let input = input.clone();
            move || {
                let pm = pm.lock().unwrap_or_else(|p| p.into_inner());
                let mut deny_reason = None;

                let active_names: Vec<String> = pm
                    .active_plugin_infos()
                    .filter(|info| {
                        info.manifest.events.iter().any(|e| {
                            crate::bridge::PluginEvent::parse(e).is_some_and(|pe| pe.matches_event_kind(&kind))
                        })
                    })
                    .map(|info| info.name.clone())
                    .collect();

                for name in active_names {
                    match pm.call_plugin(&name, "on_event", &input) {
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
                            tracing::warn!(plugin = %name, error = %e, "plugin hook dispatch failed");
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
