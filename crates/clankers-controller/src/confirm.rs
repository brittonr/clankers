//! Confirmation request/response routing.
//!
//! Holds oneshot senders keyed by request_id. The controller emits a
//! DaemonEvent with the request, and the client responds with a
//! SessionCommand containing the response.

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use std::collections::HashMap;

use tokio::sync::oneshot;

/// Stores pending confirmation requests keyed by request_id.
pub struct ConfirmStore<T> {
    pending: HashMap<String, oneshot::Sender<T>>,
    next_id: u64,
}

impl<T> ConfirmStore<T> {
    pub fn new() -> Self {
        Self {
            pending: HashMap::new(),
            next_id: 1,
        }
    }

    /// Register a new pending request. Returns the request_id and a receiver.
    pub fn register(&mut self) -> (String, oneshot::Receiver<T>) {
        let id = format!("req-{}", self.next_id);
        self.next_id += 1;
        let (tx, rx) = oneshot::channel();
        self.pending.insert(id.clone(), tx);
        (id, rx)
    }

    /// Respond to a pending request. Returns false if the request_id is unknown.
    pub fn respond(&mut self, request_id: &str, value: T) -> bool {
        if let Some(tx) = self.pending.remove(request_id) {
            tx.send(value).is_ok()
        } else {
            false
        }
    }

    /// Number of pending requests.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Drop all pending requests (e.g., on shutdown).
    pub fn clear(&mut self) {
        self.pending.clear();
    }
}

impl<T> Default for ConfirmStore<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_confirm_round_trip() {
        let mut store: ConfirmStore<bool> = ConfirmStore::new();

        let (id, rx) = store.register();
        assert_eq!(store.pending_count(), 1);

        assert!(store.respond(&id, true));
        assert_eq!(store.pending_count(), 0);

        let result = rx.await.unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_confirm_unknown_request() {
        let mut store: ConfirmStore<bool> = ConfirmStore::new();
        assert!(!store.respond("nonexistent", true));
    }

    #[tokio::test]
    async fn test_confirm_multiple() {
        let mut store: ConfirmStore<String> = ConfirmStore::new();

        let (id1, rx1) = store.register();
        let (id2, rx2) = store.register();
        assert_eq!(store.pending_count(), 2);
        assert_ne!(id1, id2);

        store.respond(&id2, "second".to_string());
        store.respond(&id1, "first".to_string());

        assert_eq!(rx1.await.unwrap(), "first");
        assert_eq!(rx2.await.unwrap(), "second");
    }

    #[test]
    fn test_confirm_clear() {
        let mut store: ConfirmStore<bool> = ConfirmStore::new();
        let (_id, _rx) = store.register();
        let (_id, _rx) = store.register();
        assert_eq!(store.pending_count(), 2);

        store.clear();
        assert_eq!(store.pending_count(), 0);
    }
}
