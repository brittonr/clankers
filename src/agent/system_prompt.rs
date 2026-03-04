//! System prompt assembly: context files, AGENTS.md, SYSTEM.md, specs, skills
//!
//! Matches pi's context loading behavior:
//! - AGENTS.md (or CLAUDE.md) from global dir + walk up from cwd
//! - SYSTEM.md replaces the base system prompt
//! - APPEND_SYSTEM.md appends to whatever base prompt is used
//! - Each context file is labeled with its path in the prompt

use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use crate::config::paths::ClankersPaths;
use crate::config::paths::ProjectPaths;
use crate::prompts;
use crate::skills;
use crate::specs::SpecEngine;

/// A context file with its source path and content
#[derive(Debug, Clone)]
pub struct ContextFile {
    pub path: PathBuf,
    pub content: String,
}

/// All discovered context resources
pub struct PromptResources {
    pub skills: Vec<skills::Skill>,
    pub prompts: Vec<prompts::PromptTemplate>,
    pub context_files: Vec<String>,
    /// AGENTS.md / CLAUDE.md files (global + walk up from cwd)
    pub agents_files: Vec<ContextFile>,
    pub spec_context: String,
    /// Custom system prompt from SYSTEM.md (replaces base prompt if present)
    pub system_prompt_override: Option<String>,
    /// Append to system prompt from APPEND_SYSTEM.md
    pub append_system_prompt: Option<String>,
}

/// Discover all prompt resources from global and project paths
pub fn discover_resources(global: &ClankersPaths, project: &ProjectPaths) -> PromptResources {
    let skills = skills::discover_skills(&global.global_skills_dir, Some(&project.skills_dir));
    let prompts = prompts::discover_prompts(&global.global_prompts_dir, Some(&project.prompts_dir));
    let context_files = load_context_files(project);
    let agents_files = load_agents_files(&global.global_config_dir, &project.root);
    let spec_context = load_spec_context(&project.root);
    let system_prompt_override = load_system_md(&global.global_config_dir, &project.config_dir);
    let append_system_prompt = load_append_system_md(&global.global_config_dir, &project.config_dir);

    PromptResources {
        skills,
        prompts,
        context_files,
        agents_files,
        spec_context,
        system_prompt_override,
        append_system_prompt,
    }
}

/// Assemble the full system prompt from all sources.
///
/// Matches pi's assembly order:
/// 1. SYSTEM.md replaces base prompt (if present), otherwise use base_prompt
/// 2. APPEND_SYSTEM.md appended
/// 3. AGENTS.md / CLAUDE.md files (labeled with path)
/// 4. Context files (.clankers/context.md, .clankers/context/*.md)
/// 5. Spec context (from openspec/ if present)
/// 6. Skills listing
/// 7. Settings prefix/suffix
pub fn assemble_system_prompt(
    base_prompt: &str,
    resources: &PromptResources,
    settings_prefix: Option<&str>,
    settings_suffix: Option<&str>,
) -> String {
    let mut parts = Vec::new();

    // Settings prefix first
    if let Some(prefix) = settings_prefix
        && !prefix.is_empty()
    {
        parts.push(prefix.to_string());
    }

    // Base prompt (SYSTEM.md overrides if present)
    if let Some(ref custom) = resources.system_prompt_override {
        parts.push(custom.clone());
    } else {
        parts.push(base_prompt.to_string());
    }

    // APPEND_SYSTEM.md
    if let Some(ref append) = resources.append_system_prompt {
        parts.push(append.clone());
    }

    // AGENTS.md / CLAUDE.md files (with path headers, like pi does)
    if !resources.agents_files.is_empty() {
        let mut section = String::from("# Project Context\n\nProject-specific instructions and guidelines:\n");
        for ctx_file in &resources.agents_files {
            section.push_str(&format!("\n## {}\n\n{}\n", ctx_file.path.display(), ctx_file.content));
        }
        parts.push(section);
    }

    // Context files
    for ctx in &resources.context_files {
        if !ctx.is_empty() {
            parts.push(ctx.clone());
        }
    }

    // Spec context
    if !resources.spec_context.is_empty() {
        parts.push(resources.spec_context.clone());
    }

    // Skills
    let skills_ctx = skills::format_skills_for_context(&resources.skills);
    if !skills_ctx.is_empty() {
        parts.push(skills_ctx);
    }

    // Settings suffix
    if let Some(suffix) = settings_suffix
        && !suffix.is_empty()
    {
        parts.push(suffix.to_string());
    }

    parts.join("\n\n")
}

