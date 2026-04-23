## Context

`clankers-skills/src/lib.rs` scans skill directories and loads `SKILL.md` files with frontmatter parsing. Skills are read-only — the agent can load and use them but cannot create, edit, or delete them. The system prompt lists available skills and the agent can request loading a skill's content, but procedural knowledge learned during sessions must be manually captured by the user.

Hermes' `skill_manager_tool.py` gives the agent create/edit/patch/delete/write_file/remove_file actions. Skills go through frontmatter validation and security scanning (checking for prompt injection, exfiltration payloads, invisible unicode). Hermes also periodically nudges the agent to create skills after sustained tool-calling work.

## Goals / Non-Goals

**Goals:**
- `skill_manage` agent tool with actions: create, edit, patch, delete
- Agent-created skills stored in `~/.clankers/agent/skills/<name>/SKILL.md`
- Frontmatter validation: require name + description, enforce length limits
- Security scanning: block content with prompt injection patterns, exfiltration commands, invisible unicode
- Category subdirectory support for organization
- Supporting file management: write/remove files in `references/`, `templates/`, `assets/` subdirectories
- Skill creation nudge: after N tool-calling turns without skill use, inject a reminder

**Non-Goals:**
- Skill hub / marketplace (external distribution is a separate concern)
- Automatic skill creation without agent decision (agent must explicitly choose to create)
- Skill versioning or rollback (filesystem + git covers this)
- Cross-machine skill sync (iroh blob sharing could do this later)

## Decisions

**Tool schema:** Single `skill_manage` tool with an `action` parameter:
- `create(name, description, content, category?)` — validate frontmatter, security scan, write to `~/.clankers/agent/skills/[category/]<name>/SKILL.md`
- `edit(name, content)` — full rewrite of SKILL.md content, re-validate
- `patch(name, old_text, new_text, file?)` — targeted find-and-replace within SKILL.md or a supporting file
- `delete(name)` — remove the skill directory entirely (user-created only, refuse to delete bundled skills)
- `write_file(name, path, content)` — write a supporting file (must be in allowed subdirectories)
- `remove_file(name, path)` — remove a supporting file

**Security scanning:** Port Hermes' threat pattern list adapted for our context:
- Prompt injection patterns: "ignore previous instructions", "you are now", "system prompt override"
- Exfiltration patterns: curl/wget with secret env vars, reading credential files
- Invisible unicode characters used for injection
- Reject content that matches any pattern with a clear error message

**Write protection:** Only skills under `~/.clankers/agent/skills/` are writable. Skills in `.clankers/skills/` (project-level) and bundled skills are read-only from the agent's perspective. The skill path is resolved and checked against the writable root before any write operation.

**Nudge logic:** Track tool-calling iterations in the agent turn loop. After a configurable interval (default: 15 turns of sustained tool use), inject a system message reminding the agent it can create skills from successful approaches. Reset the counter when `skill_manage` is called. Nudge interval is configurable in settings and can be disabled (set to 0).

**Content limits:** Max 100k chars per SKILL.md (prevents accidentally dumping entire codebases into skills). Max 1MB per supporting file.

## Risks / Trade-offs

- **Security scanning false positives:** Legitimate skill content might match threat patterns (e.g., a skill about SSH key management mentions `~/.ssh`). Mitigate with narrow patterns and clear error messages that explain what triggered the block.
- **Nudge fatigue:** Too-frequent nudges degrade the user experience. Default interval of 15 tool-calling turns is conservative. Users can disable entirely.
- **Skill quality:** Agent-created skills may be low quality or too specific. This is acceptable — the user can review and delete bad skills. The alternative (never learning) is worse.
- **Disk usage:** No limit on number of skills. If the agent creates hundreds, skill loading could slow down. Mitigate later with lazy loading if it becomes a problem.
