//! Credential pool with load balancing and automatic failover
//!
//! Manages multiple credentials (accounts) for a single provider, rotating
//! between them to spread load and automatically failing over when one
//! credential gets rate-limited or exhausted.
//!
//! # Strategies
//!
//! - **RoundRobin** — Rotate through credentials evenly on each request.
//! - **Failover** — Use the primary credential until it fails, then switch to the next healthy one.
//!   (Default)
//!
//! # Per-credential health tracking
//!
//! Each credential slot has its own miniature circuit breaker:
//! - On 429/5xx errors, the slot enters cooldown with exponential backoff
//! - Cooldown-expired slots move to half-open (one probe allowed)
//! - A successful probe resets the slot to healthy
//!
//! This is independent of the router-level circuit breaker (which tracks
//! `provider:model` pairs). The credential pool tracks `provider:account`.
//!
//! # Usage
//!
//! ```ignore
//! let pool = CredentialPool::new(
//!     vec![("default", cred_a), ("work", cred_b)],
//!     SelectionStrategy::Failover,
//! );
//!
//! // In a request:
//! let lease = pool.select().await?;
//! let token = lease.token();
//! match do_request(token).await {
//!     Ok(_) => lease.report_success(),
//!     Err(e) if e.is_rate_limit() => lease.report_failure(429),
//!     Err(e) => lease.report_failure(500),
//! }
//! ```

use std::sync::atomic::AtomicU64;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use tokio::sync::RwLock;
use tracing::debug;
use tracing::info;
use tracing::warn;

use crate::auth::StoredCredential;

// ── Selection strategy ──────────────────────────────────────────────────

/// Strategy for selecting which credential to use next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionStrategy {
    /// Rotate through credentials evenly. Spreads load across all accounts.
    RoundRobin,
    /// Use the primary (first) credential until it fails, then switch.
    /// Best for "use my main account, fall back to backup". This is the default.
    #[default]
    Failover,
}

// ── Slot health ─────────────────────────────────────────────────────────

/// Health state for a single credential slot.
#[derive(Debug)]
#[derive(Default)]
struct SlotHealth {
    /// Whether the slot is currently in cooldown (rate-limited/exhausted).
    in_cooldown: bool,
    /// When the cooldown expires (requests can be attempted again).
    cooldown_until: Option<Instant>,
    /// Number of consecutive errors (for exponential backoff).
    consecutive_errors: u32,
    /// HTTP status of the last error.
    last_error_status: Option<u16>,
    /// Total successful requests through this slot.
    success_count: u64,
    /// Total failed requests through this slot.
    failure_count: u64,
    /// When the last request was made (for least-recently-used).
    last_used: Option<Instant>,
}


impl SlotHealth {
    /// Whether this slot is healthy enough to try.
    fn is_available(&self) -> bool {
        if !self.in_cooldown {
            return true;
        }
        // Cooldown expired → half-open (allow probe)
        self.cooldown_until.is_some_and(|until| Instant::now() >= until)
    }

    /// Record a successful request.
    fn record_success(&mut self) {
        self.in_cooldown = false;
        self.cooldown_until = None;
        self.consecutive_errors = 0;
        self.last_error_status = None;
        self.success_count += 1;
        self.last_used = Some(Instant::now());
    }

    /// Record a failed request with the given HTTP status code.
    fn record_failure(&mut self, status: u16) {
        self.consecutive_errors += 1;
        self.last_error_status = Some(status);
        self.failure_count += 1;
        self.last_used = Some(Instant::now());

        // Only enter cooldown for rate-limit / server errors
        if crate::retry::is_retryable_status(status) {
            self.in_cooldown = true;
            // Exponential backoff: 2^errors seconds, capped at 5 minutes
            let backoff_secs = 2u64.pow(self.consecutive_errors.min(8)).min(300);
            self.cooldown_until = Some(Instant::now() + Duration::from_secs(backoff_secs));
        }
    }

    /// Seconds remaining in cooldown (0 if healthy).
    fn cooldown_remaining_secs(&self) -> u64 {
        self.cooldown_until
            .map(|until| until.saturating_duration_since(Instant::now()).as_secs())
            .unwrap_or(0)
    }
}

// ── Credential slot ─────────────────────────────────────────────────────

/// A single credential slot in the pool.
#[derive(Debug)]
struct CredentialSlot {
    /// Account name (e.g., "default", "work", "backup")
    account: String,
    /// The credential itself
    credential: StoredCredential,
    /// Health tracking
    health: RwLock<SlotHealth>,
}

