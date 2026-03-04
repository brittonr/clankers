//! In-memory event cache with 5-minute TTL.
//!
//! Uses thread-local storage since WASM is single-threaded.
//! Cache persists across function calls within the same plugin instance
//! but is lost on reload.

use std::cell::RefCell;

use crate::icalendar::Event;

thread_local! {
    static CACHE: RefCell<Option<CachedEvents>> = RefCell::new(None);
}

struct CachedEvents {
    events: Vec<Event>,
    /// Unix timestamp (seconds) when this cache was populated.
    fetched_at: u64,
}

/// TTL in seconds (5 minutes).
const CACHE_TTL_SECS: u64 = 300;

/// Get cached events if the cache is fresh (< 5 minutes old).
pub fn get_cached(now_unix: u64) -> Option<Vec<Event>> {
    CACHE.with(|c| {
        let cache = c.borrow();
        cache.as_ref().and_then(|cached| {
            if now_unix.saturating_sub(cached.fetched_at) < CACHE_TTL_SECS {
                Some(cached.events.clone())
            } else {
                None
            }
        })
    })
}

/// Store events in the cache.
pub fn set_cache(events: Vec<Event>, now_unix: u64) {
    CACHE.with(|c| {
        *c.borrow_mut() = Some(CachedEvents {
            events,
            fetched_at: now_unix,
        });
    });
}

/// Invalidate the cache (called after create/update/delete).
pub fn invalidate() {
    CACHE.with(|c| {
        *c.borrow_mut() = None;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::icalendar::{CalDateTime, Event};

    fn make_event(uid: &str, summary: &str) -> Event {
        Event {
            uid: uid.to_string(),
            summary: summary.to_string(),
            start: CalDateTime {
                timestamp: "20260303T100000".to_string(),
                timezone: None,
                date_only: false,
            },
            end: None,
            duration: None,
            location: None,
            description: None,
            attendees: vec![],
            calendar: String::new(),
            etag: None,
            href: None,
            all_day: false,
            status: None,
        }
    }

    #[test]
    fn empty_cache_returns_none() {
        invalidate();
        assert!(get_cached(1000).is_none());
    }

    #[test]
    fn set_and_get_within_ttl() {
        invalidate();
        let events = vec![make_event("1", "Test")];
        set_cache(events.clone(), 1000);
        let cached = get_cached(1100).unwrap();
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].uid, "1");
    }

    #[test]
    fn expired_cache_returns_none() {
        invalidate();
        set_cache(vec![make_event("1", "Test")], 1000);
        // 301 seconds later — past TTL
        assert!(get_cached(1301).is_none());
    }

    #[test]
    fn at_ttl_boundary_still_valid() {
        invalidate();
        set_cache(vec![make_event("1", "Test")], 1000);
        // Exactly 299 seconds — still within TTL
        assert!(get_cached(1299).is_some());
    }

    #[test]
    fn at_ttl_exact_boundary_expired() {
        invalidate();
        set_cache(vec![make_event("1", "Test")], 1000);
        // Exactly 300 seconds — expired
        assert!(get_cached(1300).is_none());
    }

    #[test]
    fn invalidate_clears_cache() {
        set_cache(vec![make_event("1", "Test")], 1000);
        invalidate();
        assert!(get_cached(1001).is_none());
    }

    #[test]
    fn set_replaces_previous() {
        invalidate();
        set_cache(vec![make_event("1", "First")], 1000);
        set_cache(vec![make_event("2", "Second")], 1100);
        let cached = get_cached(1200).unwrap();
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].uid, "2");
    }
}
