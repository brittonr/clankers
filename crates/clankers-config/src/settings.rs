//! Settings loading (global + project JSON)

use std::collections::BTreeMap;
use std::path::Path;

use clanker_message::ThinkingLevel;
use clankers_agent_defs::definition::AgentScope;
use serde::Deserialize;
use serde::Serialize;
use serde_json;

use crate::core::NeutralKeymapConfig;
use crate::core::NeutralSettingsSummary;
use crate::core::PromptServiceConfig;
use crate::core::SkillServiceConfig;
use crate::core::ThemeSelection;
use crate::keybindings::KeymapConfig;
use crate::model_roles::ModelRoles;

/// Full settings, merged from global + project
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// Default model to use
    #[serde(default = "default_model")]
    pub model: String,

    /// Default thinking/reasoning level.
    /// Use "off", "low", "medium", "high", "xhigh", or "max".
    #[serde(default = "default_thinking_level")]
    pub thinking_level: String,

    /// Default max tokens for output
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,

    /// Agent scope for discovery
    #[serde(default)]
    pub agent_scope: AgentScope,

    /// Whether to confirm before running project agents
    #[serde(default = "default_true")]
    pub confirm_project_agents: bool,

    /// Whether to create git worktrees for sessions (opt-in — writes go to
    /// a hidden worktree directory which surprises users expecting in-place edits)
    #[serde(default)]
    pub use_worktrees: bool,

    /// Custom system prompt prefix
    #[serde(default)]
    pub system_prompt_prefix: Option<String>,

    /// Custom system prompt suffix
    #[serde(default)]
    pub system_prompt_suffix: Option<String>,

    /// Theme name
    #[serde(default)]
    pub theme: Option<String>,

    /// Max output lines before truncation
    #[serde(default = "default_max_lines")]
    pub max_output_lines: usize,

    /// Max output bytes before truncation
    #[serde(default = "default_max_bytes")]
    pub max_output_bytes: usize,

    /// Bash command timeout in seconds (0 = no timeout)
    #[serde(default)]
    pub bash_timeout: u64,

    /// Auto-launch inside Zellij when available
    #[serde(default)]
    pub zellij: Option<bool>,

    /// Keymap configuration (preset + overrides)
    #[serde(default)]
    pub keymap: KeymapConfig,

    /// Model roles — route different tasks to different models
    #[serde(default, rename = "modelRoles")]
    pub model_roles: ModelRoles,

    /// Whether plan mode is enabled by default
    #[serde(default)]
    pub plan_mode: bool,

    /// Leader menu customization (add/override/hide items).
    #[serde(default)]
    pub leader_menu: LeaderMenuConfig,

    /// Memory capacity limits (cross-session learning loop)
    #[serde(default)]
    pub memory: MemoryLimits,

    /// Skill-management settings.
    #[serde(default)]
    pub skills: SkillSettings,

    /// Context compression settings (LLM-based summarization)
    #[serde(default)]
    pub compression: CompressionSettings,

    /// Routing policy configuration (auto model selection by complexity)
    #[serde(default)]
    pub routing: Option<clankers_model_selection::config::RoutingPolicyConfig>,

    /// Cost tracking configuration (budget limits and warnings)
    #[serde(default)]
    pub cost_tracking: Option<clankers_model_selection::cost_tracker::CostTrackerConfig>,

    /// Max number of subagent panes to auto-create in the BSP tiling.
    /// When the limit is reached, new subagents only appear in the overview panel.
    /// Set to 0 to disable auto-pane creation entirely.
    #[serde(default = "default_max_subagent_panes")]
    pub max_subagent_panes: usize,

    /// Tools to disable (by name). Merged from global + project settings.
    /// Tools in this list are not registered with the agent.
    #[serde(default)]
    pub disabled_tools: Vec<String>,

    /// Model Context Protocol server configuration.
    #[serde(default)]
    pub mcp: McpSettings,

    /// Stateful browser automation configuration.
    #[serde(default, rename = "browserAutomation", alias = "browser_automation")]
    pub browser_automation: BrowserAutomationSettings,

    /// External memory/personalization provider configuration.
    #[serde(default, rename = "externalMemory", alias = "external_memory")]
    pub external_memory: ExternalMemorySettings,

    /// Optional Steel Scheme turn-planning activation profile.
    ///
    /// Missing config uses the reviewed bundled profile by default. Explicit
    /// disable remains the Rust-native kill switch; Rust still validates hashes
    /// and session/UCAN authority before constructing the turn adapter.
    #[serde(default, rename = "steelTurnPlanning", alias = "steel_turn_planning")]
    pub steel_turn_planning: SteelTurnPlanningSettings,

    /// Agent-visible Steel eval tool profile material.
    ///
    /// Missing config publishes the safe pure default profile. Set
    /// `steelEval.enabled = false` to omit the tool explicitly.
    #[serde(default, rename = "steelEval", alias = "steel_eval")]
    pub steel_eval: SteelEvalSettings,

    /// Steel-mediated substrate for built-in tools, plugins, and subagents.
    #[serde(default, rename = "steelToolSubstrate", alias = "steel_tool_substrate")]
    pub steel_tool_substrate: SteelToolSubstrateSettings,

    /// Hook system configuration.
    #[serde(default)]
    pub hooks: clankers_hooks::HooksConfig,

    /// Command to run automatically after the agent finishes a turn.
    /// When set, enables auto-test mode (e.g. "cargo nextest run", "npm test").
    /// Use `/autotest` to toggle on/off during a session.
    #[serde(default)]
    pub auto_test_command: Option<String>,

    /// Disable prompt caching (send requests without cache_control breakpoints).
    /// When false (default), tool result compaction is also skipped because
    /// prompt caching provides larger cost savings than compaction.
    #[serde(default)]
    pub no_cache: bool,

    /// Cache TTL for prompt caching. Default is "5m" (ephemeral).
    /// Set to "1h" for 1-hour cache at 2× base input cost (useful for
    /// long-running agentic tasks where turns exceed the 5-minute window).
    #[serde(default)]
    pub cache_ttl: Option<String>,

    /// When true, scan nix/bash tool output for /nix/store/ paths and append
    /// a compact annotation listing referenced packages. Default: false.
    #[serde(default)]
    pub annotate_store_refs: bool,

    /// When true, the default interactive mode auto-starts a background daemon
    /// and attaches the TUI to a daemon session instead of running an in-process
    /// agent. Override with `--daemon` / `--no-daemon` CLI flags.
    #[serde(default = "default_true")]
    pub use_daemon: bool,

    /// Whether the TUI should dump recent conversation blocks to terminal
    /// scrollback after leaving the alternate screen. `None` keeps the default
    /// enabled behavior while `Some(false)` disables the dump explicitly.
    #[serde(default, alias = "scrollback_on_exit")]
    pub scrollback_on_exit: Option<bool>,

    /// Default capability restrictions for all sessions (including local).
    ///
    /// When set, every agent session gets a capability gate that enforces
    /// these restrictions at tool execution time — the LLM cannot bypass
    /// them. Capabilities are specified as UCAN capability objects.
    ///
    /// Example (settings.json):
    /// ```json
    /// "defaultCapabilities": [
    ///   { "ToolUse": { "tool_pattern": "read,write,edit,bash,rg" } },
    ///   { "ShellExecute": { "command_pattern": "*", "working_dir": "/home/user/project" } },
    ///   { "FileAccess": { "prefix": "/home/user/project", "read_only": false } }
    /// ]
    /// ```
    ///
    /// When absent (default), local sessions have full access. Remote sessions
    /// are still constrained by their UCAN token capabilities.
    #[serde(default)]
    pub default_capabilities: Option<Vec<clankers_ucan::Capability>>,
}

/// Model Context Protocol settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpSettings {
    /// Named MCP servers. Global and project settings deep-merge by server name.
    #[serde(default)]
    pub servers: BTreeMap<String, McpServerConfig>,
}

/// One MCP server entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerConfig {
    /// Disable without deleting the server entry.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Server transport.
    pub transport: McpTransport,
    /// Stdio executable. Required when `transport = "stdio"`.
    #[serde(default)]
    pub command: Option<String>,
    /// Stdio arguments.
    #[serde(default)]
    pub args: Vec<String>,
    /// HTTP endpoint. Required when `transport = "http"`.
    #[serde(default)]
    pub url: Option<String>,
    /// Environment variables forwarded to stdio servers.
    #[serde(default)]
    pub env_allowlist: Vec<String>,
    /// HTTP header names mapped to environment variables containing values.
    #[serde(default)]
    pub header_env: BTreeMap<String, String>,
    /// Allow only these MCP tool names before publication. Empty means all.
    #[serde(default)]
    pub include_tools: Vec<String>,
    /// Exclude these MCP tool names after include filtering.
    #[serde(default)]
    pub exclude_tools: Vec<String>,
    /// Optional visible tool-name prefix. Defaults to `mcp_<server>`.
    #[serde(default)]
    pub tool_prefix: Option<String>,
    /// Request timeout in milliseconds.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpTransport {
    Stdio,
    Http,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpServerConfigError {
    MissingStdioCommand,
    BlankStdioCommand,
    UnexpectedStdioCommand,
    MissingHttpUrl,
    BlankHttpUrl,
    HeaderWithoutEnvName { header: String },
    BlankEnvAllowlistEntry,
}

