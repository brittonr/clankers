//! Helpers for turning fired schedule payloads into executable agent prompts.

use std::fmt::Write as _;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;

use serde_json::Value;

const SCRIPT_TIMEOUT_SECS: u64 = 300;
const MAX_SCRIPT_OUTPUT_BYTES: usize = 50 * 1024;
const SKILL_FILE_NAME: &str = "SKILL.md";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScheduledPrompt {
    pub(crate) text: String,
    pub(crate) loaded_skills: Vec<String>,
    pub(crate) script_path: Option<PathBuf>,
}

pub(crate) fn build_scheduled_prompt(payload: &Value, cwd: &Path) -> Result<Option<ScheduledPrompt>, String> {
    let Some(prompt) = payload.get("prompt").and_then(|v| v.as_str()).filter(|s| !s.trim().is_empty()) else {
        return Ok(None);
    };

    let skills = load_requested_skills(payload, cwd)?;
    let script = run_requested_script(payload, cwd)?;

    if skills.is_empty() && script.is_none() {
        return Ok(Some(ScheduledPrompt {
            text: prompt.to_string(),
            loaded_skills: Vec::new(),
            script_path: None,
        }));
    }

    let mut text = String::new();
    if !skills.is_empty() {
        writeln!(text, "The following skills are loaded for this scheduled job:").ok();
        for skill in &skills {
            writeln!(text, "\n--- Skill: {} ---\n{}", skill.name, skill.content).ok();
        }
    }

    if let Some(script) = &script {
        if !text.is_empty() {
            text.push('\n');
        }
        writeln!(text, "Pre-job script output from {}:", script.path.display()).ok();
        writeln!(text, "```text\n{}\n```", script.output.trim_end()).ok();
    }

    if !text.is_empty() {
        text.push_str("\n\nScheduled job prompt:\n");
    }
    text.push_str(prompt);

    Ok(Some(ScheduledPrompt {
        text,
        loaded_skills: skills.into_iter().map(|skill| skill.name).collect(),
        script_path: script.map(|script| script.path),
    }))
}

#[derive(Debug, Clone)]
struct LoadedSkill {
    name: String,
    content: String,
}

#[derive(Debug, Clone)]
struct ScriptRun {
    path: PathBuf,
    output: String,
}

fn load_requested_skills(payload: &Value, cwd: &Path) -> Result<Vec<LoadedSkill>, String> {
    let Some(values) = payload.get("skills") else {
        return Ok(Vec::new());
    };
    let Some(skill_names) = values.as_array() else {
        return Err("schedule payload field 'skills' must be an array of strings".to_string());
    };

    let paths = crate::config::ClankersPaths::get();
    let project_skills_dir = crate::config::ProjectPaths::resolve(cwd).skills_dir;
    let records = discover_skill_records(&paths.global_skills_dir, Some(&project_skills_dir));

    let mut loaded = Vec::with_capacity(skill_names.len());
    for value in skill_names {
        let Some(name) = value.as_str().filter(|name| !name.trim().is_empty()) else {
            return Err("schedule payload field 'skills' must be an array of strings".to_string());
        };
        let Some(record) = records.iter().find(|record| record.display_name == name || record.name == name) else {
            return Err(format!("scheduled skill not found: {name}"));
        };
        let content = std::fs::read_to_string(&record.path)
            .map_err(|err| format!("failed to read scheduled skill {}: {err}", record.path.display()))?;
        loaded.push(LoadedSkill {
            name: record.display_name.clone(),
            content,
        });
    }
    Ok(loaded)
}

fn run_requested_script(payload: &Value, cwd: &Path) -> Result<Option<ScriptRun>, String> {
    let Some(script) = payload.get("script").and_then(|v| v.as_str()).filter(|s| !s.trim().is_empty()) else {
        return Ok(None);
    };
    let path = resolve_script_path(script, cwd)?;
    let output = run_script(&path, cwd)?;
    Ok(Some(ScriptRun { path, output }))
}

fn resolve_script_path(script: &str, cwd: &Path) -> Result<PathBuf, String> {
    let raw = PathBuf::from(script);
    if raw.is_absolute() && raw.is_file() {
        return Ok(raw);
    }

    let mut candidates = Vec::new();
    if raw.is_absolute() {
        candidates.push(raw);
    } else {
        candidates.push(cwd.join(&raw));
        if let Some(home) = dirs::home_dir() {
            candidates.push(home.join(".clankers/agent/scripts").join(&raw));
            candidates.push(home.join(".hermes/scripts").join(&raw));
        }
    }

    candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| format!("scheduled script not found: {script}"))
}

fn run_script(path: &Path, cwd: &Path) -> Result<String, String> {
    let mut command = Command::new("timeout");
    command.arg(format!("{SCRIPT_TIMEOUT_SECS}s"));
    configure_script_command(&mut command, path);
    command
        .current_dir(cwd)
        .env_clear()
        .envs(crate::tools::sandbox::sanitized_env())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = command
        .output()
        .map_err(|err| format!("failed to run scheduled script {}: {err}", path.display()))?;
    let mut combined = String::new();
    combined.push_str(&String::from_utf8_lossy(&output.stdout));
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    let combined = truncate_output(&combined);

    if output.status.success() {
        Ok(combined)
    } else {
        Err(format!(
            "scheduled script {} exited with {}:\n{}",
            path.display(),
            output.status,
            combined.trim_end()
        ))
    }
}

fn configure_script_command(command: &mut Command, path: &Path) {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("py") => {
            command.arg("python3").arg(path);
        }
        Some("rs") => {
            command.arg("cargo").arg("-q").arg("-Zscript").arg(path);
        }
        _ => {
            command.arg("sh").arg(path);
        }
    }
}

