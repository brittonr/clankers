# skill-manage-tool Specification

## Purpose
TBD - created by archiving change agent-learning-loop. Update Purpose after archive.
## Requirements
### Requirement: Agent can create skills from experience

The agent SHALL be able to create a new skill by calling the `skill_manage` tool with action `create`. The tool writes a SKILL.md file to `~/.clankers/agent/skills/<name>/SKILL.md` using the provided content.

#### Scenario: Create a new skill
- **WHEN** the agent calls `skill_manage` with `{"action": "create", "name": "deploy-k8s", "content": "---\nname: deploy-k8s\n..."}`
- **THEN** the tool creates `~/.clankers/agent/skills/deploy-k8s/SKILL.md` with the provided content and returns success

#### Scenario: Skill already exists
- **WHEN** the agent calls `skill_manage` with action `create` and a skill with that name already exists
- **THEN** the tool returns an error suggesting `patch` or `edit` instead

#### Scenario: Invalid skill name
- **WHEN** the agent calls `skill_manage` with a name containing characters other than lowercase alphanumeric, hyphens, or underscores
- **THEN** the tool returns an error with the naming constraints

### Requirement: Agent can patch existing skills

The agent SHALL be able to make targeted edits to an existing skill by calling the `skill_manage` tool with action `patch`. This uses exact string replacement (like the `edit` tool).

#### Scenario: Patch a skill
- **WHEN** the agent calls `skill_manage` with `{"action": "patch", "name": "deploy-k8s", "old_text": "kubectl apply", "new_text": "kubectl apply --server-side"}`
- **THEN** the tool reads the SKILL.md, replaces the exact substring, writes it back, and returns success

#### Scenario: Old text not found
- **WHEN** the agent calls `skill_manage` with action `patch` and `old_text` does not appear in the skill
- **THEN** the tool returns an error with a snippet of the current skill content

### Requirement: Agent can overwrite skills

The agent SHALL be able to fully replace a skill's content by calling the `skill_manage` tool with action `edit`. This replaces the entire SKILL.md file.

#### Scenario: Full rewrite
- **WHEN** the agent calls `skill_manage` with `{"action": "edit", "name": "deploy-k8s", "content": "<new full content>"}`
- **THEN** the tool overwrites `~/.clankers/agent/skills/deploy-k8s/SKILL.md` with the new content

### Requirement: Agent can delete skills

The agent SHALL be able to remove a skill by calling the `skill_manage` tool with action `delete`.

#### Scenario: Delete a skill
- **WHEN** the agent calls `skill_manage` with `{"action": "delete", "name": "deploy-k8s"}`
- **THEN** the tool removes the `~/.clankers/agent/skills/deploy-k8s/` directory and returns success

#### Scenario: Delete nonexistent skill
- **WHEN** the agent calls `skill_manage` with action `delete` for a skill that doesn't exist
- **THEN** the tool returns an error indicating the skill was not found

### Requirement: Agent can write supporting files

The agent SHALL be able to add or update files within a skill's directory (references, templates, scripts) by calling `skill_manage` with action `write_file`.

#### Scenario: Add a reference file
- **WHEN** the agent calls `skill_manage` with `{"action": "write_file", "name": "deploy-k8s", "file_path": "references/common-errors.md", "file_content": "..."}`
- **THEN** the tool writes the file to `~/.clankers/agent/skills/deploy-k8s/references/common-errors.md`, creating parent directories as needed

#### Scenario: Path traversal blocked
- **WHEN** the agent calls `skill_manage` with a `file_path` containing `..` or an absolute path
- **THEN** the tool returns an error rejecting the path

### Requirement: Agent can list skills

The agent SHALL be able to list installed skills by calling `skill_manage` with action `list`.

#### Scenario: List skills
- **WHEN** the agent calls `skill_manage` with `{"action": "list"}`
- **THEN** the tool returns skill names, descriptions, and source directories (global vs project)

### Requirement: System prompt guides skill creation

The system prompt SHALL include instructions telling the agent when to create skills: after completing complex tasks (5+ tool calls), after recovering from errors, after user corrections, and when discovering non-trivial workflows.

#### Scenario: Guidance present
- **WHEN** the agent starts a session with the skill_manage tool available
- **THEN** the system prompt contains a section explaining when and how to create skills

### Requirement: Skills are scoped to global directory only

The `skill_manage` tool SHALL only write to `~/.clankers/agent/skills/`, never to project-local `.clankers/skills/`. Project skills are manually authored.

#### Scenario: Writes go to global dir
- **WHEN** the agent creates a skill
- **THEN** the skill is written under the global skills directory regardless of the current working directory