impl std::fmt::Display for McpServerConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingStdioCommand => f.write_str("stdio MCP servers must declare `command`"),
            Self::BlankStdioCommand => f.write_str("stdio MCP server `command` cannot be blank"),
            Self::UnexpectedStdioCommand => f.write_str("HTTP MCP servers cannot declare `command`"),
            Self::MissingHttpUrl => f.write_str("HTTP MCP servers must declare `url`"),
            Self::BlankHttpUrl => f.write_str("HTTP MCP server `url` cannot be blank"),
            Self::HeaderWithoutEnvName { header } => {
                write!(f, "HTTP MCP header `{header}` must map to a non-empty environment variable name")
            }
            Self::BlankEnvAllowlistEntry => f.write_str("MCP `envAllowlist` entries cannot be blank"),
        }
    }
}

impl std::error::Error for McpServerConfigError {}

impl McpServerConfig {
    pub fn validate(&self) -> Result<(), McpServerConfigError> {
        if self.env_allowlist.iter().any(|entry| entry.trim().is_empty()) {
            return Err(McpServerConfigError::BlankEnvAllowlistEntry);
        }

        for (header, env_name) in &self.header_env {
            if header.trim().is_empty() || env_name.trim().is_empty() {
                return Err(McpServerConfigError::HeaderWithoutEnvName { header: header.clone() });
            }
        }

        match self.transport {
            McpTransport::Stdio => match self.command.as_deref() {
                Some(command) if !command.trim().is_empty() => Ok(()),
                Some(_) => Err(McpServerConfigError::BlankStdioCommand),
                None => Err(McpServerConfigError::MissingStdioCommand),
            },
            McpTransport::Http => {
                if self.command.is_some() {
                    return Err(McpServerConfigError::UnexpectedStdioCommand);
                }
                match self.url.as_deref() {
                    Some(url) if !url.trim().is_empty() => Ok(()),
                    Some(_) => Err(McpServerConfigError::BlankHttpUrl),
                    None => Err(McpServerConfigError::MissingHttpUrl),
                }
            }
        }
    }

    pub fn publishes_tool(&self, tool_name: &str) -> bool {
        let included = self.include_tools.is_empty() || self.include_tools.iter().any(|tool| tool == tool_name);
        let excluded = self.exclude_tools.iter().any(|tool| tool == tool_name);
        included && !excluded
    }

    pub fn published_tool_name(&self, server_name: &str, tool_name: &str) -> String {
        let prefix = self.tool_prefix.as_deref().map(str::to_owned).unwrap_or_else(|| format!("mcp_{server_name}"));
        format!("{prefix}_{tool_name}")
    }
}

/// Stateful browser automation settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserAutomationSettings {
    /// Publish and enable the browser tool.
    #[serde(default)]
    pub enabled: bool,
    /// Browser backend. First supported backend is local Chrome/Chromium CDP.
    #[serde(default)]
    pub backend: BrowserAutomationBackend,
    /// Existing CDP endpoint, for example `http://127.0.0.1:9222`.
    #[serde(default)]
    pub cdp_url: Option<String>,
    /// Browser executable to launch when no CDP URL is supplied.
    #[serde(default)]
    pub browser_binary: Option<String>,
    /// Optional browser profile directory.
    #[serde(default)]
    pub user_data_dir: Option<String>,
    /// Launch browser headless when clankers owns the browser process.
    #[serde(default = "default_true")]
    pub headless: bool,
    /// Allow JavaScript evaluation actions.
    #[serde(default)]
    pub allow_evaluate: bool,
    /// Allow screenshot actions.
    #[serde(default = "default_true")]
    pub allow_screenshots: bool,
    /// Request/action timeout in milliseconds.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// Optional URL origin allowlist. Empty means all origins are allowed.
    #[serde(default)]
    pub allowed_origins: Vec<String>,
}

impl Default for BrowserAutomationSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            backend: BrowserAutomationBackend::default(),
            cdp_url: None,
            browser_binary: None,
            user_data_dir: None,
            headless: true,
            allow_evaluate: false,
            allow_screenshots: true,
            timeout_ms: None,
            allowed_origins: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BrowserAutomationBackend {
    #[default]
    Cdp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowserAutomationConfigError {
    MissingCdpEndpointOrBrowserBinary,
    BlankCdpUrl,
    BlankBrowserBinary,
    BlankUserDataDir,
    NonPositiveTimeout,
    BlankAllowedOrigin,
}

impl std::fmt::Display for BrowserAutomationConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingCdpEndpointOrBrowserBinary => {
                f.write_str("enabled browser automation requires `cdpUrl` or `browserBinary`")
            }
            Self::BlankCdpUrl => f.write_str("browser automation `cdpUrl` cannot be blank"),
            Self::BlankBrowserBinary => f.write_str("browser automation `browserBinary` cannot be blank"),
            Self::BlankUserDataDir => f.write_str("browser automation `userDataDir` cannot be blank"),
            Self::NonPositiveTimeout => f.write_str("browser automation `timeoutMs` must be greater than zero"),
            Self::BlankAllowedOrigin => f.write_str("browser automation `allowedOrigins` entries cannot be blank"),
        }
    }
}

impl std::error::Error for BrowserAutomationConfigError {}

impl BrowserAutomationSettings {
    pub fn validate(&self) -> Result<(), BrowserAutomationConfigError> {
        if !self.enabled {
            return Ok(());
        }

        if matches!(self.timeout_ms, Some(0)) {
            return Err(BrowserAutomationConfigError::NonPositiveTimeout);
        }
        if self.allowed_origins.iter().any(|origin| origin.trim().is_empty()) {
            return Err(BrowserAutomationConfigError::BlankAllowedOrigin);
        }
        if matches!(self.cdp_url.as_deref(), Some(url) if url.trim().is_empty()) {
            return Err(BrowserAutomationConfigError::BlankCdpUrl);
        }
        if matches!(self.browser_binary.as_deref(), Some(binary) if binary.trim().is_empty()) {
            return Err(BrowserAutomationConfigError::BlankBrowserBinary);
        }
        if matches!(self.user_data_dir.as_deref(), Some(dir) if dir.trim().is_empty()) {
            return Err(BrowserAutomationConfigError::BlankUserDataDir);
        }

        match self.backend {
            BrowserAutomationBackend::Cdp => {
                if self.cdp_url.is_none() && self.browser_binary.is_none() {
                    return Err(BrowserAutomationConfigError::MissingCdpEndpointOrBrowserBinary);
                }
            }
        }
        Ok(())
    }

    pub fn permits_origin(&self, origin: &str) -> bool {
        self.allowed_origins.is_empty() || self.allowed_origins.iter().any(|allowed| origin_matches(allowed, origin))
    }
}

fn origin_matches(pattern: &str, origin: &str) -> bool {
    if pattern == "*" || pattern == origin {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix("*") {
        return origin.starts_with(prefix);
    }
    false
}

// ---------------------------------------------------------------------------
// External memory provider settings
// ---------------------------------------------------------------------------

/// External memory/personalization provider settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalMemorySettings {
    /// Publish and enable the external_memory tool.
    #[serde(default)]
    pub enabled: bool,
    /// Provider kind. First-pass supported provider is local.
    #[serde(default)]
    pub provider: ExternalMemoryProvider,
    /// Safe provider label for user-visible output and metadata.
    #[serde(default)]
    pub name: Option<String>,
    /// Provider endpoint when required by a provider kind.
    #[serde(default)]
    pub endpoint: Option<String>,
    /// Environment variable name containing credentials. Values are never serialized in metadata.
    #[serde(default)]
    pub credential_env: Option<String>,
    /// Request timeout in milliseconds.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// Maximum result count returned to the agent.
    #[serde(default = "default_external_memory_max_results")]
    pub max_results: usize,
    /// Inject provider context into prompts before model contact.
    #[serde(default)]
    pub inject_into_prompt: bool,
}

impl Default for ExternalMemorySettings {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: ExternalMemoryProvider::default(),
            name: None,
            endpoint: None,
            credential_env: None,
            timeout_ms: None,
            max_results: default_external_memory_max_results(),
            inject_into_prompt: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExternalMemoryProvider {
    #[default]
    Local,
    Http,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalMemoryConfigError {
    BlankName,
    MissingHttpEndpoint,
    MissingCredentialEnv,
    BlankEndpoint,
    BlankCredentialEnv,
    NonPositiveTimeout,
    NonPositiveMaxResults,
}

impl std::fmt::Display for ExternalMemoryConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BlankName => f.write_str("external memory `name` cannot be blank"),
            Self::MissingHttpEndpoint => f.write_str("enabled HTTP external memory requires `endpoint`"),
            Self::MissingCredentialEnv => f.write_str("enabled HTTP external memory requires `credentialEnv`"),
            Self::BlankEndpoint => f.write_str("external memory `endpoint` cannot be blank"),
            Self::BlankCredentialEnv => f.write_str("external memory `credentialEnv` cannot be blank"),
            Self::NonPositiveTimeout => f.write_str("external memory `timeoutMs` must be greater than zero"),
            Self::NonPositiveMaxResults => f.write_str("external memory `maxResults` must be greater than zero"),
        }
    }
}

impl std::error::Error for ExternalMemoryConfigError {}

