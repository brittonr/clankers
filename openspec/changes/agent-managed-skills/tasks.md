## 1. Write operations in clankers-skills

- [x] 1.1 Add `write_skill(root, name, category, content) -> Result<PathBuf>` to `crates/clankers-skills/src/lib.rs`
- [x] 1.2 Add `edit_skill(root, name, content) -> Result<()>`
- [x] 1.3 Add `patch_skill(root, name, old_text, new_text, file) -> Result<()>`
- [x] 1.4 Add `delete_skill(root, name) -> Result<()>`
- [x] 1.5 Add `write_skill_file(root, name, path, content) -> Result<()>` for supporting files
- [x] 1.6 Add `remove_skill_file(root, name, path) -> Result<()>`
- [x] 1.7 Implement writable-root check: only allow writes under `~/.clankers/agent/skills/`

## 2. Validation

- [x] 2.1 Implement `validate_frontmatter(content) -> Result<()>`: require `---` delimiters, `name` (max 64 chars), `description` (max 1024 chars), non-empty body
- [x] 2.2 Implement `validate_name(name) -> Result<()>`: lowercase alphanumeric + hyphens + dots + underscores, max 64 chars
- [x] 2.3 Implement `validate_category(category) -> Result<()>`: same rules as name, single directory segment, no path separators
- [x] 2.4 Implement `validate_content_size(content) -> Result<()>`: max 100,000 chars for SKILL.md, max 1MB for supporting files
- [x] 2.5 Implement `validate_supporting_path(path) -> Result<()>`: must be in allowed subdirectories (references/, templates/, assets/, scripts/), no path traversal

## 3. Security scanning

- [x] 3.1 Create `crates/clankers-skills/src/security.rs`
- [x] 3.2 Define threat patterns: prompt injection (ignore instructions, you are now, system prompt override), exfiltration (curl/wget with secret vars, reading credential files), role hijack
- [x] 3.3 Define invisible unicode character set (U+200B, U+200C, U+200D, U+2060, U+FEFF, etc.)
- [x] 3.4 Implement `scan_content(content) -> Result<(), SecurityError>` that checks all patterns and invisible chars
- [x] 3.5 Call security scan before every write operation (create, edit, patch, write_file)

## 4. Agent tool

- [ ] 4.1 Define `skill_manage` tool schema in `crates/clankers-agent/src/tool/` with action parameter and sub-fields per action
- [ ] 4.2 Implement tool dispatch routing by action to the corresponding clankers-skills function
- [ ] 4.3 Register tool in `crates/clankers-agent/src/tool/mod.rs`

## 5. Skill creation nudge

- [ ] 5.1 Add `skills.creation_nudge_interval` config option (default 15, 0 to disable) in `crates/clankers-config/src/settings.rs`
- [ ] 5.2 Track tool-calling turn counter in agent turn loop state
- [ ] 5.3 Inject nudge system message when counter reaches the configured interval
- [ ] 5.4 Reset counter when `skill_manage` tool is called
- [ ] 5.5 Reset counter when a skill is loaded (via slash command or auto-activation)

## 6. Tests

- [x] 6.1 Unit test: create, edit, patch, delete operations on a temp directory
- [x] 6.2 Unit test: frontmatter validation rejects missing name, missing description, oversized content
- [x] 6.3 Unit test: security scan blocks prompt injection patterns
- [x] 6.4 Unit test: security scan blocks invisible unicode
- [x] 6.5 Unit test: writable-root check rejects project-level skill deletion
- [x] 6.6 Unit test: path traversal in supporting file paths is rejected
- [ ] 6.7 Unit test: nudge fires after configured interval, resets on skill_manage
