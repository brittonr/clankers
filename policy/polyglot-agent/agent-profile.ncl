let NonEmpty = std.contract.from_predicate (fun value => std.is_string value && std.string.length value > 0) in
let SafeName = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.is_match "^[A-Za-z0-9_.-]+$" value
) in
let Positive = std.contract.from_predicate (fun value =>
  std.is_number value && value > 0
) in
let Temperature = std.contract.from_predicate (fun value =>
  std.is_number value && value >= 0 && value <= 2
) in
let ToolMode = std.contract.from_predicate (fun value =>
  value == "host" || value == "wasm" || value == "disabled_placeholder"
) in
let JsonSchema = {
  type | NonEmpty,
  properties | {_ : Dyn} | default = {},
  required | Array String | default = [],
  additionalProperties | Bool | default = false,
} in
let PromptTemplate = {
  name | SafeName,
  template | NonEmpty,
  required_variables | Array SafeName | default = [],
} in
let ToolManifest = {
  name | SafeName,
  mode | ToolMode,
  description | NonEmpty,
  input_schema | JsonSchema,
  output_schema | JsonSchema,
  required_capabilities | Array NonEmpty | default = [],
  ucan_ability | NonEmpty | optional,
} in
let RuntimeProfile = {
  name | SafeName,
  steel_profile | SafeName,
  wasm_profile | SafeName,
  max_prompt_bytes | Positive,
  max_tool_calls | Positive,
  max_steel_host_calls | Positive,
  max_wasm_memory_bytes | Positive,
  max_wasm_fuel | Positive,
} in
let ModelProfile = {
  name | SafeName,
  provider | SafeName,
  model | NonEmpty,
  temperature | Temperature,
  max_output_tokens | Positive,
} in
let AgentProfile = {
  schema | NonEmpty,
  name | SafeName,
  persona | NonEmpty,
  prompts | Array PromptTemplate,
  model_profiles | Array ModelProfile,
  tools | Array ToolManifest,
  runtime_profiles | Array RuntimeProfile,
  default_model_profile | SafeName,
  default_runtime_profile | SafeName,
  compatibility = {
    class | SafeName,
    version | NonEmpty,
  },
  receipt_policy = {
    schema | NonEmpty,
    safe_fields | Array SafeName,
    redacted_fields | Array SafeName,
  },
} in
{
  schema = "clankers.polyglot_agent.profile.v1",
  name = "polyglot-default",
  persona = "Concise Clankers agent with Nickel-validated prompts, Rust-owned authority, Steel Scheme orchestration, Wasm tool execution, and UCAN runtime grants.",
  prompts = [
    {
      name = "system",
      template = m%"
        You are {{agent_name}}. Use only tools enabled by the Rust host. Steel orchestration may request typed host actions; Wasm tools may execute only through declared imports.
      "%,
      required_variables = ["agent_name"],
    },
    {
      name = "tool_result_summary",
      template = "Summarize tool {{tool_name}} result with status {{status}} without exposing raw secrets.",
      required_variables = ["tool_name", "status"],
    },
  ],
  model_profiles = [
    {
      name = "balanced",
      provider = "router",
      model = "default-balanced",
      temperature = 0.2,
      max_output_tokens = 4096,
    },
  ],
  tools = [
    {
      name = "steel_orchestrate",
      mode = "host",
      description = "Run trusted Steel Scheme orchestration through typed Rust host functions.",
      input_schema = {
        type = "object",
        properties = {
          script_id = { type = "string" },
          state = { type = "object" },
        },
        required = ["script_id", "state"],
        additionalProperties = false,
      },
      output_schema = {
        type = "object",
        properties = { receipt = { type = "object" } },
        required = ["receipt"],
        additionalProperties = false,
      },
      required_capabilities = ["steel-orchestration"],
      ucan_ability = "clankers/steel/orchestrate.run",
    },
    {
      name = "wasm_tool_execute",
      mode = "wasm",
      description = "Execute a capability-limited Wasm tool with explicit host imports.",
      input_schema = {
        type = "object",
        properties = {
          plugin = { type = "string" },
          payload = { type = "object" },
        },
        required = ["plugin", "payload"],
        additionalProperties = false,
      },
      output_schema = {
        type = "object",
        properties = { receipt = { type = "object" } },
        required = ["receipt"],
        additionalProperties = false,
      },
      required_capabilities = ["wasm-tool-execution"],
      ucan_ability = "clankers/wasm/tool.execute",
    },
  ],
  runtime_profiles = [
    {
      name = "default",
      steel_profile = "trusted-orchestration",
      wasm_profile = "capability-limited-tool",
      max_prompt_bytes = 65536,
      max_tool_calls = 64,
      max_steel_host_calls = 32,
      max_wasm_memory_bytes = 67108864,
      max_wasm_fuel = 10000000,
    },
  ],
  default_model_profile = "balanced",
  default_runtime_profile = "default",
  compatibility = {
    class = "current",
    version = "1",
  },
  receipt_policy = {
    schema = "clankers.polyglot_agent.receipt.v1",
    safe_fields = ["schema", "status", "profile", "tool", "policy_hash", "receipt_hash", "ucan_ability", "resource", "issue_code"],
    redacted_fields = ["raw_prompt", "provider_payload", "compact_ucan", "raw_proof", "credential", "tool_body", "absolute_path"],
  },
} | AgentProfile
