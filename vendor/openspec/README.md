# openspec

[![CI](https://github.com/brittonr/openspec/actions/workflows/ci.yml/badge.svg)](https://github.com/brittonr/openspec/actions/workflows/ci.yml)

Spec-driven development engine. Parse markdown specs into structured
requirements, manage change lifecycles with artifact dependency graphs,
and verify implementation progress.

Extracted from [clankers](https://github.com/brittonr/clankers).

## Usage

```toml
[dependencies]
openspec = { git = "https://github.com/brittonr/openspec" }
```

### Filesystem API (default)

```rust
use openspec::SpecEngine;

let engine = SpecEngine::new(project_root);
engine.init()?;                        // create openspec/ directory
let specs = engine.discover_specs();   // scan specs/*.md
let changes = engine.discover_changes();
let change = engine.create_change("my-feature", None)?;
let report = engine.verify_change("my-feature");
let context = engine.specs_for_context(); // for LLM system prompts
```

### Pure core API (no filesystem, wasm-compatible)

```rust
use openspec::core::spec::parse_spec_content;
use openspec::core::change::parse_task_progress_content;
use openspec::core::verify::verify_from_content;
use openspec::core::artifact::ArtifactGraph;
use openspec::core::schema::builtin_spec_driven;

// Parse spec markdown
let spec = parse_spec_content(markdown, "auth").unwrap();
for req in &spec.requirements {
    println!("{} [{:?}]", req.heading, req.strength);
}

// Parse task progress from string
let progress = parse_task_progress_content(tasks_md).unwrap();
println!("{}/{} done", progress.done, progress.total);

// Verify a change
let report = verify_from_content(Some(tasks_md), true);
println!("{}", report.summary());

// Artifact graph from state (pure, no filesystem)
let schema = builtin_spec_driven();
let existing = ["proposal.md".to_string()].into_iter().collect();
let graph = ArtifactGraph::from_state(&schema.artifacts, &existing);
if let Some(next) = graph.next_ready() {
    println!("Next: {}", next.id);
}
```

### Disable filesystem features (for wasm32)

```toml
[dependencies]
openspec = { git = "https://github.com/brittonr/openspec", default-features = false }
```

## WASM Plugin

The `openspec-plugin/` directory contains a clankers WASM plugin that
exposes spec operations as LLM-callable tools:

- `spec_list` — list specs with requirement counts
- `spec_parse` — parse markdown into structured requirements
- `change_list` — list changes with task progress
- `change_verify` — check task completion and spec coverage
- `artifact_status` — show artifact dependency graph state

Build:
```sh
cd openspec-plugin
cargo build --target wasm32-unknown-unknown --release
```

## Spec format

OpenSpec uses markdown with RFC 2119 keywords and GIVEN/WHEN/THEN scenarios:

```markdown
## Purpose

What this spec defines.

## Requirements

### Feature Name

The system MUST support this feature.

GIVEN a precondition
WHEN an action happens
THEN the expected outcome occurs
```

## License

AGPL-3.0-or-later
