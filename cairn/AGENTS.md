# Cairn Agent Notes

## Human/oracle escalation checkpoints

Review findings routed to `human` are escalation signals, not normal prose nits. Do not clear them by rewording only.

- Repeated `omission` findings in `scope=review` mean the reviewer could not verify the evidence packet. Before rerunning a gate, supply the complete artifact or an untruncated excerpt and add an explicit `H#` task with checked-in `oracle-checkpoint` evidence when human judgment is needed.
- When proposal, design, or specs require deterministic contracts, add contract-specific tasks that name a concrete fixture/helper/command, include `[covers=...]`, and point `V#` tasks at `[evidence=...]`. Generic "test it" tasks are insufficient for request shape, stream boundaries, retry policy, security/redaction policy, receipt fields, or discovery visibility obligations.
- The project-local drift rail is `scripts/check-cairn-review-gates.rs`. It keeps sanitized fixtures, diagnostics, and guidance aligned; update it together with Cairn authoring guidance when adding new repeated omission classes.
- Repeated `preference` findings in `scope=tasks` mean the task ledger is making a claim that is not traceable to the proposal, design, or specs. Remove the claim, add the missing requirement/design decision, or add an explicit `H#` task documenting the accepted human/oracle decision.
- If a `design.md` decision is the oracle, label it with a `## Decision` heading and reference it from the `H#` task. The task still needs `[covers=...]` and `[evidence=...]` metadata.
- `H#` evidence must live under the change's `evidence/` directory and include the standard metadata: `Artifact-Type: oracle-checkpoint`, `Task-ID`, `Covers`, `Reviewed-Evidence`, `Decision`, and `Follow-Up`; include reviewer/oracle identity when available.
- Put compact verification summaries near the top of long design artifacts so truncation cannot hide retry contracts, fixture coverage, or finish-line checks from stage review.
