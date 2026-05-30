let NonEmpty = std.contract.from_predicate (fun value => std.is_string value && std.string.length value > 0) in
let SafeName = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.is_match "^[A-Za-z0-9_.:-]+$" value
) in
let Positive = std.contract.from_predicate (fun value => std.is_number value && value > 0) in
let BoolContract = std.contract.from_predicate (fun value => std.is_bool value) in
let FallbackMode = std.contract.from_predicate (fun value =>
  value == "rust_native" || value == "block"
) in
let RolloutStage = std.contract.from_predicate (fun value =>
  value == "comparison" || value == "default" || value == "disabled"
) in
let RedactionClass = std.contract.from_predicate (fun value =>
  value == "metadata_only" || value == "public_summary"
) in
let SeamName = std.contract.from_predicate (fun value =>
  value == "steel.host.plan_turn" || value == "steel.host.execute_turn" || value == "steel.host.route_action"
) in
let HostAction = {
  name | SeamName,
  dynamic_runtime_action | SafeName,
  target_prefix | NonEmpty,
  required_session_capabilities | Array NonEmpty,
  ucan_ability | NonEmpty,
} in
let RuntimeBudget = {
  steel_profile | SafeName,
  max_source_bytes | Positive,
  max_output_bytes | Positive,
  max_host_calls | Positive,
  max_steps | Positive,
  max_plan_items | Positive,
  max_input_bytes | Positive,
} in
let ScriptBinding = {
  id | SafeName,
  source_kind | SafeName,
  path | NonEmpty,
  blake3 | NonEmpty,
} in
let ReceiptPolicy = {
  schema | NonEmpty,
  destination_prefix | NonEmpty,
  redaction | RedactionClass,
  safe_fields | Array SafeName,
  redacted_fields | Array SafeName,
} in
let OrchestrationProfile = {
  schema | NonEmpty,
  name | SafeName,
  enabled | BoolContract,
  default | BoolContract,
  planning_seam | SeamName,
  rollout_stage | RolloutStage,
  fallback_mode | FallbackMode,
  script | ScriptBinding,
  runtime_budget | RuntimeBudget,
  allowed_host_actions | Array HostAction,
  receipt_policy | ReceiptPolicy,
  audit = {
    owner | SafeName,
    review_required | BoolContract,
    expansion_requires_profile_update | BoolContract,
  },
} in
{
  schema = "clankers.steel_default_orchestration.profile.v1",
  name = "steel-plan-turn-default",
  enabled = true,
  default = true,
  planning_seam = "steel.host.plan_turn",
  rollout_stage = "default",
  fallback_mode = "rust_native",
  script = {
    id = "default-plan-turn-v1",
    source_kind = "repo-file",
    path = "policy/steel-default-orchestration/scripts/default-plan-turn.scm",
    blake3 = "b3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  },
  runtime_budget = {
    steel_profile = "trusted-orchestration",
    max_source_bytes = 4096,
    max_output_bytes = 2048,
    max_host_calls = 4,
    max_steps = 512,
    max_plan_items = 8,
    max_input_bytes = 8192,
  },
  allowed_host_actions = [
    {
      name = "steel.host.plan_turn",
      dynamic_runtime_action = "host_function:steel.host.plan_turn",
      target_prefix = "turn:",
      required_session_capabilities = ["steel-orchestration", "turn-planning"],
      ucan_ability = "clankers/steel/orchestrate.plan_turn",
    },
    {
      name = "steel.host.execute_turn",
      dynamic_runtime_action = "host_function:steel.host.execute_turn",
      target_prefix = "session:",
      required_session_capabilities = ["steel-orchestration", "turn-execution"],
      ucan_ability = "clankers/steel/orchestrate.execute_turn",
    },
  ],
  receipt_policy = {
    schema = "clankers.steel_default_orchestration.receipt.v1",
    destination_prefix = "target/steel-default-orchestration",
    redaction = "metadata_only",
    safe_fields = ["schema", "status", "seam", "profile", "script_hash", "policy_hash", "plan_hash", "fallback_status", "receipt_hash"],
    redacted_fields = ["raw_prompt", "provider_payload", "compact_ucan", "raw_proof", "credential", "script_source", "tool_body", "absolute_path"],
  },
  audit = {
    owner = "rust-host",
    review_required = true,
    expansion_requires_profile_update = true,
  },
} | OrchestrationProfile
