use std::sync::Arc;

use async_trait::async_trait;
use tracing;

use crate::payload::HookPayload;
use crate::point::HookPoint;
use crate::verdict::HookVerdict;

/// Priority constants — lower number runs first.
pub const PRIORITY_GIT_HOOKS: u32 = 100;
pub const PRIORITY_SCRIPT_HOOKS: u32 = 200;
pub const PRIORITY_PLUGIN_HOOKS: u32 = 300;

/// A single hook handler (script, git, or plugin).
#[async_trait]
pub trait HookHandler: Send + Sync {
    /// Handler name for logging.
    fn name(&self) -> &str;
    /// Execution priority (lower = runs first).
    fn priority(&self) -> u32;
    /// Whether this handler cares about the given hook point.
    fn subscribes_to(&self, point: HookPoint) -> bool;
    /// Execute the hook. Returns a verdict for pre-hooks.
    async fn handle(&self, point: HookPoint, payload: &HookPayload) -> HookVerdict;
}

/// Dispatches hooks to all registered handlers in priority order.
pub struct HookPipeline {
    handlers: Vec<Arc<dyn HookHandler>>,
    disabled_hooks: std::collections::HashSet<String>,
}

impl HookPipeline {
    /// Create an empty pipeline.
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            disabled_hooks: std::collections::HashSet::new(),
        }
    }

    /// Register a handler. Handlers are sorted by priority on each fire.
    pub fn register(&mut self, handler: Arc<dyn HookHandler>) {
        self.handlers.push(handler);
        self.handlers.sort_by_key(|h| h.priority());
    }

    /// Mark specific hook points as disabled.
    pub fn set_disabled_hooks(&mut self, names: impl IntoIterator<Item = String>) {
        self.disabled_hooks = names.into_iter().collect();
    }

    /// Fire a hook synchronously (waits for all handlers).
    ///
    /// For pre-hooks, returns the merged verdict (Deny takes priority).
    /// For post-hooks, always returns Continue.
    pub async fn fire(&self, point: HookPoint, payload: &HookPayload) -> HookVerdict {
        if self.disabled_hooks.contains(point.to_filename()) {
            return HookVerdict::Continue;
        }

        let mut verdict = HookVerdict::Continue;

        for handler in &self.handlers {
            if !handler.subscribes_to(point) {
                continue;
            }

            tracing::debug!(hook = %point, handler = handler.name(), "firing hook");

            match handler.handle(point, payload).await {
                v @ HookVerdict::Deny { .. } => {
                    tracing::info!(
                        hook = %point,
                        handler = handler.name(),
                        "hook denied operation"
                    );
                    // Short-circuit on deny for pre-hooks
                    if point.is_pre_hook() {
                        return v;
                    }
                    verdict = verdict.merge(v);
                }
                v => {
                    verdict = verdict.merge(v);
                }
            }
        }

        verdict
    }

    /// Fire a hook asynchronously (fire-and-forget for post-hooks).
    /// Spawns a background task — does not block the caller.
    pub fn fire_async(&self, point: HookPoint, payload: HookPayload) {
        if self.disabled_hooks.contains(point.to_filename()) {
            return;
        }

        let handlers: Vec<Arc<dyn HookHandler>> =
            self.handlers.iter().filter(|h| h.subscribes_to(point)).cloned().collect();

        if handlers.is_empty() {
            return;
        }

        tokio::spawn(async move {
            for handler in &handlers {
                tracing::debug!(hook = %point, handler = handler.name(), "firing async hook");
                let _ = handler.handle(point, &payload).await;
            }
        });
    }

    /// Whether any handler is registered for this hook point.
    pub fn has_handlers(&self, point: HookPoint) -> bool {
        !self.disabled_hooks.contains(point.to_filename()) && self.handlers.iter().any(|h| h.subscribes_to(point))
    }

    /// Number of registered handlers.
    pub fn handler_count(&self) -> usize {
        self.handlers.len()
    }
}

