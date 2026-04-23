# Clankers settings contract.
#
# Import this in your settings.ncl to get type checking, defaults, and
# deep merge:
#
#   (import "clankers://settings") & {
#     model = "claude-opus-4-6",
#     planMode = true,
#   }
#
# Or write a plain record without importing — the contract is optional.

{
  # Default model for LLM requests.
  model | String | default = "claude-sonnet-4-5",

  # Max output tokens per request.
  maxTokens | Number | default = 16384,

  # Agent scope for agent definition discovery ("user", "project", "both").
  agentScope | String | default = "user",

  # Confirm before running project-level agents.
  confirmProjectAgents | Bool | default = true,

  # Use git worktrees for sessions (writes go to hidden worktree dir).
  useWorktrees | Bool | default = false,

  # Custom system prompt prefix (prepended to default prompt).
  systemPromptPrefix | String | optional,

  # Custom system prompt suffix (appended to default prompt).
  systemPromptSuffix | String | optional,

  # Theme name.
  theme | String | optional,

  # Max output lines before truncation.
  maxOutputLines | Number | default = 2000,

  # Max output bytes before truncation.
  maxOutputBytes | Number | default = 51200,

  # Bash command timeout in seconds (0 = no timeout).
  bashTimeout | Number | default = 0,

  # Auto-launch inside Zellij when available.
  zellij | Bool | optional,

  # Enable plan mode by default.
  planMode | Bool | default = false,

  # Max subagent panes in BSP tiling (0 = disable auto-creation).
  maxSubagentPanes | Number | default = 4,

  # Tools to disable by name. Disabled tools are not registered with the agent.
  disabledTools | Array String | default = [],

  # Disable prompt caching.
  noCache | Bool | default = false,

  # Cache TTL for prompt caching ("5m" = ephemeral, "1h" = 1-hour).
  cacheTtl | String | optional,

  # Annotate nix/bash output with store path package names.
  annotateStoreRefs | Bool | default = false,

  # Auto-start daemon for interactive mode.
  useDaemon | Bool | default = true,

  # Command to run after agent turns (e.g. "cargo nextest run").
  autoTestCommand | String | optional,

  # Keymap configuration.
  keymap | {
    # Preset: "helix" or "vim".
    preset | String | default = "helix",
  } | default = {},

  # Memory capacity limits for cross-session learning.
  memory | {
    # Max chars for global-scope memories (~800 tokens).
    globalCharLimit | Number | default = 2200,
    # Max chars for per-project memories (~500 tokens).
    projectCharLimit | Number | default = 1375,
  } | default = {},

  # Context compression settings.
  compression | {
    # Model for manual compression requests (empty = cheapest available).
    model | String | optional,
    # Cheap/fast model for structured summary generation.
    summaryModel | String | default = "haiku",
    # Recent messages to keep intact during manual compression.
    keepRecent | Number | default = 4,
    # Fraction of context window reserved for recent-message tail protection.
    tailBudgetFraction | Number | default = 0.40,
    # Minimum message count before compression kicks in.
    minMessages | Number | default = 5,
  } | default = {},

  # Hook system configuration.
  hooks | {
    # Master enable/disable.
    enabled | Bool | default = true,
    # Directory for hook scripts.
    hooksDir | String | optional,
    # Hook points to disable (by kebab-case name).
    disabledHooks | Array String | default = [],
    # Timeout for script hooks in seconds.
    scriptTimeoutSecs | Number | default = 10,
    # Manage .git/hooks/ shims.
    manageGitHooks | Bool | default = false,
  } | default = {},

  # Leader menu customization.
  leaderMenu | {
    items | Array {
      key | String,
      label | String,
      command | String,
      submenu | String | optional,
    } | default = [],
    hide | Array {
      key | String,
      submenu | String | optional,
    } | default = [],
  } | default = {},
}
