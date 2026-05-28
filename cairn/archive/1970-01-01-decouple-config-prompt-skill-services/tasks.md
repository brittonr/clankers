## Phase 1: Service boundaries

- [x] [serial] I1: Split display-neutral settings/path DTOs from TUI theme/keymap projection so reusable config code does not depend on `clankers-tui` or `ratatui`. [covers=r[config-prompt-skill-services.config-core.display-neutral]]
- [x] [serial] I2: Define prompt assembly service traits for system prompt, host context, filesystem context, context references, and skill snippets with safe embedded defaults. [covers=r[config-prompt-skill-services.prompt-service.host-owned]]
- [x] [parallel] I3: Define skill resolver service traits and desktop adapters for global/project skill roots without implicit lookup in generic runtime paths. [covers=r[config-prompt-skill-services.skill-service.explicit-roots]]
- [x] [serial] I4: Wire desktop CLI/TUI/daemon construction through explicit config/prompt/skill adapters while keeping embedded defaults dotdir-free. [covers=r[config-prompt-skill-services.desktop-adapter.parity]]

## Phase 2: Verification

- [x] [parallel] V1: Add config dependency/source rails rejecting TUI/ratatui/router/UCAN/runtime leakage in reusable config-core modules. [covers=r[config-prompt-skill-services.verification.config-rail]]
- [x] [parallel] V2: Add prompt/skill fixtures for host-only assembly, filesystem disabled fail-closed, explicit desktop-enabled context, missing skill service, and safe metadata redaction. [covers=r[config-prompt-skill-services.verification.prompt-skill-fixtures]]
- [x] [serial] V3: Run config/runtime/prompt/skill focused tests, embedded SDK rail if public docs change, Cairn validate/gates, and `git diff --check`. [covers=r[config-prompt-skill-services.verification.closeout]]
