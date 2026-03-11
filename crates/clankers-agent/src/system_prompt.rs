//! System prompt assembly: context files, AGENTS.md, SYSTEM.md, specs, skills
//!
//! Matches pi's context loading behavior:
//! - AGENTS.md (or CLAUDE.md) from global dir + walk up from cwd
//! - SYSTEM.md replaces the base system prompt
//! - APPEND_SYSTEM.md appends to whatever base prompt is used
//! - Each context file is labeled with its path in the prompt

use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::Path;
use std::path::PathBuf;

use clankers_config::paths::ClankersPaths;
use clankers_config::paths::ProjectPaths;
use clankers_prompts as prompts;
use clankers_skills as skills;
#[cfg(feature = "openspec")]
use clankers_specs::SpecEngine;

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

/// Format AGENTS.md / CLAUDE.md files into a project context section
fn format_agents_section(agents_files: &[ContextFile]) -> String {
    let mut section = String::from("# Project Context\n\nProject-specific instructions and guidelines:\n");
    for ctx_file in agents_files {
        let _ = writeln!(&mut section, "\n## {}\n", ctx_file.path.display());
        let _ = writeln!(&mut section, "{}", ctx_file.content);
    }
    section
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
        parts.push(format_agents_section(&resources.agents_files));
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

/// Load markdown files from a directory (sorted by path)
fn load_md_files_from_dir(dir: &Path) -> Vec<String> {
    if !dir.is_dir() {
        return Vec::new();
    }

    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("md"))
        .map(|e| e.path())
        .collect();
    paths.sort();

    paths
        .into_iter()
        .filter_map(|path| std::fs::read_to_string(&path).ok().filter(|content| !content.trim().is_empty()))
        .collect()
}

/// Load context files from .clankers/context.md and .clankers/context/*.md
fn load_context_files(project: &ProjectPaths) -> Vec<String> {
    let mut files = Vec::new();

    // Single context file
    if let Some(content) = read_non_empty_file(&project.context_file) {
        files.push(content);
    }

    // Context directory (*.md files, sorted)
    files.extend(load_md_files_from_dir(&project.context_dir));

    files
}

/// Candidate filenames for context files (matches pi: AGENTS.md or CLAUDE.md)
const CONTEXT_FILE_CANDIDATES: &[&str] = &["AGENTS.md", "CLAUDE.md"];

/// Try to load an AGENTS.md or CLAUDE.md from a directory.
/// Returns the first candidate found.
fn load_context_file_from_dir(dir: &Path) -> Option<ContextFile> {
    for &filename in CONTEXT_FILE_CANDIDATES {
        let file_path = dir.join(filename);
        if let Some(content) = read_non_empty_file(&file_path) {
            return Some(ContextFile {
                path: file_path,
                content,
            });
        }
    }
    None
}

/// Canonicalize a path, falling back to the original on error
fn canonicalize_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Walk up from a directory to root, collecting AGENTS.md / CLAUDE.md files
fn collect_ancestor_context_files(start_dir: &Path) -> Vec<ContextFile> {
    let mut files = Vec::new();
    let mut current = start_dir.to_path_buf();
    let root = Path::new("/");

    loop {
        if let Some(ctx) = load_context_file_from_dir(&current) {
            files.push(ctx);
        }

        if current == root {
            break;
        }

        match current.parent() {
            Some(p) if p != current => current = p.to_path_buf(),
            _ => break,
        }
    }

    // Reverse so root-level comes first (parent before child, like pi)
    files.reverse();
    files
}

/// Deduplicate context files by canonical path
fn deduplicate_context_files(files: Vec<ContextFile>) -> Vec<ContextFile> {
    let mut seen_paths: HashSet<PathBuf> = HashSet::new();
    let mut unique_files = Vec::new();

    for ctx in files {
        let canonical = canonicalize_path(&ctx.path);
        if seen_paths.insert(canonical) {
            unique_files.push(ctx);
        }
    }

    unique_files
}

