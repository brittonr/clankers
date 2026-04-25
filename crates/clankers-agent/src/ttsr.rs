//! TTSR — Time Traveling Streamed Rules
//!
//! Zero-context rules that inject only when regex triggers match the model's
//! output stream mid-generation. This allows dynamically injecting corrective
//! guidance without bloating the system prompt.
//!
//! Rules are defined in `.clankers/ttsr.json` or `~/.clankers/agent/ttsr.json`:
//! ```json
//! [
//!   {
//!     "name": "no-unwrap",
//!     "trigger": "\\.unwrap\\(\\)",
//!     "injection": "Avoid using .unwrap() — prefer .context()? or proper error handling.",
//!     "mode": "append_system",
//!     "cooldown_secs": 60,
//!     "max_fires": 3
//!   },
//!   {
//!     "name": "sql-injection-guard",
//!     "trigger": "format!\\(\"SELECT.*\\{",
//!     "injection": "Use parameterized queries instead of string interpolation for SQL.",
//!     "mode": "interrupt",
//!     "max_fires": 1
//!   }
//! ]
//! ```
//!
//! Modes:
//! - `append_system` — Append text to the system prompt for subsequent turns
//! - `prepend_user` — Add text at the start of the next user message
//! - `interrupt` — Abort the current generation and inject as a user correction
//! - `log` — Just log the match (no injection)

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use std::time::Instant;

use regex::Regex;
use serde::Deserialize;
use serde::Serialize;

/// A single TTSR rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsrRule {
    /// Human-readable name for this rule
    pub name: String,
    /// Regex pattern to match against the streaming output
    pub trigger: String,
    /// Text to inject when the trigger fires
    pub injection: String,
    /// How the injection is delivered
    #[serde(default)]
    pub mode: InjectionMode,
    /// Minimum seconds between firings of this rule (default: 30)
    #[serde(default = "default_cooldown")]
    pub cooldown_secs: u64,
    /// Maximum number of times this rule can fire per session (0 = unlimited)
    #[serde(default)]
    pub max_fires: u32,
    /// Whether this rule is enabled (default: true)
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_cooldown() -> u64 {
    30
}
fn default_true() -> bool {
    true
}

/// How a triggered rule's injection is delivered
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InjectionMode {
    /// Append the injection text to the system prompt for all future turns
    #[default]
    AppendSystem,
    /// Prepend the injection text to the next user message
    PrependUser,
    /// Abort the current generation and inject as a user correction
    Interrupt,
    /// Just log the match, don't inject anything
    Log,
}

/// A compiled TTSR rule with its regex
struct CompiledRule {
    rule: TtsrRule,
    regex: Regex,
}

/// Runtime state for a rule
struct RuleState {
    fire_count: u32,
    last_fired: Option<Instant>,
}

/// A triggered rule firing
#[derive(Debug, Clone)]
pub struct TtsrFiring {
    /// The rule that fired
    pub rule_name: String,
    /// The injection text
    pub injection: String,
    /// How the injection should be delivered
    pub mode: InjectionMode,
    /// The matched text that triggered the rule
    pub matched_text: String,
}

/// The TTSR engine that watches streaming output and fires rules
pub struct TtsrEngine {
    rules: Vec<CompiledRule>,
    states: Vec<RuleState>,
    /// Rolling window of recent output text for matching
    window: String,
    /// Maximum window size (chars) to avoid unbounded growth
    max_window: usize,
    /// Accumulated system prompt additions from fired rules
    system_additions: Vec<String>,
    /// Pending user-message prepends
    user_prepends: Vec<String>,
}

impl TtsrEngine {
    /// Create a new engine from a list of rules
    pub fn new(rules: Vec<TtsrRule>) -> Self {
        let mut compiled = Vec::new();
        let mut states = Vec::new();

        for rule in rules {
            if !rule.enabled {
                continue;
            }
            match Regex::new(&rule.trigger) {
                Ok(regex) => {
                    compiled.push(CompiledRule { rule, regex });
                    states.push(RuleState {
                        fire_count: 0,
                        last_fired: None,
                    });
                }
                Err(e) => {
                    tracing::warn!("TTSR rule '{}' has invalid regex '{}': {}", rule.name, rule.trigger, e);
                }
            }
        }

        Self {
            rules: compiled,
            states,
            window: String::new(),
            max_window: 4096,
            system_additions: Vec::new(),
            user_prepends: Vec::new(),
        }
    }

    /// Create an empty engine (no rules)
    pub fn empty() -> Self {
        Self {
            rules: Vec::new(),
            states: Vec::new(),
            window: String::new(),
            max_window: 4096,
            system_additions: Vec::new(),
            user_prepends: Vec::new(),
        }
    }

