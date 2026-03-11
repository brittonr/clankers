use serde_json::Value;

/// Result from a hook handler.
#[derive(Debug, Clone, Default)]
pub enum HookVerdict {
    /// Proceed unchanged.
    #[default]
    Continue,
    /// Proceed with a modified payload (pre-hooks only).
    Modify(Value),
    /// Abort the operation (pre-hooks only).
    Deny { reason: String },
}

impl HookVerdict {
    /// Merge two verdicts. Deny takes priority, then Modify, then Continue.
    pub fn merge(self, other: Self) -> Self {
        match (&self, &other) {
            (Self::Deny { .. }, _) => self,
            (_, Self::Deny { .. }) => other,
            (Self::Modify(_), _) => self,
            (_, Self::Modify(_)) => other,
            _ => Self::Continue,
        }
    }

    /// Check if this verdict allows the operation to proceed.
    pub fn is_allowed(&self) -> bool {
        !matches!(self, Self::Deny { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_continue_continue() {
        let v = HookVerdict::Continue.merge(HookVerdict::Continue);
        assert!(matches!(v, HookVerdict::Continue));
    }

    #[test]
    fn merge_deny_wins_over_continue() {
        let v = HookVerdict::Continue.merge(HookVerdict::Deny { reason: "no".into() });
        assert!(matches!(v, HookVerdict::Deny { .. }));
    }

    #[test]
    fn merge_deny_wins_over_modify() {
        let v = HookVerdict::Modify(serde_json::json!({})).merge(HookVerdict::Deny { reason: "blocked".into() });
        assert!(matches!(v, HookVerdict::Deny { .. }));
    }

    #[test]
    fn merge_modify_wins_over_continue() {
        let v = HookVerdict::Continue.merge(HookVerdict::Modify(serde_json::json!({"x": 1})));
        assert!(matches!(v, HookVerdict::Modify(_)));
    }

    #[test]
    fn first_deny_preserved() {
        let d1 = HookVerdict::Deny { reason: "first".into() };
        let d2 = HookVerdict::Deny { reason: "second".into() };
        let v = d1.merge(d2);
        match v {
            HookVerdict::Deny { reason } => assert_eq!(reason, "first"),
            _ => panic!("expected Deny"),
        }
    }

    #[test]
    fn is_allowed() {
        assert!(HookVerdict::Continue.is_allowed());
        assert!(HookVerdict::Modify(serde_json::json!({})).is_allowed());
        assert!(!HookVerdict::Deny { reason: "no".into() }.is_allowed());
    }
}
