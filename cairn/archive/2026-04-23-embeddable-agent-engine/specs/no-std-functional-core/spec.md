## MODIFIED Requirements

### Requirement: Future deterministic extractions MUST reuse the same boundary

Any later orchestration logic moved under this capability MUST enter `clankers-core` as explicit state, input, and effect transformations when it does not require direct I/O. Shell-specific protocol, runtime, and transport types MUST stay in adapter code, and reusable host-facing harness semantics MUST be staged through `clankers-engine` rather than being left controller-specific.

#### Scenario: future pure logic moves into the core

- **WHEN** a later deterministic orchestration rule is migrated under the `no-std-functional-core` capability
- **THEN** that rule is implemented in `clankers-core`
- **THEN** shell adapters are limited to translation and effect execution

#### Scenario: shell-native types stay outside the core boundary

- **WHEN** the migrated slice needs `DaemonEvent`, `AgentEvent`, Tokio, or transport-specific values
- **THEN** those shell-native values are created and consumed in adapter code
- **THEN** raw shell-native or protocol-native types do not appear in exported `clankers-core` boundary types including state, input, effect, outcome, or error types

#### Scenario: reusable engine boundary stages future extractions

- **WHEN** Clankers migrates another reusable orchestration slice after the initial prompt-lifecycle extraction
- **THEN** the first host-facing landing zone for that reusable logic is `clankers-engine`
- **THEN** controller and agent shells adapt the engine boundary instead of keeping controller-only reusable policy

#### Scenario: turn orchestration extraction targets the embeddable engine path

- **WHEN** Clankers migrates prompt, model, tool, retry, or continuation policy that belongs in an embedded agent harness
- **THEN** that migration is planned as `clankers-agent` and `clankers-controller` shell work around an engine-owned contract
- **THEN** the deterministic portions remain eligible for later downward movement into `clankers-core`
