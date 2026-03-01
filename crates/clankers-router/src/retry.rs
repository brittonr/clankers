//! Retry logic with exponential backoff and jitter

use std::time::Duration;

/// Retry configuration
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retries
    pub max_retries: u32,
    /// Initial backoff duration
    pub initial_backoff: Duration,
    /// Maximum backoff duration
    pub max_backoff: Duration,
    /// Backoff multiplier
    pub multiplier: f64,
    /// Whether to add jitter to avoid thundering herd
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(30),
            multiplier: 2.0,
            jitter: true,
        }
    }
}

impl RetryConfig {
    /// Calculate backoff for a given attempt number (0-indexed).
    ///
    /// Uses "full jitter" strategy: `random(0, min(cap, base * 2^attempt))`.
    /// This decorrelates retry storms when many clients hit rate limits at once.
    /// With `jitter = false`, returns the deterministic exponential value.
    pub fn backoff_for(&self, attempt: u32) -> Duration {
        let secs = self.initial_backoff.as_secs_f64() * self.multiplier.powi(attempt as i32);
        let capped = secs.min(self.max_backoff.as_secs_f64());
        if self.jitter {
            let jittered = rand::random::<f64>() * capped;
            // Ensure at least half the base backoff so we don't retry instantly
            let floor = (self.initial_backoff.as_secs_f64() * 0.5).min(capped);
            Duration::from_secs_f64(floor + jittered * (1.0 - floor / capped.max(0.001)))
        } else {
            Duration::from_secs_f64(capped)
        }
    }

    /// Create a config with no jitter (for deterministic tests).
    pub fn deterministic() -> Self {
        Self {
            jitter: false,
            ..Default::default()
        }
    }
}

/// Check if an HTTP status code is retryable
pub fn is_retryable_status(status: u16) -> bool {
    matches!(status, 429 | 500 | 502 | 503 | 529)
}

/// Check if an error message suggests a retryable condition
pub fn is_retryable_error(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    lower.contains("rate limit")
        || lower.contains("overloaded")
        || lower.contains("timeout")
        || lower.contains("connection reset")
        || lower.contains("connection refused")
        || lower.contains("temporarily unavailable")
}

/// Parse Retry-After header value (seconds)
pub fn parse_retry_after(header_value: &str) -> Option<Duration> {
    header_value.trim().parse::<u64>().ok().map(Duration::from_secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backoff_calculation_deterministic() {
        let config = RetryConfig::deterministic();
        assert_eq!(config.backoff_for(0), Duration::from_secs(1));
        assert_eq!(config.backoff_for(1), Duration::from_secs(2));
        assert_eq!(config.backoff_for(2), Duration::from_secs(4));
    }

    #[test]
    fn test_backoff_with_jitter_bounded() {
        let config = RetryConfig::default();
        for attempt in 0..5 {
            let backoff = config.backoff_for(attempt);
            assert!(
                backoff <= config.max_backoff,
                "attempt {attempt}: backoff {:?} exceeds max {:?}",
                backoff,
                config.max_backoff
            );
            // With jitter, should still be positive
            assert!(backoff > Duration::ZERO, "attempt {attempt}: backoff should be positive");
        }
    }

    #[test]
    fn test_backoff_jitter_varies() {
        let config = RetryConfig::default();
        // Run several times — with jitter, at least some should differ
        let values: Vec<Duration> = (0..20).map(|_| config.backoff_for(2)).collect();
        let all_same = values.windows(2).all(|w| w[0] == w[1]);
        assert!(!all_same, "jittered backoffs should not all be identical");
    }

    #[test]
    fn test_backoff_capped() {
        let config = RetryConfig {
            max_backoff: Duration::from_secs(5),
            jitter: false,
            ..Default::default()
        };
        assert!(config.backoff_for(10) <= Duration::from_secs(5));
    }

    #[test]
    fn test_retryable_status() {
        assert!(is_retryable_status(429));
        assert!(is_retryable_status(500));
        assert!(!is_retryable_status(200));
        assert!(!is_retryable_status(401));
    }

    #[test]
    fn test_parse_retry_after() {
        assert_eq!(parse_retry_after("5"), Some(Duration::from_secs(5)));
        assert_eq!(parse_retry_after("not_a_number"), None);
    }
}
