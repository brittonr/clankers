use super::*;
use crate::auth::StoredCredential;

fn api_key(key: &str) -> StoredCredential {
    StoredCredential::ApiKey {
        api_key: key.to_string(),
        label: None,
    }
}

fn oauth_token(token: &str) -> StoredCredential {
    StoredCredential::OAuth {
        access_token: token.to_string(),
        refresh_token: "refresh".to_string(),
        expires_at_ms: i64::MAX,
        label: None,
    }
}

#[tokio::test]
async fn test_single_credential_pool() {
    let pool = CredentialPool::single("default".into(), api_key("sk-test"));
    assert_eq!(pool.len(), 1);
    assert!(!pool.is_multi());

    let lease = pool.select().await.unwrap();
    assert_eq!(lease.token(), "sk-test");
    assert_eq!(lease.account(), "default");
    assert!(!lease.is_oauth());
    lease.report_success().await;
}

#[tokio::test]
async fn test_failover_uses_primary_first() {
    let pool = CredentialPool::new(
        vec![
            ("primary".into(), api_key("key-1")),
            ("backup".into(), api_key("key-2")),
        ],
        SelectionStrategy::Failover,
    );

    // Should always pick primary when healthy
    for _ in 0..5 {
        let lease = pool.select().await.unwrap();
        assert_eq!(lease.token(), "key-1");
        lease.report_success().await;
    }
}

#[tokio::test]
async fn test_failover_switches_on_rate_limit() {
    let pool = CredentialPool::new(
        vec![
            ("primary".into(), api_key("key-1")),
            ("backup".into(), api_key("key-2")),
        ],
        SelectionStrategy::Failover,
    );

    // Primary gets rate-limited
    let lease = pool.select().await.unwrap();
    assert_eq!(lease.token(), "key-1");
    lease.report_failure(429).await;

    // Should now use backup
    let lease = pool.select().await.unwrap();
    assert_eq!(lease.token(), "key-2");
    assert_eq!(lease.account(), "backup");
    lease.report_success().await;
}

#[tokio::test]
async fn test_round_robin_rotates() {
    let pool = CredentialPool::new(
        vec![
            ("a".into(), api_key("key-a")),
            ("b".into(), api_key("key-b")),
            ("c".into(), api_key("key-c")),
        ],
        SelectionStrategy::RoundRobin,
    );

    let mut tokens = Vec::new();
    for _ in 0..6 {
        let lease = pool.select().await.unwrap();
        tokens.push(lease.token().to_string());
        lease.report_success().await;
    }

    // Should cycle: a, b, c, a, b, c
    assert_eq!(tokens, vec!["key-a", "key-b", "key-c", "key-a", "key-b", "key-c"]);
}

#[tokio::test]
async fn test_round_robin_skips_unhealthy() {
    let pool = CredentialPool::new(
        vec![
            ("a".into(), api_key("key-a")),
            ("b".into(), api_key("key-b")),
            ("c".into(), api_key("key-c")),
        ],
        SelectionStrategy::RoundRobin,
    );

    // Mark 'b' as unhealthy
    let lease = pool.select().await.unwrap(); // a
    lease.report_success().await;
    let lease = pool.select().await.unwrap(); // b
    lease.report_failure(429).await;

    // Next selections should skip b
    let lease = pool.select().await.unwrap();
    assert_eq!(lease.token(), "key-c");
    lease.report_success().await;

    let lease = pool.select().await.unwrap();
    assert_eq!(lease.token(), "key-a");
    lease.report_success().await;
}

#[tokio::test]
async fn test_all_exhausted_returns_none() {
    let pool = CredentialPool::new(
        vec![("a".into(), api_key("key-a")), ("b".into(), api_key("key-b"))],
        SelectionStrategy::Failover,
    );

    // Exhaust both
    let lease = pool.select().await.unwrap();
    lease.report_failure(429).await;
    let lease = pool.select().await.unwrap();
    lease.report_failure(429).await;

    // Now both are in cooldown
    assert!(pool.select().await.is_none());
}

