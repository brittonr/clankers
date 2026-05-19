# Author-time Nickel contract sketch for embedded lego composition.
# This file is intentionally not consumed by generic SDK crates at runtime.
# The checked export for current tooling lives beside it as lego-contracts.json.

let NonEmptyString = String in
let BoundaryPolicy = {
  green | Array NonEmptyString,
  yellow | Array NonEmptyString,
  red | Array NonEmptyString,
} in
let CapabilityPack = {
  name | NonEmptyString,
  capabilities | Array NonEmptyString,
  dangerous | Bool,
  requires_human_approval | Bool,
} in
let ToolManifest = {
  name | NonEmptyString,
  runtime | NonEmptyString,
  capabilities | Array NonEmptyString,
  approval | NonEmptyString,
  redaction | NonEmptyString,
  schema_hash_input | NonEmptyString,
} in
let ProviderFixture = {
  name | NonEmptyString,
  retryable | Bool,
  terminal | Bool,
  usage | Bool,
} in
let SessionResumeEvidence = {
  product | NonEmptyString,
  restored_context | Bool,
  missing_session_fail_closed | Bool,
  owns_storage_dto | Bool,
} in
let RuntimeDispatch = {
  kind | NonEmptyString,
  loader | NonEmptyString,
  forbidden_loaders | Array NonEmptyString,
  manifest_requires | Array NonEmptyString,
  dispatch_owner | NonEmptyString,
  policy_checked_before_dispatch | Bool,
} in
{
  schema = "clankers.embedded_lego.contracts.v1",
  crate_boundaries | BoundaryPolicy,
  capability_packs | Array CapabilityPack,
  capability_pack_composition_fixture | NonEmptyString,
  tool_catalog | Array ToolManifest,
  product_dogfood = {
    manifest | NonEmptyString,
    allowed_green_deps | Array NonEmptyString,
    receipt_evidence | Array NonEmptyString,
  },
  provider_adapter_fixtures | Array ProviderFixture,
  session_resume_evidence | Array SessionResumeEvidence,
  plugin_runtime_dispatch_fixture | NonEmptyString,
  runtime_dispatch_matrix | Array RuntimeDispatch,
}
