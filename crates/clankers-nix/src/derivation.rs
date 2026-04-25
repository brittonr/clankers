//! Derivation reading via nix-compat.
//!
//! Parses `.drv` files to extract build metadata the agent can reason about:
//! builder, system, inputs, outputs, and filtered environment variables.

use std::collections::HashSet;
use std::path::Path;

use nix_compat::derivation::Derivation;
use serde::Serialize;

use crate::error::*;
use crate::store_path::parse_store_path;

/// Agent-friendly representation of a parsed derivation.
#[derive(Debug, Clone, Serialize)]
pub struct DerivationInfo {
    /// Name of the derivation (from the `name` env var, or the drv filename)
    pub name: String,
    /// Builder program (e.g., "/nix/store/...-bash-5.2/bin/bash")
    pub builder: String,
    /// Build system (e.g., "x86_64-linux")
    pub system: String,
    /// Named outputs and their paths
    pub outputs: Vec<OutputInfo>,
    /// Input derivations (direct build dependencies)
    pub input_drvs: Vec<InputDrvInfo>,
    /// Input sources (non-derivation inputs, e.g., source tarballs)
    pub input_srcs: Vec<String>,
    /// Environment variables set during build (filtered)
    pub build_env: Vec<(String, String)>,
}

/// A derivation output.
#[derive(Debug, Clone, Serialize)]
pub struct OutputInfo {
    pub name: String,
    pub path: String,
    pub hash_algo: Option<String>,
    pub hash: Option<String>,
}

/// An input derivation reference.
#[derive(Debug, Clone, Serialize)]
pub struct InputDrvInfo {
    pub path: String,
    pub name: String,
    pub requested_outputs: Vec<String>,
}

/// Environment variables to always include (even if large).
const INCLUDE_VARS: &[&str] = &[
    "name",
    "version",
    "pname",
    "system",
    "src",
    "out",
    "buildInputs",
    "nativeBuildInputs",
    "propagatedBuildInputs",
    "configureFlags",
    "cmakeFlags",
    "mesonFlags",
    "meta",
];

/// Environment variables to always exclude.
const EXCLUDE_VARS: &[&str] = &["__sandboxProfile", "__impureHostDeps"];

/// Maximum length for phase variables (buildPhase, installPhase, etc.)
const PHASE_TRUNCATE_LEN: usize = 500;

/// Maximum length for arbitrary env vars not in the include list.
const MAX_VAR_LEN: usize = 2000;

/// Read and parse a `.drv` file into a [`DerivationInfo`].
pub fn read_derivation(drv_path: &Path) -> Result<DerivationInfo, NixError> {
    let path_str = drv_path.display().to_string();

    let bytes = std::fs::read(drv_path).map_err(|e| NixError::DerivationIo {
        path: path_str.clone(),
        source: e,
    })?;

    let drv = Derivation::from_aterm_bytes(&bytes).map_err(|e| NixError::DerivationParse {
        path: path_str.clone(),
        reason: format!("{e:?}"),
    })?;

    let name = drv.environment.get("name").map(|v| v.to_string()).unwrap_or_else(|| {
        // Fall back to the drv filename
        drv_path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown").to_string()
    });

    let outputs = drv
        .outputs
        .iter()
        .map(|(oname, output)| {
            let hash_algo = output.ca_hash.as_ref().map(|h| format!("{h:?}"));
            OutputInfo {
                name: oname.clone(),
                path: output.path_str().into_owned(),
                hash_algo,
                hash: None,
            }
        })
        .collect();

    let input_drvs = drv
        .input_derivations
        .iter()
        .map(|(store_path, requested)| {
            let abs_path = store_path.to_absolute_path();
            let name: String = store_path.name().clone();
            InputDrvInfo {
                path: abs_path,
                name,
                requested_outputs: requested.iter().cloned().collect(),
            }
        })
        .collect();

    let input_srcs = drv.input_sources.iter().map(|sp| sp.to_absolute_path()).collect();

    let build_env = filter_env(&drv.environment);

    Ok(DerivationInfo {
        name,
        builder: drv.builder.clone(),
        system: drv.system.clone(),
        outputs,
        input_drvs,
        input_srcs,
        build_env,
    })
}

/// Summarize a derivation's dependency tree to a bounded depth.
///
/// Produces a human-readable tree like:
/// ```text
/// hello-2.12.1
/// ├── bash-5.2-p26
/// ├── gcc-13.3.0 (cc)
/// └── hello-2.12.1-src (source)
/// ```
pub fn dependency_summary(drv_path: &Path, max_depth: usize) -> Result<String, NixError> {
    let info = read_derivation(drv_path)?;
    let mut lines = Vec::new();
    lines.push(info.name.clone());
    build_dep_tree(&info, &mut lines, "", max_depth, 0);
    Ok(lines.join("\n"))
}

