//! Integration tests for clankers-actor.
//!
//! Tests actor spawning, linking, monitoring, supervision, and cascading
//! shutdown over the registry — exercising the full lifecycle.

use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;
use std::time::Duration;

use clankers_actor::process::DeathReason;
use clankers_actor::registry::ProcessRegistry;
use clankers_actor::signal::Signal;
use clankers_actor::supervisor::Supervisor;
use clankers_actor::supervisor::SupervisorConfig;
use clankers_actor::supervisor::SupervisorStrategy;
use tokio::sync::mpsc;

// ── Multi-level process tree ────────────────────────────────

#[tokio::test]
async fn three_level_process_tree() {
    let reg = ProcessRegistry::new();

    let root = reg.spawn(Some("root".into()), None, |_id, mut rx| async move {
        while let Some(signal) = rx.recv().await {
            if matches!(signal, Signal::Kill) {
                return DeathReason::Killed;
            }
        }
        DeathReason::Normal
    });

    let mid = reg.spawn(Some("mid".into()), Some(root), |_id, mut rx| async move {
        while let Some(signal) = rx.recv().await {
            if matches!(signal, Signal::Kill) {
                return DeathReason::Killed;
            }
        }
        DeathReason::Normal
    });

    let leaf = reg.spawn(Some("leaf".into()), Some(mid), |_id, mut rx| async move {
        while let Some(signal) = rx.recv().await {
            if matches!(signal, Signal::Kill) {
                return DeathReason::Killed;
            }
        }
        DeathReason::Normal
    });

    // Verify tree structure
    assert_eq!(reg.children(root), vec![mid]);
    assert_eq!(reg.children(mid), vec![leaf]);
    assert!(reg.children(leaf).is_empty());
    assert_eq!(reg.len(), 3);

    let tree = reg.process_tree();
    assert_eq!(tree.len(), 3);

    // shutdown_children sends signals to direct children only (not grandchildren)
    // So we shut down mid's children first, then root's children
    reg.shutdown_children(mid, Duration::from_millis(200)).await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Leaf (grandchild) should be gone
    assert!(reg.get(leaf).is_none());

    reg.shutdown_children(root, Duration::from_millis(200)).await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Mid (direct child) should be gone
    assert!(reg.get(mid).is_none());

    // Root still alive (shutdown_children doesn't kill the parent)
    assert!(reg.get(root).is_some());

    reg.send(root, Signal::Kill);
    tokio::time::sleep(Duration::from_millis(50)).await;
}

// ── Link cascade ────────────────────────────────────────────

#[tokio::test]
async fn link_cascade_propagates_death() {
    let reg = ProcessRegistry::new();
    let (done_tx, mut done_rx) = mpsc::unbounded_channel::<(String, DeathReason)>();

    let tx1 = done_tx.clone();
    let a = reg.spawn(Some("a".into()), None, move |_id, mut rx| async move {
        while let Some(signal) = rx.recv().await {
            if let Signal::LinkDied { reason, .. } = signal {
                let _ = tx1.send(("a".into(), reason));
                return DeathReason::Normal;
            }
        }
        DeathReason::Normal
    });

    let tx2 = done_tx.clone();
    let b = reg.spawn(Some("b".into()), None, move |_id, mut rx| async move {
        while let Some(signal) = rx.recv().await {
            if let Signal::LinkDied { reason, .. } = signal {
                let _ = tx2.send(("b".into(), reason));
                return DeathReason::Normal;
            }
        }
        DeathReason::Normal
    });

    // c will die immediately, triggering cascading LinkDied to a and b
    let c = reg.spawn(Some("c".into()), None, |_id, _rx| async { DeathReason::Failed("crash".into()) });

    // Link a<->c and b<->c
    reg.link(a, c, None);
    reg.link(b, c, None);

    // Wait for cascade
    tokio::time::sleep(Duration::from_millis(200)).await;

    let mut notifications = Vec::new();
    while let Ok(n) = done_rx.try_recv() {
        notifications.push(n);
    }

    // Both a and b should have been notified
    assert_eq!(notifications.len(), 2);
    let names: Vec<&str> = notifications.iter().map(|(n, _)| n.as_str()).collect();
    assert!(names.contains(&"a"));
    assert!(names.contains(&"b"));

    // All should have the "crash" reason
    for (_, reason) in &notifications {
        assert_eq!(*reason, DeathReason::Failed("crash".into()));
    }
}

