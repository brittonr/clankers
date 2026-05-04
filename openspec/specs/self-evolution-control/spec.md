# Self-Evolution Control Specification

## Purpose

This specification defines the disabled-by-default self-evolution outer loop that evaluates isolated candidate artifacts through the clankers MCP/session-control substrate and requires explicit human approval before any candidate application or promotion.

## Requirements

### Requirement: Self-Evolution Run Model [r[self-evolution-control.run-model]]
The system MUST model self-evolution as an auditable offline run with explicit target artifacts, baseline metrics, candidate outputs, verification evidence, receipts, and promotion recommendation.

#### Scenario: Run records baseline and candidate [r[self-evolution-control.run-model.scenario.baseline-candidate]]
- GIVEN a self-evolution run targets a skill, prompt, tool description, or code path
- WHEN the run starts
- THEN it MUST record the baseline artifact identity, evaluation command or dataset, candidate output location, run id, optional operator-supplied candidate source, and safe metadata before recommending any promotion

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
Self-evolved candidates MUST require explicit human approval before installation, merge, replacement, or application of an active artifact, and any application path MUST validate the approval receipt before mutation.

#### Scenario: Human approves candidate [r[self-evolution-control.promotion-gate.scenario.approve]]
- GIVEN a self-evolution run recommends a candidate
- WHEN a human approves promotion through the normal confirmation/session path
- THEN clankers MAY apply the candidate using the documented install/merge path and MUST record the approval receipt

#### Scenario: No approval means no promotion [r[self-evolution-control.promotion-gate.scenario.no-approval]]
- GIVEN a self-evolution run completes without explicit human approval
- WHEN the workflow exits
- THEN active artifacts MUST remain unchanged and the run MUST report the candidate location for later review

#### Scenario: Apply requires matching approval [r[self-evolution-control.promotion-gate.scenario.apply-requires-approval]]
- GIVEN a candidate application request references a run receipt and approval receipt
- WHEN the receipts do not match on run id, target path, candidate path, approval state, or approval status
- THEN clankers MUST reject application before writing to the target artifact

### Requirement: Self-Evolution Application Model [r[self-evolution-control.application-model]]
The system MUST model candidate application as an explicit, auditable action that links the run receipt, approval receipt, candidate artifact, target artifact, apply mode, backup plan, verification command, and application outcome.

#### Scenario: Application receipt captures plan and outcome [r[self-evolution-control.application-model.scenario.receipt]]
- GIVEN an approved self-evolution candidate is prepared for application
- WHEN the application command runs in dry-run or live mode
- THEN it MUST produce an application receipt with run id, target path, candidate path, apply mode, pre-apply hash, planned or actual backup path, post-apply hash when available, verification status, and whether bytes were applied

#### Scenario: Application remains explicit [r[self-evolution-control.application-model.scenario.explicit-action]]
- GIVEN a run recommends a candidate and an approval receipt exists
- WHEN no explicit application action is invoked
- THEN active target artifacts MUST remain unchanged

### Requirement: Self-Evolution Application Validation [r[self-evolution-control.application-validation]]
The application path MUST validate the full receipt chain and current target state before any live mutation is attempted.

#### Scenario: Stale target hash is rejected [r[self-evolution-control.application-validation.scenario.stale-target]]
- GIVEN the target artifact has changed since the run receipt recorded its baseline hash
- WHEN candidate application is requested
- THEN clankers MUST reject the request before writing and report a stale-target error with safe hash metadata

#### Scenario: Non-promotable receipt is rejected [r[self-evolution-control.application-validation.scenario.non-promotable]]
- GIVEN the run receipt marks the candidate as not recommended, evaluation failed, unchanged, or not awaiting human approval
- WHEN candidate application is requested
- THEN clankers MUST reject the request before writing and explain that the candidate is not eligible for application

#### Scenario: Unsupported apply mode is rejected [r[self-evolution-control.application-validation.scenario.unsupported-mode]]
- GIVEN the application request names an apply mode other than the implemented first-pass local file replacement mode
- WHEN validation runs
- THEN clankers MUST return an actionable unsupported-mode error without touching the target or candidate

### Requirement: Self-Evolution Application Execution [r[self-evolution-control.application-execution]]
The first implementation MUST support only local file replacement from an isolated candidate and MUST create rollback evidence before mutating the target.

#### Scenario: Dry-run preflight does not mutate [r[self-evolution-control.application-execution.scenario.dry-run]]
- GIVEN a valid run receipt, approval receipt, candidate file, and unchanged target file
- WHEN the user runs application with dry-run enabled
- THEN clankers MUST validate the request and report the planned backup, target hash transition, and verification command without modifying the target or creating a live backup

#### Scenario: Live replace-file creates backup and receipt [r[self-evolution-control.application-execution.scenario.replace-file]]
- GIVEN a valid approved candidate and unchanged target file
- WHEN the user explicitly runs live `replace-file` application
- THEN clankers MUST create a backup of the target, copy candidate bytes to the target, compute post-apply hash, run or record verification, and write `application.json`

#### Scenario: Verification failure is visible [r[self-evolution-control.application-execution.scenario.verification-failed]]
- GIVEN live application writes candidate bytes to the target but the configured verification command fails
- WHEN the application receipt is written
- THEN it MUST mark the status as verification failed, preserve backup metadata, and include rollback instructions rather than reporting success

### Requirement: Self-Evolution Application CLI [r[self-evolution-control.application-cli]]
The system MUST expose application through an explicit CLI action with required receipt, approval, mode, and verification arguments.

#### Scenario: Apply command parses required inputs [r[self-evolution-control.application-cli.scenario.parse]]
- GIVEN a user invokes `clankers self-evolution apply` with receipt path, approval path, apply mode, verification command, and dry-run or live flags
- WHEN CLI parsing succeeds
- THEN clankers MUST construct an application request without inferring hidden receipts or sessions

#### Scenario: Live apply is opt-in [r[self-evolution-control.application-cli.scenario.live-opt-in]]
- GIVEN a user invokes the apply command without an explicit live-apply flag or equivalent confirmation
- WHEN the command would mutate a target
- THEN clankers MUST default to dry-run or reject the request rather than writing by surprise

### Requirement: Self-Evolution Documentation [r[self-evolution-control.documentation]]
The implementation MUST document how to run self-evolution safely, what artifacts are generated, how metrics are interpreted, and how promotion is gated.

#### Scenario: Docs explain safe workflow [r[self-evolution-control.documentation.scenario.safe-workflow]]
- GIVEN a user reads the self-evolution documentation
- WHEN they configure a run
- THEN the docs MUST explain offline experimentation, isolated candidate writes, baseline-vs-candidate evidence, receipt review, and human approval before adoption

### Requirement: Self-Evolution Application Documentation [r[self-evolution-control.application-documentation]]
The implementation MUST document the run-to-approval-to-application workflow, receipt review checklist, dry-run preflight, backup location, rollback instructions, and first-pass local-file limitation.

#### Scenario: Docs explain apply safety workflow [r[self-evolution-control.application-documentation.scenario.workflow]]
- GIVEN a user reads the self-evolution documentation
- WHEN they prepare to apply a candidate
- THEN the docs MUST explain how to review `receipt.json`, `approval.json`, and `application.json`, how dry-run differs from live apply, where backups are written, and how to restore the prior target bytes
