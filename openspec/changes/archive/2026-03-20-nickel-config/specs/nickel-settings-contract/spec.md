## ADDED Requirements

### Requirement: Embedded contract covers full Settings schema
The binary SHALL embed a Nickel contract (`settings-contract.ncl`) via `include_str!` that declares every field in the `Settings` struct with its Nickel type, default value, and (where applicable) enum constraints.

#### Scenario: Contract matches Settings::default()
- **WHEN** the embedded contract is evaluated with no user overrides (empty merge)
- **THEN** the resulting JSON deserializes to a `Settings` value equal to `Settings::default()`

#### Scenario: New Settings field without contract entry
- **WHEN** a developer adds a field to `Settings` with a default value but does not add a corresponding entry to the contract
- **THEN** the contract-default sync test fails

### Requirement: Contract validates field types
The contract SHALL use Nickel type annotations (`| String`, `| Number`, `| Bool`, `| Array String`, etc.) so that type errors in user config are caught at evaluation time with a Nickel diagnostic.

#### Scenario: Wrong type for string field
- **WHEN** a user config sets `model = 42` (number instead of string)
- **THEN** the Nickel evaluator reports a contract violation naming the `model` field and expected type `String`

#### Scenario: Wrong type in nested object
- **WHEN** a user config sets `hooks.scriptTimeoutSecs = "ten"` (string instead of number)
- **THEN** the Nickel evaluator reports a contract violation naming the field path and expected type `Number`

### Requirement: Contract provides default values via Nickel annotations
Each field in the contract SHALL use `| default = <value>` so that users only need to specify the fields they want to override. Omitted fields get the contract default.

#### Scenario: Minimal user config
- **WHEN** a user config is `(import "clankers://settings") & { model = "claude-opus-4-6" }`
- **THEN** all other fields resolve to their contract defaults, and `model` resolves to `"claude-opus-4-6"`

#### Scenario: User specifies only nested fields
- **WHEN** a user config is `(import "clankers://settings") & { hooks.disabledHooks = ["pre-tool"] }`
- **THEN** `hooks.enabled`, `hooks.scriptTimeoutSecs`, and other hook fields resolve to their contract defaults

### Requirement: Custom import resolution for contract
The Nickel evaluator SHALL resolve `import "clankers://settings"` to the embedded contract content. This avoids users needing to know the filesystem path of the contract.

#### Scenario: User imports contract by pseudo-URL
- **WHEN** a `.ncl` file contains `import "clankers://settings"`
- **THEN** the evaluator resolves this to the embedded contract and evaluation succeeds

#### Scenario: User writes plain record without import
- **WHEN** a `.ncl` file contains a plain record `{ model = "opus" }` without importing the contract
- **THEN** evaluation succeeds (the contract is not mandatory — it's a convenience for validation and defaults)