/// Load context files from .clankers/context.md and .clankers/context/*.md
fn load_context_files(project: &ProjectPaths) -> Vec<String> {
    let mut files = Vec::new();

    // Single context file
    if project.context_file.is_file()
        && let Ok(content) = std::fs::read_to_string(&project.context_file)
        && !content.trim().is_empty()
    {
        files.push(content);
    }

    // Context directory (*.md files, sorted)
    if project.context_dir.is_dir() {
        let mut paths: Vec<_> = std::fs::read_dir(&project.context_dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("md"))
            .map(|e| e.path())
            .collect();
        paths.sort();

        for path in paths {
            if let Ok(content) = std::fs::read_to_string(&path)
                && !content.trim().is_empty()
            {
                files.push(content);
            }
        }
    }

    files
}

/// Candidate filenames for context files (matches pi: AGENTS.md or CLAUDE.md)
const CONTEXT_FILE_CANDIDATES: &[&str] = &["AGENTS.md", "CLAUDE.md"];

/// Try to load an AGENTS.md or CLAUDE.md from a directory.
/// Returns the first candidate found.
fn load_context_file_from_dir(dir: &Path) -> Option<ContextFile> {
    for &filename in CONTEXT_FILE_CANDIDATES {
        let file_path = dir.join(filename);
        if file_path.is_file()
            && let Ok(content) = std::fs::read_to_string(&file_path)
            && !content.trim().is_empty()
        {
            return Some(ContextFile {
                path: file_path,
                content,
            });
        }
    }
    None
}

/// Load AGENTS.md / CLAUDE.md files from global dir + walk up from cwd.
/// Matches pi's behavior:
/// 1. Global config dir (e.g. ~/.clankers/agent/AGENTS.md)
/// 2. Walk up from cwd to root, collecting all found files
/// 3. Deduplicate by path
/// 4. Global first, then ancestors from root→cwd order
fn load_agents_files(global_config_dir: &Path, cwd: &Path) -> Vec<ContextFile> {
    let mut files = Vec::new();
    let mut seen_paths: HashSet<PathBuf> = HashSet::new();

    // 1. Global context file
    if let Some(ctx) = load_context_file_from_dir(global_config_dir) {
        let canonical = std::fs::canonicalize(&ctx.path).unwrap_or_else(|_| ctx.path.clone());
        seen_paths.insert(canonical);
        files.push(ctx);
    }

    // 2. Walk up from cwd to root
    let mut ancestor_files = Vec::new();
    let mut current = cwd.to_path_buf();
    let root = Path::new("/");
    loop {
        if let Some(ctx) = load_context_file_from_dir(&current) {
            let canonical = std::fs::canonicalize(&ctx.path).unwrap_or_else(|_| ctx.path.clone());
            if !seen_paths.contains(&canonical) {
                seen_paths.insert(canonical);
                ancestor_files.push(ctx);
            }
        }
        if current == root {
            break;
        }
        let parent = current.parent().map(|p| p.to_path_buf());
        match parent {
            Some(p) if p != current => current = p,
            _ => break,
        }
    }

    // 3. Reverse so root-level comes first (parent before child, like pi)
    ancestor_files.reverse();
    files.extend(ancestor_files);

    files
}

/// Load SYSTEM.md — replaces the default system prompt.
/// Project-level (.clankers/SYSTEM.md) takes precedence over global (~/.clankers/agent/SYSTEM.md).
fn load_system_md(global_config_dir: &Path, project_config_dir: &Path) -> Option<String> {
    let project_path = project_config_dir.join("SYSTEM.md");
    if project_path.is_file()
        && let Ok(content) = std::fs::read_to_string(&project_path)
        && !content.trim().is_empty()
    {
        return Some(content);
    }
    let global_path = global_config_dir.join("SYSTEM.md");
    if global_path.is_file()
        && let Ok(content) = std::fs::read_to_string(&global_path)
        && !content.trim().is_empty()
    {
        return Some(content);
    }
    None
}

/// Load APPEND_SYSTEM.md — appends to whatever system prompt is used.
/// Project-level takes precedence over global.
fn load_append_system_md(global_config_dir: &Path, project_config_dir: &Path) -> Option<String> {
    let project_path = project_config_dir.join("APPEND_SYSTEM.md");
    if project_path.is_file()
        && let Ok(content) = std::fs::read_to_string(&project_path)
        && !content.trim().is_empty()
    {
        return Some(content);
    }
    let global_path = global_config_dir.join("APPEND_SYSTEM.md");
    if global_path.is_file()
        && let Ok(content) = std::fs::read_to_string(&global_path)
        && !content.trim().is_empty()
    {
        return Some(content);
    }
    None
}

