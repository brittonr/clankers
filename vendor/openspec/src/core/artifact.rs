//! Artifact dependency graph (DAG)

use std::collections::HashMap;
use std::collections::HashSet;

use petgraph::algo::toposort;
use petgraph::graph::DiGraph;
use petgraph::graph::NodeIndex;
use serde::Deserialize;
use serde::Serialize;

use super::schema::SchemaArtifact;

/// State of an artifact
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArtifactState {
    Blocked, // dependencies not met
    Ready,   // all dependencies done, can create
    Done,    // artifact file exists
}

/// An artifact in the dependency graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub id: String,
    pub generates: String,     // file glob pattern
    pub requires: Vec<String>, // dependency artifact IDs
    pub state: ArtifactState,
}

/// The full artifact dependency graph for a change
#[derive(Debug)]
pub struct ArtifactGraph {
    pub artifacts: Vec<Artifact>,
    graph: DiGraph<String, ()>,
    _node_map: HashMap<String, NodeIndex>,
}

impl ArtifactGraph {
    /// Build from schema artifacts and existing files (pure version)
    pub fn from_state(schema_artifacts: &[SchemaArtifact], existing_files: &HashSet<String>) -> Self {
        let mut graph = DiGraph::new();
        let mut node_map = HashMap::new();

        // Add nodes
        for art in schema_artifacts {
            let idx = graph.add_node(art.id.clone());
            node_map.insert(art.id.clone(), idx);
        }

        // Add edges (dependency -> artifact)
        for art in schema_artifacts {
            if let Some(&to) = node_map.get(&art.id) {
                for dep in &art.requires {
                    if let Some(&from) = node_map.get(dep) {
                        graph.add_edge(from, to, ());
                    }
                }
            }
        }

        // Detect states
        let mut artifacts: Vec<Artifact> = schema_artifacts
            .iter()
            .map(|sa| {
                let exists = check_file_exists_for_artifact(existing_files, &sa.generates);
                Artifact {
                    id: sa.id.clone(),
                    generates: sa.generates.clone(),
                    requires: sa.requires.clone(),
                    state: if exists {
                        ArtifactState::Done
                    } else {
                        ArtifactState::Blocked
                    },
                }
            })
            .collect();

        // Update Ready state: blocked artifacts whose deps are all Done
        loop {
            let mut changed = false;
            for i in 0..artifacts.len() {
                if artifacts[i].state == ArtifactState::Blocked {
                    let all_deps_done = artifacts[i]
                        .requires
                        .iter()
                        .all(|dep| artifacts.iter().any(|a| a.id == *dep && a.state == ArtifactState::Done));
                    if all_deps_done {
                        artifacts[i].state = ArtifactState::Ready;
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }

        Self {
            artifacts,
            graph,
            _node_map: node_map,
        }
    }

    #[cfg(feature = "fs")]
    /// Build from schema artifacts and detect states from filesystem
    pub fn build(schema_artifacts: &[SchemaArtifact], change_dir: &std::path::Path) -> Self {
        let mut graph = DiGraph::new();
        let mut node_map = HashMap::new();

        // Add nodes
        for art in schema_artifacts {
            let idx = graph.add_node(art.id.clone());
            node_map.insert(art.id.clone(), idx);
        }

        // Add edges (dependency -> artifact)
        for art in schema_artifacts {
            if let Some(&to) = node_map.get(&art.id) {
                for dep in &art.requires {
                    if let Some(&from) = node_map.get(dep) {
                        graph.add_edge(from, to, ());
                    }
                }
            }
        }

        // Detect states
        let mut artifacts: Vec<Artifact> = schema_artifacts
            .iter()
            .map(|sa| {
                let exists = file_exists_for_artifact(change_dir, &sa.generates);
                Artifact {
                    id: sa.id.clone(),
                    generates: sa.generates.clone(),
                    requires: sa.requires.clone(),
                    state: if exists {
                        ArtifactState::Done
                    } else {
                        ArtifactState::Blocked
                    },
                }
            })
            .collect();

        // Update Ready state: blocked artifacts whose deps are all Done
        loop {
            let mut changed = false;
            for i in 0..artifacts.len() {
                if artifacts[i].state == ArtifactState::Blocked {
                    let all_deps_done = artifacts[i]
                        .requires
                        .iter()
                        .all(|dep| artifacts.iter().any(|a| a.id == *dep && a.state == ArtifactState::Done));
                    if all_deps_done {
                        artifacts[i].state = ArtifactState::Ready;
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }

        Self {
            artifacts,
            graph,
            _node_map: node_map,
        }
    }

    /// Get the first ready artifact (topological order)
    pub fn next_ready(&self) -> Option<&Artifact> {
        let order = toposort(&self.graph, None).ok()?;
        for idx in order {
            let id = &self.graph[idx];
            if let Some(art) = self.artifacts.iter().find(|a| a.id == *id)
                && art.state == ArtifactState::Ready
            {
                return Some(art);
            }
        }
        None
    }

    /// Get all ready artifacts
    pub fn all_ready(&self) -> Vec<&Artifact> {
        self.artifacts.iter().filter(|a| a.state == ArtifactState::Ready).collect()
    }

    /// Check if all artifacts are done
    pub fn is_complete(&self) -> bool {
        self.artifacts.iter().all(|a| a.state == ArtifactState::Done)
    }
}

/// Check if a file exists for an artifact (pure version using existing files set)
fn check_file_exists_for_artifact(existing_files: &HashSet<String>, generates: &str) -> bool {
    // Simple check: if generates contains *, check if any matching file exists
    // Otherwise check exact path
    if generates.contains('*') {
        // Glob: check if any file matches the pattern (simplified)
        let prefix = generates.split('*').next().unwrap_or("");
        existing_files.iter().any(|f| f.starts_with(prefix))
    } else {
        existing_files.contains(generates)
    }
}

#[cfg(feature = "fs")]
fn file_exists_for_artifact(change_dir: &std::path::Path, generates: &str) -> bool {
    // Simple check: if generates contains *, check if any matching file exists
    // Otherwise check exact path
    if generates.contains('*') {
        // Glob: check if any file matches under the change dir
        let pattern = change_dir.join(generates).to_string_lossy().to_string();
        glob_has_match(&pattern)
    } else {
        change_dir.join(generates).exists()
    }
}

#[cfg(feature = "fs")]
fn glob_has_match(pattern: &str) -> bool {
    // Simple glob: just check if directory exists for ** patterns
    let dir = pattern.split('*').next().unwrap_or("");
    std::path::Path::new(dir).is_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_artifact_from_state() {
        let artifacts = vec![SchemaArtifact {
            id: "proposal".to_string(),
            generates: "proposal.md".to_string(),
            requires: vec![],
        }];

        let mut existing = HashSet::new();
        existing.insert("proposal.md".to_string());

        let graph = ArtifactGraph::from_state(&artifacts, &existing);
        assert_eq!(graph.artifacts[0].state, ArtifactState::Done);
    }

    #[test]
    fn test_artifact_state_ready_when_deps_done_pure() {
        let artifacts = vec![
            SchemaArtifact {
                id: "proposal".to_string(),
                generates: "proposal.md".to_string(),
                requires: vec![],
            },
            SchemaArtifact {
                id: "design".to_string(),
                generates: "design.md".to_string(),
                requires: vec!["proposal".to_string()],
            },
        ];

        let mut existing = HashSet::new();
        existing.insert("proposal.md".to_string());

        let graph = ArtifactGraph::from_state(&artifacts, &existing);
        assert_eq!(graph.artifacts[0].state, ArtifactState::Done);
        assert_eq!(graph.artifacts[1].state, ArtifactState::Ready);
    }

    #[cfg(all(test, feature = "fs"))]
    mod fs_tests {
        use tempfile::TempDir;

        use super::*;

        #[test]
        fn test_artifact_state_done_when_file_exists() {
            let dir = TempDir::new().expect("failed to create temp dir for test");
            std::fs::write(dir.path().join("proposal.md"), "# Proposal").expect("failed to write proposal file");

            let artifacts = vec![SchemaArtifact {
                id: "proposal".to_string(),
                generates: "proposal.md".to_string(),
                requires: vec![],
            }];

            let graph = ArtifactGraph::build(&artifacts, dir.path());
            assert_eq!(graph.artifacts[0].state, ArtifactState::Done);
        }

        #[test]
        fn test_artifact_state_ready_when_deps_done() {
            let dir = TempDir::new().expect("failed to create temp dir for test");
            std::fs::write(dir.path().join("proposal.md"), "# Proposal").expect("failed to write proposal file");

            let artifacts = vec![
                SchemaArtifact {
                    id: "proposal".to_string(),
                    generates: "proposal.md".to_string(),
                    requires: vec![],
                },
                SchemaArtifact {
                    id: "design".to_string(),
                    generates: "design.md".to_string(),
                    requires: vec!["proposal".to_string()],
                },
            ];

            let graph = ArtifactGraph::build(&artifacts, dir.path());
            assert_eq!(graph.artifacts[0].state, ArtifactState::Done);
            assert_eq!(graph.artifacts[1].state, ArtifactState::Ready);
        }

        #[test]
        fn test_artifact_state_blocked_when_deps_not_done() {
            let dir = TempDir::new().expect("failed to create temp dir for test");

            let artifacts = vec![
                SchemaArtifact {
                    id: "proposal".to_string(),
                    generates: "proposal.md".to_string(),
                    requires: vec![],
                },
                SchemaArtifact {
                    id: "design".to_string(),
                    generates: "design.md".to_string(),
                    requires: vec!["proposal".to_string()],
                },
            ];

            let graph = ArtifactGraph::build(&artifacts, dir.path());
            assert_eq!(graph.artifacts[0].state, ArtifactState::Ready);
            assert_eq!(graph.artifacts[1].state, ArtifactState::Blocked);
        }

        #[test]
        fn test_next_ready() {
            let dir = TempDir::new().expect("failed to create temp dir for test");
            std::fs::write(dir.path().join("proposal.md"), "done").expect("failed to write proposal file");

            let artifacts = vec![
                SchemaArtifact {
                    id: "proposal".to_string(),
                    generates: "proposal.md".to_string(),
                    requires: vec![],
                },
                SchemaArtifact {
                    id: "design".to_string(),
                    generates: "design.md".to_string(),
                    requires: vec!["proposal".to_string()],
                },
            ];

            let graph = ArtifactGraph::build(&artifacts, dir.path());
            let next = graph.next_ready();
            assert!(next.is_some());
            assert_eq!(next.expect("next ready artifact should exist").id, "design");
        }

        #[test]
        fn test_is_complete() {
            let dir = TempDir::new().expect("failed to create temp dir for test");
            std::fs::write(dir.path().join("proposal.md"), "done").expect("failed to write proposal file");
            std::fs::write(dir.path().join("design.md"), "done").expect("failed to write design file");

            let artifacts = vec![
                SchemaArtifact {
                    id: "proposal".to_string(),
                    generates: "proposal.md".to_string(),
                    requires: vec![],
                },
                SchemaArtifact {
                    id: "design".to_string(),
                    generates: "design.md".to_string(),
                    requires: vec!["proposal".to_string()],
                },
            ];

            let graph = ArtifactGraph::build(&artifacts, dir.path());
            assert!(graph.is_complete());
        }
    }
}
