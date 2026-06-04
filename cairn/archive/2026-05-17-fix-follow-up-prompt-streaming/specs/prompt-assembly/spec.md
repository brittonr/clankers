## MODIFIED Requirements

### Requirement: Prompt assembly service [r[prompt-assembly.service]]

The system MUST provide a reusable prompt assembly service that builds system/user prompt context from explicit host policy, host-provided context, and optionally enabled Clankers discovery sources without depending on CLI, TUI, daemon, or provider request types. Prompt assembly MUST be repeatable for multiple prompts in one session and MUST return prompt data to the caller without mutating session busy state, follow-up state, or model-turn completion state.

#### Scenario: repeated prompt assembly does not suppress execution [r[prompt-assembly.service.repeated-prompt-no-suppression]]

- GIVEN a session has already assembled, dispatched, streamed, and completed one prompt
- WHEN a later prompt is assembled in the same session
- THEN prompt assembly MUST return the assembled prompt and safe provenance for that later prompt
- THEN it MUST NOT decide that the later prompt is already completed because prior prompt metadata exists
- THEN shell/controller code MUST still submit the later assembled prompt through the normal accepted-prompt lifecycle
