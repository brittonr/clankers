# Author-time Nickel contract and default policy for Steel-requested live mutation.
# Runtime enforcement consumes exported typed data; generic SDK/engine crates must not
# evaluate Nickel per mutation call.

let NonEmptyString = String in
let BoolFlag = Bool in
let TargetClass = {
  name | NonEmptyString,
  resource_prefix | NonEmptyString,
  allowed_path_roots | Array NonEmptyString,
  denied_path_patterns | Array NonEmptyString,
  allowed_verbs | Array NonEmptyString,
  approval_tier | NonEmptyString,
  preflight_profile | NonEmptyString,
  verification_profile | NonEmptyString,
  rollback_required | BoolFlag,
} in
let MutationVerb = {
  name | NonEmptyString,
  host_function | NonEmptyString,
  ucan_ability | NonEmptyString,
  requires_approval | BoolFlag,
  writes_bytes | BoolFlag,
} in
let RuntimeProfile = {
  name | NonEmptyString,
  steel_profile | NonEmptyString,
  max_source_bytes | Number,
  max_output_bytes | Number,
  max_host_calls | Number,
  ambient_authority | BoolFlag,
} in
let ReceiptPolicy = {
  schema | NonEmptyString,
  include_policy_hash | BoolFlag,
  include_safe_ucan_metadata | BoolFlag,
  redact_fields | Array NonEmptyString,
  forbidden_receipt_markers | Array NonEmptyString,
} in
{
  schema = "clankers.steel_self_mutation.policy.v1",
  target_classes | Array TargetClass = [
    {
      name = "skill",
      resource_prefix = "skill:",
      allowed_path_roots = ["skills/", ".clankers/skills/", "crates/clankers-skills/fixtures/"],
      denied_path_patterns = ["../", "/.git/", "**/.env*", "**/*secret*"],
      allowed_verbs = ["propose_mutation", "apply_mutation", "rollback_mutation"],
      approval_tier = "human-review",
      preflight_profile = "clean-or-checkpoint",
      verification_profile = "skill-frontmatter-and-smoke",
      rollback_required = true,
    },
    {
      name = "prompt",
      resource_prefix = "prompt:",
      allowed_path_roots = ["prompts/", "crates/clankers-prompts/"],
      denied_path_patterns = ["../", "/.git/", "**/.env*", "**/*secret*"],
      allowed_verbs = ["propose_mutation", "apply_mutation", "rollback_mutation"],
      approval_tier = "human-review",
      preflight_profile = "clean-or-checkpoint",
      verification_profile = "prompt-schema-and-smoke",
      rollback_required = true,
    },
    {
      name = "tool_description",
      resource_prefix = "tool:",
      allowed_path_roots = ["crates/clankers-tool-host/", "crates/clankers-runtime/fixtures/"],
      denied_path_patterns = ["../", "/.git/", "**/.env*", "**/*secret*"],
      allowed_verbs = ["propose_mutation", "apply_mutation", "rollback_mutation"],
      approval_tier = "human-review",
      preflight_profile = "clean-or-checkpoint",
      verification_profile = "tool-schema-and-smoke",
      rollback_required = true,
    },
    {
      name = "repo_code",
      resource_prefix = "repo:",
      allowed_path_roots = ["src/", "crates/", "scripts/", "policy/"],
      denied_path_patterns = ["../", "/.git/", "**/.env*", "**/*secret*", "target/", "result"],
      allowed_verbs = ["propose_mutation", "apply_mutation", "commit_mutation", "rollback_mutation"],
      approval_tier = "explicit-code-approval",
      preflight_profile = "clean-git-or-preserve-wip",
      verification_profile = "focused-rust-and-cairn-gates",
      rollback_required = true,
    },
    {
      name = "orchestration_pack",
      resource_prefix = "steel-pack:",
      allowed_path_roots = [".clankers/steel/"],
      denied_path_patterns = ["../", "/.git/", "**/.env*", "**/*secret*", "target/", "result"],
      allowed_verbs = ["propose_mutation", "apply_mutation", "rollback_mutation"],
      approval_tier = "human-oracle-checkpoint",
      preflight_profile = "isolated-steel-pack-staging",
      verification_profile = "steel-pack-validation-gates",
      rollback_required = true,
    },
  ],
  mutation_verbs | Array MutationVerb = [
    {
      name = "propose_mutation",
      host_function = "steel.host.propose_mutation",
      ucan_ability = "clankers/steel/mutation.propose",
      requires_approval = false,
      writes_bytes = false,
    },
    {
      name = "apply_mutation",
      host_function = "steel.host.apply_mutation",
      ucan_ability = "clankers/steel/mutation.apply",
      requires_approval = true,
      writes_bytes = true,
    },
    {
      name = "commit_mutation",
      host_function = "steel.host.commit_mutation",
      ucan_ability = "clankers/steel/mutation.commit",
      requires_approval = true,
      writes_bytes = false,
    },
    {
      name = "rollback_mutation",
      host_function = "steel.host.rollback_mutation",
      ucan_ability = "clankers/steel/mutation.rollback",
      requires_approval = true,
      writes_bytes = true,
    },
  ],
  runtime_profiles | Array RuntimeProfile = [
    {
      name = "steel-live-mutation-default-deny",
      steel_profile = "default",
      max_source_bytes = 32768,
      max_output_bytes = 16384,
      max_host_calls = 0,
      ambient_authority = false,
    },
    {
      name = "steel-live-mutation-requester",
      steel_profile = "mutation-requester",
      max_source_bytes = 65536,
      max_output_bytes = 32768,
      max_host_calls = 16,
      ambient_authority = false,
    },
  ],
  ucan = {
    required = true,
    audience_binding = "session-or-daemon-instance",
    deny_wildcard_resources = true,
    max_delegation_depth = 1,
    safe_receipt_fields = ["ability", "resource", "issuer_fingerprint", "audience_fingerprint", "expiry_status", "delegation_depth", "authorization_outcome"],
  },
  receipt | ReceiptPolicy = {
    schema = "clankers.steel_self_mutation.receipt.v1",
    include_policy_hash = true,
    include_safe_ucan_metadata = true,
    redact_fields = ["patch_body", "source", "raw_diagnostics", "absolute_paths", "provider_payloads", "credentials"],
    forbidden_receipt_markers = ["ucan_compact_token", "private_key", "bearer_token", "api_key", "raw_proof", "SECRET", "PASSWORD"],
  },
}
