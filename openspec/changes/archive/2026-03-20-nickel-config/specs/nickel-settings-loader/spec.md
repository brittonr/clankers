## ADDED Requirements

### Requirement: Nickel file detection and preference
The settings loader SHALL check for `settings.ncl` alongside `settings.json` at each config layer (pi fallback, global, project). When both exist, the `.ncl` file SHALL take precedence.

#### Scenario: Only JSON exists
- **WHEN** a config layer has `settings.json` but no `settings.ncl`
- **THEN** the loader reads and parses `settings.json` as before

#### Scenario: Only Nickel exists
- **WHEN** a config layer has `settings.ncl` but no `settings.json`
- **THEN** the loader evaluates the `.ncl` file through the Nickel evaluator and produces a `serde_json::Value`

#### Scenario: Both formats exist
- **WHEN** a config layer has both `settings.ncl` and `settings.json`
- **THEN** the loader uses `settings.ncl` and ignores `settings.json`

#### Scenario: Neither exists
- **WHEN** a config layer has neither `settings.ncl` nor `settings.json`
- **THEN** the loader returns `None` for that layer (existing behavior)

### Requirement: Nickel evaluation produces JSON-compatible output
The Nickel evaluator SHALL use `Context::eval_deep_for_export()` followed by `Context::expr_to_json()` to produce a JSON string. The result SHALL be parsed into `serde_json::Value` and fed into the existing `Settings` deserialization pipeline.

#### Scenario: Valid Nickel config
- **WHEN** a `.ncl` file contains valid Nickel that evaluates to a record
- **THEN** the output is a `serde_json::Value::Object` compatible with `Settings` deserialization

#### Scenario: Nickel eval error
- **WHEN** a `.ncl` file contains a Nickel evaluation error (type error, contract violation, syntax error)
- **THEN** the loader SHALL report the Nickel diagnostic message including source file path and position, and fall back to `Settings::default()`

### Requirement: Mixed-format layer merge
The 3-layer merge SHALL work when layers use different formats. Each layer independently resolves to `Option<serde_json::Value>` regardless of source format, then merges proceed as before.

#### Scenario: Global is Nickel, project is JSON
- **WHEN** global config is `settings.ncl` and project config is `settings.json`
- **THEN** both resolve to `serde_json::Value` and merge correctly with project values overriding global values

#### Scenario: Pi fallback is JSON, global is Nickel
- **WHEN** pi fallback is `settings.json` and global is `settings.ncl`
- **THEN** both resolve to `serde_json::Value` and merge correctly with global values overriding pi values

### Requirement: Deep recursive merge for JSON layers
The `merge_into()` function SHALL recursively merge nested objects instead of replacing them at the top level. When both target and source have an object at the same key, the source object's fields SHALL be merged into the target object recursively.

#### Scenario: Nested object partial override
- **WHEN** global config has `{"hooks": {"enabled": true, "scriptTimeoutSecs": 10}}` and project config has `{"hooks": {"disabledHooks": ["pre-tool"]}}`
- **THEN** the merged result has `hooks.enabled = true`, `hooks.scriptTimeoutSecs = 10`, and `hooks.disabledHooks = ["pre-tool"]`

#### Scenario: Scalar field override within nested object
- **WHEN** global config has `{"memory": {"globalCharLimit": 2200}}` and project config has `{"memory": {"globalCharLimit": 4400}}`
- **THEN** the merged result has `memory.globalCharLimit = 4400`

#### Scenario: Array fields are replaced not merged
- **WHEN** global config has `{"disabledTools": ["bash"]}` and project config has `{"disabledTools": ["commit"]}`
- **THEN** the merged result has `disabledTools = ["commit"]` (project replaces global, arrays are not concatenated)

### Requirement: Feature-gated Nickel dependency
The Nickel evaluator SHALL be gated behind a `nickel` cargo feature in `clankers-config`. When the feature is disabled, `.ncl` files SHALL be ignored and only `.json` loading is available.

#### Scenario: Feature enabled
- **WHEN** `clankers-config` is compiled with `features = ["nickel"]`
- **THEN** `.ncl` file detection and evaluation are active

#### Scenario: Feature disabled
- **WHEN** `clankers-config` is compiled without the `nickel` feature
- **THEN** the loader only checks for `.json` files, `.ncl` files are not detected or evaluated
