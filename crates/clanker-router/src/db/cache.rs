//! Response cache for completion requests.
//!
//! Caches model responses keyed by a SHA-256 hash of the normalized request
//! (model + messages + system prompt + tools). Entries have a configurable TTL
//! and are automatically evicted when the cache exceeds a size limit.
//!
//! # When caching makes sense
//!
//! - Identical sub-agent prompts fired repeatedly across sessions
//! - Deterministic requests (temperature=0)
//! - Tool schema / system prompt haven't changed
//!
//! # Cache key
//!
//! `SHA-256(model || system_prompt || messages_json || tools_json)`
//!
//! Temperature, thinking config, etc. are included in the hash so different
//! sampling params produce different cache keys.

use chrono::DateTime;
use chrono::Utc;
use redb::ReadableTable;
use redb::ReadableTableMetadata;
use redb::TableDefinition;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;

use super::RouterDb;
use super::db_err;
use crate::error::Result;

/// Table: hex-encoded SHA-256 hash → serialized CachedResponse
pub(crate) const TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("router_cache");

/// Table: cache_key → expiry timestamp (millis since epoch) for TTL eviction
pub(crate) const TTL_TABLE: TableDefinition<&str, i64> = TableDefinition::new("router_cache_ttl");

/// Default TTL: 1 hour.
const DEFAULT_TTL_SECS: i64 = 3600;

/// Maximum number of cached responses.
const MAX_ENTRIES: u64 = 1_000;

/// A cached completion response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedResponse {
    /// The cache key (hex SHA-256).
    pub key: String,
    /// Model that produced the response.
    pub model: String,
    /// Provider that served the response.
    pub provider: String,
    /// The complete stream events (serialized).
    pub events: Vec<crate::streaming::StreamEvent>,
    /// When this was cached.
    pub cached_at: DateTime<Utc>,
    /// When this entry expires.
    pub expires_at: DateTime<Utc>,
    /// Number of times this entry has been served.
    pub hit_count: u32,
    /// Token counts from the original response.
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// Inputs for computing a cache key.
pub struct CacheKeyInput<'a> {
    pub model: &'a str,
    pub system_prompt: Option<&'a str>,
    pub messages: &'a [serde_json::Value],
    pub tools: &'a [crate::provider::ToolDefinition],
    pub temperature: Option<f64>,
    pub thinking_enabled: bool,
}

impl<'a> CacheKeyInput<'a> {
    /// Compute the SHA-256 cache key.
    pub fn compute_key(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.model.as_bytes());
        hasher.update(b"|");
        if let Some(sp) = self.system_prompt {
            hasher.update(sp.as_bytes());
        }
        hasher.update(b"|");
        // Serialize messages deterministically
        if let Ok(json) = serde_json::to_string(self.messages) {
            hasher.update(json.as_bytes());
        }
        hasher.update(b"|");
        if let Ok(json) = serde_json::to_string(self.tools) {
            hasher.update(json.as_bytes());
        }
        hasher.update(b"|");
        if let Some(t) = self.temperature {
            hasher.update(t.to_bits().to_le_bytes());
        }
        hasher.update(b"|");
        hasher.update(if self.thinking_enabled { b"T" } else { b"F" });

        let hash = hasher.finalize();
        hex::encode(hash)
    }
}

/// Accessor for the response cache.
pub struct ResponseCache<'db> {
    db: &'db RouterDb,
    ttl_secs: i64,
}

impl<'db> ResponseCache<'db> {
    pub(crate) fn new(db: &'db RouterDb) -> Self {
        Self {
            db,
            ttl_secs: DEFAULT_TTL_SECS,
        }
    }

    /// Create a cache accessor with a custom TTL.
    pub fn with_ttl(db: &'db RouterDb, ttl_secs: i64) -> Self {
        Self { db, ttl_secs }
    }

    /// Look up a cached response by key. Returns `None` if missing or expired.
    pub fn get(&self, key: &str) -> Result<Option<CachedResponse>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;
        let ttl_table = tx.open_table(TTL_TABLE).map_err(db_err)?;

        // Check TTL first
        if let Some(expires) = ttl_table.get(key).map_err(db_err)? {
            let now = Utc::now().timestamp_millis();
            if now >= expires.value() {
                // Expired — don't return it (will be cleaned up later)
                return Ok(None);
            }
        }

