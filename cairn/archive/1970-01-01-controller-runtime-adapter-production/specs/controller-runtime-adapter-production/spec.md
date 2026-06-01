## ADDED Requirements

### Requirement: Controller command execution uses runtime adapters [r[controller-runtime-adapter-production.command-path]]

Controller command policy MUST express prompt and control execution through runtime/session adapter interfaces rather than directly mutating concrete `Agent` state inside reusable command branches.

#### Scenario: Prompt path is adapter-backed [r[controller-runtime-adapter-production.command-path.prompt]]
- GIVEN a `SessionCommand::Prompt` reaches the controller
- WHEN command handling constructs authorization, core input, busy state, and prompt execution
- THEN concrete model/agent execution MUST be invoked through a `ControllerRuntimeAdapter` or equivalent injected runtime seam
- AND command policy MUST consume adapter completion and semantic events rather than provider or agent-native stream details

#### Scenario: Control path is adapter-backed [r[controller-runtime-adapter-production.command-path.controls]]
- GIVEN abort/reset, thinking-level, disabled-tool, or other runtime control commands reach the controller
- WHEN command handling applies the requested control
- THEN concrete agent mutations MUST be delegated to a runtime/control adapter unless the state is explicitly controller-owned
- AND the branch MUST still update reducer/controller state through the existing centralized effect interpretation paths

### Requirement: Production and fake runtime share command branches [r[controller-runtime-adapter-production.shared-path]]

Fake runtime fixtures and daemon production mode MUST exercise the same controller command branches for prompt and control lifecycle behavior.

#### Scenario: Fake runtime proves production command lifecycle [r[controller-runtime-adapter-production.shared-path.fake-runtime]]
- GIVEN a fake runtime adapter records prompts and controls
- WHEN controller command fixtures run
- THEN prompt text, image count, session id, model, thinking controls, disabled-tool controls, abort/reset, and completion status MUST be observable through the fake adapter
- AND the fixture MUST not construct providers, sockets, TUI state, desktop storage, or a concrete `Agent`

#### Scenario: Production adapter preserves daemon behavior [r[controller-runtime-adapter-production.shared-path.production-adapter]]
- GIVEN daemon mode uses an agent-backed runtime adapter
- WHEN prompt/control commands execute
- THEN emitted `DaemonEvent`s, session busy lifecycle, cancellation behavior, and `_session_id` metadata MUST match the previous production behavior

### Requirement: Adapter ownership is explicit [r[controller-runtime-adapter-production.ownership]]

Any remaining `clankers-controller -> clankers-agent` dependency MUST be isolated behind an adapter owner receipt with a convergence condition toward root/daemon injection.

#### Scenario: Source rails identify direct agent mutation [r[controller-runtime-adapter-production.ownership.source-rail]]
- GIVEN controller source-boundary rails inspect command policy modules
- WHEN direct `Agent` method calls appear outside the production adapter owner or compatibility constructor
- THEN validation MUST fail with the offending function, expected adapter owner, and violated requirement id

### Requirement: Runtime adapter migration is behavior-preserving [r[controller-runtime-adapter-production.verification]]

The migration MUST be covered by focused fake-runtime fixtures, production adapter parity tests, daemon/attach parity checks, and architecture rails.

#### Scenario: Closeout proves shared adapter path [r[controller-runtime-adapter-production.verification.closeout]]
- GIVEN the migration is complete
- WHEN closeout validation runs
- THEN fake-runtime fixtures, production parity tests, FCIS boundary rails, lego architecture rails, Cairn gates/validate, and diff checks MUST pass or include explicit checked evidence for environmental limitations