impl Default for HookPipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::payload::HookData;

    struct AlwaysContinue;

    #[async_trait]
    impl HookHandler for AlwaysContinue {
        fn name(&self) -> &str {
            "always-continue"
        }
        fn priority(&self) -> u32 {
            200
        }
        fn subscribes_to(&self, _point: HookPoint) -> bool {
            true
        }
        async fn handle(&self, _point: HookPoint, _payload: &HookPayload) -> HookVerdict {
            HookVerdict::Continue
        }
    }

    struct AlwaysDeny;

    #[async_trait]
    impl HookHandler for AlwaysDeny {
        fn name(&self) -> &str {
            "always-deny"
        }
        fn priority(&self) -> u32 {
            100
        }
        fn subscribes_to(&self, point: HookPoint) -> bool {
            point.is_pre_hook()
        }
        async fn handle(&self, _point: HookPoint, _payload: &HookPayload) -> HookVerdict {
            HookVerdict::Deny {
                reason: "denied by test".into(),
            }
        }
    }

    struct OnlyTools;

    #[async_trait]
    impl HookHandler for OnlyTools {
        fn name(&self) -> &str {
            "only-tools"
        }
        fn priority(&self) -> u32 {
            150
        }
        fn subscribes_to(&self, point: HookPoint) -> bool {
            matches!(point, HookPoint::PreTool | HookPoint::PostTool)
        }
        async fn handle(&self, _point: HookPoint, _payload: &HookPayload) -> HookVerdict {
            HookVerdict::Continue
        }
    }

    fn test_payload() -> HookPayload {
        HookPayload {
            hook: "test".into(),
            session_id: "sess-1".into(),
            timestamp: chrono::Utc::now(),
            data: HookData::Empty {},
        }
    }

    #[tokio::test]
    async fn empty_pipeline_returns_continue() {
        let p = HookPipeline::new();
        let v = p.fire(HookPoint::PreTool, &test_payload()).await;
        assert!(matches!(v, HookVerdict::Continue));
    }

    #[tokio::test]
    async fn deny_handler_blocks_pre_hook() {
        let mut p = HookPipeline::new();
        p.register(Arc::new(AlwaysContinue));
        p.register(Arc::new(AlwaysDeny));
        let v = p.fire(HookPoint::PreTool, &test_payload()).await;
        assert!(matches!(v, HookVerdict::Deny { .. }));
    }

    #[tokio::test]
    async fn handlers_sorted_by_priority() {
        let mut p = HookPipeline::new();
        // Register in reverse priority order
        p.register(Arc::new(AlwaysContinue)); // 200
        p.register(Arc::new(AlwaysDeny)); // 100
        // AlwaysDeny (100) should run first — deny short-circuits
        let v = p.fire(HookPoint::PreTool, &test_payload()).await;
        assert!(matches!(v, HookVerdict::Deny { .. }));
    }

    #[tokio::test]
    async fn subscription_filtering() {
        let mut p = HookPipeline::new();
        p.register(Arc::new(OnlyTools));
        assert!(p.has_handlers(HookPoint::PreTool));
        assert!(p.has_handlers(HookPoint::PostTool));
        assert!(!p.has_handlers(HookPoint::PrePrompt));
    }

    #[tokio::test]
    async fn disabled_hooks_skipped() {
        let mut p = HookPipeline::new();
        p.register(Arc::new(AlwaysDeny));
        p.set_disabled_hooks(vec!["pre-tool".into()]);
        let v = p.fire(HookPoint::PreTool, &test_payload()).await;
        assert!(matches!(v, HookVerdict::Continue));
    }

    #[test]
    fn handler_count() {
        let mut p = HookPipeline::new();
        assert_eq!(p.handler_count(), 0);
        p.register(Arc::new(AlwaysContinue));
        assert_eq!(p.handler_count(), 1);
        p.register(Arc::new(AlwaysDeny));
        assert_eq!(p.handler_count(), 2);
    }
}
