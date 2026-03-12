//! Supervisor — watches children, restarts per strategy, tracks restart rate.

use std::collections::VecDeque;
use std::time::Duration;
use std::time::Instant;

use tokio::sync::mpsc;
use tracing::info;
use tracing::warn;

use crate::process::DeathReason;
use crate::registry::ProcessRegistry;
use crate::signal::Signal;

/// Strategy for restarting failed children.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisorStrategy {
    /// Restart only the failed child.
    OneForOne,
    /// Restart all children if any one fails.
    OneForAll,
    /// Restart the failed child and all children started after it.
    RestForOne,
}

/// Configuration for a supervisor.
#[derive(Debug, Clone)]
pub struct SupervisorConfig {
    pub strategy: SupervisorStrategy,
    pub max_restarts: u32,
    pub restart_window: Duration,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            strategy: SupervisorStrategy::OneForOne,
            max_restarts: 5,
            restart_window: Duration::from_secs(60),
        }
    }
}

/// Supervised actor that monitors children and restarts them on failure.
pub struct Supervisor {
    config: SupervisorConfig,
    /// Ordered list of child names for RestForOne strategy.
    child_order: Vec<String>,
    /// Restart timestamps within the window (for rate limiting).
    restart_history: VecDeque<Instant>,
}

impl Supervisor {
    /// Create a new supervisor with the given configuration.
    pub fn new(config: SupervisorConfig) -> Self {
        Self {
            config,
            child_order: Vec::new(),
            restart_history: VecDeque::new(),
        }
    }

    /// Register a child name in the ordered list.
    pub fn add_child(&mut self, name: String) {
        if !self.child_order.contains(&name) {
            self.child_order.push(name);
        }
    }

    /// Remove a child name from the ordered list.
    pub fn remove_child(&mut self, name: &str) {
        self.child_order.retain(|n| n != name);
    }

    /// Check if a restart is allowed (under the rate limit).
    pub fn can_restart(&mut self) -> bool {
        let now = Instant::now();
        let cutoff = now - self.config.restart_window;

        // Prune old entries
        while self.restart_history.front().is_some_and(|t| *t < cutoff) {
            self.restart_history.pop_front();
        }

        self.restart_history.len() < self.config.max_restarts as usize
    }

    /// Record a restart attempt.
    pub fn record_restart(&mut self) {
        self.restart_history.push_back(Instant::now());
    }

    /// Get child names that should be restarted given a failed child name.
    pub fn children_to_restart(&self, failed: &str) -> Vec<String> {
        match self.config.strategy {
            SupervisorStrategy::OneForOne => {
                vec![failed.to_string()]
            }
            SupervisorStrategy::OneForAll => self.child_order.clone(),
            SupervisorStrategy::RestForOne => {
                if let Some(pos) = self.child_order.iter().position(|n| n == failed) {
                    self.child_order[pos..].to_vec()
                } else {
                    vec![failed.to_string()]
                }
            }
        }
    }