impl ExternalMemorySettings {
    pub fn validate(&self) -> Result<(), ExternalMemoryConfigError> {
        if !self.enabled {
            return Ok(());
        }

        if matches!(self.name.as_deref(), Some(name) if name.trim().is_empty()) {
            return Err(ExternalMemoryConfigError::BlankName);
        }
        if matches!(self.endpoint.as_deref(), Some(endpoint) if endpoint.trim().is_empty()) {
            return Err(ExternalMemoryConfigError::BlankEndpoint);
        }
        if matches!(self.credential_env.as_deref(), Some(env) if env.trim().is_empty()) {
            return Err(ExternalMemoryConfigError::BlankCredentialEnv);
        }
        if matches!(self.timeout_ms, Some(0)) {
            return Err(ExternalMemoryConfigError::NonPositiveTimeout);
        }
        if self.max_results == 0 {
            return Err(ExternalMemoryConfigError::NonPositiveMaxResults);
        }

        match self.provider {
            ExternalMemoryProvider::Local => Ok(()),
            ExternalMemoryProvider::Http => {
                if self.endpoint.is_none() {
                    Err(ExternalMemoryConfigError::MissingHttpEndpoint)
                } else if self.credential_env.is_none() {
                    Err(ExternalMemoryConfigError::MissingCredentialEnv)
                } else {
                    Ok(())
                }
            }
        }
    }

    pub fn safe_provider_name(&self) -> &str {
        self.name.as_deref().unwrap_or(match self.provider {
            ExternalMemoryProvider::Local => "local",
            ExternalMemoryProvider::Http => "http",
        })
    }
}

fn default_external_memory_max_results() -> usize {
    8
}

// ---------------------------------------------------------------------------
// Steel eval agent tool settings
// ---------------------------------------------------------------------------

/// Settings for publishing the agent-visible `steel_eval` built-in tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SteelEvalSettings {
    /// Publish the `steel_eval` tool when true. Defaults to the safe pure profile.
    #[serde(default = "default_steel_eval_enabled")]
    pub enabled: bool,
    /// Default reviewed profile identifier.
    #[serde(default = "default_steel_eval_profile_id")]
    pub default_profile: String,
    /// Default profile material. Missing fields keep conservative defaults.
    #[serde(default)]
    pub profile: SteelEvalProfileSettings,
    /// Additional named profile material available to explicit tool requests.
    #[serde(default)]
    pub profiles: Vec<SteelEvalProfileSettings>,
}