#[tokio::test]
async fn test_select_all_available() {
    let pool = CredentialPool::new(
        vec![
            ("a".into(), api_key("key-a")),
            ("b".into(), api_key("key-b")),
            ("c".into(), api_key("key-c")),
        ],
        SelectionStrategy::Failover,
    );

    // Mark 'b' as unhealthy
    {
        let lease = pool.select().await.unwrap();
        lease.report_success().await;
    }
    // Directly report failure on slot 1 (b)
    pool.slots[1].health.write().await.record_failure(429);

    let available = pool.select_all_available().await;
    assert_eq!(available.len(), 2);
    assert_eq!(available[0].token(), "key-a");
    assert_eq!(available[1].token(), "key-c");
}

#[tokio::test]
async fn test_consecutive_errors_increase_cooldown() {
    let pool = CredentialPool::single("default".into(), api_key("sk-test"));

    // First failure: 2^1 = 2s cooldown
    let lease = pool.select().await.unwrap();
    lease.report_failure(429).await;

    let summaries = pool.slot_summaries().await;
    assert_eq!(summaries[0].consecutive_errors, 1);
    assert!(summaries[0].in_cooldown);
    assert!(summaries[0].cooldown_remaining_secs <= 2);
}

#[tokio::test]
async fn test_success_resets_health() {
    let pool = CredentialPool::single("default".into(), api_key("sk-test"));

    // Fail a few times
    pool.slots[0].health.write().await.record_failure(429);
    pool.slots[0].health.write().await.record_failure(429);

    let summaries = pool.slot_summaries().await;
    assert_eq!(summaries[0].consecutive_errors, 2);

    // Reset via success
    pool.slots[0].health.write().await.record_success();

    let summaries = pool.slot_summaries().await;
    assert_eq!(summaries[0].consecutive_errors, 0);
    assert!(!summaries[0].in_cooldown);
}

#[tokio::test]
async fn test_reset_health() {
    let pool = CredentialPool::new(
        vec![("a".into(), api_key("key-a")), ("b".into(), api_key("key-b"))],
        SelectionStrategy::Failover,
    );

    pool.slots[0].health.write().await.record_failure(429);
    pool.slots[1].health.write().await.record_failure(429);

    pool.reset_health().await;

    let summaries = pool.slot_summaries().await;
    assert!(summaries[0].is_available);
    assert!(summaries[1].is_available);
}

#[tokio::test]
async fn test_non_retryable_error_no_cooldown() {
    let pool = CredentialPool::single("default".into(), api_key("sk-test"));

    // 401 is not retryable — should NOT enter cooldown
    let lease = pool.select().await.unwrap();
    lease.report_failure(401).await;

    let summaries = pool.slot_summaries().await;
    assert!(!summaries[0].in_cooldown);
    assert_eq!(summaries[0].failure_count, 1);
}

#[tokio::test]
async fn test_oauth_credential() {
    let pool = CredentialPool::single("default".into(), oauth_token("oat-123"));

    let lease = pool.select().await.unwrap();
    assert!(lease.is_oauth());
    assert_eq!(lease.token(), "oat-123");
}

#[tokio::test]
async fn test_slot_summary_display() {
    let pool = CredentialPool::new(
        vec![
            ("personal".into(), oauth_token("oat-1")),
            ("work".into(), api_key("sk-2")),
        ],
        SelectionStrategy::Failover,
    );

    let summaries = pool.slot_summaries().await;
    let s = format!("{}", summaries[0]);
    assert!(s.contains("personal"));
    assert!(s.contains("oauth"));
    assert!(s.contains("healthy"));

    let s = format!("{}", summaries[1]);
    assert!(s.contains("work"));
    assert!(s.contains("api-key"));
}

#[tokio::test]
async fn test_total_requests_counter() {
    let pool = CredentialPool::single("default".into(), api_key("sk-test"));
    assert_eq!(pool.total_requests(), 0);

    let lease = pool.select().await.unwrap();
    lease.report_success().await;

    let lease = pool.select().await.unwrap();
    lease.report_success().await;

    assert_eq!(pool.total_requests(), 2);
}
