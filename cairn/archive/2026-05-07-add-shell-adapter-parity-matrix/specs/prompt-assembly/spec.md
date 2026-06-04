## ADDED Requirements

### Requirement: Prompt assembly matrix participation [r[prompt-assembly.shell-adapter-parity-matrix]]
The system MUST verify prompt assembly as a host-owned input to shell adapter parity rather than as a hidden dependency of the reusable engine.

#### Scenario: prompt source varies without engine dependency [r[prompt-assembly.shell-adapter-parity-matrix.prompt-source]]
- GIVEN shell parity matrix cases include empty, static, project-context, OpenSpec-context, and host-supplied prompt sources where supported
- WHEN the shell adapter submits accepted work to the engine
- THEN the engine receives already-prepared prompt data
- THEN the engine does not read project files, skills, OpenSpec context, or prompt templates directly
