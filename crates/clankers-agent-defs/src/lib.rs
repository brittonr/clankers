//! Agent definition system (first-class)

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

pub mod definition;
pub mod discovery;
pub mod identity;
pub mod security;

use std::collections::HashMap;

use definition::AgentConfig;

/// Registry of discovered agent definitions
#[derive(Debug, Default)]
pub struct AgentRegistry {
    agents: HashMap<String, AgentConfig>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, config: AgentConfig) {
        self.agents.insert(config.name.clone(), config);
    }

    pub fn get(&self, name: &str) -> Option<&AgentConfig> {
        self.agents.get(name)
    }

    pub fn list(&self) -> Vec<&AgentConfig> {
        let mut agents: Vec<_> = self.agents.values().collect();
        agents.sort_by_key(|a| &a.name);
        agents
    }

    pub fn names(&self) -> Vec<&str> {
        let mut names: Vec<_> = self.agents.keys().map(|s| s.as_str()).collect();
        names.sort_unstable();
        names
    }

    pub fn len(&self) -> usize {
        self.agents.len()
    }
    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
    }
}
