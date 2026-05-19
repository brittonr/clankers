//! Stable UCAN vocabulary for Clankers effect requests.

use std::fmt;
use std::path::Component;
use std::path::Path;

const SCHEME_PREFIX: &str = "clankers:";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectKind {
    FileRead,
    FileWrite,
    ShellExec,
    NetworkFetch,
    SecretRead,
    BrowserAct,
    SchedulerEnqueue,
    RemoteExec,
    ProviderRequest,
    DeliverySend,
    ArtifactRead,
    ArtifactWrite,
    PluginInvoke,
    McpInvoke,
}

impl EffectKind {
    #[must_use]
    pub const fn ability(self) -> &'static str {
        match self {
            Self::FileRead => "file/read",
            Self::FileWrite => "file/write",
            Self::ShellExec => "shell/exec",
            Self::NetworkFetch => "network/fetch",
            Self::SecretRead => "secret/read",
            Self::BrowserAct => "browser/act",
            Self::SchedulerEnqueue => "scheduler/enqueue",
            Self::RemoteExec => "remote/exec",
            Self::ProviderRequest => "provider/request",
            Self::DeliverySend => "delivery/send",
            Self::ArtifactRead => "artifact/read",
            Self::ArtifactWrite => "artifact/write",
            Self::PluginInvoke => "plugin/invoke",
            Self::McpInvoke => "mcp/invoke",
        }
    }

    #[must_use]
    pub const fn resource_class(self) -> &'static str {
        match self {
            Self::FileRead | Self::FileWrite => "file",
            Self::ShellExec => "shell",
            Self::NetworkFetch => "network",
            Self::SecretRead => "secret",
            Self::BrowserAct => "browser",
            Self::SchedulerEnqueue => "scheduler",
            Self::RemoteExec => "remote",
            Self::ProviderRequest => "provider",
            Self::DeliverySend => "delivery",
            Self::ArtifactRead | Self::ArtifactWrite => "artifact",
            Self::PluginInvoke => "plugin",
            Self::McpInvoke => "mcp",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectCapability {
    resource: String,
    ability: &'static str,
}

impl EffectCapability {
    pub fn new(kind: EffectKind, target: impl AsRef<str>) -> VocabularyResult<Self> {
        Ok(Self {
            resource: normalize_resource(kind, target.as_ref())?,
            ability: kind.ability(),
        })
    }

    #[must_use]
    pub const fn resource(&self) -> &str {
        self.resource.as_str()
    }

    #[must_use]
    pub const fn ability(&self) -> &str {
        self.ability
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VocabularyError {
    EmptyTarget { class: &'static str },
    InvalidFilePath { path: String },
    ParentTraversal { path: String },
    UnknownEffect { name: String },
}

impl fmt::Display for VocabularyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTarget { class } => write!(formatter, "{class} effect target is empty"),
            Self::InvalidFilePath { path } => write!(formatter, "invalid file effect path: {path}"),
            Self::ParentTraversal { path } => write!(formatter, "file effect path traverses parent: {path}"),
            Self::UnknownEffect { name } => write!(formatter, "unknown Clankers effect: {name}"),
        }
    }
}

impl std::error::Error for VocabularyError {}

pub type VocabularyResult<T> = Result<T, VocabularyError>;

pub fn parse_effect_kind(name: &str) -> VocabularyResult<EffectKind> {
    match name {
        "file.read" => Ok(EffectKind::FileRead),
        "file.write" => Ok(EffectKind::FileWrite),
        "shell.exec" => Ok(EffectKind::ShellExec),
        "network.fetch" => Ok(EffectKind::NetworkFetch),
        "secret.read" => Ok(EffectKind::SecretRead),
        "browser.act" => Ok(EffectKind::BrowserAct),
        "scheduler.enqueue" => Ok(EffectKind::SchedulerEnqueue),
        "remote.exec" => Ok(EffectKind::RemoteExec),
        "provider.request" => Ok(EffectKind::ProviderRequest),
        "delivery.send" => Ok(EffectKind::DeliverySend),
        "artifact.read" => Ok(EffectKind::ArtifactRead),
        "artifact.write" => Ok(EffectKind::ArtifactWrite),
        "plugin.invoke" => Ok(EffectKind::PluginInvoke),
        "mcp.invoke" => Ok(EffectKind::McpInvoke),
        other => Err(VocabularyError::UnknownEffect { name: other.to_owned() }),
    }
}

pub fn normalize_resource(kind: EffectKind, target: &str) -> VocabularyResult<String> {
    let target = target.trim();
    if target.is_empty() {
        return Err(VocabularyError::EmptyTarget {
            class: kind.resource_class(),
        });
    }
    if matches!(kind, EffectKind::FileRead | EffectKind::FileWrite) {
        return normalize_file_resource(target);
    }
    Ok(format!("{SCHEME_PREFIX}{}:{}", kind.resource_class(), percent_encode(target.as_bytes())))
}

fn normalize_file_resource(path: &str) -> VocabularyResult<String> {
    if path.as_bytes().contains(&0) {
        return Err(VocabularyError::InvalidFilePath { path: path.to_owned() });
    }
    let normalized = normalize_path_segments(path)?;
    Ok(format!("{SCHEME_PREFIX}file:{normalized}"))
}

fn normalize_path_segments(path: &str) -> VocabularyResult<String> {
    let mut output = Vec::new();
    let source = Path::new(path);
    for component in source.components() {
        match component {
            Component::Prefix(_) => {
                return Err(VocabularyError::InvalidFilePath { path: path.to_owned() });
            }
            Component::RootDir => output.clear(),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(VocabularyError::ParentTraversal { path: path.to_owned() });
            }
            Component::Normal(segment) => output.push(segment.to_string_lossy().into_owned()),
        }
    }
    if output.is_empty() {
        return Err(VocabularyError::InvalidFilePath { path: path.to_owned() });
    }
    Ok(format!("/{}", output.join("/")))
}

