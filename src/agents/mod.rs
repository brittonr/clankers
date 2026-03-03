//! Agent definition system (first-class)

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
        names.sort();
        names
    }

    pub fn len(&self) -> usize {
        self.agents.len()
    }
    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
    }
}
