//! Per-provider/model rate-limit and health state.
//!
//! Tracks 429s, 5xxs, and cooldown windows so the router can preemptively
//! skip unhealthy providers and route to alternatives.
//!
//! Each provider+model pair has its own [`RateLimitState`] that records:
//! - When the last rate-limit (429) or server error (5xx) occurred
//! - How many consecutive errors have happened (for exponential backoff)
//! - When the cooldown expires (provider is considered healthy again)
//! - RPM/TPM counters for the current minute window

use std::collections::HashMap;

use chrono::DateTime;
use chrono::Utc;
use redb::ReadableTable;
use redb::ReadableTableMetadata;
use redb::TableDefinition;
use serde::Deserialize;
use serde::Serialize;

use super::RouterDb;
use super::db_err;
use crate::error::Result;

/// Table: "provider:model" → serialized RateLimitState
pub(crate) const TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("router_rate_limits");

/// Circuit breaker state.
///
/// - **Closed**: healthy, all requests pass through.
/// - **Open**: unhealthy, requests are blocked until cooldown expires.
/// - **HalfOpen**: cooldown expired, one probe request is allowed through. If it succeeds → Closed;
///   if it fails → Open again with longer backoff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum CircuitState {
    /// All requests pass through.
    #[default]
    Closed,
    /// Requests are blocked; waiting for cooldown.
    Open,
    /// Cooldown expired; allow one probe request.
    HalfOpen,
}

/// Rate limit and health state for a provider+model pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitState {
    /// Provider name.
    pub provider: String,
    /// Model ID (or "*" for provider-wide state).
    pub model: String,
    /// Circuit breaker state.
    #[serde(default)]
    pub circuit: CircuitState,
    /// When the last error occurred.
    pub last_error_at: Option<DateTime<Utc>>,
    /// HTTP status of the last error (429, 500, 502, 503, 529).
    pub last_error_status: Option<u16>,
    /// Number of consecutive errors (resets on success).
    pub consecutive_errors: u32,
    /// When the cooldown expires (don't route here until then).
    pub cooldown_until: Option<DateTime<Utc>>,
    /// Retry-After value from the last 429, if any (seconds).
    pub retry_after_secs: Option<u64>,
    /// Request count in the current minute window.
    pub rpm_count: u32,
    /// Token count in the current minute window.
    pub tpm_count: u64,
    /// Start of the current minute window.
    pub window_start: Option<DateTime<Utc>>,
    /// When this state was last updated.
    pub updated_at: DateTime<Utc>,
}

impl RateLimitState {
    /// Create a fresh (healthy) state.
    pub fn new(provider: &str, model: &str) -> Self {
        Self {
            provider: provider.to_string(),
            model: model.to_string(),
            circuit: CircuitState::Closed,
            last_error_at: None,
            last_error_status: None,
            consecutive_errors: 0,
            cooldown_until: None,
            retry_after_secs: None,
            rpm_count: 0,
            tpm_count: 0,
            window_start: None,
            updated_at: Utc::now(),
        }
    }

    /// Whether this provider+model is currently in cooldown.
    pub fn is_cooling_down(&self) -> bool {
        self.cooldown_until.map(|until| Utc::now() < until).unwrap_or(false)
    }

    /// Compute the effective circuit state (handles time-based transitions).
    ///
    /// - Open + cooldown expired → HalfOpen (allow one probe)
    /// - HalfOpen persists until a success (→ Closed) or failure (→ Open)
    pub fn effective_circuit(&self) -> CircuitState {
        match self.circuit {
            CircuitState::Open if !self.is_cooling_down() => CircuitState::HalfOpen,
            other => other,
        }
    }

    /// Whether this provider+model is considered healthy enough to try.
    ///
    /// Returns `true` for Closed and HalfOpen (probe allowed), `false` for Open.
    pub fn is_healthy(&self) -> bool {
        match self.effective_circuit() {
            CircuitState::Closed | CircuitState::HalfOpen => true,
            CircuitState::Open => false,
        }
    }