impl Default for SteelEvalSettings {
    fn default() -> Self {
        Self {
            enabled: default_steel_eval_enabled(),
            default_profile: default_steel_eval_profile_id(),
            profile: SteelEvalProfileSettings::default(),
            profiles: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SteelEvalProfileSettings {
    /// Reviewed profile id.
    #[serde(default = "default_steel_eval_profile_id")]
    pub id: String,
    /// Maximum accepted source bytes before evaluation.
    #[serde(default = "default_steel_eval_max_source_bytes")]
    pub max_source_bytes: u64,
    /// Maximum returned output bytes.
    #[serde(default = "default_steel_eval_max_output_bytes")]
    pub max_output_bytes: u64,
    /// Maximum approved host calls. Defaults to zero for pure eval only.
    #[serde(default)]
    pub max_host_calls: u64,
    /// Maximum fixture/runtime steps.
    #[serde(default = "default_steel_eval_max_steps")]
    pub max_steps: u64,
    /// Capabilities granted to this reviewed eval profile.
    #[serde(default)]
    pub session_capabilities: Vec<String>,
    /// Explicitly registered host functions for this reviewed profile.
    #[serde(default)]
    pub host_functions: Vec<SteelEvalHostFunctionSettings>,
}

impl Default for SteelEvalProfileSettings {
    fn default() -> Self {
        Self {
            id: default_steel_eval_profile_id(),
            max_source_bytes: default_steel_eval_max_source_bytes(),
            max_output_bytes: default_steel_eval_max_output_bytes(),
            max_host_calls: 0,
            max_steps: default_steel_eval_max_steps(),
            session_capabilities: Vec::new(),
            host_functions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SteelEvalHostFunctionSettings {
    pub name: String,
    pub required_capability: String,
    pub output: String,
}

fn default_steel_eval_enabled() -> bool {
    true
}

fn default_steel_eval_profile_id() -> String {
    "default".to_string()
}

fn default_steel_eval_max_source_bytes() -> u64 {
    4096
}

fn default_steel_eval_max_output_bytes() -> u64 {
    1024
}

fn default_steel_eval_max_steps() -> u64 {
    256
}

// ---------------------------------------------------------------------------
// Steel turn-planning activation settings
// ---------------------------------------------------------------------------

/// Optional settings for activating the reviewed Steel Scheme turn-planning seam.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SteelTurnPlanningSettings {
    /// Enable config-driven construction of `AgentTurnSteelPlanningConfig`.
    #[serde(default = "default_steel_turn_planning_enabled")]
    pub enabled: bool,
    /// Reviewed Nickel-exported profile JSON path.
    #[serde(default)]
    pub profile_path: Option<String>,
    /// Reviewed Steel Scheme script path.
    #[serde(default)]
    pub script_path: Option<String>,
    /// Expected BLAKE3 hash for the script source (`b3:<hex>`).
    #[serde(default)]
    pub script_blake3: Option<String>,
    /// Expected BLAKE3 hash for the profile JSON (`b3:<hex>`). When absent,
    /// Rust computes and records the profile hash without treating config as authority.
    #[serde(default)]
    pub profile_blake3: Option<String>,
    /// Optional rollout override. Missing means use the reviewed profile.
    #[serde(default)]
    pub rollout_stage: Option<SteelTurnPlanningRolloutStage>,
    /// Optional fallback override. Missing means use the reviewed profile.
    #[serde(default)]
    pub fallback_mode: Option<SteelTurnPlanningFallbackMode>,
    /// Optional seam override. The only supported value is `steel.host.plan_turn`.
    #[serde(default)]
    pub planning_seam: Option<String>,
    /// Session capabilities actually available to this session.
    #[serde(default)]
    pub session_capabilities: Vec<String>,
    /// UCAN abilities actually granted to this session/script context.
    #[serde(default)]
    pub granted_ucan_abilities: Vec<String>,
    /// Basalt-backed UCAN authority grants for invoking the reviewed Steel planner.
    #[serde(default)]
    pub ucan_authority_grants: Vec<SteelTurnPlanningAuthorityGrantSettings>,
    /// Host actions disabled by user/session policy.
    #[serde(default)]
    pub disabled_actions: Vec<String>,
    /// Optional receipt destination prefix. Must remain under `target/`.
    #[serde(default)]
    pub receipt_prefix: Option<String>,
    /// Optional max turn input bytes override.
    #[serde(default)]
    pub max_input_bytes: Option<u64>,
    /// Optional max script bytes guard.
    #[serde(default = "default_steel_turn_planning_max_source_bytes")]
    pub max_source_bytes: u64,
}

impl Default for SteelTurnPlanningSettings {
    fn default() -> Self {
        Self {
            enabled: default_steel_turn_planning_enabled(),
            profile_path: None,
            script_path: None,
            script_blake3: None,
            profile_blake3: None,
            rollout_stage: None,
            fallback_mode: None,
            planning_seam: None,
            session_capabilities: default_steel_turn_planning_session_capabilities(),
            granted_ucan_abilities: default_steel_turn_planning_ucan_abilities(),
            ucan_authority_grants: Vec::new(),
            disabled_actions: Vec::new(),
            receipt_prefix: None,
            max_input_bytes: None,
            max_source_bytes: default_steel_turn_planning_max_source_bytes(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SteelTurnPlanningAuthorityGrantSettings {
    pub resource: String,
    pub ability: String,
    pub audience: String,
    #[serde(default)]
    pub proof_reference: Option<String>,
    #[serde(default)]
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub revoked: bool,
    #[serde(default)]
    pub caveats: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelTurnPlanningRolloutStage {
    Disabled,
    Comparison,
    Default,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelTurnPlanningFallbackMode {
    RustNative,
    Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SteelTurnPlanningConfigError {
    MissingProfilePath,
    MissingScriptPath,
    BlankProfilePath,
    BlankScriptPath,
    BlankHash,
    BlankCapability,
    BlankUcanAbility,
    BlankUcanAuthorityGrant,
    BlankDisabledAction,
    NonPositiveMaxInputBytes,
    NonPositiveMaxSourceBytes,
    ReceiptOutsideTarget,
}

impl std::fmt::Display for SteelTurnPlanningConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingProfilePath => f.write_str("enabled Steel turn planning requires `profilePath`"),
            Self::MissingScriptPath => f.write_str("enabled Steel turn planning requires `scriptPath`"),
            Self::BlankProfilePath => f.write_str("Steel turn planning `profilePath` cannot be blank"),
            Self::BlankScriptPath => f.write_str("Steel turn planning `scriptPath` cannot be blank"),
            Self::BlankHash => f.write_str("Steel turn planning hashes cannot be blank"),
            Self::BlankCapability => f.write_str("Steel turn planning session capabilities cannot be blank"),
            Self::BlankUcanAbility => f.write_str("Steel turn planning UCAN abilities cannot be blank"),
            Self::BlankUcanAuthorityGrant => f.write_str("Steel turn planning UCAN authority grants cannot contain blank resource, ability, audience, proof reference, or caveat entries"),
            Self::BlankDisabledAction => f.write_str("Steel turn planning disabled actions cannot be blank"),
            Self::NonPositiveMaxInputBytes => {
                f.write_str("Steel turn planning `maxInputBytes` must be greater than zero")
            }
            Self::NonPositiveMaxSourceBytes => {
                f.write_str("Steel turn planning `maxSourceBytes` must be greater than zero")
            }
            Self::ReceiptOutsideTarget => f.write_str("Steel turn planning `receiptPrefix` must stay under target/"),
        }
    }
}

impl std::error::Error for SteelTurnPlanningConfigError {}

impl SteelTurnPlanningSettings {
    #[must_use]
    pub fn uses_bundled_profile(&self) -> bool {
        self.profile_path.is_none() && self.script_path.is_none()
    }

    pub fn validate(&self) -> Result<(), SteelTurnPlanningConfigError> {
        if !self.enabled {
            return Ok(());
        }
        match self.profile_path.as_deref() {
            Some(path) if path.trim().is_empty() => return Err(SteelTurnPlanningConfigError::BlankProfilePath),
            Some(_) => {}
            None if self.script_path.is_some() => return Err(SteelTurnPlanningConfigError::MissingProfilePath),
            None => {}
        }
        match self.script_path.as_deref() {
            Some(path) if path.trim().is_empty() => return Err(SteelTurnPlanningConfigError::BlankScriptPath),
            Some(_) => {}
            None if self.profile_path.is_some() => return Err(SteelTurnPlanningConfigError::MissingScriptPath),
            None => {}
        }
        if self.script_blake3.as_deref().is_some_and(|hash| hash.trim().is_empty())
            || self.profile_blake3.as_deref().is_some_and(|hash| hash.trim().is_empty())
        {
            return Err(SteelTurnPlanningConfigError::BlankHash);
        }
        if self.session_capabilities.iter().any(|capability| capability.trim().is_empty()) {
            return Err(SteelTurnPlanningConfigError::BlankCapability);
        }
        if self.granted_ucan_abilities.iter().any(|ability| ability.trim().is_empty()) {
            return Err(SteelTurnPlanningConfigError::BlankUcanAbility);
        }
        if self.ucan_authority_grants.iter().any(|grant| {
            grant.resource.trim().is_empty()
                || grant.ability.trim().is_empty()
                || grant.audience.trim().is_empty()
                || grant.proof_reference.as_deref().is_some_and(|proof| proof.trim().is_empty())
                || grant.caveats.iter().any(|caveat| caveat.trim().is_empty())
        }) {
            return Err(SteelTurnPlanningConfigError::BlankUcanAuthorityGrant);
        }
        if self.disabled_actions.iter().any(|action| action.trim().is_empty()) {
            return Err(SteelTurnPlanningConfigError::BlankDisabledAction);
        }
        if matches!(self.max_input_bytes, Some(0)) {
            return Err(SteelTurnPlanningConfigError::NonPositiveMaxInputBytes);
        }
        if self.max_source_bytes == 0 {
            return Err(SteelTurnPlanningConfigError::NonPositiveMaxSourceBytes);
        }
        if let Some(prefix) = &self.receipt_prefix
            && !prefix.starts_with("target/")
        {
            return Err(SteelTurnPlanningConfigError::ReceiptOutsideTarget);
        }
        Ok(())
    }
}

fn default_steel_turn_planning_enabled() -> bool {
    true
}

fn default_steel_turn_planning_session_capabilities() -> Vec<String> {
    vec![
        "steel-orchestration".to_string(),
        "turn-planning".to_string(),
        "turn-execution".to_string(),
    ]
}

fn default_steel_turn_planning_ucan_abilities() -> Vec<String> {
    vec![
        "clankers/steel/orchestrate.plan_turn".to_string(),
        "clankers/steel/orchestrate.execute_turn".to_string(),
    ]
}

fn default_steel_turn_planning_max_source_bytes() -> u64 {
    4096
}

// ---------------------------------------------------------------------------
// Steel tool/plugin/subagent substrate activation settings
// ---------------------------------------------------------------------------

/// Optional settings for activating Steel-mediated tool/plugin/subagent dispatch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SteelToolSubstrateSettings {
    /// Enable substrate planning. Defaults on; set false as the Rust-native kill switch.
    #[serde(default = "default_steel_tool_substrate_enabled")]
    pub enabled: bool,
    /// Rollout stage for substrate dispatch.
    #[serde(default)]
    pub rollout_stage: Option<SteelToolSubstrateRolloutStage>,
    /// Fallback behavior when Steel does not authorize a typed plan.
    #[serde(default)]
    pub fallback_mode: Option<SteelToolSubstrateFallbackMode>,
    /// Session capabilities available to Steel host functions.
    #[serde(default = "default_steel_tool_substrate_session_capabilities")]
    pub session_capabilities: Vec<String>,
    /// UCAN-style abilities granted to the substrate context.
    #[serde(default = "default_steel_tool_substrate_ucan_abilities")]
    pub granted_ucan_abilities: Vec<String>,
    /// Executor kinds disabled for substrate authorization.
    #[serde(default)]
    pub disabled_executors: Vec<String>,
    /// Host actions disabled by user/session policy.
    #[serde(default)]
    pub disabled_actions: Vec<String>,
    /// Optional receipt destination prefix. Must remain under `target/`.
    #[serde(default)]
    pub receipt_prefix: Option<String>,
    /// Optional max input bytes override.
    #[serde(default)]
    pub max_input_bytes: Option<u64>,
    /// Optional max script bytes guard.
    #[serde(default = "default_steel_tool_substrate_max_source_bytes")]
    pub max_source_bytes: u64,
}

impl Default for SteelToolSubstrateSettings {
    fn default() -> Self {
        Self {
            enabled: default_steel_tool_substrate_enabled(),
            rollout_stage: Some(SteelToolSubstrateRolloutStage::Default),
            fallback_mode: Some(SteelToolSubstrateFallbackMode::RustNative),
            session_capabilities: default_steel_tool_substrate_session_capabilities(),
            granted_ucan_abilities: default_steel_tool_substrate_ucan_abilities(),
            disabled_executors: Vec::new(),
            disabled_actions: Vec::new(),
            receipt_prefix: None,
            max_input_bytes: Some(200_000),
            max_source_bytes: default_steel_tool_substrate_max_source_bytes(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelToolSubstrateRolloutStage {
    Disabled,
    Comparison,
    Default,
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelToolSubstrateFallbackMode {
    RustNative,
    Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SteelToolSubstrateConfigError {
    BlankCapability,
    BlankUcanAbility,
    BlankDisabledExecutor,
    BlankDisabledAction,
    NonPositiveMaxInputBytes,
    NonPositiveMaxSourceBytes,
    ReceiptOutsideTarget,
}

impl std::fmt::Display for SteelToolSubstrateConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BlankCapability => f.write_str("Steel tool substrate session capabilities cannot be blank"),
            Self::BlankUcanAbility => f.write_str("Steel tool substrate UCAN abilities cannot be blank"),
            Self::BlankDisabledExecutor => f.write_str("Steel tool substrate disabled executors cannot be blank"),
            Self::BlankDisabledAction => f.write_str("Steel tool substrate disabled actions cannot be blank"),
            Self::NonPositiveMaxInputBytes => {
                f.write_str("Steel tool substrate `maxInputBytes` must be greater than zero")
            }
            Self::NonPositiveMaxSourceBytes => {
                f.write_str("Steel tool substrate `maxSourceBytes` must be greater than zero")
            }
            Self::ReceiptOutsideTarget => f.write_str("Steel tool substrate `receiptPrefix` must stay under target/"),
        }
    }
}

impl std::error::Error for SteelToolSubstrateConfigError {}

impl SteelToolSubstrateSettings {
    pub fn validate(&self) -> Result<(), SteelToolSubstrateConfigError> {
        if !self.enabled {
            return Ok(());
        }
        if self.session_capabilities.iter().any(|capability| capability.trim().is_empty()) {
            return Err(SteelToolSubstrateConfigError::BlankCapability);
        }
        if self.granted_ucan_abilities.iter().any(|ability| ability.trim().is_empty()) {
            return Err(SteelToolSubstrateConfigError::BlankUcanAbility);
        }
        if self.disabled_executors.iter().any(|executor| executor.trim().is_empty()) {
            return Err(SteelToolSubstrateConfigError::BlankDisabledExecutor);
        }
        if self.disabled_actions.iter().any(|action| action.trim().is_empty()) {
            return Err(SteelToolSubstrateConfigError::BlankDisabledAction);
        }
        if matches!(self.max_input_bytes, Some(0)) {
            return Err(SteelToolSubstrateConfigError::NonPositiveMaxInputBytes);
        }
        if self.max_source_bytes == 0 {
            return Err(SteelToolSubstrateConfigError::NonPositiveMaxSourceBytes);
        }
        if let Some(prefix) = &self.receipt_prefix
            && !prefix.starts_with("target/")
        {
            return Err(SteelToolSubstrateConfigError::ReceiptOutsideTarget);
        }
        Ok(())
    }
}

fn default_steel_tool_substrate_enabled() -> bool {
    true
}

fn default_steel_tool_substrate_session_capabilities() -> Vec<String> {
    vec!["steel-tool-substrate".to_string(), "tool-dispatch".to_string()]
}

fn default_steel_tool_substrate_ucan_abilities() -> Vec<String> {
    vec!["clankers/steel/tool.call".to_string()]
}

fn default_steel_tool_substrate_max_source_bytes() -> u64 {
    4096
}

// ---------------------------------------------------------------------------
// Leader menu user config
// ---------------------------------------------------------------------------

/// User-configurable leader menu items and hide rules.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LeaderMenuConfig {
    /// Items to add or override in the leader menu.
    #[serde(default)]
    pub items: Vec<LeaderMenuItemConfig>,
    /// Items to hide from the leader menu.
    #[serde(default)]
    pub hide: Vec<LeaderMenuHideConfig>,
}

/// A user-defined leader menu item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderMenuItemConfig {
    /// Key to press.
    pub key: char,
    /// Display label.
    pub label: String,
    /// Slash command to execute (e.g. "/shell git status").
    pub command: String,
    /// Submenu name. If omitted, goes to root.
    #[serde(default)]
    pub submenu: Option<String>,
}

/// Hides a specific leader menu entry by key + placement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderMenuHideConfig {
    /// Key to hide.
    pub key: char,
    /// Submenu name. If omitted, hides from root.
    #[serde(default)]
    pub submenu: Option<String>,
}

// ---------------------------------------------------------------------------
// Memory limits
// ---------------------------------------------------------------------------

/// Capacity limits for cross-session memory.
///
/// The agent's memory tool checks these before saving new entries.
/// Character counts refer to the sum of `entry.text.len()` within each scope.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryLimits {
    /// Max chars for global-scope memories (default: 2200 ≈ 800 tokens).
    #[serde(default = "default_global_char_limit")]
    pub global_char_limit: usize,
    /// Max chars for per-project memories (default: 1375 ≈ 500 tokens).
    #[serde(default = "default_project_char_limit")]
    pub project_char_limit: usize,
}

fn default_global_char_limit() -> usize {
    2200
}
fn default_project_char_limit() -> usize {
    1375
}

impl Default for MemoryLimits {
    fn default() -> Self {
        Self {
            global_char_limit: default_global_char_limit(),
            project_char_limit: default_project_char_limit(),
        }
    }
}

// ---------------------------------------------------------------------------
// Skill settings
// ---------------------------------------------------------------------------

/// Configuration for agent-managed skill creation reminders.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSettings {
    /// Number of consecutive tool-calling turns before nudging skill creation.
    /// 0 disables nudging.
    #[serde(default = "default_creation_nudge_interval")]
    pub creation_nudge_interval: usize,
}

const DEFAULT_CREATION_NUDGE_INTERVAL: usize = 15;

fn default_creation_nudge_interval() -> usize {
    DEFAULT_CREATION_NUDGE_INTERVAL
}

impl Default for SkillSettings {
    fn default() -> Self {
        Self {
            creation_nudge_interval: default_creation_nudge_interval(),
        }
    }
}

// ---------------------------------------------------------------------------
// Compression settings
// ---------------------------------------------------------------------------

/// Configuration for LLM-based context compression.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressionSettings {
    /// Model to use for manual compression requests.
    /// When absent, uses the cheapest available model from the active provider.
    #[serde(default)]
    pub model: Option<String>,
    /// Cheap/fast model to use for automatic structured summary generation.
    #[serde(default = "default_summary_model")]
    pub summary_model: String,
    /// Number of recent messages to keep intact during manual compression.
    #[serde(default = "default_keep_recent")]
    pub keep_recent: usize,
    /// Fraction of the context window reserved for recent-message tail protection.
    #[serde(
        default = "default_tail_context_fraction",
        rename = "tailBudgetFraction",
        alias = "tail_budget_fraction"
    )]
    pub tail_context_fraction: f64,
    /// Minimum message count before compression is allowed.
    #[serde(default = "default_min_messages")]
    pub min_messages: usize,
}

const DEFAULT_KEEP_RECENT: usize = 4;
const DEFAULT_TAIL_CONTEXT_FRACTION: f64 = 0.40;
const DEFAULT_MIN_MESSAGES: usize = 5;
const DEFAULT_SUMMARY_MODEL: &str = "haiku";

fn default_keep_recent() -> usize {
    DEFAULT_KEEP_RECENT
}
fn default_summary_model() -> String {
    DEFAULT_SUMMARY_MODEL.to_string()
}
fn default_tail_context_fraction() -> f64 {
    DEFAULT_TAIL_CONTEXT_FRACTION
}
fn default_min_messages() -> usize {
    DEFAULT_MIN_MESSAGES
}

impl Default for CompressionSettings {
    fn default() -> Self {
        Self {
            model: None,
            summary_model: default_summary_model(),
            keep_recent: default_keep_recent(),
            tail_context_fraction: default_tail_context_fraction(),
            min_messages: default_min_messages(),
        }
    }
}

fn default_model() -> String {
    "openai-codex/gpt-5.5".to_string()
}
fn default_max_tokens() -> usize {
    16384
}
fn default_thinking_level() -> String {
    "max".to_string()
}
fn default_true() -> bool {
    true
}
fn default_max_lines() -> usize {
    2000
}
fn default_max_bytes() -> usize {
    50 * 1024
}
fn default_max_subagent_panes() -> usize {
    4
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            model: default_model(),
            thinking_level: default_thinking_level(),
            max_tokens: default_max_tokens(),
            agent_scope: AgentScope::default(),
            confirm_project_agents: true,
            use_worktrees: false,
            system_prompt_prefix: None,
            system_prompt_suffix: None,
            theme: None,
            max_output_lines: default_max_lines(),
            max_output_bytes: default_max_bytes(),
            bash_timeout: 0,
            zellij: None,
            keymap: KeymapConfig::default(),
            model_roles: ModelRoles::default(),
            plan_mode: false,
            leader_menu: LeaderMenuConfig::default(),
            memory: MemoryLimits::default(),
            skills: SkillSettings::default(),
            compression: CompressionSettings::default(),
            routing: None,
            cost_tracking: None,
            max_subagent_panes: default_max_subagent_panes(),
            disabled_tools: Vec::new(),
            mcp: McpSettings::default(),
            browser_automation: BrowserAutomationSettings::default(),
            external_memory: ExternalMemorySettings::default(),
            steel_turn_planning: SteelTurnPlanningSettings::default(),
            steel_eval: SteelEvalSettings::default(),
            steel_tool_substrate: SteelToolSubstrateSettings::default(),
            hooks: clankers_hooks::HooksConfig::default(),
            auto_test_command: None,
            no_cache: false,
            cache_ttl: None,
            annotate_store_refs: false,
            use_daemon: true,
            scrollback_on_exit: None,
            default_capabilities: None,
        }
    }
}

impl Settings {
    /// Parsed thinking level for provider requests and TUI state.
    pub fn parsed_thinking_level(&self) -> ThinkingLevel {
        ThinkingLevel::from_str_or_budget(&self.thinking_level).unwrap_or(ThinkingLevel::Max)
    }