// ── Credential pool ─────────────────────────────────────────────────────

/// Thread-safe pool of credentials with load balancing and failover.
pub struct CredentialPool {
    slots: Vec<CredentialSlot>,
    strategy: SelectionStrategy,
    /// Round-robin counter (only used with RoundRobin strategy)
    rr_counter: AtomicUsize,
    /// Total requests dispatched through this pool
    total_requests: AtomicU64,
}

impl std::fmt::Debug for CredentialPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CredentialPool")
            .field("slots", &self.slots.len())
            .field("strategy", &self.strategy)
            .field("total_requests", &self.total_requests.load(Ordering::Relaxed))
            .finish()
    }
}

/// A leased credential from the pool. Reports success/failure when done.
pub struct CredentialLease<'pool> {
    pool: &'pool CredentialPool,
    slot_index: usize,
}

impl<'pool> CredentialLease<'pool> {
    /// Get the token string for use in API requests.
    pub fn token(&self) -> &str {
        self.pool.slots[self.slot_index].credential.token()
    }

    /// Get the credential itself.
    pub fn credential(&self) -> &StoredCredential {
        &self.pool.slots[self.slot_index].credential
    }

    /// Get the account name for this credential.
    pub fn account(&self) -> &str {
        &self.pool.slots[self.slot_index].account
    }

    /// Whether this credential is an OAuth token.
    pub fn is_oauth(&self) -> bool {
        self.pool.slots[self.slot_index].credential.is_oauth()
    }

    /// Report that the request using this credential succeeded.
    pub async fn report_success(&self) {
        let mut health = self.pool.slots[self.slot_index].health.write().await;
        health.record_success();
    }

    /// Report that the request using this credential failed.
    pub async fn report_failure(&self, status: u16) {
        let slot = &self.pool.slots[self.slot_index];
        let mut health = slot.health.write().await;
        health.record_failure(status);

        if health.in_cooldown {
            warn!(
                "credential '{}' entered cooldown (HTTP {}, {} consecutive errors, {}s backoff)",
                slot.account,
                status,
                health.consecutive_errors,
                health.cooldown_remaining_secs(),
            );
        }
    }
}

impl CredentialPool {
    /// Create a new credential pool.
    ///
    /// `credentials` is a list of `(account_name, credential)` pairs.
    /// The first credential is considered the "primary" for Failover strategy.
    pub fn new(credentials: Vec<(String, StoredCredential)>, strategy: SelectionStrategy) -> Self {
        assert!(!credentials.is_empty(), "CredentialPool requires at least one credential");
        let slots = credentials
            .into_iter()
            .map(|(account, credential)| CredentialSlot {
                account,
                credential,
                health: RwLock::new(SlotHealth::default()),
            })
            .collect();
        Self {
            slots,
            strategy,
            rr_counter: AtomicUsize::new(0),
            total_requests: AtomicU64::new(0),
        }
    }

    /// Create a pool with a single credential (backwards compatible).
    pub fn single(account: String, credential: StoredCredential) -> Self {
        Self::new(vec![(account, credential)], SelectionStrategy::Failover)
    }

    /// Number of credentials in the pool.
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Whether the pool is empty (should never be true after construction).
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// Whether the pool has more than one credential.
    pub fn is_multi(&self) -> bool {
        self.slots.len() > 1
    }

    /// Select a credential for the next request.
    ///
    /// Returns a `CredentialLease` that gives access to the credential and
    /// must be used to report success/failure after the request completes.
    ///
    /// Returns `None` if all credentials are in cooldown.
    pub async fn select(&self) -> Option<CredentialLease<'_>> {
        self.total_requests.fetch_add(1, Ordering::Relaxed);

