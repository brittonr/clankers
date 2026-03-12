//! Global process registry for actor lookup and lifecycle management.

use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use dashmap::DashMap;
use tokio::sync::mpsc;

use crate::process::DeathReason;
use crate::process::ProcessHandle;
use crate::process::ProcessId;
use crate::signal::Signal;

/// Thread-safe process registry. Manages actor spawning, lookup, and shutdown.
#[derive(Clone)]
pub struct ProcessRegistry {
    inner: Arc<RegistryInner>,
}

struct RegistryInner {
    processes: DashMap<ProcessId, ProcessHandle>,
    names: DashMap<String, ProcessId>,
    next_id: AtomicU64,
    /// Links: process_id → set of linked process_ids with optional tags
    links: DashMap<ProcessId, Vec<(ProcessId, Option<i64>)>>,
    /// Monitors: watched_id → set of watcher_ids
    monitors: DashMap<ProcessId, Vec<ProcessId>>,
}

impl ProcessRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RegistryInner {
                processes: DashMap::new(),
                names: DashMap::new(),
                next_id: AtomicU64::new(1),
                links: DashMap::new(),
                monitors: DashMap::new(),
            }),
        }
    }

    /// Spawn a new actor process with default settings (`die_when_link_dies = true`).
    ///
    /// The `factory` receives the signal receiver and must return a future
    /// that runs the actor logic and produces a `DeathReason` on exit.
    pub fn spawn<F, Fut>(&self, name: Option<String>, parent: Option<ProcessId>, factory: F) -> ProcessId
    where
        F: FnOnce(ProcessId, mpsc::UnboundedReceiver<Signal>) -> Fut,
        Fut: Future<Output = DeathReason> + Send + 'static,
    {
        self.spawn_opts(name, parent, true, factory)
    }

    /// Spawn a new actor process with explicit options.
    ///
    /// When `die_when_link_dies` is false, the process receives `LinkDied`
    /// signals instead of being killed — used by supervisors that need to
    /// handle child deaths.
    pub fn spawn_opts<F, Fut>(
        &self,
        name: Option<String>,
        parent: Option<ProcessId>,
        die_when_link_dies: bool,
        factory: F,
    ) -> ProcessId
    where
        F: FnOnce(ProcessId, mpsc::UnboundedReceiver<Signal>) -> Fut,
        Fut: Future<Output = DeathReason> + Send + 'static,
    {
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let (signal_tx, signal_rx) = mpsc::unbounded_channel();

        let registry = self.clone();
        let fut = factory(id, signal_rx);

        let join = tokio::spawn(async move {
            let reason = fut.await;
            registry.on_process_exit(id, &reason);
            reason
        });

        let handle = ProcessHandle {
            id,
            signal_tx,
            join: Some(join),
            name: name.clone(),
            parent,
            started_at: Instant::now(),
            die_when_link_dies,
        };

        if let Some(ref n) = name {
            self.inner.names.insert(n.clone(), id);
        }
        self.inner.processes.insert(id, handle);

        id
    }

    /// Look up a process by ID.
    pub fn get(&self, id: ProcessId) -> Option<dashmap::mapref::one::Ref<'_, ProcessId, ProcessHandle>> {
        self.inner.processes.get(&id)
    }

    /// Look up a process by name.
    pub fn get_by_name(&self, name: &str) -> Option<ProcessId> {
        self.inner.names.get(name).map(|r| *r.value())
    }

    /// Send a signal to a process by ID. Returns false if process not found.
    pub fn send(&self, id: ProcessId, signal: Signal) -> bool {
        if let Some(handle) = self.inner.processes.get(&id) {
            handle.send(signal)
        } else {
            false
        }
    }

    /// Get child process IDs for a given parent.
    pub fn children(&self, parent_id: ProcessId) -> Vec<ProcessId> {
        self.inner
            .processes
            .iter()
            .filter(|entry| entry.value().parent == Some(parent_id))
            .map(|entry| entry.value().id)
            .collect()
    }

    /// List all process IDs.
    pub fn all_ids(&self) -> Vec<ProcessId> {
        self.inner.processes.iter().map(|e| *e.key()).collect()
    }

    /// Get the number of registered processes.
    pub fn len(&self) -> usize {
        self.inner.processes.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.processes.is_empty()
    }

    /// Establish a bidirectional link between two processes.
    pub fn link(&self, a: ProcessId, b: ProcessId, tag: Option<i64>) {
        self.inner.links.entry(a).or_default().push((b, tag));
        self.inner.links.entry(b).or_default().push((a, tag));
    }

    /// Remove a link between two processes.
    pub fn unlink(&self, a: ProcessId, b: ProcessId) {
        if let Some(mut links) = self.inner.links.get_mut(&a) {
            links.retain(|(pid, _)| *pid != b);
        }
        if let Some(mut links) = self.inner.links.get_mut(&b) {
            links.retain(|(pid, _)| *pid != a);
        }
    }

    /// Register a monitor (unidirectional: watcher is notified when watched dies).
    pub fn monitor(&self, watcher: ProcessId, watched: ProcessId) {
        self.inner.monitors.entry(watched).or_default().push(watcher);
    }

    /// Remove a monitor.
    pub fn stop_monitoring(&self, watcher: ProcessId, watched: ProcessId) {
        if let Some(mut watchers) = self.inner.monitors.get_mut(&watched) {
            watchers.retain(|w| *w != watcher);
        }
    }

    /// Initiate hierarchical shutdown: send Shutdown to all children, then wait.
    pub async fn shutdown_children(&self, parent_id: ProcessId, timeout: Duration) {
        let child_ids = self.children(parent_id);
        if child_ids.is_empty() {
            return;
        }

        // Send graceful shutdown to all children
        for &child_id in &child_ids {
            self.send(child_id, Signal::Shutdown { timeout });
        }

        // Wait for timeout, then force-kill survivors
        tokio::time::sleep(timeout).await;

        for &child_id in &child_ids {
            if self.inner.processes.contains_key(&child_id) {
                self.send(child_id, Signal::Kill);
            }
        }
    }

    /// Remove a process from the registry. Called internally on exit.
    fn remove(&self, id: ProcessId) {
        if let Some((_, handle)) = self.inner.processes.remove(&id)
            && let Some(ref name) = handle.name
        {
            self.inner.names.remove(name);
        }
        self.inner.links.remove(&id);
        self.inner.monitors.remove(&id);
    }

    /// Called when a process exits. Notifies linked and monitoring processes.
    ///
    /// Linked processes with `die_when_link_dies = true` are killed on
    /// abnormal exits (Failed/Killed). Supervisors set this to false so
    /// they receive `LinkDied` signals instead.
    fn on_process_exit(&self, id: ProcessId, reason: &DeathReason) {
        let is_abnormal = matches!(reason, DeathReason::Failed(_) | DeathReason::Killed);

        // Notify linked processes
        if let Some((_, links)) = self.inner.links.remove(&id) {
            for (linked_id, tag) in links {
                // Remove reverse link
                if let Some(mut reverse) = self.inner.links.get_mut(&linked_id) {
                    reverse.retain(|(pid, _)| *pid != id);
                }

                // Check if the linked process should die automatically
                let should_kill = is_abnormal
                    && self
                        .inner
                        .processes
                        .get(&linked_id)
                        .is_some_and(|h| h.die_when_link_dies);

                if should_kill {
                    self.send(linked_id, Signal::Kill);
                } else {
                    self.send(linked_id, Signal::LinkDied {
                        process_id: id,
                        tag,
                        reason: reason.clone(),
                    });
                }
            }
        }

        // Notify monitors
        if let Some((_, watchers)) = self.inner.monitors.remove(&id) {
            for watcher_id in watchers {
                self.send(watcher_id, Signal::ProcessDied {
                    process_id: id,
                    reason: reason.clone(),
                });
            }
        }

        // Remove from registry
        self.remove(id);
    }

    /// Build a snapshot of process info for all processes (for `clankers ps`).
    pub fn process_tree(&self) -> Vec<ProcessInfo> {
        self.inner
            .processes
            .iter()
            .map(|entry| {
                let h = entry.value();
                ProcessInfo {
                    id: h.id,
                    name: h.name.clone(),
                    parent: h.parent,
                    children: self.children(h.id),
                    uptime: h.uptime(),
                }
            })
            .collect()
    }
}