fn truncate_output(output: &str) -> String {
    if output.len() <= MAX_SCRIPT_OUTPUT_BYTES {
        return output.to_string();
    }
    let boundary = output
        .char_indices()
        .map(|(idx, _)| idx)
        .take_while(|idx| *idx <= MAX_SCRIPT_OUTPUT_BYTES)
        .last()
        .unwrap_or(0);
    let mut truncated = output[..boundary].to_string();
    truncated.push_str("\n...[truncated scheduled script output]...");
    truncated
}

#[derive(Debug, Clone)]
struct SkillRecord {
    name: String,
    display_name: String,
    path: PathBuf,
}

fn discover_skill_records(global_dir: &Path, project_dir: Option<&Path>) -> Vec<SkillRecord> {
    let mut records = Vec::new();
    records.extend(scan_skill_root(global_dir));
    if let Some(project_dir) = project_dir {
        records.extend(scan_skill_root(project_dir));
    }

    records.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    records.dedup_by(|a, b| a.display_name == b.display_name);
    records
}

fn scan_skill_root(root: &Path) -> Vec<SkillRecord> {
    let mut records = Vec::new();
    if root.is_dir() {
        scan_skill_root_inner(root, root, &mut records);
    }
    records
}

fn scan_skill_root_inner(root: &Path, current: &Path, records: &mut Vec<SkillRecord>) {
    let Ok(entries) = std::fs::read_dir(current) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let skill_file = path.join(SKILL_FILE_NAME);
        if skill_file.is_file() {
            if let Some(record) = load_skill_record(root, &skill_file) {
                records.push(record);
            }
            continue;
        }
        scan_skill_root_inner(root, &path, records);
    }
}

fn load_skill_record(root: &Path, skill_file: &Path) -> Option<SkillRecord> {
    let content = std::fs::read_to_string(skill_file).ok()?;
    let skill_dir = skill_file.parent()?;
    let name = extract_frontmatter_field(&content, "name")
        .or_else(|| skill_dir.file_name().map(|name| name.to_string_lossy().to_string()))?;
    let rel_dir = skill_dir.strip_prefix(root).ok()?;
    let category = rel_dir.parent().and_then(|parent| {
        if parent.as_os_str().is_empty() {
            None
        } else {
            Some(parent.to_string_lossy().to_string())
        }
    });
    let display_name = if let Some(category) = category {
        format!("{category}/{name}")
    } else {
        name.clone()
    };
    Some(SkillRecord {
        name,
        display_name,
        path: skill_file.to_path_buf(),
    })
}

fn extract_frontmatter_field(content: &str, key: &str) -> Option<String> {
    let mut lines = content.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    for line in lines {
        if line.trim() == "---" {
            break;
        }
        let Some((found_key, value)) = line.split_once(':') else {
            continue;
        };
        if found_key.trim() == key {
            return Some(value.trim().trim_matches('"').to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tempfile::TempDir;

    use super::build_scheduled_prompt;

    #[test]
    fn empty_or_missing_prompt_returns_none() {
        let dir = TempDir::new().unwrap();
        assert!(build_scheduled_prompt(&json!({}), dir.path()).unwrap().is_none());
        assert!(build_scheduled_prompt(&json!({"prompt": ""}), dir.path()).unwrap().is_none());
    }

    #[test]
    fn plain_prompt_passes_through() {
        let dir = TempDir::new().unwrap();
        let prompt = build_scheduled_prompt(&json!({"prompt": "run report"}), dir.path()).unwrap().unwrap();
        assert_eq!(prompt.text, "run report");
        assert!(prompt.loaded_skills.is_empty());
        assert!(prompt.script_path.is_none());
    }

    #[test]
    fn script_stdout_is_injected() {
        let dir = TempDir::new().unwrap();
        let script = dir.path().join("preflight.sh");
        std::fs::write(&script, "printf 'context from script\\n'").unwrap();
        let prompt =
            build_scheduled_prompt(&json!({"prompt": "use it", "script": script.to_string_lossy()}), dir.path())
                .unwrap()
                .unwrap();
        assert!(prompt.text.contains("Pre-job script output"));
        assert!(prompt.text.contains("context from script"));
        assert!(prompt.text.ends_with("Scheduled job prompt:\nuse it"));
        assert_eq!(prompt.script_path.as_deref(), Some(script.as_path()));
    }

    #[test]
    fn skill_content_is_injected_from_project_skills() {
        let dir = TempDir::new().unwrap();
        let skill_dir = dir.path().join(".clankers/skills/test-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: test-skill\ndescription: test\n---\n\n# Test Skill\nUse this context.",
        )
        .unwrap();

        let prompt = build_scheduled_prompt(&json!({"prompt": "do it", "skills": ["test-skill"]}), dir.path())
            .unwrap()
            .unwrap();
        assert_eq!(prompt.loaded_skills, vec!["test-skill"]);
        assert!(prompt.text.contains("--- Skill: test-skill ---"));
        assert!(prompt.text.contains("Use this context."));
        assert!(prompt.text.ends_with("Scheduled job prompt:\ndo it"));
    }

    #[test]
    fn script_failure_errors() {
        let dir = TempDir::new().unwrap();
        let script = dir.path().join("bad.sh");
        std::fs::write(&script, "echo nope; exit 7").unwrap();
        let err = build_scheduled_prompt(&json!({"prompt": "use it", "script": script.to_string_lossy()}), dir.path())
            .unwrap_err();
        assert!(err.contains("exited with"));
        assert!(err.contains("nope"));
    }
}
