## MODIFIED Requirements

### Requirement: Process-job policy splits from the root tool [r[remaining-coupling-drain.process-job-policy]]

The agent-visible `process` tool MUST stay a thin JSON-to-typed-request adapter over process-job services. Native process management, backend capability rules, durable storage mapping, redaction, notification policy, and retention/GC MUST be owned by runtime/process service modules or backend adapters.

#### Scenario: process contracts move to a neutral owner [r[remaining-coupling-drain.process-job-policy.neutral-contract-owner]]
- GIVEN process-job admission, profile, redaction, notification, retention, and receipt DTOs are shared by root tools and backend adapters
- WHEN a process-job drain slice changes those contracts
- THEN the reusable contract MUST live in a green neutral owner with no root shell, daemon, TUI, procmon, pueue, systemd, or global path dependencies
- AND root process modules MUST only parse agent input, call typed services, and project typed receipts