        match table.get(key).map_err(db_err)? {
            Some(value) => {
                let entry = serde_json::from_slice(value.value()).map_err(|e| crate::Error::Config {
                    message: format!("failed to deserialize cache entry: {e}"),
                })?;
                Ok(Some(entry))
            }
            None => Ok(None),
        }
    }

    /// Store a response in the cache.
    pub fn put(&self, entry: &CachedResponse) -> Result<()> {
        let bytes = serde_json::to_vec(entry).map_err(|e| crate::Error::Config {
            message: format!("failed to serialize cache entry: {e}"),
        })?;
        let expires_ms = entry.expires_at.timestamp_millis();

        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(TABLE).map_err(db_err)?;
            let mut ttl_table = tx.open_table(TTL_TABLE).map_err(db_err)?;

            table.insert(entry.key.as_str(), bytes.as_slice()).map_err(db_err)?;
            ttl_table.insert(entry.key.as_str(), expires_ms).map_err(db_err)?;

            // Evict oldest if over limit
            let len = table.len().map_err(db_err)?;
            if len > MAX_ENTRIES {
                let to_remove = len - MAX_ENTRIES;
                let keys: Vec<String> = ttl_table
                    .iter()
                    .map_err(db_err)?
                    .take(to_remove as usize)
                    .filter_map(|item| item.ok().map(|(k, _)| k.value().to_string()))
                    .collect();
                for key in &keys {
                    table.remove(key.as_str()).map_err(db_err)?;
                    ttl_table.remove(key.as_str()).map_err(db_err)?;
                }
            }
        }
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// Build a CachedResponse from request parameters and stream events.
    pub fn build_entry(
        &self,
        key: &str,
        provider: &str,
        model: &str,
        events: Vec<crate::streaming::StreamEvent>,
        input_tokens: u64,
        output_tokens: u64,
    ) -> CachedResponse {
        let now = Utc::now();
        CachedResponse {
            key: key.to_string(),
            model: model.to_string(),
            provider: provider.to_string(),
            events,
            cached_at: now,
            expires_at: now + chrono::Duration::seconds(self.ttl_secs),
            hit_count: 0,
            input_tokens,
            output_tokens,
        }
    }

    /// Increment the hit count for a cached entry.
    pub fn record_hit(&self, key: &str) -> Result<()> {
        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(TABLE).map_err(db_err)?;

            // Read, deserialize, bump, then re-insert
            let updated = {
                let value = table.get(key).map_err(db_err)?;
                match value {
                    Some(v) => {
                        let bytes = v.value().to_vec();
                        drop(v); // release the immutable borrow
                        serde_json::from_slice::<CachedResponse>(&bytes).ok().map(|mut entry| {
                            entry.hit_count += 1;
                            entry
                        })
                    }
                    None => None,
                }
            };

            if let Some(entry) = updated {
                let bytes = serde_json::to_vec(&entry).map_err(|e| crate::Error::Config {
                    message: format!("failed to serialize cache entry: {e}"),
                })?;
                table.insert(key, bytes.as_slice()).map_err(db_err)?;
            }
        }
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// Evict all expired entries. Returns the number removed.
    pub fn evict_expired(&self) -> Result<u64> {
        let now = Utc::now().timestamp_millis();

        let tx = self.db.begin_write()?;
        let removed = {
            let mut table = tx.open_table(TABLE).map_err(db_err)?;
            let mut ttl_table = tx.open_table(TTL_TABLE).map_err(db_err)?;

            let expired_keys: Vec<String> = ttl_table
                .iter()
                .map_err(db_err)?
                .filter_map(|item| {
                    let (k, v) = item.ok()?;
                    if v.value() <= now {
                        Some(k.value().to_string())
                    } else {
                        None
                    }
                })
                .collect();

            let count = expired_keys.len() as u64;
            for key in &expired_keys {
                table.remove(key.as_str()).map_err(db_err)?;
                ttl_table.remove(key.as_str()).map_err(db_err)?;
            }
            count
        };
        tx.commit().map_err(db_err)?;
        Ok(removed)
    }

    /// Number of entries currently in the cache.
    pub fn len(&self) -> Result<u64> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;
        table.len().map_err(db_err)
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> Result<bool> {
        Ok(self.len()? == 0)
    }

    /// Remove all cache entries.
    pub fn clear(&self) -> Result<u64> {
        let tx = self.db.begin_write()?;
        let count = {
            let mut table = tx.open_table(TABLE).map_err(db_err)?;
            let mut ttl_table = tx.open_table(TTL_TABLE).map_err(db_err)?;
            let count = table.len().map_err(db_err)?;
            table.retain(|_, _| false).map_err(db_err)?;
            ttl_table.retain(|_, _| false).map_err(db_err)?;
            count
        };
        tx.commit().map_err(db_err)?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::streaming::StreamEvent;

    fn test_db() -> RouterDb {
        RouterDb::in_memory().unwrap()
    }

    fn sample_events() -> Vec<StreamEvent> {
        vec![
            StreamEvent::MessageStart {
                message: crate::streaming::MessageMetadata {
                    id: "msg-1".into(),
                    model: "sonnet".into(),
                    role: "assistant".into(),
                },
            },
            StreamEvent::ContentBlockStart {
                index: 0,
                content_block: crate::streaming::ContentBlock::Text { text: String::new() },
            },
            StreamEvent::ContentBlockDelta {
                index: 0,
                delta: crate::streaming::ContentDelta::TextDelta { text: "Hello!".into() },
            },
            StreamEvent::ContentBlockStop { index: 0 },
            StreamEvent::MessageStop,
        ]
    }

    #[test]
    fn test_cache_key_deterministic() {
        let input = CacheKeyInput {
            model: "sonnet",
            system_prompt: Some("You are helpful."),
            messages: &[serde_json::json!({"role": "user", "content": "hello"})],
            tools: &[],
            temperature: Some(0.0),
            thinking_enabled: false,
        };
        let key1 = input.compute_key();
        let key2 = input.compute_key();
        assert_eq!(key1, key2);
        assert_eq!(key1.len(), 64); // SHA-256 hex = 64 chars
    }

    #[test]
    fn test_cache_key_varies_with_model() {
        let input1 = CacheKeyInput {
            model: "sonnet",
            system_prompt: None,
            messages: &[],
            tools: &[],
            temperature: None,
            thinking_enabled: false,
        };
        let input2 = CacheKeyInput {
            model: "gpt-4o",
            system_prompt: None,
            messages: &[],
            tools: &[],
            temperature: None,
            thinking_enabled: false,
        };
        assert_ne!(input1.compute_key(), input2.compute_key());
    }

    #[test]
    fn test_cache_key_varies_with_temperature() {
        let input1 = CacheKeyInput {
            model: "sonnet",
            system_prompt: None,
            messages: &[],
            tools: &[],
            temperature: Some(0.0),
            thinking_enabled: false,
        };
        let input2 = CacheKeyInput {
            model: "sonnet",
            system_prompt: None,
            messages: &[],
            tools: &[],
            temperature: Some(1.0),
            thinking_enabled: false,
        };
        assert_ne!(input1.compute_key(), input2.compute_key());
    }

    #[test]
    fn test_put_and_get() {
        let db = test_db();
        let cache = db.cache();

        let entry = cache.build_entry("abc123", "anthropic", "sonnet", sample_events(), 100, 50);
        cache.put(&entry).unwrap();

        let got = cache.get("abc123").unwrap().unwrap();
        assert_eq!(got.model, "sonnet");
        assert_eq!(got.provider, "anthropic");
        assert_eq!(got.events.len(), 5);
        assert_eq!(got.hit_count, 0);
        assert_eq!(got.input_tokens, 100);
        assert_eq!(got.output_tokens, 50);
    }

    #[test]
    fn test_get_missing() {
        let db = test_db();
        assert!(db.cache().get("nonexistent").unwrap().is_none());
    }

    #[test]
    fn test_get_expired() {
        let db = test_db();
        let cache = ResponseCache::with_ttl(&db, -1); // Already expired

        let entry = cache.build_entry("expired-key", "anthropic", "sonnet", sample_events(), 100, 50);
        cache.put(&entry).unwrap();

        // Should not return expired entries
        assert!(cache.get("expired-key").unwrap().is_none());
    }

    #[test]
    fn test_record_hit() {
        let db = test_db();
        let cache = db.cache();

        let entry = cache.build_entry("hit-test", "anthropic", "sonnet", sample_events(), 100, 50);
        cache.put(&entry).unwrap();

        cache.record_hit("hit-test").unwrap();
        cache.record_hit("hit-test").unwrap();
        cache.record_hit("hit-test").unwrap();

        let got = cache.get("hit-test").unwrap().unwrap();
        assert_eq!(got.hit_count, 3);
    }

    #[test]
    fn test_evict_expired() {
        let db = test_db();
        let cache = ResponseCache::with_ttl(&db, -1); // Everything expires immediately

        for i in 0..5 {
            let key = format!("key-{i}");
            let entry = cache.build_entry(&key, "anthropic", "sonnet", vec![], 10, 5);
            cache.put(&entry).unwrap();
        }

        let removed = cache.evict_expired().unwrap();
        assert_eq!(removed, 5);
        assert_eq!(cache.len().unwrap(), 0);
    }

    #[test]
    fn test_len_and_is_empty() {
        let db = test_db();
        let cache = db.cache();

        assert!(cache.is_empty().unwrap());
        assert_eq!(cache.len().unwrap(), 0);

        let entry = cache.build_entry("k1", "anthropic", "sonnet", vec![], 10, 5);
        cache.put(&entry).unwrap();

        assert!(!cache.is_empty().unwrap());
        assert_eq!(cache.len().unwrap(), 1);
    }

    #[test]
    fn test_clear() {
        let db = test_db();
        let cache = db.cache();

        for i in 0..5 {
            let key = format!("key-{i}");
            let entry = cache.build_entry(&key, "anthropic", "sonnet", vec![], 10, 5);
            cache.put(&entry).unwrap();
        }

        let cleared = cache.clear().unwrap();
        assert_eq!(cleared, 5);
        assert!(cache.is_empty().unwrap());
    }
}