/// Load spec context from openspec/ directory
fn load_spec_context(project_root: &Path) -> String {
    let engine = SpecEngine::new(project_root);
    if engine.is_initialized() {
        engine.specs_for_context()
    } else {
        String::new()
    }
}

/// Default base system prompt when no agent definition is specified
pub fn default_system_prompt() -> &'static str {
    r#"You are clankers, a terminal coding agent. You help users by reading files, executing commands, editing code, and writing new files.

Guidelines:
- Use tools to explore the codebase before making changes
- Read files before editing them to understand context
- Make precise, surgical edits rather than full file rewrites
- Run tests after making changes to verify correctness
- Be concise in responses
- Show file paths clearly when discussing files

## Handling Missing Commands/Packages

When a command is not found (e.g., `python: command not found`), use Nix to run it ephemerally. **NEVER use `nix profile install`** - it causes conflicts.

**Quick command execution:**
```bash
nix-shell -p <package> --run "<command>"
```

**Examples:**
```bash
# Run Python script
nix-shell -p python3 --run "python3 script.py"

# With multiple packages
nix-shell -p nodejs nodePackages.npm --run "npm install"

# Compile C code
nix-shell -p gcc --run "gcc program.c -o program"
```

**For interactive development:**
```bash
# Enter shell with tools
nix-shell -p python3 nodejs gcc

# Or use nix develop for project environments
nix develop
```

**Run applications directly:**
```bash
nix run nixpkgs#python3 -- script.py
nix run nixpkgs#nodejs -- --version
```

This keeps the system clean while providing access to any package when needed.

## HEARTBEAT.md (daemon mode)