/// Load AGENTS.md / CLAUDE.md files from global dir + walk up from cwd.
/// Matches pi's behavior:
/// 1. Global config dir (e.g. ~/.clankers/agent/AGENTS.md)
/// 2. Walk up from cwd to root, collecting all found files
/// 3. Deduplicate by path
/// 4. Global first, then ancestors from root→cwd order
fn load_agents_files(global_config_dir: &Path, cwd: &Path) -> Vec<ContextFile> {
    let mut files = Vec::new();

    // 1. Global context file
    if let Some(ctx) = load_context_file_from_dir(global_config_dir) {
        files.push(ctx);
    }

    // 2. Walk up from cwd to root
    files.extend(collect_ancestor_context_files(cwd));

    // 3. Deduplicate by canonical path
    deduplicate_context_files(files)
}

/// Load a configuration file, with project-level overriding global
fn load_config_file(global_dir: &Path, project_dir: &Path, filename: &str) -> Option<String> {
    // Try project-level first
    let project_path = project_dir.join(filename);
    if let Some(content) = read_non_empty_file(&project_path) {
        return Some(content);
    }

    // Fall back to global
    let global_path = global_dir.join(filename);
    read_non_empty_file(&global_path)
}

/// Read a file if it exists and has non-empty content
fn read_non_empty_file(path: &Path) -> Option<String> {
    if !path.is_file() {
        return None;
    }

    std::fs::read_to_string(path).ok().filter(|content| !content.trim().is_empty())
}

/// Load SYSTEM.md — replaces the default system prompt.
/// Project-level (.clankers/SYSTEM.md) takes precedence over global (~/.clankers/agent/SYSTEM.md).
fn load_system_md(global_config_dir: &Path, project_config_dir: &Path) -> Option<String> {
    load_config_file(global_config_dir, project_config_dir, "SYSTEM.md")
}

/// Load APPEND_SYSTEM.md — appends to whatever system prompt is used.
/// Project-level takes precedence over global.
fn load_append_system_md(global_config_dir: &Path, project_config_dir: &Path) -> Option<String> {
    load_config_file(global_config_dir, project_config_dir, "APPEND_SYSTEM.md")
}

/// Load spec context from openspec/ directory
#[cfg(feature = "openspec")]
fn load_spec_context(project_root: &Path) -> String {
    let engine = SpecEngine::new(project_root);
    if engine.is_initialized() {
        engine.specs_for_context()
    } else {
        String::new()
    }
}

#[cfg(not(feature = "openspec"))]
fn load_spec_context(_project_root: &Path) -> String {
    String::new()
}

/// Feature flags controlling which system prompt sections are included.
#[derive(Debug, Clone, Default)]
pub struct PromptFeatures {
    /// Nix is available on this system (`which nix` succeeded at startup).
    pub nix_available: bool,
    /// Multiple models/roles are configured (model switching makes sense).
    pub multi_model: bool,
    /// Running in daemon or RPC mode (HEARTBEAT.md is relevant).
    pub daemon_mode: bool,
    /// Process monitor is active (procmon tool is registered).
    pub process_monitor: bool,
}

const BASE_PROMPT: &str = "\
You are clankers, a terminal coding agent. You help users by reading files, executing commands, editing code, and writing new files.

Guidelines:
- Use tools to explore the codebase before making changes
- Read files before editing them to understand context
- Make precise, surgical edits rather than full file rewrites
- Run tests after making changes to verify correctness
- Be concise in responses
- Show file paths clearly when discussing files";

const NIX_SECTION: &str = "

## Handling Missing Commands/Packages

When a command is not found, use Nix to run it ephemerally. **NEVER use `nix profile install`**.

```bash
nix-shell -p <package> --run \"<command>\"
```";

const MODEL_SWITCHING_SECTION: &str = "

## Model Switching

You have a `switch_model` tool. Use it when the task is simpler than expected \
(switch to 'smol' for speed/cost savings), harder than expected (switch to 'slow' \
for maximum capability), or when transitioning between hard and easy sub-tasks. \
Don't switch unnecessarily. The switch takes effect on your next response.";

const HEARTBEAT_SECTION: &str = "

## HEARTBEAT.md (daemon mode)

You have a HEARTBEAT.md in your session directory. A background scheduler reads \
it periodically. Use it for reminders and recurring tasks.";

const PROCMON_SECTION: &str = "

## Process Monitoring

You have a `procmon` tool to inspect child processes. Actions: list, summary, \
inspect (by PID), history.";