    /// Run the supervisor loop. Processes signals and manages child restarts.
    ///
    /// The `restart_fn` is called for each child that needs restarting.
    /// It receives the child name and should spawn a new process.
    /// Returns `true` if the restart succeeded.
    pub async fn run<F>(
        &mut self,
        mut signal_rx: mpsc::UnboundedReceiver<Signal>,
        registry: &ProcessRegistry,
        mut restart_fn: F,
    ) -> DeathReason
    where
        F: FnMut(&str, &ProcessRegistry) -> bool,
    {
        loop {
            let Some(signal) = signal_rx.recv().await else {
                return DeathReason::Normal;
            };

            match signal {
                Signal::Kill => {
                    info!("supervisor: received Kill");
                    return DeathReason::Killed;
                }
                Signal::Shutdown { timeout } => {
                    info!("supervisor: received Shutdown, timeout={timeout:?}");
                    return DeathReason::Shutdown;
                }
                Signal::LinkDied { process_id, reason, .. } => {
                    let name = registry
                        .get(process_id)
                        .and_then(|h| h.name.clone())
                        .unwrap_or_else(|| format!("pid:{process_id}"));

                    match &reason {
                        DeathReason::Normal | DeathReason::Shutdown => {
                            info!("supervisor: child {name} exited: {reason}");
                            self.remove_child(&name);
                        }
                        DeathReason::Failed(msg) => {
                            warn!("supervisor: child {name} failed: {msg}");
                            if !self.can_restart() {
                                warn!("supervisor: max restart rate exceeded, shutting down");
                                return DeathReason::Failed(format!(
                                    "max restart rate exceeded ({} in {:?})",
                                    self.config.max_restarts, self.config.restart_window
                                ));
                            }

                            let to_restart = self.children_to_restart(&name);
                            for child_name in &to_restart {
                                self.record_restart();
                                if !restart_fn(child_name, registry) {
                                    warn!("supervisor: failed to restart {child_name}");
                                }
                            }
                        }
                        DeathReason::Killed => {
                            info!("supervisor: child {name} was killed");
                            self.remove_child(&name);
                        }
                    }
                }
                Signal::ProcessDied { process_id, reason } => {
                    info!("supervisor: monitored process {process_id} died: {reason}");
                }
                _ => {
                    // Ignore other signals
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_one_for_one_strategy() {
        let config = SupervisorConfig {
            strategy: SupervisorStrategy::OneForOne,
            ..Default::default()
        };
        let mut sup = Supervisor::new(config);
        sup.add_child("a".to_string());
        sup.add_child("b".to_string());
        sup.add_child("c".to_string());

        let to_restart = sup.children_to_restart("b");
        assert_eq!(to_restart, vec!["b"]);
    }

    #[test]
    fn test_one_for_all_strategy() {
        let config = SupervisorConfig {
            strategy: SupervisorStrategy::OneForAll,
            ..Default::default()
        };
        let mut sup = Supervisor::new(config);
        sup.add_child("a".to_string());
        sup.add_child("b".to_string());
        sup.add_child("c".to_string());

        let to_restart = sup.children_to_restart("b");
        assert_eq!(to_restart, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_rest_for_one_strategy() {
        let config = SupervisorConfig {
            strategy: SupervisorStrategy::RestForOne,
            ..Default::default()
        };
        let mut sup = Supervisor::new(config);
        sup.add_child("a".to_string());
        sup.add_child("b".to_string());
        sup.add_child("c".to_string());

        let to_restart = sup.children_to_restart("b");
        assert_eq!(to_restart, vec!["b", "c"]);
    }

    #[test]
    fn test_rest_for_one_first_child() {
        let config = SupervisorConfig {
            strategy: SupervisorStrategy::RestForOne,
            ..Default::default()
        };
        let mut sup = Supervisor::new(config);
        sup.add_child("a".to_string());
        sup.add_child("b".to_string());
        sup.add_child("c".to_string());

        let to_restart = sup.children_to_restart("a");
        assert_eq!(to_restart, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_rest_for_one_last_child() {
        let config = SupervisorConfig {
            strategy: SupervisorStrategy::RestForOne,
            ..Default::default()
        };
        let mut sup = Supervisor::new(config);
        sup.add_child("a".to_string());
        sup.add_child("b".to_string());
        sup.add_child("c".to_string());

        let to_restart = sup.children_to_restart("c");
        assert_eq!(to_restart, vec!["c"]);
    }

    #[test]
    fn test_restart_rate_limiting() {
        let config = SupervisorConfig {
            max_restarts: 3,
            restart_window: Duration::from_secs(60),
            ..Default::default()
        };
        let mut sup = Supervisor::new(config);

        assert!(sup.can_restart());
        sup.record_restart();
        assert!(sup.can_restart());
        sup.record_restart();
        assert!(sup.can_restart());
        sup.record_restart();
        // 3 restarts = max
        assert!(!sup.can_restart());
    }

    #[test]
    fn test_add_remove_child() {
        let mut sup = Supervisor::new(SupervisorConfig::default());
        sup.add_child("a".to_string());
        sup.add_child("b".to_string());
        assert_eq!(sup.child_order, vec!["a", "b"]);

        // Duplicate add is idempotent
        sup.add_child("a".to_string());
        assert_eq!(sup.child_order, vec!["a", "b"]);

        sup.remove_child("a");
        assert_eq!(sup.child_order, vec!["b"]);
    }

    #[test]
    fn test_unknown_child_restart() {
        let config = SupervisorConfig {
            strategy: SupervisorStrategy::RestForOne,
            ..Default::default()
        };
        let sup = Supervisor::new(config);

        // Unknown child falls back to just that child
        let to_restart = sup.children_to_restart("unknown");
        assert_eq!(to_restart, vec!["unknown"]);
    }
}