fn percent_encode(bytes: &[u8]) -> String {
    use std::fmt::Write;

    let mut encoded = String::new();
    for byte in bytes {
        if is_unreserved(*byte) {
            encoded.push(char::from(*byte));
        } else {
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

const fn is_unreserved(byte: u8) -> bool {
    matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~')
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURES: &[(EffectKind, &str, &str, &str)] = &[
        (
            EffectKind::FileRead,
            " /workspace/./src/lib.rs ",
            "file/read",
            "clankers:file:/workspace/src/lib.rs",
        ),
        (EffectKind::FileWrite, "/workspace/src/lib.rs", "file/write", "clankers:file:/workspace/src/lib.rs"),
        (
            EffectKind::ShellExec,
            "cargo test -p clankers-ucan",
            "shell/exec",
            "clankers:shell:cargo%20test%20-p%20clankers-ucan",
        ),
        (
            EffectKind::NetworkFetch,
            "https://example.com/a b",
            "network/fetch",
            "clankers:network:https%3A%2F%2Fexample.com%2Fa%20b",
        ),
        (EffectKind::SecretRead, "rbw:item", "secret/read", "clankers:secret:rbw%3Aitem"),
        (EffectKind::BrowserAct, "page:click", "browser/act", "clankers:browser:page%3Aclick"),
        (EffectKind::SchedulerEnqueue, "job:nightly", "scheduler/enqueue", "clankers:scheduler:job%3Anightly"),
        (EffectKind::RemoteExec, "host:cmd", "remote/exec", "clankers:remote:host%3Acmd"),
        (
            EffectKind::ProviderRequest,
            "openai:gpt-4o",
            "provider/request",
            "clankers:provider:openai%3Agpt-4o",
        ),
        (EffectKind::DeliverySend, "origin", "delivery/send", "clankers:delivery:origin"),
        (EffectKind::ArtifactRead, "b3:abc123", "artifact/read", "clankers:artifact:b3%3Aabc123"),
        (EffectKind::ArtifactWrite, "kind:report", "artifact/write", "clankers:artifact:kind%3Areport"),
        (EffectKind::PluginInvoke, "plugin/tool", "plugin/invoke", "clankers:plugin:plugin%2Ftool"),
        (EffectKind::McpInvoke, "server/tool", "mcp/invoke", "clankers:mcp:server%2Ftool"),
    ];

    #[test]
    fn known_effects_have_stable_abilities_and_resources() {
        for (kind, target, ability, resource) in FIXTURES {
            let capability = EffectCapability::new(*kind, *target).expect("fixture should normalize");
            assert_eq!(capability.ability(), *ability);
            assert_eq!(capability.resource(), *resource);
        }
    }

    #[test]
    fn effect_names_parse_to_known_kinds() {
        assert_eq!(parse_effect_kind("file.read").expect("file read"), EffectKind::FileRead);
        assert_eq!(parse_effect_kind("mcp.invoke").expect("mcp invoke"), EffectKind::McpInvoke);
    }

    #[test]
    fn unknown_effects_fail_closed() {
        let error = parse_effect_kind("daemon.root").expect_err("unknown effect denied");
        assert!(matches!(error, VocabularyError::UnknownEffect { .. }));
    }

    #[test]
    fn file_paths_reject_parent_traversal() {
        let error =
            EffectCapability::new(EffectKind::FileRead, "/workspace/../secret").expect_err("parent traversal denied");
        assert!(matches!(error, VocabularyError::ParentTraversal { .. }));
    }

    #[test]
    fn empty_targets_fail_closed() {
        let error = EffectCapability::new(EffectKind::NetworkFetch, " ").expect_err("empty target denied");
        assert!(matches!(error, VocabularyError::EmptyTarget { .. }));
    }
}