You have a file called HEARTBEAT.md in your session directory. A background
scheduler reads this file periodically and prompts you with its contents.
Use it for reminders and recurring tasks. When asked to remember or schedule
something, write it to HEARTBEAT.md. When you act on a task, mark it done
or remove it."#
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn make_test_resources() -> PromptResources {
        PromptResources {
            skills: vec![],
            prompts: vec![],
            context_files: vec![],
            agents_files: vec![],
            spec_context: String::new(),
            system_prompt_override: None,
            append_system_prompt: None,
        }
    }

    #[test]
    fn test_assemble_base_only() {
        let resources = make_test_resources();
        let result = assemble_system_prompt("Base prompt", &resources, None, None);
        assert_eq!(result, "Base prompt");
    }

    #[test]
    fn test_assemble_with_prefix_suffix() {
        let resources = make_test_resources();
        let result = assemble_system_prompt("Base", &resources, Some("PREFIX"), Some("SUFFIX"));
        assert!(result.starts_with("PREFIX"));
        assert!(result.ends_with("SUFFIX"));
        assert!(result.contains("Base"));
    }

    #[test]
    fn test_assemble_with_agents_files() {
        let mut resources = make_test_resources();
        resources.agents_files = vec![ContextFile {
            path: PathBuf::from("/project/AGENTS.md"),
            content: "# Project Instructions\nBe helpful".to_string(),
        }];

        let result = assemble_system_prompt("Base", &resources, None, None);
        assert!(result.contains("Base"));
        assert!(result.contains("Project Instructions"));
        assert!(result.contains("Be helpful"));
        // Should include the path as a header
        assert!(result.contains("/project/AGENTS.md"));
    }

    #[test]
    fn test_assemble_with_context_files() {
        let mut resources = make_test_resources();
        resources.context_files = vec!["Context 1".to_string(), "Context 2".to_string()];

        let result = assemble_system_prompt("Base", &resources, None, None);
        assert!(result.contains("Base"));
        assert!(result.contains("Context 1"));
        assert!(result.contains("Context 2"));
    }

    #[test]
    fn test_assemble_with_spec_context() {
        let mut resources = make_test_resources();
        resources.spec_context = "Spec context from openspec/".to_string();

        let result = assemble_system_prompt("Base", &resources, None, None);
        assert!(result.contains("Base"));
        assert!(result.contains("Spec context from openspec"));
    }

    #[test]
    fn test_assemble_order() {
        let mut resources = make_test_resources();
        resources.agents_files = vec![ContextFile {
            path: PathBuf::from("AGENTS.md"),
            content: "AGENTS_CONTENT".to_string(),
        }];
        resources.context_files = vec!["CONTEXT".to_string()];
        resources.spec_context = "SPEC".to_string();

        let result = assemble_system_prompt("BASE", &resources, Some("PREFIX"), Some("SUFFIX"));

        let prefix_pos = result.find("PREFIX").unwrap();
        let base_pos = result.find("BASE").unwrap();
        let agents_pos = result.find("AGENTS_CONTENT").unwrap();
        let context_pos = result.find("CONTEXT").unwrap();
        let spec_pos = result.find("SPEC").unwrap();
        let suffix_pos = result.find("SUFFIX").unwrap();

        assert!(prefix_pos < base_pos);
        assert!(base_pos < agents_pos);
        assert!(agents_pos < context_pos);
        assert!(context_pos < spec_pos);
        assert!(spec_pos < suffix_pos);
    }

    #[test]
    fn test_assemble_skips_empty_prefix_suffix() {
        let resources = make_test_resources();
        let result = assemble_system_prompt("Base", &resources, Some(""), Some(""));
        assert_eq!(result, "Base");
    }

    #[test]
    fn test_assemble_system_md_overrides_base() {
        let mut resources = make_test_resources();
        resources.system_prompt_override = Some("Custom system prompt".to_string());

        let result = assemble_system_prompt("Default base", &resources, None, None);
        assert!(result.contains("Custom system prompt"));
        assert!(!result.contains("Default base"));
    }

    #[test]
    fn test_assemble_append_system_md() {
        let mut resources = make_test_resources();
        resources.append_system_prompt = Some("Appended instructions".to_string());

        let result = assemble_system_prompt("Base", &resources, None, None);
        assert!(result.contains("Base"));
        assert!(result.contains("Appended instructions"));
        // Append should come after base
        assert!(result.find("Base").unwrap() < result.find("Appended").unwrap());
    }

    #[test]
    fn test_default_system_prompt_not_empty() {
        let prompt = default_system_prompt();
        assert!(!prompt.is_empty());
        assert!(prompt.contains("clankers"));
        assert!(prompt.contains("coding agent"));
    }

    #[test]
    fn test_load_context_files_single() {
        let temp = TempDir::new().unwrap();
        let clankers_dir = temp.path().join(".clankers");
        std::fs::create_dir_all(&clankers_dir).unwrap();

        let context_file = clankers_dir.join("context.md");
        std::fs::write(&context_file, "Test context").unwrap();

        let project = ProjectPaths::resolve(temp.path());
        let files = load_context_files(&project);

        assert_eq!(files.len(), 1);
        assert_eq!(files[0], "Test context");
    }

    #[test]
    fn test_load_context_files_directory() {
        let temp = TempDir::new().unwrap();
        let context_dir = temp.path().join(".clankers").join("context");
        std::fs::create_dir_all(&context_dir).unwrap();

        std::fs::write(context_dir.join("a.md"), "Context A").unwrap();
        std::fs::write(context_dir.join("b.md"), "Context B").unwrap();
        std::fs::write(context_dir.join("ignore.txt"), "Ignored").unwrap();

        let project = ProjectPaths::resolve(temp.path());
        let files = load_context_files(&project);

        assert_eq!(files.len(), 2); // Only .md files
        assert!(files.iter().any(|f| f.contains("Context A")));
        assert!(files.iter().any(|f| f.contains("Context B")));
    }

    #[test]
    fn test_load_context_files_empty() {
        let temp = TempDir::new().unwrap();
        let project = ProjectPaths::resolve(temp.path());
        let files = load_context_files(&project);
        assert!(files.is_empty());
    }

    #[test]
    fn test_load_agents_files_from_cwd() {
        let temp = TempDir::new().unwrap();
        let global = temp.path().join("global");
        std::fs::create_dir_all(&global).unwrap();
        let cwd = temp.path().join("project");
        std::fs::create_dir_all(&cwd).unwrap();

        std::fs::write(cwd.join("AGENTS.md"), "# Instructions").unwrap();

        let files = load_agents_files(&global, &cwd);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].content, "# Instructions");
    }

    #[test]
    fn test_load_agents_files_global() {
        let temp = TempDir::new().unwrap();
        let global = temp.path().join("global");
        std::fs::create_dir_all(&global).unwrap();
        let cwd = temp.path().join("project");
        std::fs::create_dir_all(&cwd).unwrap();

        std::fs::write(global.join("AGENTS.md"), "Global rules").unwrap();

        let files = load_agents_files(&global, &cwd);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].content, "Global rules");
    }

    #[test]
    fn test_load_agents_files_claude_md_fallback() {
        let temp = TempDir::new().unwrap();
        let global = temp.path().join("global");
        std::fs::create_dir_all(&global).unwrap();
        let cwd = temp.path().join("project");
        std::fs::create_dir_all(&cwd).unwrap();

        // CLAUDE.md should work as fallback
        std::fs::write(cwd.join("CLAUDE.md"), "Claude instructions").unwrap();

        let files = load_agents_files(&global, &cwd);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].content, "Claude instructions");
    }

    #[test]
    fn test_load_agents_files_agents_md_preferred_over_claude() {
        let temp = TempDir::new().unwrap();
        let global = temp.path().join("global");
        std::fs::create_dir_all(&global).unwrap();
        let cwd = temp.path().join("project");
        std::fs::create_dir_all(&cwd).unwrap();

        // Both exist — AGENTS.md should win
        std::fs::write(cwd.join("AGENTS.md"), "AGENTS wins").unwrap();
        std::fs::write(cwd.join("CLAUDE.md"), "CLAUDE loses").unwrap();

        let files = load_agents_files(&global, &cwd);
        assert_eq!(files.len(), 1);
        assert!(files[0].content.contains("AGENTS wins"));
    }

    #[test]
    fn test_load_agents_files_hierarchy() {
        let temp = TempDir::new().unwrap();
        let global = temp.path().join("global");
        std::fs::create_dir_all(&global).unwrap();
        let parent = temp.path().join("parent");
        let child = parent.join("child");
        std::fs::create_dir_all(&child).unwrap();

        std::fs::write(parent.join("AGENTS.md"), "Parent").unwrap();
        std::fs::write(child.join("AGENTS.md"), "Child").unwrap();

        let files = load_agents_files(&global, &child);
        assert_eq!(files.len(), 2);
        // Parent should come before child (root→cwd order)
        assert_eq!(files[0].content, "Parent");
        assert_eq!(files[1].content, "Child");
    }

    #[test]
    fn test_load_agents_files_deduplication() {
        let temp = TempDir::new().unwrap();
        let global = temp.path().join("global");
        std::fs::create_dir_all(&global).unwrap();

        // Global and cwd are the same directory
        std::fs::write(global.join("AGENTS.md"), "Shared").unwrap();

        let files = load_agents_files(&global, &global);
        // Should not appear twice
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn test_load_agents_files_none() {
        let temp = TempDir::new().unwrap();
        let global = temp.path().join("global");
        std::fs::create_dir_all(&global).unwrap();
        let cwd = temp.path().join("project");
        std::fs::create_dir_all(&cwd).unwrap();

        let files = load_agents_files(&global, &cwd);
        assert!(files.is_empty());
    }

    #[test]
    fn test_load_system_md_project_overrides_global() {
        let temp = TempDir::new().unwrap();
        let global = temp.path().join("global");
        std::fs::create_dir_all(&global).unwrap();
        let project = temp.path().join(".clankers");
        std::fs::create_dir_all(&project).unwrap();

        std::fs::write(global.join("SYSTEM.md"), "Global system").unwrap();
        std::fs::write(project.join("SYSTEM.md"), "Project system").unwrap();

        let result = load_system_md(&global, &project);
        assert_eq!(result.unwrap(), "Project system");
    }

    #[test]
    fn test_load_system_md_global_fallback() {
        let temp = TempDir::new().unwrap();
        let global = temp.path().join("global");
        std::fs::create_dir_all(&global).unwrap();
        let project = temp.path().join(".clankers");
        std::fs::create_dir_all(&project).unwrap();

        std::fs::write(global.join("SYSTEM.md"), "Global system").unwrap();

        let result = load_system_md(&global, &project);
        assert_eq!(result.unwrap(), "Global system");
    }

    #[test]
    fn test_load_append_system_md() {
        let temp = TempDir::new().unwrap();
        let global = temp.path().join("global");
        std::fs::create_dir_all(&global).unwrap();
        let project = temp.path().join(".clankers");
        std::fs::create_dir_all(&project).unwrap();

        std::fs::write(project.join("APPEND_SYSTEM.md"), "Extra instructions").unwrap();

        let result = load_append_system_md(&global, &project);
        assert_eq!(result.unwrap(), "Extra instructions");
    }
}