    /// Display-neutral projection for embeddable/runtime service adapters.
    #[must_use]
    pub fn neutral_summary(&self) -> NeutralSettingsSummary {
        NeutralSettingsSummary {
            model: self.model.clone(),
            thinking_level: self.thinking_level.clone(),
            theme: self.theme.clone().map(ThemeSelection::named),
            keymap: NeutralKeymapConfig {
                preset: self.keymap.preset.to_string(),
                normal: self.keymap.normal.iter().map(|(key, value)| (key.clone(), value.clone())).collect(),
                insert: self.keymap.insert.iter().map(|(key, value)| (key.clone(), value.clone())).collect(),
            },
            skills: SkillServiceConfig {
                enabled: self.skills.creation_nudge_interval > 0,
                requested: Vec::new(),
            },
            prompt: PromptServiceConfig {
                allow_filesystem_context: true,
                allow_context_references: true,
                skill_service_required: self.skills.creation_nudge_interval > 0,
            },
        }
    }

    /// Load settings by merging pi fallback, global, and project files.
    /// Priority (highest wins): project > global (~/.clankers) > pi fallback (~/.pi)
    pub fn load(global_path: &Path, project_path: &Path) -> Self {
        Self::load_with_pi_fallback(None, global_path, project_path)
    }

    /// Load settings with an optional ~/.pi/agent/settings.json fallback.
    /// Priority (highest wins): project > global (~/.clankers) > pi fallback (~/.pi)
    pub fn load_with_pi_fallback(pi_settings_path: Option<&Path>, global_path: &Path, project_path: &Path) -> Self {
        let pi = pi_settings_path.and_then(Self::load_file).map(Self::normalize_pi_settings);
        let global = Self::load_file(global_path);
        let project = Self::load_file(project_path);
        Self::merge_layers(pi, global, project)
    }

    /// Load settings with Nickel support. Checks `.ncl` paths first, falls
    /// back to `.json` at each layer.
    ///
    /// Priority (highest wins): project > global > pi fallback.
    /// At each layer: `.ncl` preferred over `.json` when both exist.
    pub fn load_with_nickel(
        pi_settings_path: Option<&Path>,
        global_json: &Path,
        global_ncl: &Path,
        project_json: &Path,
        project_ncl: &Path,
    ) -> Self {
        let pi = pi_settings_path.and_then(Self::load_file).map(Self::normalize_pi_settings);
        let global = Self::load_layer(Some(global_ncl), global_json);
        let project = Self::load_layer(Some(project_ncl), project_json);
        Self::merge_layers(pi, global, project)
    }

    /// Load a single config layer. Checks `.ncl` first (if the nickel feature
    /// is enabled), then falls back to `.json`.
    fn load_layer(ncl_path: Option<&Path>, json_path: &Path) -> Option<serde_json::Value> {
        #[cfg(feature = "nickel")]
        if let Some(ncl) = ncl_path
            && ncl.exists()
        {
            match crate::nickel::eval_ncl_file(ncl) {
                Ok(value) => return Some(value),
                Err(e) => {
                    eprintln!("warning: failed to evaluate {}: {e}", ncl.display());
                    // Fall through to JSON
                }
            }
        }
        #[cfg(not(feature = "nickel"))]
        let _ = ncl_path;

        Self::load_file(json_path)
    }

