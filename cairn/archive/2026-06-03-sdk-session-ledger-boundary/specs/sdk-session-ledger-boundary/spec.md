## ADDED Requirements

### Requirement: Session persistence DTO owners are inventoried [r[sdk-session-ledger-boundary.inventory]]

Desktop session setup, restore, merge, replay, controller persistence, runtime ledger, and SDK examples MUST be inventoried by DTO owner and storage boundary.

#### Scenario: inventory separates desktop and SDK storage [r[sdk-session-ledger-boundary.inventory.dto-owners]]
- GIVEN a session path stores, restores, replays, searches, or merges conversation history
- WHEN architecture inventory runs
- THEN it MUST classify the path as desktop compatibility storage, neutral ledger boundary, engine-message conversion, or display replay projection
- AND each `AgentMessage`/`clankers-session` use MUST name an adapter owner or migration target

### Requirement: SDK storage uses neutral ledger boundaries [r[sdk-session-ledger-boundary.ledger-boundary]]

Generic SDK session/resume behavior MUST use host-owned ledger/session-store DTOs or engine messages, not Clankers desktop session stores, DB/search indexes, JSONL/automerge details, message IDs, or session directories.

#### Scenario: selected path moves behind ledger DTOs [r[sdk-session-ledger-boundary.ledger-boundary.selected-path]]
- GIVEN a restore or resume path is selected for migration
- WHEN reusable behavior reconstructs model context
- THEN it MUST consume neutral ledger/session-store DTOs or engine messages
- AND desktop transcript conversion MUST occur at a compatibility adapter edge

#### Scenario: embedders own stores [r[sdk-session-ledger-boundary.ledger-boundary.sdk-owned-store]]
- GIVEN an embedded product persists session history
- WHEN SDK examples and docs describe the storage path
- THEN the product MUST own its storage schema and provide neutral ledger records
- AND it MUST NOT depend on `clankers-session`, Clankers DB, or global session directories

### Requirement: Desktop session compatibility remains adapter-owned [r[sdk-session-ledger-boundary.desktop-compat]]

Existing Clankers session formats MUST remain readable through desktop adapters while reusable SDK paths see stable neutral DTOs.

#### Scenario: compatibility adapter owns AgentMessage [r[sdk-session-ledger-boundary.desktop-compat.adapter-owned]]
- GIVEN `.jsonl`, automerge, branch, compaction, or display replay paths still use `AgentMessage`
- WHEN they feed reusable runtime or controller behavior
- THEN adapter code MUST convert to neutral ledger, engine, or semantic DTOs first
- AND remaining direct use MUST carry an owner receipt

### Requirement: Session ledger boundary is verified [r[sdk-session-ledger-boundary.verification]]

Verification MUST prove neutral resume fixtures, missing-session fail-closed behavior, and unchanged desktop replay semantics.

#### Scenario: resume fixtures cover restored context [r[sdk-session-ledger-boundary.verification.resume-fixtures]]
- GIVEN session-resume fixtures run
- WHEN user, assistant, and tool history is restored
- THEN the follow-up model request MUST preserve role/text order and fail closed for missing required sessions

#### Scenario: desktop replay parity is preserved [r[sdk-session-ledger-boundary.verification.desktop-parity]]
- GIVEN standalone restore or daemon attach replay displays persisted history
- WHEN replay converts desktop session records
- THEN timestamps, finalized hashes, tool results, branch/compaction context, and semantic events MUST match the existing behavior contract