// ── Monitor without link ────────────────────────────────────

#[tokio::test]
async fn monitor_does_not_cascade() {
    let reg = ProcessRegistry::new();
    let (notify_tx, mut notify_rx) = mpsc::unbounded_channel::<DeathReason>();

    let watcher = reg.spawn(Some("watcher".into()), None, move |_id, mut rx| async move {
        while let Some(signal) = rx.recv().await {
            match signal {
                Signal::ProcessDied { reason, .. } => {
                    let _ = notify_tx.send(reason);
                    // Watcher stays alive — monitors don't cascade
                }
                Signal::Kill => return DeathReason::Killed,
                _ => {}
            }
        }
        DeathReason::Normal
    });

    let watched = reg.spawn(Some("watched".into()), None, |_id, _rx| async { DeathReason::Failed("oops".into()) });

    reg.monitor(watcher, watched);

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Watcher received notification
    let reason = notify_rx.recv().await.unwrap();
    assert_eq!(reason, DeathReason::Failed("oops".into()));

    // Watcher is still alive (monitor doesn't kill)
    assert!(reg.get(watcher).is_some());

    reg.send(watcher, Signal::Kill);
    tokio::time::sleep(Duration::from_millis(50)).await;
}

// ── Unlink prevents notification ────────────────────────────

#[tokio::test]
async fn unlink_prevents_death_notification() {
    let reg = ProcessRegistry::new();
    let (notify_tx, mut notify_rx) = mpsc::unbounded_channel::<String>();

    let tx = notify_tx.clone();
    let a = reg.spawn(Some("a".into()), None, move |_id, mut rx| async move {
        while let Some(signal) = rx.recv().await {
            match signal {
                Signal::LinkDied { .. } => {
                    let _ = tx.send("link_died".into());
                    return DeathReason::Normal;
                }
                Signal::Kill => return DeathReason::Killed,
                _ => {}
            }
        }
        DeathReason::Normal
    });

    let b = reg.spawn(Some("b".into()), None, |_id, mut rx| async move {
        // Wait to be killed
        while let Some(signal) = rx.recv().await {
            if matches!(signal, Signal::Kill) {
                return DeathReason::Failed("killed".into());
            }
        }
        DeathReason::Normal
    });

    reg.link(a, b, None);
    reg.unlink(a, b);

    // Now kill b — a should NOT receive LinkDied
    reg.send(b, Signal::Kill);
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Check: no notification received
    assert!(notify_rx.try_recv().is_err());

    // a should still be alive
    assert!(reg.get(a).is_some());

    reg.send(a, Signal::Kill);
    tokio::time::sleep(Duration::from_millis(50)).await;
}

// ── Named lookup ────────────────────────────────────────────

#[tokio::test]
async fn name_lookup_and_signal() {
    let reg = ProcessRegistry::new();
    let (done_tx, mut done_rx) = mpsc::unbounded_channel::<String>();

    let tx = done_tx.clone();
    let _id = reg.spawn(Some("worker-1".into()), None, move |_id, mut rx| async move {
        while let Some(signal) = rx.recv().await {
            if let Signal::Message(msg) = signal {
                if let Some(text) = msg.downcast_ref::<String>() {
                    let _ = tx.send(text.clone());
                    return DeathReason::Normal;
                }
            }
        }
        DeathReason::Normal
    });

    // Look up by name and send a signal
    let pid = reg.get_by_name("worker-1").unwrap();
    reg.send(pid, Signal::Message(Box::new("hello from test".to_string())));

    let msg = done_rx.recv().await.unwrap();
    assert_eq!(msg, "hello from test");
}

