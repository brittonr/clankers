# ADDED Requirements

### Requirement: evidence page [r[dogfood-full-readiness-evidence-page.checkpoint-binding]]

The evidence page MUST bind the readiness checkpoint tag to its exact target commit.

#### Scenario: Checkpoint tag target is explicit
- GIVEN the evidence page is reviewed
- WHEN operators inspect the checkpoint
- THEN the page names `internal-readiness-2026-05-26-dogfood-full` and its target commit

### Requirement: evidence page [r[dogfood-full-readiness-evidence-page.harness-index]]

The evidence page MUST index the full harness run and pass/fail counts without embedding raw generated logs.

#### Scenario: Harness receipt is indexed
- GIVEN a full harness receipt exists under target
- WHEN the page is generated or edited
- THEN it records run id, mode, payload commit, passed count, failed count, and local receipt path

### Requirement: evidence page [r[dogfood-full-readiness-evidence-page.dogfood-facts]]

The evidence page MUST record the BG-process TUI dogfood facts that made the full run operator-visible.

#### Scenario: Dogfood receipt facts are indexed
- GIVEN the full harness includes BG-process TUI dogfood
- WHEN operators read the evidence page
- THEN it states active process visibility, command visibility, layout toggle visibility, and cleanup status

### Requirement: evidence page [r[dogfood-full-readiness-evidence-page.scope-boundary]]

The evidence page MUST state the internal-readiness scope boundary.

#### Scenario: Scope boundary avoids overclaim
- GIVEN the checkpoint evidence is published in docs
- WHEN operators evaluate readiness
- THEN the page says the evidence is internal/trusted dogfood and not public unattended production readiness