/// Build the default system prompt with only relevant sections included.
///
/// Sections are conditionally appended based on which features are active.
/// When `features` is `None`, returns the legacy full prompt for backward compat.
pub fn build_default_system_prompt(features: &PromptFeatures) -> String {
    let mut parts = vec![BASE_PROMPT.to_string()];

    if features.nix_available {
        parts.push(NIX_SECTION.to_string());
    }
    if features.multi_model {
        parts.push(MODEL_SWITCHING_SECTION.to_string());
    }
    if features.daemon_mode {
        parts.push(HEARTBEAT_SECTION.to_string());
    }
    if features.process_monitor {
        parts.push(PROCMON_SECTION.to_string());
    }

    parts.concat()
}

/// Default base system prompt when no agent definition is specified.
///
/// Returns the full prompt with all sections for backward compat.
/// Prefer `build_default_system_prompt()` with explicit feature flags.
pub fn default_system_prompt() -> String {
    build_default_system_prompt(&PromptFeatures {
        nix_available: true,
        multi_model: true,
        daemon_mode: true,
        process_monitor: true,
    })
}

/// Detect whether `nix` is available on this system.
///
/// Runs `which nix` once; cache the result for the session.
pub fn detect_nix() -> bool {
    std::process::Command::new("which")
        .arg("nix")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
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

        let prefix_pos = result.find("PREFIX").expect("PREFIX should be in result");
        let base_pos = result.find("BASE").expect("BASE should be in result");
        let agents_pos = result.find("AGENTS_CONTENT").expect("AGENTS_CONTENT should be in result");
        let context_pos = result.find("CONTEXT").expect("CONTEXT should be in result");
        let spec_pos = result.find("SPEC").expect("SPEC should be in result");
        let suffix_pos = result.find("SUFFIX").expect("SUFFIX should be in result");

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
        assert!(
            result.find("Base").expect("Base should be in result")
                < result.find("Appended").expect("Appended should be in result")
        );
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
        let temp = TempDir::new().expect("failed to create temp dir");
        let clankers_dir = temp.path().join(".clankers");
        std::fs::create_dir_all(&clankers_dir).expect("failed to create .clankers dir");

        let context_file = clankers_dir.join("context.md");
        std::fs::write(&context_file, "Test context").expect("failed to write context file");

        let project = ProjectPaths::resolve(temp.path());
        let files = load_context_files(&project);

        assert_eq!(files.len(), 1);
        assert_eq!(files[0], "Test context");
    }

    #[test]
    fn test_load_context_files_directory() {
        let temp = TempDir::new().expect("failed to create temp dir");
        let context_dir = temp.path().join(".clankers").join("context");
        std::fs::create_dir_all(&context_dir).expect("failed to create context dir");

        std::fs::write(context_dir.join("a.md"), "Context A").expect("failed to write a.md");
        std::fs::write(context_dir.join("b.md"), "Context B").expect("failed to write b.md");
        std::fs::write(context_dir.join("ignore.txt"), "Ignored").expect("failed to write ignore.txt");

        let project = ProjectPaths::resolve(temp.path());
        let files = load_context_files(&project);

        assert_eq!(files.len(), 2); // Only .md files
        assert!(files.iter().any(|f| f.contains("Context A")));
        assert!(files.iter().any(|f| f.contains("Context B")));
    }

    #[test]
    fn test_load_context_files_empty() {
        let temp = TempDir::new().expect("failed to create temp dir");
        let project = ProjectPaths::resolve(temp.path());
        let files = load_context_files(&project);
        assert!(files.is_empty());
    }

    #[test]
    fn test_load_agents_files_from_cwd() {
        let temp = TempDir::new().expect("failed to create temp dir");
        let global = temp.path().join("global");
        std::fs::create_dir_all(&global).expect("failed to create global dir");
        let cwd = temp.path().join("project");
        std::fs::create_dir_all(&cwd).expect("failed to create cwd dir");

        std::fs::write(cwd.join("AGENTS.md"), "# Instructions").expect("failed to write AGENTS.md");

        let files = load_agents_files(&global, &cwd);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].content, "# Instructions");
    }

    #[test]
    fn test_load_agents_files_global() {
        let temp = TempDir::new().expect("failed to create temp dir");
        let global = temp.path().join("global");
        std::fs::create_dir_all(&global).expect("failed to create global dir");
        let cwd = temp.path().join("project");
        std::fs::create_dir_all(&cwd).expect("failed to create cwd dir");

        std::fs::write(global.join("AGENTS.md"), "Global rules").expect("failed to write AGENTS.md");

        let files = load_agents_files(&global, &cwd);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].content, "Global rules");
    }

    #[test]
    fn test_load_agents_files_claude_md_fallback() {
        let temp = TempDir::new().expect("failed to create temp dir");
        let global = temp.path().join("global");
        std::fs::create_dir_all(&global).expect("failed to create global dir");
        let cwd = temp.path().join("project");
        std::fs::create_dir_all(&cwd).expect("failed to create cwd dir");

        // CLAUDE.md should work as fallback
        std::fs::write(cwd.join("CLAUDE.md"), "Claude instructions").expect("failed to write CLAUDE.md");

        let files = load_agents_files(&global, &cwd);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].content, "Claude instructions");
    }

    #[test]
    fn test_load_agents_files_agents_md_preferred_over_claude() {
        let temp = TempDir::new().expect("failed to create temp dir");
        let global = temp.path().join("global");
        std::fs::create_dir_all(&global).expect("failed to create global dir");
        let cwd = temp.path().join("project");
        std::fs::create_dir_all(&cwd).expect("failed to create cwd dir");

        // Both exist — AGENTS.md should win
        std::fs::write(cwd.join("AGENTS.md"), "AGENTS wins").expect("failed to write AGENTS.md");
        std::fs::write(cwd.join("CLAUDE.md"), "CLAUDE loses").expect("failed to write CLAUDE.md");

        let files = load_agents_files(&global, &cwd);
        assert_eq!(files.len(), 1);
        assert!(files[0].content.contains("AGENTS wins"));
    }

    #[test]
    fn test_load_agents_files_hierarchy() {
        let temp = TempDir::new().expect("failed to create temp dir");
        let global = temp.path().join("global");
        std::fs::create_dir_all(&global).expect("failed to create global dir");
        let parent = temp.path().join("parent");
        let child = parent.join("child");
        std::fs::create_dir_all(&child).expect("failed to create child dir");

        std::fs::write(parent.join("AGENTS.md"), "Parent").expect("failed to write parent AGENTS.md");
        std::fs::write(child.join("AGENTS.md"), "Child").expect("failed to write child AGENTS.md");

        let files = load_agents_files(&global, &child);
        assert_eq!(files.len(), 2);
        // Parent should come before child (root→cwd order)
        assert_eq!(files[0].content, "Parent");
        assert_eq!(files[1].content, "Child");
    }

    #[test]
    fn test_load_agents_files_deduplication() {
        let temp = TempDir::new().expect("failed to create temp dir");
        let global = temp.path().join("global");
        std::fs::create_dir_all(&global).expect("failed to create global dir");

        // Global and cwd are the same directory
        std::fs::write(global.join("AGENTS.md"), "Shared").expect("failed to write AGENTS.md");

        let files = load_agents_files(&global, &global);
        // Should not appear twice
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn test_load_agents_files_none() {
        let temp = TempDir::new().expect("failed to create temp dir");
        let global = temp.path().join("global");
        std::fs::create_dir_all(&global).expect("failed to create global dir");
        let cwd = temp.path().join("project");
        std::fs::create_dir_all(&cwd).expect("failed to create cwd dir");

        let files = load_agents_files(&global, &cwd);
        assert!(files.is_empty());
    }

    #[test]
    fn test_load_system_md_project_overrides_global() {
        let temp = TempDir::new().expect("failed to create temp dir");
        let global = temp.path().join("global");
        std::fs::create_dir_all(&global).expect("failed to create global dir");
        let project = temp.path().join(".clankers");
        std::fs::create_dir_all(&project).expect("failed to create project dir");

        std::fs::write(global.join("SYSTEM.md"), "Global system").expect("failed to write global SYSTEM.md");
        std::fs::write(project.join("SYSTEM.md"), "Project system").expect("failed to write project SYSTEM.md");

        let result = load_system_md(&global, &project);
        assert_eq!(result.expect("should have SYSTEM.md"), "Project system");
    }

    #[test]
    fn test_load_system_md_global_fallback() {
        let temp = TempDir::new().expect("failed to create temp dir");
        let global = temp.path().join("global");
        std::fs::create_dir_all(&global).expect("failed to create global dir");
        let project = temp.path().join(".clankers");
        std::fs::create_dir_all(&project).expect("failed to create project dir");

        std::fs::write(global.join("SYSTEM.md"), "Global system").expect("failed to write SYSTEM.md");

        let result = load_system_md(&global, &project);
        assert_eq!(result.expect("should have SYSTEM.md"), "Global system");
    }

    #[test]
    fn test_load_append_system_md() {
        let temp = TempDir::new().expect("failed to create temp dir");
        let global = temp.path().join("global");
        std::fs::create_dir_all(&global).expect("failed to create global dir");
        let project = temp.path().join(".clankers");
        std::fs::create_dir_all(&project).expect("failed to create project dir");

        std::fs::write(project.join("APPEND_SYSTEM.md"), "Extra instructions")
            .expect("failed to write APPEND_SYSTEM.md");

        let result = load_append_system_md(&global, &project);
        assert_eq!(result.expect("should have APPEND_SYSTEM.md"), "Extra instructions");
    }

    // ── PromptFeatures / build_default_system_prompt ────────────────

    #[test]
    fn prompt_headless_no_nix() {
        let features = PromptFeatures {
            nix_available: false,
            multi_model: false,
            daemon_mode: false,
            process_monitor: false,
        };
        let prompt = build_default_system_prompt(&features);
        assert!(prompt.contains("clankers"));
        assert!(prompt.contains("coding agent"));
        assert!(!prompt.contains("nix-shell"));
        assert!(!prompt.contains("switch_model"));
        assert!(!prompt.contains("HEARTBEAT"));
        assert!(!prompt.contains("procmon"));
    }

    #[test]
    fn prompt_headless_with_nix() {
        let features = PromptFeatures {
            nix_available: true,
            multi_model: false,
            daemon_mode: false,
            process_monitor: false,
        };
        let prompt = build_default_system_prompt(&features);
        assert!(prompt.contains("nix-shell"));
        assert!(!prompt.contains("switch_model"));
        assert!(!prompt.contains("HEARTBEAT"));
    }

    #[test]
    fn prompt_interactive() {
        let features = PromptFeatures {
            nix_available: true,
            multi_model: true,
            daemon_mode: false,
            process_monitor: false,
        };
        let prompt = build_default_system_prompt(&features);
        assert!(prompt.contains("nix-shell"));
        assert!(prompt.contains("switch_model"));
        assert!(!prompt.contains("HEARTBEAT"));
    }

    #[test]
    fn prompt_daemon_all_sections() {
        let features = PromptFeatures {
            nix_available: true,
            multi_model: true,
            daemon_mode: true,
            process_monitor: true,
        };
        let prompt = build_default_system_prompt(&features);
        assert!(prompt.contains("nix-shell"));
        assert!(prompt.contains("switch_model"));
        assert!(prompt.contains("HEARTBEAT"));
        assert!(prompt.contains("procmon"));
    }

    #[test]
    fn prompt_base_always_present() {
        let features = PromptFeatures::default();
        let prompt = build_default_system_prompt(&features);
        assert!(prompt.contains("clankers"));
        assert!(prompt.contains("coding agent"));
        assert!(prompt.contains("Guidelines"));
    }

    #[test]
    fn prompt_system_md_overrides_features() {
        let features = PromptFeatures {
            nix_available: true,
            multi_model: true,
            daemon_mode: true,
            process_monitor: true,
        };
        let conditional_prompt = build_default_system_prompt(&features);
        let mut resources = make_test_resources();
        resources.system_prompt_override = Some("Custom system prompt".to_string());

        let result = assemble_system_prompt(&conditional_prompt, &resources, None, None);
        assert!(result.contains("Custom system prompt"));
        assert!(!result.contains("nix-shell"));
    }

    #[test]
    fn prompt_default_backward_compat() {
        let prompt = default_system_prompt();
        // default_system_prompt() with all features should have all sections
        assert!(prompt.contains("clankers"));
        assert!(prompt.contains("nix-shell"));
        assert!(prompt.contains("switch_model"));
        assert!(prompt.contains("HEARTBEAT"));
        assert!(prompt.contains("procmon"));
    }
}