    /// Feed streaming text into the engine. Returns any firings that occurred.
    pub fn feed(&mut self, text: &str) -> Vec<TtsrFiring> {
        if self.rules.is_empty() {
            return Vec::new();
        }

        self.window.push_str(text);

        // Trim window to max size (keeping the tail)
        if self.window.len() > self.max_window {
            let trim_to = self.window.len() - (self.max_window / 2);
            // Find a char boundary
            let boundary = self.window.ceil_char_boundary(trim_to);
            self.window = self.window[boundary..].to_string();
        }

        let now = Instant::now();
        let mut firings = Vec::new();

        for (i, compiled) in self.rules.iter().enumerate() {
            let state = &mut self.states[i];

            // Check max_fires
            if compiled.rule.max_fires > 0 && state.fire_count >= compiled.rule.max_fires {
                continue;
            }

            // Check cooldown
            if let Some(last) = state.last_fired
                && now.duration_since(last).as_secs() < compiled.rule.cooldown_secs
            {
                continue;
            }

            // Check trigger
            if let Some(mat) = compiled.regex.find(&self.window) {
                let matched_text = mat.as_str().to_string();
                state.fire_count += 1;
                state.last_fired = Some(now);

                let firing = TtsrFiring {
                    rule_name: compiled.rule.name.clone(),
                    injection: compiled.rule.injection.clone(),
                    mode: compiled.rule.mode,
                    matched_text,
                };

                // Apply side effects
                match compiled.rule.mode {
                    InjectionMode::AppendSystem => {
                        self.system_additions.push(compiled.rule.injection.clone());
                    }
                    InjectionMode::PrependUser => {
                        self.user_prepends.push(compiled.rule.injection.clone());
                    }
                    InjectionMode::Interrupt | InjectionMode::Log => {
                        // These are handled by the caller
                    }
                }

                tracing::info!(
                    "TTSR rule '{}' fired (mode={:?}, fires={})",
                    compiled.rule.name,
                    compiled.rule.mode,
                    state.fire_count
                );

                firings.push(firing);
            }
        }

        firings
    }

    /// Get all accumulated system prompt additions and clear them
    pub fn take_system_additions(&mut self) -> Vec<String> {
        std::mem::take(&mut self.system_additions)
    }

    /// Get all accumulated user message prepends and clear them
    pub fn take_user_prepends(&mut self) -> Vec<String> {
        std::mem::take(&mut self.user_prepends)
    }

    /// Reset the matching window (e.g., between turns)
    pub fn reset_window(&mut self) {
        self.window.clear();
    }

    /// Check if any rules are loaded
    pub fn is_active(&self) -> bool {
        !self.rules.is_empty()
    }

    /// Get stats for all rules
    pub fn stats(&self) -> Vec<(String, u32)> {
        self.rules
            .iter()
            .zip(self.states.iter())
            .map(|(r, s)| (r.rule.name.clone(), s.fire_count))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_rule(name: &str, trigger: &str, injection: &str, mode: InjectionMode) -> TtsrRule {
        TtsrRule {
            name: name.to_string(),
            trigger: trigger.to_string(),
            injection: injection.to_string(),
            mode,
            cooldown_secs: 0, // No cooldown for tests
            max_fires: 0,     // Unlimited
            enabled: true,
        }
    }

    #[test]
    fn test_basic_trigger() {
        let rules = vec![make_rule(
            "no-unwrap",
            r"\.unwrap\(\)",
            "Use proper error handling",
            InjectionMode::AppendSystem,
        )];
        let mut engine = TtsrEngine::new(rules);

        let firings = engine.feed("let x = foo.unwrap()");
        assert_eq!(firings.len(), 1);
        assert_eq!(firings[0].rule_name, "no-unwrap");
    }

    #[test]
    fn test_no_match() {
        let rules = vec![make_rule(
            "no-unwrap",
            r"\.unwrap\(\)",
            "Use proper error handling",
            InjectionMode::Log,
        )];
        let mut engine = TtsrEngine::new(rules);

        let firings = engine.feed("let x = foo.context(\"err\")?");
        assert!(firings.is_empty());
    }

    #[test]
    fn test_max_fires() {
        let mut rule = make_rule("test", r"bad", "fix it", InjectionMode::Log);
        rule.max_fires = 2;

        let mut engine = TtsrEngine::new(vec![rule]);

        engine.reset_window();
        assert_eq!(engine.feed("bad").len(), 1);
        engine.reset_window();
        assert_eq!(engine.feed("bad").len(), 1);
        engine.reset_window();
        assert_eq!(engine.feed("bad").len(), 0); // Max reached
    }

    #[test]
    fn test_system_additions() {
        let rules = vec![make_rule(
            "test",
            r"pattern",
            "injected text",
            InjectionMode::AppendSystem,
        )];
        let mut engine = TtsrEngine::new(rules);

        engine.feed("some pattern here");
        let additions = engine.take_system_additions();
        assert_eq!(additions.len(), 1);
        assert_eq!(additions[0], "injected text");

        // Should be cleared
        assert!(engine.take_system_additions().is_empty());
    }

    #[test]
    fn test_empty_engine() {
        let mut engine = TtsrEngine::empty();
        assert!(!engine.is_active());
        assert!(engine.feed("anything").is_empty());
    }

    #[test]
    fn test_window_trim() {
        let rules = vec![make_rule("test", r"needle", "found", InjectionMode::Log)];
        let mut engine = TtsrEngine::new(rules);
        engine.max_window = 100;

        // Fill window past max
        engine.feed(&"x".repeat(200));
        assert!(engine.window.len() <= 100);
    }

    #[test]
    fn test_disabled_rule() {
        let mut rule = make_rule("test", r"pattern", "injection", InjectionMode::Log);
        rule.enabled = false;
        let engine = TtsrEngine::new(vec![rule]);
        assert!(!engine.is_active()); // Disabled rules aren't loaded
    }

    #[test]
    fn test_user_prepends() {
        let rules = vec![make_rule(
            "test",
            r"TODO",
            "Please complete all TODOs",
            InjectionMode::PrependUser,
        )];
        let mut engine = TtsrEngine::new(rules);

        engine.feed("found a TODO item");
        let prepends = engine.take_user_prepends();
        assert_eq!(prepends.len(), 1);
    }

    #[test]
    fn test_load_from_json() {
        let json = r#"[
            {"name": "test", "trigger": "foo", "injection": "bar"}
        ]"#;
        let rules: Vec<TtsrRule> = serde_json::from_str(json).expect("failed to parse ttsr rules from json");
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].name, "test");
    }
}