// ── Supervisor one-for-one ──────────────────────────────────

#[tokio::test]
async fn supervisor_one_for_one_restart() {
    let config = SupervisorConfig {
        strategy: SupervisorStrategy::OneForOne,
        max_restarts: 3,
        restart_window: Duration::from_secs(60),
    };
    let mut sup = Supervisor::new(config);
    sup.add_child("worker".into());

    // Simulate 3 failures
    for _ in 0..3 {
        assert!(sup.can_restart());
        let to_restart = sup.children_to_restart("worker");
        assert_eq!(to_restart, vec!["worker"]);
        sup.record_restart();
    }

    // 4th should be rejected
    assert!(!sup.can_restart());
}

// ── Supervisor rest-for-one ordering ────────────────────────

#[tokio::test]
async fn supervisor_rest_for_one_ordering() {
    let config = SupervisorConfig {
        strategy: SupervisorStrategy::RestForOne,
        max_restarts: 10,
        restart_window: Duration::from_secs(60),
    };
    let mut sup = Supervisor::new(config);
    sup.add_child("db".into());
    sup.add_child("cache".into());
    sup.add_child("api".into());
    sup.add_child("web".into());

    // If cache fails, restart cache + everything after it
    let to_restart = sup.children_to_restart("cache");
    assert_eq!(to_restart, vec!["cache", "api", "web"]);

    // If web fails, only restart web
    let to_restart = sup.children_to_restart("web");
    assert_eq!(to_restart, vec!["web"]);

    // If db fails, restart everything
    let to_restart = sup.children_to_restart("db");
    assert_eq!(to_restart, vec!["db", "cache", "api", "web"]);
}

// ── Concurrent spawning stress ──────────────────────────────

#[tokio::test]
async fn concurrent_spawn_stress() {
    let reg = ProcessRegistry::new();
    let counter = Arc::new(AtomicU32::new(0));

    let mut handles = Vec::new();
    for i in 0..50 {
        let r = reg.clone();
        let c = counter.clone();
        handles.push(tokio::spawn(async move {
            r.spawn(Some(format!("worker-{i}")), None, move |_id, _rx| {
                let c = c.clone();
                async move {
                    c.fetch_add(1, Ordering::Relaxed);
                    DeathReason::Normal
                }
            });
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    // Wait for all to complete
    tokio::time::sleep(Duration::from_millis(200)).await;

    assert_eq!(counter.load(Ordering::Relaxed), 50);

    // All should have cleaned up
    assert!(reg.is_empty());
}

// ── Graceful shutdown with timeout ──────────────────────────

#[tokio::test]
async fn shutdown_with_stuck_child_falls_back_to_kill() {
    let reg = ProcessRegistry::new();

    let parent = reg.spawn(Some("parent".into()), None, |_id, mut rx| async move {
        while let Some(signal) = rx.recv().await {
            if matches!(signal, Signal::Kill) {
                return DeathReason::Killed;
            }
        }
        DeathReason::Normal
    });

    // Stuck child ignores Shutdown signals
    let _stuck = reg.spawn(Some("stuck".into()), Some(parent), |_id, mut rx| async move {
        loop {
            match rx.recv().await {
                Some(Signal::Kill) => return DeathReason::Killed,
                Some(Signal::Shutdown { .. }) => {
                    // Ignore shutdown — simulate a stuck process
                    continue;
                }
                None => return DeathReason::Normal,
                _ => continue,
            }
        }
    });

    // Short timeout — stuck child won't respond to Shutdown
    reg.shutdown_children(parent, Duration::from_millis(100)).await;

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Stuck child should have been force-killed
    assert!(reg.get(_stuck).is_none());

    reg.send(parent, Signal::Kill);
    tokio::time::sleep(Duration::from_millis(50)).await;
}