/// Filter derivation environment variables for agent consumption.
fn filter_env(env: &std::collections::BTreeMap<String, bstr::BString>) -> Vec<(String, String)> {
    let include_set: HashSet<&str> = INCLUDE_VARS.iter().copied().collect();
    let exclude_set: HashSet<&str> = EXCLUDE_VARS.iter().copied().collect();

    let mut result = Vec::new();

    for (key, value) in env {
        // Always skip excluded vars
        if exclude_set.contains(key.as_str()) {
            continue;
        }

        // Skip passthru variables
        if key.starts_with("passthru") {
            continue;
        }

        let val_str = value.to_string();

        // Phase variables get truncated
        if key.ends_with("Phase") {
            let truncated = if val_str.len() > PHASE_TRUNCATE_LEN {
                format!("{}... ({} chars)", &val_str[..PHASE_TRUNCATE_LEN], val_str.len())
            } else {
                val_str
            };
            result.push((key.clone(), truncated));
            continue;
        }

        // Include list: always include regardless of length
        if include_set.contains(key.as_str()) {
            result.push((key.clone(), val_str));
            continue;
        }

        // Other vars: skip if too long
        if val_str.len() <= MAX_VAR_LEN {
            result.push((key.clone(), val_str));
        }
    }

    result
}

/// Build a tree representation of derivation dependencies.
/// Recursion is bounded by max_depth parameter (typically 10).
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(no_recursion, reason = "depth bounded by max_depth parameter")
)]
fn build_dep_tree(
    info: &DerivationInfo,
    lines: &mut Vec<String>,
    prefix: &str,
    max_depth: usize,
    current_depth: usize,
) {
    if current_depth >= max_depth {
        return;
    }

    let total = info.input_drvs.len() + info.input_srcs.len();
    let mut idx = 0;

    for input_drv in &info.input_drvs {
        idx += 1;
        let is_last = idx == total;
        let connector = if is_last { "└── " } else { "├── " };

        // Extract the human name from the drv name (strip .drv suffix)
        let display_name = input_drv.name.strip_suffix(".drv").unwrap_or(&input_drv.name);

        let outputs_str = if input_drv.requested_outputs.len() == 1 && input_drv.requested_outputs[0] == "out" {
            String::new()
        } else {
            format!(" ({})", input_drv.requested_outputs.join(", "))
        };

        lines.push(format!("{prefix}{connector}{display_name}{outputs_str}"));

        // Recurse if we can read the drv
        if current_depth + 1 < max_depth {
            let child_prefix = if is_last {
                format!("{prefix}    ")
            } else {
                format!("{prefix}│   ")
            };
            let drv_path = Path::new(&input_drv.path);
            if let Ok(child_info) = read_derivation(drv_path) {
                build_dep_tree(&child_info, lines, &child_prefix, max_depth, current_depth + 1);
            }
        }
    }

    for src_path in &info.input_srcs {
        idx += 1;
        let is_last = idx == total;
        let connector = if is_last { "└── " } else { "├── " };

        let name = parse_store_path(src_path).map(|p| p.name).unwrap_or_else(|_| src_path.clone());

        lines.push(format!("{prefix}{connector}{name} (source)"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_env_includes_name() {
        let mut env = std::collections::BTreeMap::new();
        env.insert("name".to_string(), bstr::BString::from("hello-2.12.1"));
        env.insert("__sandboxProfile".to_string(), bstr::BString::from("should be excluded"));
        env.insert("passthruFoo".to_string(), bstr::BString::from("should be excluded"));

        let filtered = filter_env(&env);
        let keys: Vec<&str> = filtered.iter().map(|(k, _)| k.as_str()).collect();

        assert!(keys.contains(&"name"));
        assert!(!keys.contains(&"__sandboxProfile"));
        assert!(!keys.contains(&"passthruFoo"));
    }

    #[test]
    fn filter_env_truncates_phases() {
        let mut env = std::collections::BTreeMap::new();
        let long_phase = "x".repeat(1000);
        env.insert("buildPhase".to_string(), bstr::BString::from(long_phase.as_str()));

        let filtered = filter_env(&env);
        let (_, value) = filtered.iter().find(|(k, _)| k == "buildPhase").unwrap();
        assert!(value.len() < long_phase.len());
        assert!(value.contains("1000 chars"));
    }

    #[test]
    fn filter_env_skips_large_unknown_vars() {
        let mut env = std::collections::BTreeMap::new();
        let huge = "x".repeat(3000);
        env.insert("randomVar".to_string(), bstr::BString::from(huge.as_str()));
        env.insert("name".to_string(), bstr::BString::from("hello"));

        let filtered = filter_env(&env);
        let keys: Vec<&str> = filtered.iter().map(|(k, _)| k.as_str()).collect();

        assert!(keys.contains(&"name"));
        assert!(!keys.contains(&"randomVar"));
    }

    #[test]
    fn filter_env_keeps_include_list_regardless_of_size() {
        let mut env = std::collections::BTreeMap::new();
        let huge = "x".repeat(5000);
        env.insert("buildInputs".to_string(), bstr::BString::from(huge.as_str()));

        let filtered = filter_env(&env);
        let keys: Vec<&str> = filtered.iter().map(|(k, _)| k.as_str()).collect();
        assert!(keys.contains(&"buildInputs"));
    }

    #[test]
    fn read_nonexistent_drv() {
        let result = read_derivation(Path::new("/nix/store/nonexistent.drv"));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), NixError::DerivationIo { .. }));
    }
}
