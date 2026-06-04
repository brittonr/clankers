## Why

Clankers loads skills as read-only markdown files from `~/.clankers/agent/skills/` and `.clankers/skills/`. The agent cannot create, edit, or delete skills — procedural knowledge learned during a session evaporates unless the user manually writes a skill file. Hermes gives the agent a `skill_manage` tool that creates, edits, patches, and deletes skills with security scanning, turning successful task approaches into reusable procedural memory. This closes the learning loop: the agent can encode "how I solved this" into a skill that activates automatically in future similar contexts.

## What Changes

- Add a `skill_manage` agent tool with actions: create, edit, patch, delete, write_file, remove_file
- Skills created by the agent go into `~/.clankers/agent/skills/` with proper frontmatter validation
- Security scan agent-created skill content for prompt injection and exfiltration patterns before writing
- Support category subdirectories for organization
- Add periodic nudge logic: after N tool-calling iterations without skill use, inject a reminder that skills can be created from successful approaches

## Capabilities

### New Capabilities
- `skill-management`: Agent tool for creating, editing, patching, and deleting skills with content validation, security scanning, and frontmatter enforcement.
- `skill-creation-nudge`: Periodic prompt injection reminding the agent to capture successful approaches as skills.

### Modified Capabilities

## Impact

- `crates/clankers-skills/` — add write/edit/delete operations, content validation, security scanning
- `crates/clankers-agent/` — register `skill_manage` tool, add nudge logic to turn loop
- `crates/clankers-config/src/settings.rs` — skill creation nudge interval config
- Filesystem: agent writes to `~/.clankers/agent/skills/<name>/SKILL.md`