    /// Seconds remaining in the cooldown window (0 if healthy).
    pub fn cooldown_remaining_secs(&self) -> i64 {
        self.cooldown_until.map(|until| (until - Utc::now()).num_seconds().max(0)).unwrap_or(0)
    }

    /// Record a successful request — resets circuit to Closed.
    pub fn record_success(&mut self, tokens: u64) {
        self.circuit = CircuitState::Closed;
        self.consecutive_errors = 0;
        self.cooldown_until = None;
        self.last_error_at = None;
        self.last_error_status = None;
        self.retry_after_secs = None;
        self.bump_window(tokens);
        self.updated_at = Utc::now();
    }

    /// Record a rate-limit or server error — opens the circuit.
    pub fn record_error(&mut self, status: u16, retry_after: Option<u64>) {
        let now = Utc::now();
        self.last_error_at = Some(now);
        self.last_error_status = Some(status);
        self.consecutive_errors += 1;
        self.retry_after_secs = retry_after;
        self.circuit = CircuitState::Open;
        self.updated_at = now;

        // Calculate cooldown: use Retry-After if available, otherwise
        // exponential backoff capped at 5 minutes.
        // HalfOpen failures get doubled backoff to avoid tight probe loops.
        let cooldown_secs = if let Some(ra) = retry_after {
            ra as i64
        } else {
            let base = 2i64.pow(self.consecutive_errors.min(8));
            base.min(300) // cap at 5 minutes
        };

        self.cooldown_until = Some(now + chrono::Duration::seconds(cooldown_secs));
    }

    /// Advance the per-minute window and bump counters.
    fn bump_window(&mut self, tokens: u64) {
        let now = Utc::now();
        let window = self.window_start.unwrap_or(now);

        // Reset window if more than 60 seconds have passed
        if (now - window).num_seconds() >= 60 {
            self.rpm_count = 1;
            self.tpm_count = tokens;
            self.window_start = Some(now);
        } else {
            self.rpm_count += 1;
            self.tpm_count += tokens;
        }
    }
}

/// Accessor for rate-limit state.
pub struct RateLimitStore<'db> {
    db: &'db RouterDb,
}

impl<'db> RateLimitStore<'db> {
    pub(crate) fn new(db: &'db RouterDb) -> Self {
        Self { db }
    }

    /// Get the state for a provider+model pair.
    pub fn get(&self, provider: &str, model: &str) -> Result<Option<RateLimitState>> {
        let key = format!("{provider}:{model}");
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;
        match table.get(key.as_str()).map_err(db_err)? {
            Some(value) => {
                let state = serde_json::from_slice(value.value()).map_err(|e| crate::Error::Config {
                    message: format!("failed to deserialize rate limit state: {e}"),
                })?;
                Ok(Some(state))
            }
            None => Ok(None),
        }
    }

    /// Get or create the state for a provider+model pair.
    pub fn get_or_default(&self, provider: &str, model: &str) -> Result<RateLimitState> {
        Ok(self.get(provider, model)?.unwrap_or_else(|| RateLimitState::new(provider, model)))
    }

    /// Save a state back to the database.
    pub fn save(&self, state: &RateLimitState) -> Result<()> {
        let key = format!("{}:{}", state.provider, state.model);
        let bytes = serde_json::to_vec(state).map_err(|e| crate::Error::Config {
            message: format!("failed to serialize rate limit state: {e}"),
        })?;

        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(TABLE).map_err(db_err)?;
            table.insert(key.as_str(), bytes.as_slice()).map_err(db_err)?;
        }
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// Record a successful request for a provider+model.
    pub fn record_success(&self, provider: &str, model: &str, tokens: u64) -> Result<()> {
        let mut state = self.get_or_default(provider, model)?;
        state.record_success(tokens);
        self.save(&state)
    }

