//! Spec document parsing (markdown -> structured types)

use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

/// Strength of a requirement (RFC 2119)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RequirementStrength {
    Must,   // MUST, SHALL, REQUIRED
    Should, // SHOULD, RECOMMENDED
    May,    // MAY, OPTIONAL
}

/// A parsed requirement from a spec
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Requirement {
    pub heading: String,
    pub body: String,
    pub strength: RequirementStrength,
    pub scenarios: Vec<Scenario>,
}

/// GIVEN/WHEN/THEN scenario
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    pub name: String,
    pub given: Vec<String>,
    pub when: Vec<String>,
    pub then: Vec<String>,
}

/// A parsed spec file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spec {
    pub domain: String, // path under openspec/specs/ (e.g., "auth")
    pub file_path: PathBuf,
    pub purpose: Option<String>,
    pub requirements: Vec<Requirement>,
}

/// Scan all spec files under the specs directory
pub fn scan_specs(specs_dir: &Path) -> Vec<Spec> {
    let mut specs = Vec::new();
    if !specs_dir.is_dir() {
        return specs;
    }
    walk_spec_dir(specs_dir, specs_dir, &mut specs);
    specs
}

fn walk_spec_dir(dir: &Path, root: &Path, specs: &mut Vec<Spec>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_spec_dir(&path, root, specs);
        } else if path.extension().is_some_and(|e| e == "md")
            && let Some(spec) = parse_spec_file(&path, root)
        {
            specs.push(spec);
        }
    }
}

/// Parse a single spec markdown file
pub fn parse_spec_file(path: &Path, root: &Path) -> Option<Spec> {
    let content = std::fs::read_to_string(path).ok()?;
    let domain = path.parent()?.strip_prefix(root).ok()?.to_string_lossy().to_string();

    let mut purpose = None;
    let mut requirements = Vec::new();
    let mut current_heading: Option<String> = None;
    let mut current_body = String::new();
    let mut current_scenarios = Vec::new();

    for line in content.lines() {
        if line.starts_with("## Purpose") {
            // Flush current
            flush_requirement(&current_heading, &current_body, &current_scenarios, &mut requirements);
            current_heading = None;
            current_body.clear();
            current_scenarios.clear();
            // Next lines until next heading are purpose
        } else if line.starts_with("## ") || line.starts_with("### ") {
            // Check if previous was purpose
            if current_heading.is_none() && !current_body.is_empty() && purpose.is_none() {
                purpose = Some(current_body.trim().to_string());
            }
            flush_requirement(&current_heading, &current_body, &current_scenarios, &mut requirements);
            current_heading = Some(line.trim_start_matches('#').trim().to_string());
            current_body.clear();
            current_scenarios.clear();
        } else if line.trim().starts_with("GIVEN") || line.trim().starts_with("Given") {
            // Start of a scenario (simple parsing)
            // This is a simplified parser - proper implementation would use pulldown-cmark
            current_body.push_str(line);
            current_body.push('\n');
        } else {
            current_body.push_str(line);
            current_body.push('\n');
        }
    }

    // Flush last
    if current_heading.is_none() && !current_body.is_empty() && purpose.is_none() {
        purpose = Some(current_body.trim().to_string());
    }
    flush_requirement(&current_heading, &current_body, &current_scenarios, &mut requirements);

    Some(Spec {
        domain,
        file_path: path.to_path_buf(),
        purpose,
        requirements,
    })
}

fn flush_requirement(
    heading: &Option<String>,
    body: &str,
    scenarios: &[Scenario],
    requirements: &mut Vec<Requirement>,
) {
    if let Some(h) = heading {
        let body_trimmed = body.trim();
        if !body_trimmed.is_empty() || !scenarios.is_empty() {
            let strength = detect_strength(body_trimmed);
            let scenarios = parse_scenarios(body_trimmed);
            requirements.push(Requirement {
                heading: h.clone(),
                body: body_trimmed.to_string(),
                strength,
                scenarios,
            });
        }
    }
}

pub(crate) fn detect_strength(text: &str) -> RequirementStrength {
    let upper = text.to_uppercase();
    if upper.contains("MUST") || upper.contains("SHALL") || upper.contains("REQUIRED") {
        RequirementStrength::Must
    } else if upper.contains("SHOULD") || upper.contains("RECOMMENDED") {
        RequirementStrength::Should
    } else {
        RequirementStrength::May
    }
}

pub(crate) fn parse_scenarios(text: &str) -> Vec<Scenario> {
    // Simple GIVEN/WHEN/THEN parser
    let mut scenarios = Vec::new();
    let mut current: Option<(Vec<String>, Vec<String>, Vec<String>)> = None;
    let mut phase = ""; // "given", "when", "then"

    for line in text.lines() {
        let trimmed = line.trim();
        let lower = trimmed.to_lowercase();
        if lower.starts_with("given ") || lower == "given:" {
            if let Some((g, w, t)) = current.take() {
                scenarios.push(Scenario {
                    name: format!("Scenario {}", scenarios.len() + 1),
                    given: g,
                    when: w,
                    then: t,
                });
            }
            current = Some((vec![trimmed.to_string()], vec![], vec![]));
            phase = "given";
        } else if lower.starts_with("when ") || lower == "when:" {
            phase = "when";
            if let Some((_, ref mut w, _)) = current {
                w.push(trimmed.to_string());
            }
        } else if lower.starts_with("then ") || lower == "then:" {
            phase = "then";
            if let Some((_, _, ref mut t)) = current {
                t.push(trimmed.to_string());
            }
        } else if (trimmed.starts_with("- ") || trimmed.starts_with("AND ") || trimmed.starts_with("and "))
            && let Some((ref mut g, ref mut w, ref mut t)) = current
        {
            match phase {
                "given" => g.push(trimmed.to_string()),
                "when" => w.push(trimmed.to_string()),
                "then" => t.push(trimmed.to_string()),
                _ => {}
            }
        }
    }
    if let Some((g, w, t)) = current
        && !g.is_empty()
    {
        scenarios.push(Scenario {
            name: format!("Scenario {}", scenarios.len() + 1),
            given: g,
            when: w,
            then: t,
        });
    }
    scenarios
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_strength_must() {
        assert_eq!(detect_strength("The system MUST do X"), RequirementStrength::Must);
        assert_eq!(detect_strength("It SHALL work"), RequirementStrength::Must);
    }

    #[test]
    fn test_detect_strength_should() {
        assert_eq!(detect_strength("The system SHOULD do X"), RequirementStrength::Should);
    }

    #[test]
    fn test_detect_strength_may() {
        assert_eq!(detect_strength("This feature exists"), RequirementStrength::May);
    }

    #[test]
    fn test_parse_scenarios_basic() {
        let text = "GIVEN a user\nWHEN they login\nTHEN they see dashboard";
        let scenarios = parse_scenarios(text);
        assert_eq!(scenarios.len(), 1);
        assert!(!scenarios[0].given.is_empty());
        assert!(!scenarios[0].when.is_empty());
        assert!(!scenarios[0].then.is_empty());
    }

    #[test]
    fn test_parse_scenarios_empty() {
        let scenarios = parse_scenarios("no scenarios here");
        assert!(scenarios.is_empty());
    }
}
