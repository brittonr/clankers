//! Tests for SessionCatalog CRUD and lifecycle transitions (task 1.8).

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use clankers_protocol::SessionKey;

    use crate::modes::daemon::session_store::{
        SessionCatalog, SessionCatalogEntry, SessionLifecycle,
    };

    fn temp_catalog() -> (tempfile::TempDir, Arc<SessionCatalog>) {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("test.db");
        let db = Arc::new(redb::Database::create(&db_path).unwrap());
        let catalog = Arc::new(SessionCatalog::new(db));
        (tmp, catalog)
    }

    fn make_entry(id: &str, state: SessionLifecycle) -> SessionCatalogEntry {
        SessionCatalogEntry {
            session_id: id.to_string(),
            automerge_path: PathBuf::from(format!("/tmp/sessions/{id}.automerge")),
            model: "claude-sonnet-4-20250514".to_string(),
            created_at: "2026-03-20T10:00:00Z".to_string(),
            last_active: "2026-03-20T10:00:00Z".to_string(),
            turn_count: 0,
            state,
        }
    }

    #[test]
    fn insert_and_get() {
        let (_tmp, catalog) = temp_catalog();
        let entry = make_entry("sess-1", SessionLifecycle::Active);
        catalog.insert_session(&entry);

        let got = catalog.get_session("sess-1").unwrap();
        assert_eq!(got.session_id, "sess-1");
        assert_eq!(got.state, SessionLifecycle::Active);
        assert_eq!(got.model, "claude-sonnet-4-20250514");
    }

    #[test]
    fn get_missing_returns_none() {
        let (_tmp, catalog) = temp_catalog();
        assert!(catalog.get_session("no-such-session").is_none());
    }

    #[test]
    fn update_overwrites() {
        let (_tmp, catalog) = temp_catalog();
        let mut entry = make_entry("sess-1", SessionLifecycle::Active);
        catalog.insert_session(&entry);

        entry.turn_count = 42;
        entry.model = "opus".to_string();
        catalog.update_session(&entry);

        let got = catalog.get_session("sess-1").unwrap();
        assert_eq!(got.turn_count, 42);
        assert_eq!(got.model, "opus");
    }

    #[test]
    fn list_sessions_returns_all() {
        let (_tmp, catalog) = temp_catalog();
        catalog.insert_session(&make_entry("a", SessionLifecycle::Active));
        catalog.insert_session(&make_entry("b", SessionLifecycle::Suspended));
        catalog.insert_session(&make_entry("c", SessionLifecycle::Tombstoned));

        let all = catalog.list_sessions();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn list_by_state_filters() {
        let (_tmp, catalog) = temp_catalog();
        catalog.insert_session(&make_entry("a", SessionLifecycle::Active));
        catalog.insert_session(&make_entry("b", SessionLifecycle::Active));
        catalog.insert_session(&make_entry("c", SessionLifecycle::Suspended));
        catalog.insert_session(&make_entry("d", SessionLifecycle::Tombstoned));

        assert_eq!(catalog.list_by_state(SessionLifecycle::Active).len(), 2);
        assert_eq!(catalog.list_by_state(SessionLifecycle::Suspended).len(), 1);
        assert_eq!(catalog.list_by_state(SessionLifecycle::Tombstoned).len(), 1);
    }

    #[test]
    fn remove_session_deletes() {
        let (_tmp, catalog) = temp_catalog();
        catalog.insert_session(&make_entry("x", SessionLifecycle::Active));
        assert!(catalog.get_session("x").is_some());

        catalog.remove_session("x");
        assert!(catalog.get_session("x").is_none());
    }

    #[test]
    fn set_state_transitions() {
        let (_tmp, catalog) = temp_catalog();
        catalog.insert_session(&make_entry("s1", SessionLifecycle::Active));

        catalog.set_state("s1", SessionLifecycle::Suspended);
        assert_eq!(
            catalog.get_session("s1").unwrap().state,
            SessionLifecycle::Suspended
        );

        catalog.set_state("s1", SessionLifecycle::Tombstoned);
        assert_eq!(
            catalog.get_session("s1").unwrap().state,
            SessionLifecycle::Tombstoned
        );
    }

    #[test]
    fn set_state_missing_session_is_noop() {
        let (_tmp, catalog) = temp_catalog();
        // Should not panic
        catalog.set_state("ghost", SessionLifecycle::Active);
    }

    #[test]
    fn transition_all_batch() {
        let (_tmp, catalog) = temp_catalog();
        catalog.insert_session(&make_entry("a", SessionLifecycle::Active));
        catalog.insert_session(&make_entry("b", SessionLifecycle::Active));
        catalog.insert_session(&make_entry("c", SessionLifecycle::Suspended));

        let count = catalog.transition_all(
            SessionLifecycle::Active,
            SessionLifecycle::Suspended,
        );
        assert_eq!(count, 2);

        assert_eq!(catalog.list_by_state(SessionLifecycle::Active).len(), 0);
        assert_eq!(catalog.list_by_state(SessionLifecycle::Suspended).len(), 3);
    }

    #[test]
    fn transition_all_empty_returns_zero() {
        let (_tmp, catalog) = temp_catalog();
        let count = catalog.transition_all(
            SessionLifecycle::Active,
            SessionLifecycle::Suspended,
        );
        assert_eq!(count, 0);
    }

    // ── Key index tests ─────────────────────────────────────────────

    #[test]
    fn insert_and_lookup_key() {
        let (_tmp, catalog) = temp_catalog();
        let key = SessionKey::Iroh("abc123def456".to_string());
        catalog.insert_key(&key, "sess-1");

        assert_eq!(catalog.lookup_key(&key).unwrap(), "sess-1");
    }

    #[test]
    fn lookup_missing_key_returns_none() {
        let (_tmp, catalog) = temp_catalog();
        let key = SessionKey::Iroh("nonexistent".to_string());
        assert!(catalog.lookup_key(&key).is_none());
    }

    #[test]
    fn matrix_key_round_trip() {
        let (_tmp, catalog) = temp_catalog();
        let key = SessionKey::Matrix {
            user_id: "@alice:example.com".to_string(),
            room_id: "!room:example.com".to_string(),
        };
        catalog.insert_key(&key, "sess-matrix");
        assert_eq!(catalog.lookup_key(&key).unwrap(), "sess-matrix");
    }

    #[test]
    fn remove_keys_for_session_clears_all() {
        let (_tmp, catalog) = temp_catalog();
        let k1 = SessionKey::Iroh("key1".to_string());
        let k2 = SessionKey::Iroh("key2".to_string());
        let k3 = SessionKey::Iroh("key3".to_string());

        catalog.insert_key(&k1, "sess-1");
        catalog.insert_key(&k2, "sess-1");
        catalog.insert_key(&k3, "sess-2");

        catalog.remove_keys_for_session("sess-1");

        assert!(catalog.lookup_key(&k1).is_none());
        assert!(catalog.lookup_key(&k2).is_none());
        // k3 belongs to sess-2, should survive
        assert_eq!(catalog.lookup_key(&k3).unwrap(), "sess-2");
    }

    #[test]
    fn list_keys_returns_all_mappings() {
        let (_tmp, catalog) = temp_catalog();
        catalog.insert_key(&SessionKey::Iroh("aaa".to_string()), "s1");
        catalog.insert_key(&SessionKey::Iroh("bbb".to_string()), "s2");

        let keys = catalog.list_keys();
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn remove_session_also_removes_keys() {
        let (_tmp, catalog) = temp_catalog();
        let key = SessionKey::Iroh("linked-key".to_string());
        catalog.insert_session(&make_entry("s1", SessionLifecycle::Active));
        catalog.insert_key(&key, "s1");

        catalog.remove_session("s1");
        assert!(catalog.get_session("s1").is_none());
        assert!(catalog.lookup_key(&key).is_none());
    }

    // ── GC tests ────────────────────────────────────────────────────

    #[test]
    fn gc_removes_old_tombstoned() {
        let (_tmp, catalog) = temp_catalog();
        // Entry with last_active 30 days ago
        let mut old = make_entry("old", SessionLifecycle::Tombstoned);
        old.last_active = "2026-02-01T00:00:00Z".to_string();
        catalog.insert_session(&old);

        // Entry with last_active now
        let fresh = make_entry("fresh", SessionLifecycle::Tombstoned);
        catalog.insert_session(&fresh);

        let removed = catalog.gc_tombstoned(std::time::Duration::from_secs(7 * 86400));
        assert_eq!(removed, 1);
        assert!(catalog.get_session("old").is_none());
        assert!(catalog.get_session("fresh").is_some());
    }

    #[test]
    fn gc_ignores_non_tombstoned() {
        let (_tmp, catalog) = temp_catalog();
        let mut old_active = make_entry("active-old", SessionLifecycle::Active);
        old_active.last_active = "2026-01-01T00:00:00Z".to_string();
        catalog.insert_session(&old_active);

        let removed = catalog.gc_tombstoned(std::time::Duration::from_secs(86400));
        assert_eq!(removed, 0);
        assert!(catalog.get_session("active-old").is_some());
    }

    // ── Lifecycle transition sequences ──────────────────────────────

    #[test]
    fn full_lifecycle_active_suspended_active_tombstoned() {
        let (_tmp, catalog) = temp_catalog();
        catalog.insert_session(&make_entry("s1", SessionLifecycle::Active));

        // Daemon shutdown → suspended
        catalog.set_state("s1", SessionLifecycle::Suspended);
        assert_eq!(
            catalog.get_session("s1").unwrap().state,
            SessionLifecycle::Suspended
        );

        // Recovery → active
        catalog.set_state("s1", SessionLifecycle::Active);
        assert_eq!(
            catalog.get_session("s1").unwrap().state,
            SessionLifecycle::Active
        );

        // Kill → tombstoned
        catalog.set_state("s1", SessionLifecycle::Tombstoned);
        assert_eq!(
            catalog.get_session("s1").unwrap().state,
            SessionLifecycle::Tombstoned
        );
    }

    #[test]
    fn crash_recovery_transitions_active_to_suspended() {
        let (_tmp, catalog) = temp_catalog();
        // Simulate: daemon had 3 active sessions and crashed
        catalog.insert_session(&make_entry("a", SessionLifecycle::Active));
        catalog.insert_session(&make_entry("b", SessionLifecycle::Active));
        catalog.insert_session(&make_entry("c", SessionLifecycle::Active));

        // On restart: all active → suspended
        let count = catalog.transition_all(
            SessionLifecycle::Active,
            SessionLifecycle::Suspended,
        );
        assert_eq!(count, 3);
        assert_eq!(catalog.list_by_state(SessionLifecycle::Active).len(), 0);
        assert_eq!(catalog.list_by_state(SessionLifecycle::Suspended).len(), 3);
    }

    #[test]
    fn display_lifecycle() {
        assert_eq!(SessionLifecycle::Active.to_string(), "active");
        assert_eq!(SessionLifecycle::Suspended.to_string(), "suspended");
        assert_eq!(SessionLifecycle::Tombstoned.to_string(), "tombstoned");
    }
}
