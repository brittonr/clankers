## Phase 1: Implementation

- [x] [serial] I1: Produce an agent dependency budget inventory for provider, DB/search, config, procmon, TUI/protocol, and router edges. r[agent-concrete-dependency-drain.dependency-budget] [covers=agent-concrete-dependency-drain.dependency-budget]
- [x] [serial] I2: Select one concrete dependency family and define the neutral port/DTO owner plus app-edge adapter. r[agent-concrete-dependency-drain.neutral-ports] [covers=agent-concrete-dependency-drain.neutral-ports]
- [x] [serial] I3: Refactor the selected agent turn/compaction/tool path to use the neutral port instead of direct concrete imports. r[agent-concrete-dependency-drain.neutral-ports] [covers=agent-concrete-dependency-drain.neutral-ports]
- [x] [serial] I4: Update dependency rails and owner receipts to show the reduced budget and remaining convergence conditions. r[agent-concrete-dependency-drain.dependency-budget] [covers=agent-concrete-dependency-drain.dependency-budget]

## Phase 2: Verification

- [x] [serial] V1: Run focused agent tests for the moved seam and verify no user-visible turn behavior changed. r[agent-concrete-dependency-drain.verification] [covers=agent-concrete-dependency-drain.verification] [evidence=evidence/steel-tool-substrate-validation.md]
- [x] [serial] V2: Run agent dependency rails, `cargo check -p clankers-agent --tests`, Cairn gates/validate, and `git diff --check`. r[agent-concrete-dependency-drain.verification] [covers=agent-concrete-dependency-drain.verification] [evidence=evidence/dependency-budget.md]