        match self.strategy {
            SelectionStrategy::RoundRobin => self.select_round_robin().await,
            SelectionStrategy::Failover => self.select_failover().await,
        }
    }

    /// Select all healthy credentials (for trying each in sequence).
    ///
    /// Returns indices of all currently available slots, ordered by preference.
    /// Used by providers that want to retry with different credentials on failure.
    pub async fn select_all_available(&self) -> Vec<CredentialLease<'_>> {
        let mut leases = Vec::new();

        match self.strategy {
            SelectionStrategy::RoundRobin => {
                let start = self.rr_counter.fetch_add(1, Ordering::Relaxed) % self.slots.len();
                for i in 0..self.slots.len() {
                    let idx = (start + i) % self.slots.len();
                    if self.slots[idx].health.read().await.is_available() {
                        leases.push(CredentialLease {
                            pool: self,
                            slot_index: idx,
                        });
                    }
                }
            }
            SelectionStrategy::Failover => {
                for (idx, slot) in self.slots.iter().enumerate() {
                    if slot.health.read().await.is_available() {
                        leases.push(CredentialLease {
                            pool: self,
                            slot_index: idx,
                        });
                    }
                }
            }
        }

        leases
    }

    /// Get summary information about each credential slot.
    pub async fn slot_summaries(&self) -> Vec<SlotSummary> {
        let mut summaries = Vec::with_capacity(self.slots.len());
        for (i, slot) in self.slots.iter().enumerate() {
            let health = slot.health.read().await;
            summaries.push(SlotSummary {
                index: i,
                account: slot.account.clone(),
                is_oauth: slot.credential.is_oauth(),
                is_available: health.is_available(),
                in_cooldown: health.in_cooldown,
                cooldown_remaining_secs: health.cooldown_remaining_secs(),
                consecutive_errors: health.consecutive_errors,
                last_error_status: health.last_error_status,
                success_count: health.success_count,
                failure_count: health.failure_count,
            });
        }
        summaries
    }

    /// Total requests dispatched through this pool.
    pub fn total_requests(&self) -> u64 {
        self.total_requests.load(Ordering::Relaxed)
    }

    /// The selection strategy in use.
    pub fn strategy(&self) -> SelectionStrategy {
        self.strategy
    }

    /// Reset all health state (e.g., after a credential refresh).
    pub async fn reset_health(&self) {
        for slot in &self.slots {
            *slot.health.write().await = SlotHealth::default();
        }
    }

    /// Reset health for a specific account.
    pub async fn reset_account_health(&self, account: &str) {
        for slot in &self.slots {
            if slot.account == account {
                *slot.health.write().await = SlotHealth::default();
            }
        }
    }

    // ── Internal selection logic ─────────────────────────────────────

    async fn select_round_robin(&self) -> Option<CredentialLease<'_>> {
        let start = self.rr_counter.fetch_add(1, Ordering::Relaxed) % self.slots.len();

        // Try from the RR position forward, wrapping around
        for i in 0..self.slots.len() {
            let idx = (start + i) % self.slots.len();
            if self.slots[idx].health.read().await.is_available() {
                if i > 0 {
                    debug!("round-robin skipped {} unavailable slot(s), using '{}'", i, self.slots[idx].account);
                }
                return Some(CredentialLease {
                    pool: self,
                    slot_index: idx,
                });
            }
        }

        warn!("all {} credential(s) in cooldown", self.slots.len());
        None
    }

    async fn select_failover(&self) -> Option<CredentialLease<'_>> {
        // Try the primary (index 0) first, then fall through
        for (idx, slot) in self.slots.iter().enumerate() {
            if slot.health.read().await.is_available() {
                if idx > 0 {
                    info!("failing over from '{}' to '{}'", self.slots[0].account, slot.account);
                }
                return Some(CredentialLease {
                    pool: self,
                    slot_index: idx,
                });
            }
        }

        warn!("all {} credential(s) in cooldown", self.slots.len());
        None
    }
}

// ── Summary types ───────────────────────────────────────────────────────

/// Summary information about a credential slot (for status display).
#[derive(Debug, Clone)]
pub struct SlotSummary {
    pub index: usize,
    pub account: String,
    pub is_oauth: bool,
    pub is_available: bool,
    pub in_cooldown: bool,
    pub cooldown_remaining_secs: u64,
    pub consecutive_errors: u32,
    pub last_error_status: Option<u16>,
    pub success_count: u64,
    pub failure_count: u64,
}

impl std::fmt::Display for SlotSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let status = if self.is_available {
            "✓ healthy"
        } else {
            "✗ cooldown"
        };
        let kind = if self.is_oauth { "oauth" } else { "api-key" };
        write!(
            f,
            "[{}] {} ({}) — {} | ok:{} err:{}",
            self.index, self.account, kind, status, self.success_count, self.failure_count,
        )?;
        if self.in_cooldown {
            write!(f, " | cooldown: {}s remaining", self.cooldown_remaining_secs)?;
        }
        Ok(())
    }
}


#[cfg(test)]
mod tests;
