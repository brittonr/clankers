## ADDED Requirements

### Requirement: Skill creation
The agent SHALL be able to create new skills via a `skill_manage` tool with action `create`. Created skills SHALL be written to `~/.clankers/agent/skills/<name>/SKILL.md` with valid YAML frontmatter containing `name` and `description` fields.

#### Scenario: Create a new skill
- **WHEN** the agent calls `skill_manage` with action `create`, name `git-rebase`, description `Interactive rebase workflow`, and content with valid frontmatter
- **THEN** the system creates `~/.clankers/agent/skills/git-rebase/SKILL.md` with the provided content

#### Scenario: Create skill in category
- **WHEN** the agent calls `skill_manage` with action `create`, name `docker-compose`, category `devops`
- **THEN** the system creates `~/.clankers/agent/skills/devops/docker-compose/SKILL.md`

#### Scenario: Duplicate name rejected
- **WHEN** the agent calls `skill_manage` with action `create` and a skill with that name already exists
- **THEN** the system returns an error indicating the skill already exists

---

### Requirement: Skill editing
The agent SHALL be able to fully rewrite an existing skill's SKILL.md content via action `edit`, or perform targeted find-and-replace via action `patch`.

#### Scenario: Full edit
- **WHEN** the agent calls `skill_manage` with action `edit`, name `git-rebase`, and new content
- **THEN** the SKILL.md file is replaced with the new content after validation

#### Scenario: Targeted patch
- **WHEN** the agent calls `skill_manage` with action `patch`, name `git-rebase`, old_text and new_text
- **THEN** the first occurrence of old_text in SKILL.md is replaced with new_text

---

### Requirement: Skill deletion
The agent SHALL be able to delete agent-created skills via action `delete`. The system SHALL refuse to delete skills outside the writable root (`~/.clankers/agent/skills/`).

#### Scenario: Delete agent-created skill
- **WHEN** the agent calls `skill_manage` with action `delete` and name `git-rebase`
- **THEN** the skill directory `~/.clankers/agent/skills/git-rebase/` is removed

#### Scenario: Refuse to delete project skill
- **WHEN** the agent calls `skill_manage` with action `delete` for a skill in `.clankers/skills/`
- **THEN** the system returns an error indicating project-level skills are read-only

---

### Requirement: Supporting file management
The agent SHALL be able to write and remove supporting files in allowed subdirectories (`references/`, `templates/`, `assets/`, `scripts/`) of a skill.

#### Scenario: Write a reference file
- **WHEN** the agent calls `skill_manage` with action `write_file`, name `git-rebase`, path `references/advanced.md`, and content
- **THEN** the file is written to `~/.clankers/agent/skills/git-rebase/references/advanced.md`

#### Scenario: Reject path traversal
- **WHEN** the agent calls `skill_manage` with action `write_file` and path `../../etc/passwd`
- **THEN** the system rejects the operation

---

### Requirement: Frontmatter validation
The system SHALL validate all skill content before writing. Content MUST start with YAML frontmatter containing at minimum `name` (max 64 chars) and `description` (max 1024 chars). Content body MUST be non-empty. Total content MUST NOT exceed 100,000 characters.

#### Scenario: Missing frontmatter rejected
- **WHEN** the agent calls `skill_manage` with content that lacks YAML frontmatter
- **THEN** the system returns a validation error

#### Scenario: Content too large
- **WHEN** the agent calls `skill_manage` with content exceeding 100,000 characters
- **THEN** the system returns a size limit error

---

### Requirement: Security scanning
The system SHALL scan skill content for prompt injection patterns, exfiltration commands, and invisible unicode characters before writing. Content matching threat patterns SHALL be rejected with a descriptive error.

#### Scenario: Prompt injection blocked
- **WHEN** skill content contains "ignore all previous instructions"
- **THEN** the system rejects the write with error "matches threat pattern 'prompt_injection'"

#### Scenario: Exfiltration blocked
- **WHEN** skill content contains `curl ... $API_KEY`
- **THEN** the system rejects the write with error "matches threat pattern 'exfil_curl'"

#### Scenario: Invisible unicode blocked
- **WHEN** skill content contains zero-width space characters (U+200B)
- **THEN** the system rejects the write with error indicating invisible unicode
