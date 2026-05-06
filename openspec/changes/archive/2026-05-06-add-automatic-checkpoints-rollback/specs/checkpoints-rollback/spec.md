## ADDED Requirements

### Requirement: Automatic Pre-Mutation Checkpoints [r[checkpoints.auto-before-mutation]]
The system MUST create or reuse a namespaced working-directory checkpoint before agent-visible tools mutate tracked files when checkpoint policy is enabled.

#### Scenario: Checkpoint before write [r[checkpoints.auto-before-mutation.scenario.checkpoint-before-write]]
- GIVEN checkpoint policy is enabled and a write/edit/patch tool is about to mutate a git checkout
- WHEN the tool is authorized to run
- THEN clankers records a namespaced checkpoint before the mutation occurs

#### Scenario: Checkpoint failure blocks mutation [r[checkpoints.auto-before-mutation.scenario.checkpoint-failure-blocks-mutation]]
- GIVEN a checkpoint cannot be created for a protected mutation
- WHEN the mutation is requested
- THEN clankers blocks the mutation with an actionable checkpoint error unless policy explicitly allows best-effort mode

### Requirement: Rollback Review and Confirmation [r[checkpoints.rollback-ux]]
The system MUST require explicit confirmation before applying a checkpoint rollback and MUST expose enough review metadata to choose a rollback target.

#### Scenario: List checkpoints [r[checkpoints.rollback-ux.scenario.list-checkpoints]]
- GIVEN one or more clankers checkpoints exist
- WHEN the user lists checkpoints
- THEN clankers shows checkpoint id, label/session, created time, changed-file count, and safe repo identity

#### Scenario: Rollback confirmed [r[checkpoints.rollback-ux.scenario.rollback-confirmed]]
- GIVEN the user selects a checkpoint and confirms rollback
- WHEN rollback executes
- THEN clankers restores through the git backend and writes a safe rollback receipt