    /// Map pi-specific setting names to clankers equivalents.
    /// e.g. pi uses "defaultModel" while clankers uses "model".
    fn normalize_pi_settings(mut value: serde_json::Value) -> serde_json::Value {
        if let Some(obj) = value.as_object_mut() {
            let default_provider = obj.get("defaultProvider").and_then(serde_json::Value::as_str).map(str::to_string);
            // Map defaultModel -> model. Pi splits provider and model; Clankers
            // uses explicit provider prefixes for subscription Codex models.
            if let Some(model) = obj.remove("defaultModel") {
                let normalized_model = match (default_provider.as_deref(), model.as_str()) {
                    (Some("openai-codex"), Some(model_id)) if !model_id.contains('/') => {
                        serde_json::Value::String(format!("openai-codex/{model_id}"))
                    }
                    _ => model,
                };
                obj.entry("model").or_insert(normalized_model);
            }
        }
        value
    }

    fn load_file(path: &Path) -> Option<serde_json::Value> {
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// Merge up to three layers of settings: pi fallback < global < project
    fn merge_layers(
        pi: Option<serde_json::Value>,
        global: Option<serde_json::Value>,
        project: Option<serde_json::Value>,
    ) -> Self {
        let mut base = pi.unwrap_or_else(|| serde_json::json!({}));

        // Merge global on top of pi fallback
        if let Some(g) = global {
            Self::merge_into(&mut base, &g);
        }

        // Merge project on top
        if let Some(p) = project {
            Self::merge_into(&mut base, &p);
        }

        serde_json::from_value(base).unwrap_or_default()
    }

    /// Recursively merge source object fields into target object.
    ///
    /// When both target and source have an object at the same key, the merge
    /// recurses into the nested object so that individual fields are preserved.
    /// Non-object values (strings, numbers, arrays, bools, nulls) are replaced
    /// wholesale — arrays are NOT concatenated.
    fn merge_into(target: &mut serde_json::Value, source: &serde_json::Value) {
        if let (Some(target_obj), Some(source_obj)) = (target.as_object_mut(), source.as_object()) {
            for (key, value) in source_obj {
                match (target_obj.get_mut(key), value) {
                    // Both sides are objects → recurse
                    (Some(existing), new_val) if existing.is_object() && new_val.is_object() => {
                        Self::merge_into(existing, new_val);
                    }
                    // Otherwise replace (or insert new key)
                    _ => {
                        target_obj.insert(key.clone(), value.clone());
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_tools_from_json() {
        let json = r#"{"disabledTools": ["bash", "commit"]}"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.disabled_tools, vec!["bash".to_string(), "commit".to_string()]);
    }

    #[test]
    fn disabled_tools_default_empty() {
        let json = r"{}";
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert!(settings.disabled_tools.is_empty());
        assert!(settings.steel_turn_planning.enabled);
        assert!(settings.steel_turn_planning.uses_bundled_profile());
        assert_eq!(settings.steel_turn_planning.session_capabilities, vec![
            "steel-orchestration",
            "turn-planning",
            "turn-execution"
        ]);
        assert_eq!(settings.steel_turn_planning.granted_ucan_abilities, vec![
            "clankers/steel/orchestrate.plan_turn",
            "clankers/steel/orchestrate.execute_turn"
        ]);
        assert!(settings.steel_turn_planning.validate().is_ok());
    }

    #[test]
    fn compression_tail_context_fraction_keeps_wire_compatibility() {
        let camel_json = r#"{"compression":{"tailBudgetFraction":0.25}}"#;
        let camel_settings: Settings = serde_json::from_str(camel_json).unwrap();
        assert_eq!(camel_settings.compression.tail_context_fraction, 0.25);

        let snake_json = r#"{"compression":{"tail_budget_fraction":0.30}}"#;
        let snake_settings: Settings = serde_json::from_str(snake_json).unwrap();
        assert_eq!(snake_settings.compression.tail_context_fraction, 0.30);

        let settings = Settings {
            compression: CompressionSettings {
                tail_context_fraction: 0.35,
                ..CompressionSettings::default()
            },
            ..Settings::default()
        };
        let serialized = serde_json::to_value(settings).unwrap();
        assert_eq!(serialized["compression"]["tailBudgetFraction"], serde_json::json!(0.35));
        assert!(serialized["compression"].get("tailContextFraction").is_none());
    }

    #[test]
    fn steel_eval_from_json_defaults_enabled_and_parses_profiles() {
        let defaults: Settings = serde_json::from_str(r"{}").unwrap();
        assert!(defaults.steel_eval.enabled);
        assert_eq!(defaults.steel_eval.default_profile, "default");
        assert_eq!(defaults.steel_eval.profile.max_host_calls, 0);
        assert!(defaults.steel_eval.profile.session_capabilities.is_empty());
        assert!(defaults.steel_eval.profile.host_functions.is_empty());

        let disabled: Settings = serde_json::from_str(r#"{"steelEval":{"enabled":false}}"#).unwrap();
        assert!(!disabled.steel_eval.enabled);

        let json = r#"{
            "steelEval": {
                "enabled": true,
                "defaultProfile": "pure",
                "profile": {"maxSourceBytes": 128, "maxOutputBytes": 64},
                "profiles": [{
                    "id": "echo",
                    "maxHostCalls": 1,
                    "sessionCapabilities": ["steel.host.echo"],
                    "hostFunctions": [{"name":"steel.host.echo","requiredCapability":"steel.host.echo","output":"ok"}]
                }]
            }
        }"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert!(settings.steel_eval.enabled);
        assert_eq!(settings.steel_eval.default_profile, "pure");
        assert_eq!(settings.steel_eval.profile.max_source_bytes, 128);
        assert_eq!(settings.steel_eval.profiles[0].id, "echo");
        assert_eq!(settings.steel_eval.profiles[0].host_functions[0].output, "ok");
    }

    #[test]
    fn steel_tool_substrate_from_json_defaults_enabled_and_validates() {
        let defaults: Settings = serde_json::from_str(r"{}").unwrap();
        assert!(defaults.steel_tool_substrate.enabled);
        assert_eq!(defaults.steel_tool_substrate.rollout_stage, Some(SteelToolSubstrateRolloutStage::Default));
        assert_eq!(defaults.steel_tool_substrate.fallback_mode, Some(SteelToolSubstrateFallbackMode::RustNative));
        assert_eq!(defaults.steel_tool_substrate.session_capabilities, vec!["steel-tool-substrate", "tool-dispatch"]);
        assert_eq!(defaults.steel_tool_substrate.granted_ucan_abilities, vec!["clankers/steel/tool.call"]);
        defaults.steel_tool_substrate.validate().expect("default Steel tool substrate settings are valid");

        let disabled: Settings = serde_json::from_str(r#"{"steelToolSubstrate":{"enabled":false}}"#).unwrap();
        assert!(!disabled.steel_tool_substrate.enabled);

        let custom: Settings = serde_json::from_str(
            r#"{"steelToolSubstrate":{"rolloutStage":"block","fallbackMode":"block","disabledExecutors":["subagent"],"receiptPrefix":"target/steel-tool-plugin-substrate"}}"#,
        )
        .unwrap();
        assert_eq!(custom.steel_tool_substrate.rollout_stage, Some(SteelToolSubstrateRolloutStage::Block));
        assert_eq!(custom.steel_tool_substrate.fallback_mode, Some(SteelToolSubstrateFallbackMode::Block));
        assert_eq!(custom.steel_tool_substrate.disabled_executors, vec!["subagent"]);
        custom.steel_tool_substrate.validate().expect("custom Steel tool substrate settings are valid");
    }

    #[test]
    fn steel_turn_planning_from_json_validates_authority_shape() {
        let json = r#"{
            "steelTurnPlanning": {
                "enabled": true,
                "profilePath": "policy/steel-default-orchestration/orchestration-profile.json",
                "scriptPath": "policy/steel-default-orchestration/scripts/default-plan-turn.scm",
                "scriptBlake3": "b3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "rolloutStage": "comparison",
                "fallbackMode": "rust_native",
                "sessionCapabilities": ["steel-orchestration", "turn-planning"],
                "grantedUcanAbilities": ["clankers/steel/orchestrate.plan_turn"],
                "ucanAuthorityGrants": [{
                    "resource": "turn:session-fixture",
                    "ability": "clankers/steel/orchestrate.plan_turn",
                    "audience": "clankers:agent-turn-planning",
                    "proofReference": "settings-grant",
                    "expiresAt": "2999-01-01T00:00:00Z",
                    "caveats": ["metadata_only"]
                }],
                "receiptPrefix": "target/steel-turn-planning-config-activation"
            }
        }"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        let steel = settings.steel_turn_planning;
        assert!(steel.enabled);
        assert_eq!(steel.rollout_stage, Some(SteelTurnPlanningRolloutStage::Comparison));
        assert_eq!(steel.fallback_mode, Some(SteelTurnPlanningFallbackMode::RustNative));
        assert_eq!(steel.ucan_authority_grants.len(), 1);
        assert_eq!(steel.ucan_authority_grants[0].resource, "turn:session-fixture");
        assert_eq!(steel.ucan_authority_grants[0].proof_reference.as_deref(), Some("settings-grant"));
        assert_eq!(steel.ucan_authority_grants[0].caveats, vec!["metadata_only".to_string()]);
        steel.validate().expect("valid Steel turn-planning activation settings");
    }

    #[test]
    fn steel_turn_planning_validation_accepts_bundled_default_without_profile_paths() {
        let settings: Settings = serde_json::from_str(r#"{"steelTurnPlanning":{"enabled":true}}"#).unwrap();
        assert!(settings.steel_turn_planning.uses_bundled_profile());
        assert!(settings.steel_turn_planning.validate().is_ok());
    }

    #[test]
    fn steel_turn_planning_validation_rejects_partial_profile_paths() {
        let settings: Settings =
            serde_json::from_str(r#"{"steelTurnPlanning":{"enabled":true,"scriptPath":"script.scm"}}"#).unwrap();
        assert_eq!(settings.steel_turn_planning.validate(), Err(SteelTurnPlanningConfigError::MissingProfilePath));
    }

    #[test]
    fn steel_turn_planning_validation_rejects_receipts_outside_target() {
        let settings: Settings = serde_json::from_str(
            r#"{"steelTurnPlanning":{"enabled":true,"profilePath":"profile.json","scriptPath":"script.scm","receiptPrefix":"/tmp/leak"}}"#,
        )
        .unwrap();
        assert_eq!(settings.steel_turn_planning.validate(), Err(SteelTurnPlanningConfigError::ReceiptOutsideTarget));
    }

    #[test]
    fn steel_turn_planning_validation_rejects_blank_authority_grant() {
        let settings: Settings = serde_json::from_str(
            r#"{"steelTurnPlanning":{"enabled":true,"profilePath":"profile.json","scriptPath":"script.scm","ucanAuthorityGrants":[{"resource":"turn:session-fixture","ability":" ","audience":"clankers-runtime"}]}}"#,
        )
        .unwrap();
        assert_eq!(settings.steel_turn_planning.validate(), Err(SteelTurnPlanningConfigError::BlankUcanAuthorityGrant));
    }

    #[test]
    fn browser_automation_from_json() {
        let json = r#"{
            "browserAutomation": {
                "enabled": true,
                "backend": "cdp",
                "cdpUrl": "http://127.0.0.1:9222",
                "headless": false,
                "allowEvaluate": true,
                "allowScreenshots": false,
                "timeoutMs": 15000,
                "allowedOrigins": ["https://example.test", "http://localhost:*"]
            }
        }"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        let browser = settings.browser_automation;
        assert!(browser.enabled);
        assert_eq!(browser.backend, BrowserAutomationBackend::Cdp);
        assert_eq!(browser.cdp_url.as_deref(), Some("http://127.0.0.1:9222"));
        assert!(!browser.headless);
        assert!(browser.allow_evaluate);
        assert!(!browser.allow_screenshots);
        assert_eq!(browser.timeout_ms, Some(15000));
        browser.validate().expect("browser automation config valid");
        assert!(browser.permits_origin("https://example.test"));
        assert!(browser.permits_origin("http://localhost:3000"));
        assert!(!browser.permits_origin("https://evil.test"));
    }

    #[test]
    fn browser_automation_snake_case_alias_from_json() {
        let settings: Settings =
            serde_json::from_str(r#"{"browser_automation":{"enabled":true,"cdpUrl":"http://127.0.0.1:9222"}}"#)
                .unwrap();
        assert!(settings.browser_automation.enabled);
        assert_eq!(settings.browser_automation.cdp_url.as_deref(), Some("http://127.0.0.1:9222"));
    }

    #[test]
    fn browser_automation_defaults_disabled() {
        let settings: Settings = serde_json::from_str("{}").unwrap();
        assert!(!settings.browser_automation.enabled);
        assert_eq!(settings.browser_automation.backend, BrowserAutomationBackend::Cdp);
        assert!(settings.browser_automation.headless);
        assert!(!settings.browser_automation.allow_evaluate);
        assert!(settings.browser_automation.allow_screenshots);
        settings.browser_automation.validate().expect("disabled config is valid");
    }

    #[test]
    fn browser_automation_validation_rejects_enabled_without_endpoint() {
        let settings: Settings = serde_json::from_str(r#"{"browserAutomation":{"enabled":true}}"#).unwrap();
        assert_eq!(
            settings.browser_automation.validate(),
            Err(BrowserAutomationConfigError::MissingCdpEndpointOrBrowserBinary)
        );
    }

    #[test]
    fn browser_automation_config_deep_merges() {
        let global = serde_json::json!({
            "browserAutomation": {
                "enabled": true,
                "cdpUrl": "http://127.0.0.1:9222",
                "allowEvaluate": true,
                "allowedOrigins": ["https://example.test"]
            }
        });
        let project = serde_json::json!({
            "browserAutomation": {
                "timeoutMs": 20000,
                "allowedOrigins": ["https://project.test"]
            }
        });
        let settings = Settings::merge_layers(None, Some(global), Some(project));
        assert!(settings.browser_automation.enabled);
        assert_eq!(settings.browser_automation.cdp_url.as_deref(), Some("http://127.0.0.1:9222"));
        assert!(settings.browser_automation.allow_evaluate);
        assert_eq!(settings.browser_automation.timeout_ms, Some(20000));
        assert_eq!(settings.browser_automation.allowed_origins, vec!["https://project.test".to_string()]);
    }

    #[test]
    fn external_memory_defaults_disabled() {
        let settings: Settings = serde_json::from_str(r"{}").unwrap();
        assert!(!settings.external_memory.enabled);
        assert_eq!(settings.external_memory.provider, ExternalMemoryProvider::Local);
        assert_eq!(settings.external_memory.max_results, 8);
        assert!(!settings.external_memory.inject_into_prompt);
        settings.external_memory.validate().expect("disabled config is valid");
    }

    #[test]
    fn external_memory_local_provider_from_json() {
        let json = r#"{
            "externalMemory": {
                "enabled": true,
                "provider": "local",
                "name": "project-memory",
                "timeoutMs": 30000,
                "maxResults": 5,
                "injectIntoPrompt": true
            }
        }"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert!(settings.external_memory.enabled);
        assert_eq!(settings.external_memory.provider, ExternalMemoryProvider::Local);
        assert_eq!(settings.external_memory.safe_provider_name(), "project-memory");
        assert_eq!(settings.external_memory.timeout_ms, Some(30000));
        assert_eq!(settings.external_memory.max_results, 5);
        assert!(settings.external_memory.inject_into_prompt);
        settings.external_memory.validate().expect("local external memory config valid");
    }

    #[test]
    fn external_memory_validation_rejects_blank_policy_fields() {
        let json = r#"{
            "externalMemory": {
                "enabled": true,
                "name": " ",
                "credentialEnv": "EXTERNAL_MEMORY_TOKEN"
            }
        }"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.external_memory.validate(), Err(ExternalMemoryConfigError::BlankName));

        let json = r#"{"externalMemory":{"enabled":true,"credentialEnv":""}}"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.external_memory.validate(), Err(ExternalMemoryConfigError::BlankCredentialEnv));
    }

    #[test]
    fn external_memory_http_requires_endpoint_and_credential_env() {
        let missing_endpoint = r#"{"externalMemory":{"enabled":true,"provider":"http"}}"#;
        let settings: Settings = serde_json::from_str(missing_endpoint).unwrap();
        assert_eq!(settings.external_memory.validate(), Err(ExternalMemoryConfigError::MissingHttpEndpoint));

        let missing_credential = r#"{
            "externalMemory": {
                "enabled": true,
                "provider": "http",
                "endpoint": "https://memory.example.test/search"
            }
        }"#;
        let settings: Settings = serde_json::from_str(missing_credential).unwrap();
        assert_eq!(settings.external_memory.validate(), Err(ExternalMemoryConfigError::MissingCredentialEnv));

        let configured = r#"{
            "externalMemory": {
                "enabled": true,
                "provider": "http",
                "endpoint": "https://memory.example.test/search",
                "credentialEnv": "EXTERNAL_MEMORY_TOKEN"
            }
        }"#;
        let settings: Settings = serde_json::from_str(configured).unwrap();
        settings
            .external_memory
            .validate()
            .expect("HTTP config shape is valid before runtime credential lookup");
    }

    #[test]
    fn external_memory_project_deep_merges_global_config() {
        let global = serde_json::json!({
            "externalMemory": {
                "enabled": true,
                "provider": "local",
                "name": "global-memory",
                "maxResults": 8
            }
        });
        let project = serde_json::json!({
            "externalMemory": {
                "name": "project-memory",
                "injectIntoPrompt": true
            }
        });
        let settings = Settings::merge_layers(None, Some(global), Some(project));
        assert!(settings.external_memory.enabled);
        assert_eq!(settings.external_memory.provider, ExternalMemoryProvider::Local);
        assert_eq!(settings.external_memory.safe_provider_name(), "project-memory");
        assert_eq!(settings.external_memory.max_results, 8);
        assert!(settings.external_memory.inject_into_prompt);
    }

    #[test]
    fn mcp_stdio_server_from_json() {
        let json = r#"{
            "mcp": {
                "servers": {
                    "filesystem": {
                        "transport": "stdio",
                        "command": "npx",
                        "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"],
                        "envAllowlist": ["MCP_TOKEN"],
                        "includeTools": ["read_file", "write_file"],
                        "excludeTools": ["write_file"],
                        "toolPrefix": "fs",
                        "timeoutMs": 30000
                    }
                }
            }
        }"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        let server = settings.mcp.servers.get("filesystem").expect("server loaded");
        assert!(server.enabled);
        assert_eq!(server.transport, McpTransport::Stdio);
        assert_eq!(server.command.as_deref(), Some("npx"));
        assert_eq!(server.args.len(), 3);
        assert_eq!(server.env_allowlist, vec!["MCP_TOKEN".to_string()]);
        assert_eq!(server.timeout_ms, Some(30000));
        server.validate().expect("stdio server config valid");
        assert!(server.publishes_tool("read_file"));
        assert!(!server.publishes_tool("write_file"));
        assert_eq!(server.published_tool_name("filesystem", "read_file"), "fs_read_file");
    }

    #[test]
    fn mcp_http_server_from_json() {
        let json = r#"{
            "mcp": {
                "servers": {
                    "search": {
                        "transport": "http",
                        "url": "https://mcp.example.test/rpc",
                        "headerEnv": {"Authorization": "MCP_AUTH_HEADER"}
                    }
                }
            }
        }"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        let server = settings.mcp.servers.get("search").expect("server loaded");
        assert_eq!(server.transport, McpTransport::Http);
        assert_eq!(server.url.as_deref(), Some("https://mcp.example.test/rpc"));
        assert_eq!(server.header_env.get("Authorization").map(String::as_str), Some("MCP_AUTH_HEADER"));
        server.validate().expect("http server config valid");
        assert_eq!(server.published_tool_name("search", "query"), "mcp_search_query");
    }

    #[test]
    fn mcp_config_deep_merges_servers_by_name() {
        let global = serde_json::json!({
            "mcp": {
                "servers": {
                    "filesystem": {
                        "transport": "stdio",
                        "command": "npx",
                        "includeTools": ["read_file"]
                    }
                }
            }
        });
        let project = serde_json::json!({
            "mcp": {
                "servers": {
                    "filesystem": {
                        "toolPrefix": "fs"
                    },
                    "search": {
                        "transport": "http",
                        "url": "https://mcp.example.test/rpc"
                    }
                }
            }
        });
        let settings = Settings::merge_layers(None, Some(global), Some(project));
        let filesystem = settings.mcp.servers.get("filesystem").expect("filesystem server loaded");
        assert_eq!(filesystem.command.as_deref(), Some("npx"));
        assert_eq!(filesystem.tool_prefix.as_deref(), Some("fs"));
        assert!(settings.mcp.servers.contains_key("search"));
    }

    #[test]
    fn mcp_validation_rejects_missing_stdio_command() {
        let json = r#"{"mcp":{"servers":{"bad":{"transport":"stdio"}}}}"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        let server = settings.mcp.servers.get("bad").expect("server loaded");
        assert_eq!(server.validate(), Err(McpServerConfigError::MissingStdioCommand));
    }

    #[test]
    fn mcp_validation_rejects_blank_header_env() {
        let json = r#"{
            "mcp": {
                "servers": {
                    "bad": {
                        "transport": "http",
                        "command": "mcp-server",
                        "url": "https://mcp.example.test/rpc",
                        "headerEnv": {"Authorization": ""}
                    }
                }
            }
        }"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        let server = settings.mcp.servers.get("bad").expect("server loaded");
        assert_eq!(
            server.validate(),
            Err(McpServerConfigError::HeaderWithoutEnvName {
                header: "Authorization".to_string()
            })
        );
    }

    #[test]
    fn mcp_validation_rejects_blank_env_allowlist_entry() {
        let json = r#"{
            "mcp": {
                "servers": {
                    "bad": {"transport": "stdio", "command": "server", "envAllowlist": [""]}
                }
            }
        }"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        let server = settings.mcp.servers.get("bad").expect("server loaded");
        assert_eq!(server.validate(), Err(McpServerConfigError::BlankEnvAllowlistEntry));
    }

