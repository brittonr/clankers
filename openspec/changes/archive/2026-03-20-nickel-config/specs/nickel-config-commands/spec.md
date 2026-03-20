## ADDED Requirements

### Requirement: config init generates starter Nickel file
`clankers config init` SHALL generate a `settings.ncl` file from the embedded contract. The file SHALL include comments explaining each field and its default value.

#### Scenario: Init global config
- **WHEN** user runs `clankers config init --global`
- **THEN** a `settings.ncl` file is written to `~/.clankers/agent/settings.ncl` with the contract import and commented field descriptions

#### Scenario: Init project config
- **WHEN** user runs `clankers config init` without `--global` in a directory with `.clankers/`
- **THEN** a `settings.ncl` file is written to `.clankers/settings.ncl`

#### Scenario: File already exists
- **WHEN** `settings.ncl` already exists at the target location
- **THEN** the command exits with an error message and does not overwrite the file

### Requirement: config check validates without starting a session
`clankers config check` SHALL evaluate the full config merge (all layers) and report any errors. On success, it prints a confirmation. On failure, it prints the Nickel diagnostic or serde deserialization error.

#### Scenario: Valid config
- **WHEN** user runs `clankers config check` with valid config files
- **THEN** the command prints "Config OK" and exits with code 0

#### Scenario: Nickel contract violation
- **WHEN** user runs `clankers config check` and a `.ncl` file has a type error
- **THEN** the command prints the Nickel diagnostic (file path, position, expected vs actual type) and exits with code 1

#### Scenario: JSON parse error
- **WHEN** user runs `clankers config check` and a `.json` file has invalid JSON
- **THEN** the command prints the parse error with file path and exits with code 1

#### Scenario: Mixed layers
- **WHEN** user runs `clankers config check` with global `.ncl` and project `.json`
- **THEN** both layers are evaluated/parsed and merged, and the merged result is validated

### Requirement: config export prints resolved JSON
`clankers config export` SHALL evaluate and merge all config layers, then print the resulting `Settings` as formatted JSON to stdout.

#### Scenario: Export merged config
- **WHEN** user runs `clankers config export`
- **THEN** stdout receives the fully merged and resolved settings as pretty-printed JSON

#### Scenario: Export with specific layer
- **WHEN** user runs `clankers config export --global`
- **THEN** stdout receives only the global layer's resolved settings (no project merge)