impl Default for ProcessRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot of a process for display (no handle references).
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub id: ProcessId,
    pub name: Option<String>,
    pub parent: Option<ProcessId>,
    pub children: Vec<ProcessId>,
    pub uptime: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_spawn_and_lookup() {
        let reg = ProcessRegistry::new();

        let id = reg.spawn(Some("test-process".to_string()), None, |_id, _rx| async { DeathReason::Normal });

        assert!(reg.get(id).is_some());
        assert_eq!(reg.get_by_name("test-process"), Some(id));
        assert_eq!(reg.len(), 1);

        // Wait for process to finish
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Process should be cleaned up
        assert!(reg.get(id).is_none());
        assert_eq!(reg.get_by_name("test-process"), None);
    }

    #[tokio::test]
    async fn test_parent_child() {
        let reg = ProcessRegistry::new();

        let parent_id = reg.spawn(Some("parent".to_string()), None, |_id, mut rx| async move {
            // Wait for shutdown
            while let Some(signal) = rx.recv().await {
                if matches!(signal, Signal::Kill | Signal::Shutdown { .. }) {
                    return DeathReason::Shutdown;
                }
            }
            DeathReason::Normal
        });

        let child_id = reg.spawn(Some("child".to_string()), Some(parent_id), |_id, mut rx| async move {
            while let Some(signal) = rx.recv().await {
                if matches!(signal, Signal::Kill | Signal::Shutdown { .. }) {
                    return DeathReason::Shutdown;
                }
            }
            DeathReason::Normal
        });

        let children = reg.children(parent_id);
        assert_eq!(children, vec![child_id]);

        // Clean up
        reg.send(child_id, Signal::Kill);
        reg.send(parent_id, Signal::Kill);
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    #[tokio::test]
    async fn test_link_death_notification() {
        let reg = ProcessRegistry::new();

        let (notify_tx, mut notify_rx) = mpsc::unbounded_channel::<(ProcessId, DeathReason)>();

        // die_when_link_dies=false so "a" receives LinkDied instead of Kill
        let a_id = reg.spawn_opts(Some("a".to_string()), None, false, move |_id, mut rx| async move {
            while let Some(signal) = rx.recv().await {
                if let Signal::LinkDied { process_id, reason, .. } = signal {
                    let _ = notify_tx.send((process_id, reason));
                    return DeathReason::Normal;
                }
            }
            DeathReason::Normal
        });

        let b_id = reg.spawn(Some("b".to_string()), None, |_id, _rx| async {
            // Die immediately with a failure
            DeathReason::Failed("test error".to_string())
        });

        // Link a to b
        reg.link(a_id, b_id, Some(42));

        // Wait for b to die and notification to propagate
        tokio::time::sleep(Duration::from_millis(100)).await;

        let (dead_id, reason) = notify_rx.recv().await.unwrap();
        assert_eq!(dead_id, b_id);
        assert_eq!(reason, DeathReason::Failed("test error".to_string()));
    }

    #[tokio::test]
    async fn test_monitor() {
        let reg = ProcessRegistry::new();

        let (notify_tx, mut notify_rx) = mpsc::unbounded_channel::<(ProcessId, DeathReason)>();

        let watcher_id = reg.spawn(Some("watcher".to_string()), None, move |_id, mut rx| async move {
            while let Some(signal) = rx.recv().await {
                if let Signal::ProcessDied { process_id, reason } = signal {
                    let _ = notify_tx.send((process_id, reason));
                    return DeathReason::Normal;
                }
            }
            DeathReason::Normal
        });

        let watched_id = reg.spawn(Some("watched".to_string()), None, |_id, _rx| async { DeathReason::Normal });

        reg.monitor(watcher_id, watched_id);

        // Wait for watched to die
        tokio::time::sleep(Duration::from_millis(100)).await;

        let (dead_id, reason) = notify_rx.recv().await.unwrap();
        assert_eq!(dead_id, watched_id);
        assert_eq!(reason, DeathReason::Normal);
    }

    #[tokio::test]
    async fn test_send_to_terminated() {
        let reg = ProcessRegistry::new();

        let id = reg.spawn(None, None, |_id, _rx| async { DeathReason::Normal });

        // Wait for process to finish
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Sending to a terminated process returns false (fire-and-forget)
        assert!(!reg.send(id, Signal::Kill));
    }

    #[tokio::test]
    async fn test_hierarchical_shutdown() {
        let reg = ProcessRegistry::new();

        let (done_tx, mut done_rx) = mpsc::unbounded_channel::<ProcessId>();

        let parent_id = reg.spawn(Some("parent".to_string()), None, |_id, mut rx| async move {
            while let Some(signal) = rx.recv().await {
                if matches!(signal, Signal::Kill | Signal::Shutdown { .. }) {
                    return DeathReason::Shutdown;
                }
            }
            DeathReason::Normal
        });

        let tx1 = done_tx.clone();
        let _child1 = reg.spawn(Some("child1".to_string()), Some(parent_id), move |id, mut rx| async move {
            while let Some(signal) = rx.recv().await {
                if matches!(signal, Signal::Shutdown { .. }) {
                    let _ = tx1.send(id);
                    return DeathReason::Shutdown;
                }
            }
            DeathReason::Normal
        });

        let tx2 = done_tx;
        let _child2 = reg.spawn(Some("child2".to_string()), Some(parent_id), move |id, mut rx| async move {
            while let Some(signal) = rx.recv().await {
                if matches!(signal, Signal::Shutdown { .. }) {
                    let _ = tx2.send(id);
                    return DeathReason::Shutdown;
                }
            }
            DeathReason::Normal
        });

        assert_eq!(reg.children(parent_id).len(), 2);

        reg.shutdown_children(parent_id, Duration::from_secs(1)).await;

        // Both children should have received shutdown
        let mut exited = Vec::new();
        while let Ok(id) = done_rx.try_recv() {
            exited.push(id);
        }
        assert_eq!(exited.len(), 2);
    }

    #[tokio::test]
    async fn test_process_tree_snapshot() {
        let reg = ProcessRegistry::new();

        let parent_id = reg.spawn(Some("root".to_string()), None, |_id, mut rx| async move {
            while let Some(signal) = rx.recv().await {
                if matches!(signal, Signal::Kill) {
                    return DeathReason::Killed;
                }
            }
            DeathReason::Normal
        });

        let _child = reg.spawn(Some("child".to_string()), Some(parent_id), |_id, mut rx| async move {
            while let Some(signal) = rx.recv().await {
                if matches!(signal, Signal::Kill) {
                    return DeathReason::Killed;
                }
            }
            DeathReason::Normal
        });

        let tree = reg.process_tree();
        assert_eq!(tree.len(), 2);

        let root = tree.iter().find(|p| p.name.as_deref() == Some("root")).unwrap();
        assert!(root.parent.is_none());
        assert_eq!(root.children.len(), 1);

        let child = tree.iter().find(|p| p.name.as_deref() == Some("child")).unwrap();
        assert_eq!(child.parent, Some(parent_id));

        // Clean up
        reg.send(_child, Signal::Kill);
        reg.send(parent_id, Signal::Kill);
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    #[tokio::test]
    async fn test_die_when_link_dies_kills_on_abnormal() {
        let reg = ProcessRegistry::new();
        let (done_tx, mut done_rx) = mpsc::unbounded_channel::<DeathReason>();

        // Process A: die_when_link_dies = true (default)
        let a_id = reg.spawn(Some("a".to_string()), None, move |_id, mut rx| async move {
            while let Some(signal) = rx.recv().await {
                match signal {
                    Signal::Kill => {
                        let _ = done_tx.send(DeathReason::Killed);
                        return DeathReason::Killed;
                    }
                    Signal::LinkDied { .. } => {
                        // Should NOT reach here with die_when_link_dies=true
                        let _ = done_tx.send(DeathReason::Normal);
                        return DeathReason::Normal;
                    }
                    _ => {}
                }
            }
            DeathReason::Normal
        });

        // Process B: dies with failure
        let b_id = reg.spawn(Some("b".to_string()), None, |_id, _rx| async {
            DeathReason::Failed("crash".to_string())
        });

        reg.link(a_id, b_id, None);

        // Wait for B to die and cascade
        tokio::time::sleep(Duration::from_millis(100)).await;

        // A should have received Kill (not LinkDied)
        let reason = done_rx.recv().await.unwrap();
        assert_eq!(reason, DeathReason::Killed);
    }

    #[tokio::test]
    async fn test_die_when_link_dies_false_gets_signal() {
        let reg = ProcessRegistry::new();
        let (done_tx, mut done_rx) = mpsc::unbounded_channel::<DeathReason>();

        // Process A: die_when_link_dies = false (supervisor behavior)
        let a_id = reg.spawn_opts(
            Some("supervisor".to_string()),
            None,
            false,
            move |_id, mut rx| async move {
                while let Some(signal) = rx.recv().await {
                    match signal {
                        Signal::Kill => {
                            let _ = done_tx.send(DeathReason::Killed);
                            return DeathReason::Killed;
                        }
                        Signal::LinkDied { reason, .. } => {
                            // Should reach here with die_when_link_dies=false
                            let _ = done_tx.send(reason);
                            return DeathReason::Normal;
                        }
                        _ => {}
                    }
                }
                DeathReason::Normal
            },
        );

        // Process B: dies with failure
        let b_id = reg.spawn(Some("child".to_string()), None, |_id, _rx| async {
            DeathReason::Failed("crash".to_string())
        });

        reg.link(a_id, b_id, None);

        // Wait for B to die
        tokio::time::sleep(Duration::from_millis(100)).await;

        // A should have received LinkDied (not Kill)
        let reason = done_rx.recv().await.unwrap();
        assert_eq!(reason, DeathReason::Failed("crash".to_string()));
    }

    #[tokio::test]
    async fn test_die_when_link_dies_normal_exit_no_kill() {
        let reg = ProcessRegistry::new();
        let (done_tx, mut done_rx) = mpsc::unbounded_channel::<DeathReason>();

        // Process A: die_when_link_dies = true, but B exits normally
        let a_id = reg.spawn(Some("a".to_string()), None, move |_id, mut rx| async move {
            while let Some(signal) = rx.recv().await {
                match signal {
                    Signal::Kill => {
                        let _ = done_tx.send(DeathReason::Killed);
                        return DeathReason::Killed;
                    }
                    Signal::LinkDied { reason, .. } => {
                        // Normal exit should deliver LinkDied, not Kill
                        let _ = done_tx.send(reason);
                        return DeathReason::Normal;
                    }
                    _ => {}
                }
            }
            DeathReason::Normal
        });

        // Process B: exits normally
        let b_id = reg.spawn(Some("b".to_string()), None, |_id, _rx| async {
            DeathReason::Normal
        });

        reg.link(a_id, b_id, None);

        // Wait for B to die
        tokio::time::sleep(Duration::from_millis(100)).await;

        // A should get LinkDied (not Kill) because B exited normally
        let reason = done_rx.recv().await.unwrap();
        assert_eq!(reason, DeathReason::Normal);
    }
}
