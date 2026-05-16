//! Inspect content-addressed artifact hashes.

use std::path::PathBuf;

use clankers_artifacts::ArtifactHash;
use clankers_artifacts::ArtifactKind;
use clankers_artifacts::ArtifactStore;
use snafu::ResultExt;

use crate::cli::InspectHashArgs;
use crate::commands::CommandContext;
use crate::error::JsonSnafu;
use crate::error::Result;

/// Run `clankers inspect-hash`.
pub fn run(ctx: &CommandContext, args: InspectHashArgs) -> Result<()> {
    let hash = args.hash.parse::<ArtifactHash>().map_err(|source| crate::error::Error::Config {
        message: format!("invalid artifact hash: {source}"),
    })?;
    let store_dir = args.store_dir.unwrap_or_else(|| default_artifact_store_dir(ctx));
    let store = ArtifactStore::new(store_dir);
    let summary = store.inspect(hash).map_err(|source| crate::error::Error::Config {
        message: source.to_string(),
    })?;
    if let Some(expected_kind) = args.kind.as_deref() {
        let expected = parse_kind(expected_kind)?;
        if summary.kind != expected {
            return Err(crate::error::Error::Config {
                message: format!("artifact {hash} has kind {:?}, expected {:?}", summary.kind, expected),
            });
        }
    }
    let output = serde_json::to_string_pretty(&summary).context(JsonSnafu)?;
    println!("{output}");
    Ok(())
}

fn default_artifact_store_dir(ctx: &CommandContext) -> PathBuf {
    ctx.paths.global_config_dir.join("artifacts")
}

fn parse_kind(input: &str) -> Result<ArtifactKind> {
    match input {
        "prompt" => Ok(ArtifactKind::Prompt),
        "tool-descriptor" => Ok(ArtifactKind::ToolDescriptor),
        "model-request" => Ok(ArtifactKind::ModelRequest),
        "mcp-manifest" => Ok(ArtifactKind::McpManifest),
        "plugin-manifest" => Ok(ArtifactKind::PluginManifest),
        "skill-reference" => Ok(ArtifactKind::SkillReference),
        "session-block" => Ok(ArtifactKind::SessionBlock),
        other => Err(crate::error::Error::Config {
            message: format!("unknown artifact kind `{other}`"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_kind_accepts_stable_wire_names() {
        assert_eq!(parse_kind("prompt").expect("prompt kind"), ArtifactKind::Prompt);
        assert_eq!(parse_kind("model-request").expect("model kind"), ArtifactKind::ModelRequest);
        assert!(parse_kind("wrong-kind").is_err());
    }
}