    /// Record an error for a provider+model.
    pub fn record_error(&self, provider: &str, model: &str, status: u16, retry_after: Option<u64>) -> Result<()> {
        let mut state = self.get_or_default(provider, model)?;
        state.record_error(status, retry_after);
        self.save(&state)
    }

    /// Check if a provider+model is healthy (not in cooldown).
    pub fn is_healthy(&self, provider: &str, model: &str) -> Result<bool> {
        Ok(self.get(provider, model)?.map(|s| s.is_healthy()).unwrap_or(true))
    }

    /// List all currently unhealthy (cooling down) entries.
    pub fn unhealthy(&self) -> Result<Vec<RateLimitState>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;

        let mut entries = Vec::new();
        for item in table.iter().map_err(db_err)? {
            let (_key, value) = item.map_err(db_err)?;
            if let Ok(state) = serde_json::from_slice::<RateLimitState>(value.value())
                && state.is_cooling_down()
            {
                entries.push(state);
            }
        }
        Ok(entries)
    }

    /// List all tracked provider+model pairs with their state.
    pub fn list_all(&self) -> Result<Vec<RateLimitState>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;

        let mut entries = Vec::new();
        for item in table.iter().map_err(db_err)? {
            let (_key, value) = item.map_err(db_err)?;
            if let Ok(state) = serde_json::from_slice::<RateLimitState>(value.value()) {
                entries.push(state);
            }
        }
        Ok(entries)
    }

    /// Get a health summary as a map of "provider:model" → is_healthy.
    pub fn health_map(&self) -> Result<HashMap<String, bool>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;

        let mut map = HashMap::new();
        for item in table.iter().map_err(db_err)? {
            let (key, value) = item.map_err(db_err)?;
            if let Ok(state) = serde_json::from_slice::<RateLimitState>(value.value()) {
                map.insert(key.value().to_string(), state.is_healthy());
            }
        }
        Ok(map)
    }

    /// Remove all rate limit state.
    pub fn clear(&self) -> Result<u64> {
        let tx = self.db.begin_write()?;
        let count = {
            let mut table = tx.open_table(TABLE).map_err(db_err)?;
            let count = table.len().map_err(db_err)?;
            table.retain(|_, _| false).map_err(db_err)?;
            count
        };
        tx.commit().map_err(db_err)?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> RouterDb {
        RouterDb::in_memory().unwrap()
    }

    #[test]
    fn test_new_state_is_healthy() {
        let state = RateLimitState::new("anthropic", "sonnet");
        assert!(state.is_healthy());
        assert!(!state.is_cooling_down());
        assert_eq!(state.consecutive_errors, 0);
        assert_eq!(state.cooldown_remaining_secs(), 0);
    }

    #[test]
    fn test_record_error_triggers_cooldown() {
        let mut state = RateLimitState::new("anthropic", "sonnet");
        state.record_error(429, None);

        assert!(state.is_cooling_down());
        assert!(!state.is_healthy());
        assert_eq!(state.consecutive_errors, 1);
        assert_eq!(state.last_error_status, Some(429));
        assert!(state.cooldown_remaining_secs() > 0);
    }

    #[test]
    fn test_record_error_with_retry_after() {
        let mut state = RateLimitState::new("anthropic", "sonnet");
        state.record_error(429, Some(30));

        assert!(state.is_cooling_down());
        assert_eq!(state.retry_after_secs, Some(30));
        // Cooldown should be ~30 seconds
        assert!(state.cooldown_remaining_secs() >= 28);
        assert!(state.cooldown_remaining_secs() <= 31);
    }

    #[test]
    fn test_consecutive_errors_increase_backoff() {
        let mut state = RateLimitState::new("anthropic", "sonnet");

        state.record_error(429, None);
        let cd1 = state.cooldown_remaining_secs();

        state.record_error(429, None);
        let cd2 = state.cooldown_remaining_secs();

        state.record_error(429, None);
        let cd3 = state.cooldown_remaining_secs();

        assert!(cd2 >= cd1, "backoff should increase: cd1={cd1}, cd2={cd2}");
        assert!(cd3 >= cd2, "backoff should increase: cd2={cd2}, cd3={cd3}");
        assert_eq!(state.consecutive_errors, 3);
    }

    #[test]
    fn test_success_resets_errors() {
        let mut state = RateLimitState::new("anthropic", "sonnet");
        state.record_error(429, None);
        state.record_error(429, None);
        assert_eq!(state.consecutive_errors, 2);
        assert!(state.is_cooling_down());

        state.record_success(100);
        assert_eq!(state.consecutive_errors, 0);
        assert!(state.is_healthy());
        assert_eq!(state.cooldown_remaining_secs(), 0);
    }

    #[test]
    fn test_rpm_window() {
        let mut state = RateLimitState::new("anthropic", "sonnet");
        state.record_success(100);
        state.record_success(200);
        state.record_success(300);

        assert_eq!(state.rpm_count, 3);
        assert_eq!(state.tpm_count, 600);
    }

    #[test]
    fn test_store_get_and_save() {
        let db = test_db();
        let store = db.rate_limits();

        // Not tracked yet → None
        assert!(store.get("anthropic", "sonnet").unwrap().is_none());

        // Save a state
        let mut state = RateLimitState::new("anthropic", "sonnet");
        state.record_error(429, Some(10));
        store.save(&state).unwrap();

        // Now it's there
        let loaded = store.get("anthropic", "sonnet").unwrap().unwrap();
        assert_eq!(loaded.consecutive_errors, 1);
        assert_eq!(loaded.last_error_status, Some(429));
    }

    #[test]
    fn test_store_record_success() {
        let db = test_db();
        let store = db.rate_limits();

        // Record an error first
        store.record_error("anthropic", "sonnet", 429, None).unwrap();
        assert!(!store.is_healthy("anthropic", "sonnet").unwrap());

        // Then a success
        store.record_success("anthropic", "sonnet", 100).unwrap();
        assert!(store.is_healthy("anthropic", "sonnet").unwrap());
    }

    #[test]
    fn test_store_is_healthy_default() {
        let db = test_db();
        let store = db.rate_limits();

        // Unknown provider+model defaults to healthy
        assert!(store.is_healthy("unknown", "model").unwrap());
    }

    #[test]
    fn test_store_unhealthy_list() {
        let db = test_db();
        let store = db.rate_limits();

        store.record_error("anthropic", "sonnet", 429, None).unwrap();
        store.record_success("openai", "gpt-4o", 100).unwrap();
        store.record_error("groq", "llama", 503, None).unwrap();

        let unhealthy = store.unhealthy().unwrap();
        assert_eq!(unhealthy.len(), 2);

        let providers: Vec<&str> = unhealthy.iter().map(|s| s.provider.as_str()).collect();
        assert!(providers.contains(&"anthropic"));
        assert!(providers.contains(&"groq"));
    }

    #[test]
    fn test_store_health_map() {
        let db = test_db();
        let store = db.rate_limits();

        store.record_error("anthropic", "sonnet", 429, None).unwrap();
        store.record_success("openai", "gpt-4o", 100).unwrap();

        let map = store.health_map().unwrap();
        assert_eq!(map.get("anthropic:sonnet"), Some(&false));
        assert_eq!(map.get("openai:gpt-4o"), Some(&true));
    }

    #[test]
    fn test_store_list_all() {
        let db = test_db();
        let store = db.rate_limits();

        store.record_success("anthropic", "sonnet", 100).unwrap();
        store.record_success("openai", "gpt-4o", 200).unwrap();

        let all = store.list_all().unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_store_clear() {
        let db = test_db();
        let store = db.rate_limits();

        store.record_success("anthropic", "sonnet", 100).unwrap();
        store.record_success("openai", "gpt-4o", 200).unwrap();

        let cleared = store.clear().unwrap();
        assert_eq!(cleared, 2);
        assert!(store.list_all().unwrap().is_empty());
    }
}
