## ADDED Requirements

### Requirement: Shell adapter parity matrix coverage [r[embeddable-agent-engine.shell-adapter-parity-matrix]]
The system MUST verify Clankers shell adapters with a bounded matrix of prompt, store, confirmation, tool, model, and event-translation features while preserving engine-owned reusable turn policy.

#### Scenario: shared fixtures run across supported shell seams [r[embeddable-agent-engine.shell-adapter-parity-matrix.shared-fixtures]]
- GIVEN a matrix case with a recorded prompt, tool response, model response, and expected engine-adapter outcome
- WHEN the case runs through supported standalone agent, controller/daemon adapter, and bounded embedded or batch seams
- THEN each shell produces equivalent engine inputs, interpreted effects, terminal outcomes, and user-visible semantic events after shell-specific translation

#### Scenario: host-owned services stay outside engine policy [r[embeddable-agent-engine.shell-adapter-parity-matrix.host-owned-services]]
- GIVEN a matrix case varies prompt source, store mode, confirmation response, disabled-tool policy, tool result class, and model result class
- WHEN shell adapters execute the case
- THEN prompt assembly, store lookup, confirmation decisions, and shell event translation remain adapter-owned
- THEN model/tool continuation policy remains engine-owned and is not duplicated by the shell

#### Scenario: source-boundary rails require behavioral evidence [r[embeddable-agent-engine.shell-adapter-parity-matrix.fcis-evidence]]
- GIVEN FCIS/source-boundary checks pass syntactically
- WHEN decoupling acceptance claims shell adapter parity
- THEN at least one matrix evidence report names the shell seams and feature axes exercised
- THEN failure to execute required shell matrix cases blocks acceptance
