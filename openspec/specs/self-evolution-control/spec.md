# Self-Evolution Control Specification

## Purpose

This specification defines the disabled-by-default self-evolution outer loop that evaluates isolated candidate artifacts through the clankers MCP/session-control substrate and requires explicit human approval before promotion.

## Requirements
### Requirement: Self-Evolution Run Model [r[self-evolution-control.run-model]]
The system MUST model self-evolution as an auditable offline run with explicit target artifacts, baseline metrics, candidate outputs, verification evidence, receipts, and promotion recommendation.

#### Scenario: Run records baseline and candidate [r[self-evolution-control.run-model.scenario.baseline-candidate]]
- GIVEN a self-evolution run targets a skill, prompt, tool description, or code path
- WHEN the run starts
- THEN it MUST record the baseline artifact identity, evaluation command or dataset, candidate output location, run id, and safe metadata before recommending any promotion

#### Scenario: Unchanged candidate is not promoted on noisy score [r[self-evolution-control.run-model.scenario.unchanged-noise]]
- GIVEN a candidate artifact is byte-for-byte or semantically unchanged from the baseline
- WHEN evaluation reports a positive score delta
- THEN the run MUST classify the delta as likely evaluation noise and MUST NOT recommend automatic promotion

### Requirement: Self-Evolution via MCP Session Control [r[self-evolution-control.mcp-orchestration]]
The self-evolution workflow MUST drive clankers through the MCP session-control bridge or the same session-command substrate, rather than using privileged in-process mutation paths.

#### Scenario: Self-evolver submits prompts through MCP [r[self-evolution-control.mcp-orchestration.scenario.prompt-through-mcp]]
- GIVEN a self-evolution run needs clankers to analyze, edit, test, or review a candidate
- WHEN it dispatches work to an agent session
- THEN it MUST use the MCP session-control bridge or equivalent `SessionCommand` client path so attached users can observe and interrupt the work

#### Scenario: Self-evolver observes normal events [r[self-evolution-control.mcp-orchestration.scenario.observe-events]]
- GIVEN a self-evolution run is active
- WHEN clankers emits tool, confirmation, cost, error, or completion events
- THEN the self-evolver MUST consume those events through the same event stream available to user clients and record safe receipts

### Requirement: Self-Evolution Isolation [r[self-evolution-control.isolation]]
Self-evolution MUST write candidate artifacts to isolated output directories, branches, or worktrees and MUST NOT mutate installed active skills, prompts, tools, or production code in-place during an experiment.

#### Scenario: Candidate writes are isolated [r[self-evolution-control.isolation.scenario.candidate-output]]
- GIVEN an optimizer proposes a changed skill, prompt, tool description, or code patch
- WHEN the workflow materializes that candidate
- THEN it MUST write the candidate to a run-scoped output path, branch, or worktree distinct from the active installed artifact

#### Scenario: Live in-place mutation is rejected [r[self-evolution-control.isolation.scenario.reject-live-mutation]]
- GIVEN a self-evolution run attempts to overwrite an installed skill, active prompt, active tool description, or current production file without an isolated candidate boundary
- WHEN policy validation runs
- THEN clankers MUST reject the operation and record an actionable error receipt

### Requirement: Self-Evolution Verification and Scoring [r[self-evolution-control.verification]]
Self-evolution MUST compare baseline and candidate behavior using deterministic checks or declared evaluation datasets before recommending adoption.

#### Scenario: Candidate requires verification evidence [r[self-evolution-control.verification.scenario.requires-evidence]]
- GIVEN a candidate artifact has been generated
- WHEN the run prepares a recommendation
- THEN it MUST include baseline score, candidate score, changed-artifact evidence, verification command outcomes, and known limitations

#### Scenario: Failed eval prevents promotion [r[self-evolution-control.verification.scenario.failed-eval]]
- GIVEN evaluation, tests, constraint checks, or sandbox checks fail
- WHEN the run produces its final receipt
- THEN it MUST mark the candidate as not recommended and include sanitized failure evidence

### Requirement: Self-Evolution Human Promotion Gate [r[self-evolution-control.promotion-gate]]
Self-evolved candidates MUST require explicit human approval before installation, merge, or replacement of an active artifact.

#### Scenario: Human approves candidate [r[self-evolution-control.promotion-gate.scenario.approve]]
- GIVEN a self-evolution run recommends a candidate
- WHEN a human approves promotion through the normal confirmation/session path
- THEN clankers MAY apply the candidate using the documented install/merge path and MUST record the approval receipt

#### Scenario: No approval means no promotion [r[self-evolution-control.promotion-gate.scenario.no-approval]]
- GIVEN a self-evolution run completes without explicit human approval
- WHEN the workflow exits
- THEN active artifacts MUST remain unchanged and the run MUST report the candidate location for later review

### Requirement: Self-Evolution Documentation [r[self-evolution-control.documentation]]
The implementation MUST document how to run self-evolution safely, what artifacts are generated, how metrics are interpreted, and how promotion is gated.

#### Scenario: Docs explain safe workflow [r[self-evolution-control.documentation.scenario.safe-workflow]]
- GIVEN a user reads the self-evolution documentation
- WHEN they configure a run
- THEN the docs MUST explain offline experimentation, isolated candidate writes, baseline-vs-candidate evidence, receipt review, and human approval before adoption
