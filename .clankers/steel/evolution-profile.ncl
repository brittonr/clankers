# Repo-local Steel evolution profile. Rust validates exported JSON and checks this
# Nickel source keeps higher-order host contracts around every allowed host call.

let ScriptBinding = {
  id | String,
  path | String,
  blake3 | String,
} in

let HostContract = {
  name | String,
  wraps_host_call | String,
  mode | String,
  preconditions | Array String,
  postconditions | Array String,
} in

let Budgets = {
  max_source_bytes | Number,
  max_output_bytes | Number,
  max_host_calls | Number,
} in

let RepoEvolutionPack = {
  schema | String,
  name | String,
  abi_version | String,
  scripts | Array ScriptBinding,
  allowed_host_calls | Array String,
  host_contracts | Array HostContract,
  budgets | Budgets,
  gates | Array String,
  receipt_root | String,
  fallback_mode | String,
} in

let script_source_budget = 128 in
let output_budget = 4096 in
let host_call_budget = 4 in

{
  schema = "clankers.steel.repo_evolution_pack.v1",
  name = "clankers-default-repo-evolution",
  abi_version = "clankers.steel.repo-host-abi.v1",
  scripts = [
    {
      id = "plan-evolution",
      path = ".clankers/steel/scripts/plan-evolution.scm",
      blake3 = "b3:71396d73d050fa95a35cbf16b078c8546f2d63c684bfc9a46d152c1d4e857d8d",
    },
  ],
  allowed_host_calls = [
    "repo.propose_patch",
    "repo.run_gate",
    "repo.record_receipt",
    "repo.ask_human",
  ],
  host_contracts = [
    {
      name = "repo-propose-patch-contract",
      wraps_host_call = "repo.propose_patch",
      mode = "higher_order",
      preconditions = ["patch envelope is typed", "target paths stay under .clankers/steel"],
      postconditions = ["Rust writes only after gates pass", "receipt records patch hash"],
    },
    {
      name = "repo-run-gate-contract",
      wraps_host_call = "repo.run_gate",
      mode = "higher_order",
      preconditions = ["gate name is allowlisted", "gate command owns no credentials"],
      postconditions = ["gate receipt is hashed", "failed gates block promotion"],
    },
    {
      name = "repo-record-receipt-contract",
      wraps_host_call = "repo.record_receipt",
      mode = "higher_order",
      preconditions = ["receipt payload is safe to disclose", "receipt root stays under target"],
      postconditions = ["receipt hash is deterministic", "receipt omits secrets"],
    },
    {
      name = "repo-ask-human-contract",
      wraps_host_call = "repo.ask_human",
      mode = "higher_order",
      preconditions = ["question is safe and bounded", "no authority is granted by Steel alone"],
      postconditions = ["answer is advisory", "Rust records checkpoint receipt"],
    },
  ],
  budgets = {
    max_source_bytes = script_source_budget,
    max_output_bytes = output_budget,
    max_host_calls = host_call_budget,
  },
  gates = ["steel-pack-validate", "steel-pack-smoke"],
  receipt_root = "target/steel-repo-evolution-packs",
  fallback_mode = "block",
} | RepoEvolutionPack