    #[test]
    fn mcp_validation_rejects_http_command() {
        let json = r#"{
            "mcp": {
                "servers": {
                    "bad": {"transport": "http", "command": "server", "url": "https://mcp.example.test/rpc"}
                }
            }
        }"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        let server = settings.mcp.servers.get("bad").expect("server loaded");
        assert_eq!(server.validate(), Err(McpServerConfigError::UnexpectedStdioCommand));
    }

    #[test]
    fn disabled_tools_project_overrides_global() {
        let global = serde_json::json!({"disabledTools": ["bash"]});
        let project = serde_json::json!({"disabledTools": ["commit", "review"]});
        let settings = Settings::merge_layers(None, Some(global), Some(project));
        // Project replaces global (field-level merge, not array merge)
        assert_eq!(settings.disabled_tools, vec!["commit".to_string(), "review".to_string()]);
    }

    #[test]
    fn auto_test_command_from_json() {
        let json = r#"{"autoTestCommand": "cargo nextest run"}"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.auto_test_command, Some("cargo nextest run".to_string()));
    }

    #[test]
    fn auto_test_command_default_none() {
        let json = r"{}";
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert!(settings.auto_test_command.is_none());
    }

    #[test]
    fn auto_test_command_project_overrides_global() {
        let global = serde_json::json!({"autoTestCommand": "cargo test"});
        let project = serde_json::json!({"autoTestCommand": "cargo nextest run"});
        let settings = Settings::merge_layers(None, Some(global), Some(project));
        assert_eq!(settings.auto_test_command, Some("cargo nextest run".to_string()));
    }

    #[test]
    fn thinking_level_default_max() {
        let json = r"{}";
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.parsed_thinking_level(), ThinkingLevel::Max);
    }

    #[test]
    fn thinking_level_xhigh_aliases_max() {
        let json = r#"{"thinkingLevel": "xhigh"}"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.parsed_thinking_level(), ThinkingLevel::Max);
    }

    #[test]
    fn thinking_level_explicit_off() {
        let json = r#"{"thinkingLevel": "off"}"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.parsed_thinking_level(), ThinkingLevel::Off);
    }

    #[test]
    fn use_daemon_default_true() {
        let json = r"{}";
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert!(settings.use_daemon);
    }

    #[test]
    fn use_daemon_explicit_false() {
        let json = r#"{"useDaemon": false}"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert!(!settings.use_daemon);
    }

    #[test]
    fn use_daemon_project_overrides_global() {
        let global = serde_json::json!({"useDaemon": true});
        let project = serde_json::json!({"useDaemon": false});
        let settings = Settings::merge_layers(None, Some(global), Some(project));
        assert!(!settings.use_daemon);
    }

    #[test]
    fn scrollback_on_exit_default_none() {
        let json = r"{}";
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.scrollback_on_exit, None);
    }

    #[test]
    fn scrollback_on_exit_snake_case_false() {
        let json = r#"{"scrollback_on_exit": false}"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.scrollback_on_exit, Some(false));
    }

    #[test]
    fn scrollback_on_exit_camel_case_true() {
        let json = r#"{"scrollbackOnExit": true}"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.scrollback_on_exit, Some(true));
    }

    #[test]
    fn scrollback_on_exit_project_overrides_global() {
        let global = serde_json::json!({"scrollbackOnExit": false});
        let project = serde_json::json!({"scrollbackOnExit": true});
        let settings = Settings::merge_layers(None, Some(global), Some(project));
        assert_eq!(settings.scrollback_on_exit, Some(true));
    }

    // ── Deep merge tests ───────────────────────────────────────────

    #[test]
    fn deep_merge_nested_object_partial_override() {
        let global = serde_json::json!({
            "hooks": {
                "enabled": true,
                "scriptTimeoutSecs": 10
            }
        });
        let project = serde_json::json!({
            "hooks": {
                "disabledHooks": ["pre-tool"]
            }
        });
        let settings = Settings::merge_layers(None, Some(global), Some(project));
        assert!(settings.hooks.enabled);
        assert_eq!(settings.hooks.script_timeout_secs, 10);
        assert_eq!(settings.hooks.disabled_hooks, vec!["pre-tool".to_string()]);
    }

    #[test]
    fn deep_merge_scalar_override_within_nested_object() {
        let global = serde_json::json!({
            "memory": {"globalCharLimit": 2200}
        });
        let project = serde_json::json!({
            "memory": {"globalCharLimit": 4400}
        });
        let settings = Settings::merge_layers(None, Some(global), Some(project));
        assert_eq!(settings.memory.global_char_limit, 4400);
    }

    #[test]
    fn deep_merge_array_fields_replaced_not_merged() {
        let global = serde_json::json!({"disabledTools": ["bash"]});
        let project = serde_json::json!({"disabledTools": ["commit"]});
        let settings = Settings::merge_layers(None, Some(global), Some(project));
        assert_eq!(settings.disabled_tools, vec!["commit".to_string()]);
    }

    #[test]
    fn pi_default_provider_prefixes_codex_model() {
        let pi = Settings::normalize_pi_settings(serde_json::json!({
            "defaultProvider": "openai-codex",
            "defaultModel": "gpt-5.5"
        }));
        assert_eq!(pi.get("model").and_then(serde_json::Value::as_str), Some("openai-codex/gpt-5.5"));
    }

    #[test]
    fn deep_merge_three_layers() {
        let pi = serde_json::json!({
            "hooks": {"enabled": false, "scriptTimeoutSecs": 5},
            "memory": {"globalCharLimit": 1000}
        });
        let global = serde_json::json!({
            "hooks": {"enabled": true}
        });
        let project = serde_json::json!({
            "hooks": {"disabledHooks": ["pre-tool"]},
            "memory": {"projectCharLimit": 999}
        });
        let settings = Settings::merge_layers(Some(pi), Some(global), Some(project));
        // hooks.enabled: pi=false, global=true → true
        assert!(settings.hooks.enabled);
        // hooks.scriptTimeoutSecs: pi=5, not overridden → 5
        assert_eq!(settings.hooks.script_timeout_secs, 5);
        // hooks.disabledHooks: project sets it
        assert_eq!(settings.hooks.disabled_hooks, vec!["pre-tool".to_string()]);
        // memory.globalCharLimit: pi=1000, not overridden → 1000
        assert_eq!(settings.memory.global_char_limit, 1000);
        // memory.projectCharLimit: project=999
        assert_eq!(settings.memory.project_char_limit, 999);
    }
}
