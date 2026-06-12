//! Repo-local Steel evolution pack validation and typed plan receipts.
//!
//! Steel pack scripts remain policy/planning data. Rust owns discovery,
//! validation, host-call authorization, receipts, and every host-visible effect.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

pub use clanker_message::SteelRepoEvolutionActivationReason;
pub use clanker_message::SteelRepoEvolutionActivationStatus;
pub use clanker_message::SteelRepoEvolutionFallbackMode;
pub use clanker_message::SteelRepoEvolutionPlanReason;
pub use clanker_message::SteelRepoEvolutionPlanStatus;
use clankers_artifacts::ArtifactHash;
use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;

pub const STEEL_REPO_EVOLUTION_PACK_SCHEMA: &str = "clankers.steel.repo_evolution_pack.v1";
pub const STEEL_REPO_EVOLUTION_ACTIVATION_RECEIPT_SCHEMA: &str =
    "clankers.steel.repo_evolution_pack.activation_receipt.v1";
pub const STEEL_REPO_EVOLUTION_PLAN_SCHEMA: &str = "clankers.steel.evolution-plan.v1";
pub const STEEL_REPO_EVOLUTION_PLAN_RECEIPT_SCHEMA: &str = "clankers.steel.repo_evolution_pack.plan_receipt.v1";
pub const STEEL_REPO_EVOLUTION_ABI_VERSION: &str = "clankers.steel.repo-host-abi.v1";
pub const STEEL_REPO_EVOLUTION_PACK_ROOT: &str = ".clankers/steel";
pub const STEEL_REPO_EVOLUTION_NICKEL_PROFILE: &str = ".clankers/steel/evolution-profile.ncl";
pub const STEEL_REPO_EVOLUTION_EXPORTED_PROFILE: &str = ".clankers/steel/evolution-profile.json";
const MIN_ALLOWED_SCRIPT_BYTES: u64 = 1;
const MIN_ALLOWED_HOST_CALL_BUDGET: u64 = 1;
const DEFAULT_DENIED_HASH: &str = "b3:0000000000000000000000000000000000000000000000000000000000000000";
const CONTRACT_MODE_HIGHER_ORDER: &str = "higher_order";

const REQUIRED_NICKEL_CONTRACT_MARKERS: &[&str] = &[
    "let ScriptBinding",
    "let HostContract",
    "let Budgets",
    "let RepoEvolutionPack",
    "allowed_host_calls | Array String",
    "host_contracts | Array HostContract",
    "receipt_root | String",
    "fallback_mode | String",
    "| RepoEvolutionPack",
];

pub const STEEL_REPO_EVOLUTION_HOST_CALLS: &[&str] = &[
    "repo.read_context",
    "repo.propose_patch",
    "repo.run_gate",
    "repo.record_receipt",
    "repo.ask_human",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelRepoEvolutionPack {
    pub schema: String,
    pub name: String,
    pub abi_version: String,
    pub scripts: Vec<SteelRepoEvolutionScriptBinding>,
    pub allowed_host_calls: Vec<String>,
    pub host_contracts: Vec<SteelRepoEvolutionHostContract>,
    pub budgets: SteelRepoEvolutionBudgets,
    pub gates: Vec<String>,
    pub receipt_root: String,
    pub fallback_mode: SteelRepoEvolutionFallbackMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelRepoEvolutionScriptBinding {
    pub id: String,
    pub path: String,
    pub blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelRepoEvolutionHostContract {
    pub name: String,
    pub wraps_host_call: String,
    pub mode: String,
    pub preconditions: Vec<String>,
    pub postconditions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelRepoEvolutionBudgets {
    pub max_source_bytes: u64,
    pub max_output_bytes: u64,
    pub max_host_calls: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelRepoEvolutionActivationReceipt {
    pub schema: String,
    pub status: SteelRepoEvolutionActivationStatus,
    pub reason_code: SteelRepoEvolutionActivationReason,
    pub safe_message: String,
    pub profile_hash: Option<ArtifactHash>,
    pub script_hashes: Vec<SteelRepoEvolutionScriptReceipt>,
    pub abi_version: Option<String>,
    pub allowed_host_calls: Vec<String>,
    pub receipt_root: Option<String>,
    pub fallback_mode: Option<SteelRepoEvolutionFallbackMode>,
}

impl SteelRepoEvolutionActivationReceipt {
    #[must_use]
    pub fn receipt_hash(&self) -> ArtifactHash {
        let bytes = crate::runtime_json_bytes(self, "repo evolution activation receipt serializes");
        ArtifactHash::digest(&bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelRepoEvolutionScriptReceipt {
    pub id: String,
    pub path: String,
    pub hash: ArtifactHash,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SteelRepoEvolutionLoadError {
    #[error("failed to read exported repo Steel evolution profile {path}: {message}")]
    ReadProfile { path: PathBuf, message: String },
    #[error("failed to read repo Steel evolution Nickel contract {path}: {message}")]
    ReadNickel { path: PathBuf, message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelRepoEvolutionPlan {
    pub schema: String,
    pub intent: String,
    pub actions: Vec<SteelRepoEvolutionAction>,
    pub gates: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelRepoEvolutionAction {
    pub host_call: String,
    pub payload_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelRepoEvolutionPlanReceipt {
    pub schema: String,
    pub status: SteelRepoEvolutionPlanStatus,
    pub reason_code: SteelRepoEvolutionPlanReason,
    pub safe_message: String,
    pub plan_hash: Option<ArtifactHash>,
    pub selected_gates: Vec<String>,
    pub requested_host_calls: Vec<String>,
    pub denied_host_calls: Vec<String>,
    pub fallback_mode: SteelRepoEvolutionFallbackMode,
}

impl SteelRepoEvolutionPlanReceipt {
    #[must_use]
    pub fn receipt_hash(&self) -> ArtifactHash {
        let bytes = crate::runtime_json_bytes(self, "repo evolution plan receipt serializes");
        ArtifactHash::digest(&bytes)
    }
}

#[must_use]
pub fn inactive_repo_evolution_receipt() -> SteelRepoEvolutionActivationReceipt {
    SteelRepoEvolutionActivationReceipt {
        schema: STEEL_REPO_EVOLUTION_ACTIVATION_RECEIPT_SCHEMA.to_string(),
        status: SteelRepoEvolutionActivationStatus::Inactive,
        reason_code: SteelRepoEvolutionActivationReason::AbsentPack,
        safe_message: "repo-local Steel evolution pack is absent".to_string(),
        profile_hash: None,
        script_hashes: Vec::new(),
        abi_version: None,
        allowed_host_calls: Vec::new(),
        receipt_root: None,
        fallback_mode: None,
    }
}

pub fn load_repo_evolution_pack(
    repo_root: &Path,
) -> Result<SteelRepoEvolutionActivationReceipt, SteelRepoEvolutionLoadError> {
    let nickel_profile = repo_root.join(STEEL_REPO_EVOLUTION_NICKEL_PROFILE);
    if !nickel_profile.is_file() {
        return Ok(inactive_repo_evolution_receipt());
    }
    let nickel_text = match fs::read_to_string(&nickel_profile) {
        Ok(text) => text,
        Err(error) => {
            return Ok(denied_activation(
                SteelRepoEvolutionActivationReason::ReadNickelContract,
                format!("repo-local Steel evolution Nickel contract could not be read: {error}"),
                None,
                Vec::new(),
                None,
            ));
        }
    };
    let exported_profile = repo_root.join(STEEL_REPO_EVOLUTION_EXPORTED_PROFILE);
    let profile_text = match fs::read_to_string(&exported_profile) {
        Ok(text) => text,
        Err(error) => {
            return Ok(denied_activation(
                SteelRepoEvolutionActivationReason::InvalidProfileJson,
                format!("repo-local Steel evolution profile export could not be read: {error}"),
                None,
                Vec::new(),
                None,
            ));
        }
    };
    let script_loader = |relative_path: &str| -> Option<Vec<u8>> { fs::read(repo_root.join(relative_path)).ok() };
    Ok(validate_repo_evolution_pack_from_sources(SteelRepoEvolutionPackSources {
        profile_text: &profile_text,
        nickel_text: &nickel_text,
        script_loader,
    }))
}

#[must_use]
pub fn validate_repo_evolution_pack_from_export(
    profile_text: &str,
    script_loader: impl FnMut(&str) -> Option<Vec<u8>>,
) -> SteelRepoEvolutionActivationReceipt {
    validate_repo_evolution_pack_from_sources(SteelRepoEvolutionPackSources {
        profile_text,
        nickel_text: valid_nickel_contract_fixture(),
        script_loader,
    })
}

/// Named inputs for validating a repo-local Steel evolution pack.
pub struct SteelRepoEvolutionPackSources<'a, ScriptLoader> {
    pub profile_text: &'a str,
    pub nickel_text: &'a str,
    pub script_loader: ScriptLoader,
}

#[must_use]
pub fn validate_repo_evolution_pack_from_sources<ScriptLoader>(
    sources: SteelRepoEvolutionPackSources<'_, ScriptLoader>,
) -> SteelRepoEvolutionActivationReceipt
where ScriptLoader: for<'path> FnMut(&'path str) -> Option<Vec<u8>> {
    let SteelRepoEvolutionPackSources {
        profile_text,
        nickel_text,
        mut script_loader,
    } = sources;
    let profile_hash = ArtifactHash::digest(profile_text.as_bytes());
    if !nickel_contract_valid(nickel_text) {
        return denied_activation(
            SteelRepoEvolutionActivationReason::InvalidNickelContract,
            "repo-local Steel evolution Nickel contract is missing required higher-order contract markers",
            Some(profile_hash),
            Vec::new(),
            None,
        );
    }
    let Ok(pack) = serde_json::from_str::<SteelRepoEvolutionPack>(profile_text) else {
        return denied_activation(
            SteelRepoEvolutionActivationReason::InvalidProfileJson,
            "repo-local Steel evolution profile export is not valid JSON",
            Some(profile_hash),
            Vec::new(),
            None,
        );
    };
    if let Some(reason) = static_pack_denial(&pack) {
        return denied_activation(
            reason,
            activation_reason_message(reason),
            Some(profile_hash),
            Vec::new(),
            Some(&pack),
        );
    }
    let mut script_receipts = Vec::new();
    for script in &pack.scripts {
        let Some(script_bytes) = script_loader(&script.path) else {
            return denied_activation(
                SteelRepoEvolutionActivationReason::MissingScript,
                "repo-local Steel evolution script is missing",
                Some(profile_hash),
                script_receipts,
                Some(&pack),
            );
        };
        let actual_hash = ArtifactHash::digest(&script_bytes);
        if script_bytes.len() as u64 > pack.budgets.max_source_bytes {
            return denied_activation(
                SteelRepoEvolutionActivationReason::ScriptTooLarge,
                "repo-local Steel evolution script exceeds profile source budget",
                Some(profile_hash),
                script_receipts,
                Some(&pack),
            );
        }
        if script.blake3 != actual_hash.prefixed() {
            return denied_activation(
                SteelRepoEvolutionActivationReason::ScriptHashMismatch,
                "repo-local Steel evolution script hash mismatch",
                Some(profile_hash),
                script_receipts,
                Some(&pack),
            );
        }
        script_receipts.push(SteelRepoEvolutionScriptReceipt {
            id: script.id.clone(),
            path: script.path.clone(),
            hash: actual_hash,
        });
    }
    active_activation(profile_hash, script_receipts, &pack)
}

#[must_use]
pub fn evaluate_repo_evolution_plan(pack: &SteelRepoEvolutionPack, plan_text: &str) -> SteelRepoEvolutionPlanReceipt {
    let plan_hash = ArtifactHash::digest(plan_text.as_bytes());
    let Ok(plan) = serde_json::from_str::<SteelRepoEvolutionPlan>(plan_text) else {
        return plan_denied(
            pack,
            SteelRepoEvolutionPlanReason::MalformedPayload,
            "repo-local Steel evolution plan is not valid JSON",
            Some(plan_hash),
            Vec::new(),
            Vec::new(),
        );
    };
    if plan.schema != STEEL_REPO_EVOLUTION_PLAN_SCHEMA {
        return plan_denied(
            pack,
            SteelRepoEvolutionPlanReason::InvalidSchema,
            "repo-local Steel evolution plan schema is unsupported",
            Some(plan_hash),
            Vec::new(),
            Vec::new(),
        );
    }
    if plan.actions.is_empty() {
        return plan_denied(
            pack,
            SteelRepoEvolutionPlanReason::EmptyActions,
            "repo-local Steel evolution plan has no actions",
            Some(plan_hash),
            plan.gates,
            Vec::new(),
        );
    }
    let contract_denials = host_calls_missing_contracts(&pack.host_contracts, &plan.actions);
    if !contract_denials.is_empty() {
        return plan_denied(
            pack,
            SteelRepoEvolutionPlanReason::UnknownHostCall,
            "repo-local Steel evolution plan requested a host call without a higher-order contract",
            Some(plan_hash),
            plan.gates,
            contract_denials,
        );
    }
    let unknown_gates = unknown_values(&plan.gates, &pack.gates);
    if !unknown_gates.is_empty() {
        return plan_denied(
            pack,
            SteelRepoEvolutionPlanReason::UnknownGate,
            "repo-local Steel evolution plan selected a gate outside the pack policy",
            Some(plan_hash),
            plan.gates,
            unknown_gates,
        );
    }
    let requested_host_calls = sorted(plan.actions.iter().map(|action| action.host_call.clone()));
    let denied_host_calls = unknown_values(&requested_host_calls, &pack.allowed_host_calls);
    if !denied_host_calls.is_empty() {
        return plan_denied(
            pack,
            SteelRepoEvolutionPlanReason::UnknownHostCall,
            "repo-local Steel evolution plan requested an unauthorized host call",
            Some(plan_hash),
            plan.gates,
            denied_host_calls,
        );
    }
    SteelRepoEvolutionPlanReceipt {
        schema: STEEL_REPO_EVOLUTION_PLAN_RECEIPT_SCHEMA.to_string(),
        status: SteelRepoEvolutionPlanStatus::Accepted,
        reason_code: SteelRepoEvolutionPlanReason::Accepted,
        safe_message: "repo-local Steel evolution plan accepted for Rust host processing".to_string(),
        plan_hash: Some(plan_hash),
        selected_gates: sorted(plan.gates),
        requested_host_calls,
        denied_host_calls: Vec::new(),
        fallback_mode: pack.fallback_mode,
    }
}

fn static_pack_denial(pack: &SteelRepoEvolutionPack) -> Option<SteelRepoEvolutionActivationReason> {
    if pack.schema != STEEL_REPO_EVOLUTION_PACK_SCHEMA {
        return Some(SteelRepoEvolutionActivationReason::InvalidSchema);
    }
    if pack.abi_version != STEEL_REPO_EVOLUTION_ABI_VERSION {
        return Some(SteelRepoEvolutionActivationReason::InvalidAbiVersion);
    }
    if pack.scripts.is_empty() {
        return Some(SteelRepoEvolutionActivationReason::EmptyScripts);
    }
    if pack.allowed_host_calls.is_empty() {
        return Some(SteelRepoEvolutionActivationReason::EmptyHostCalls);
    }
    if pack.budgets.max_source_bytes < MIN_ALLOWED_SCRIPT_BYTES
        || pack.budgets.max_output_bytes < MIN_ALLOWED_SCRIPT_BYTES
        || pack.budgets.max_host_calls < MIN_ALLOWED_HOST_CALL_BUDGET
    {
        return Some(SteelRepoEvolutionActivationReason::BudgetTooSmall);
    }
    if !receipt_root_allowed(&pack.receipt_root) {
        return Some(SteelRepoEvolutionActivationReason::ReceiptRootEscape);
    }
    if pack.scripts.iter().any(|script| !pack_path_allowed(&script.path)) {
        return Some(SteelRepoEvolutionActivationReason::PathEscape);
    }
    if pack
        .allowed_host_calls
        .iter()
        .any(|host_call| !STEEL_REPO_EVOLUTION_HOST_CALLS.contains(&host_call.as_str()))
    {
        return Some(SteelRepoEvolutionActivationReason::UnknownHostCall);
    }
    if !host_contracts_cover_allowed_calls(&pack.host_contracts, &pack.allowed_host_calls) {
        return Some(SteelRepoEvolutionActivationReason::MissingHostContract);
    }
    if !higher_order_contracts_are_safe(&pack.host_contracts) {
        return Some(SteelRepoEvolutionActivationReason::InvalidHigherOrderContract);
    }
    None
}

fn active_activation(
    profile_hash: ArtifactHash,
    script_hashes: Vec<SteelRepoEvolutionScriptReceipt>,
    pack: &SteelRepoEvolutionPack,
) -> SteelRepoEvolutionActivationReceipt {
    SteelRepoEvolutionActivationReceipt {
        schema: STEEL_REPO_EVOLUTION_ACTIVATION_RECEIPT_SCHEMA.to_string(),
        status: SteelRepoEvolutionActivationStatus::Active,
        reason_code: SteelRepoEvolutionActivationReason::Active,
        safe_message: "repo-local Steel evolution pack activated after Rust validation".to_string(),
        profile_hash: Some(profile_hash),
        script_hashes,
        abi_version: Some(pack.abi_version.clone()),
        allowed_host_calls: sorted(pack.allowed_host_calls.clone()),
        receipt_root: Some(pack.receipt_root.clone()),
        fallback_mode: Some(pack.fallback_mode),
    }
}

fn denied_activation(
    reason_code: SteelRepoEvolutionActivationReason,
    message: impl Into<String>,
    profile_hash: Option<ArtifactHash>,
    script_hashes: Vec<SteelRepoEvolutionScriptReceipt>,
    pack: Option<&SteelRepoEvolutionPack>,
) -> SteelRepoEvolutionActivationReceipt {
    SteelRepoEvolutionActivationReceipt {
        schema: STEEL_REPO_EVOLUTION_ACTIVATION_RECEIPT_SCHEMA.to_string(),
        status: SteelRepoEvolutionActivationStatus::Denied,
        reason_code,
        safe_message: message.into(),
        profile_hash,
        script_hashes,
        abi_version: pack.map(|pack| pack.abi_version.clone()),
        allowed_host_calls: pack.map(|pack| sorted(pack.allowed_host_calls.clone())).unwrap_or_default(),
        receipt_root: pack.map(|pack| pack.receipt_root.clone()),
        fallback_mode: pack.map(|pack| pack.fallback_mode),
    }
}

fn plan_denied(
    pack: &SteelRepoEvolutionPack,
    reason_code: SteelRepoEvolutionPlanReason,
    message: impl Into<String>,
    plan_hash: Option<ArtifactHash>,
    selected_gates: Vec<String>,
    denied_host_calls: Vec<String>,
) -> SteelRepoEvolutionPlanReceipt {
    let status = match pack.fallback_mode {
        SteelRepoEvolutionFallbackMode::RustNative => SteelRepoEvolutionPlanStatus::FallbackUsed,
        SteelRepoEvolutionFallbackMode::Block => SteelRepoEvolutionPlanStatus::Blocked,
    };
    SteelRepoEvolutionPlanReceipt {
        schema: STEEL_REPO_EVOLUTION_PLAN_RECEIPT_SCHEMA.to_string(),
        status,
        reason_code,
        safe_message: message.into(),
        plan_hash,
        selected_gates: sorted(selected_gates),
        requested_host_calls: Vec::new(),
        denied_host_calls: sorted(denied_host_calls),
        fallback_mode: pack.fallback_mode,
    }
}

fn activation_reason_message(reason: SteelRepoEvolutionActivationReason) -> &'static str {
    match reason {
        SteelRepoEvolutionActivationReason::InvalidSchema => "repo-local Steel evolution profile schema is unsupported",
        SteelRepoEvolutionActivationReason::InvalidAbiVersion => {
            "repo-local Steel evolution ABI version is unsupported"
        }
        SteelRepoEvolutionActivationReason::EmptyScripts => "repo-local Steel evolution profile names no scripts",
        SteelRepoEvolutionActivationReason::EmptyHostCalls => "repo-local Steel evolution profile names no host calls",
        SteelRepoEvolutionActivationReason::PathEscape => "repo-local Steel evolution script path escapes pack root",
        SteelRepoEvolutionActivationReason::UnknownHostCall => {
            "repo-local Steel evolution host call is not in the Rust ABI"
        }
        SteelRepoEvolutionActivationReason::MissingHostContract => {
            "repo-local Steel evolution host call lacks a higher-order contract"
        }
        SteelRepoEvolutionActivationReason::InvalidHigherOrderContract => {
            "repo-local Steel evolution higher-order host contract is invalid"
        }
        SteelRepoEvolutionActivationReason::ReceiptRootEscape => {
            "repo-local Steel evolution receipt root escapes target/"
        }
        SteelRepoEvolutionActivationReason::BudgetTooSmall => {
            "repo-local Steel evolution budgets must be explicit and nonzero"
        }
        _ => "repo-local Steel evolution pack failed validation",
    }
}

fn nickel_contract_valid(text: &str) -> bool {
    REQUIRED_NICKEL_CONTRACT_MARKERS.iter().all(|marker| text.contains(marker))
}

fn host_contracts_cover_allowed_calls(
    contracts: &[SteelRepoEvolutionHostContract],
    allowed_host_calls: &[String],
) -> bool {
    allowed_host_calls
        .iter()
        .all(|host_call| contracts.iter().any(|contract| contract.wraps_host_call == *host_call))
}

fn higher_order_contracts_are_safe(contracts: &[SteelRepoEvolutionHostContract]) -> bool {
    !contracts.is_empty()
        && contracts.iter().all(|contract| {
            contract.mode == CONTRACT_MODE_HIGHER_ORDER
                && STEEL_REPO_EVOLUTION_HOST_CALLS.contains(&contract.wraps_host_call.as_str())
                && !contract.preconditions.is_empty()
                && !contract.postconditions.is_empty()
        })
}

fn host_calls_missing_contracts(
    contracts: &[SteelRepoEvolutionHostContract],
    actions: &[SteelRepoEvolutionAction],
) -> Vec<String> {
    actions
        .iter()
        .filter(|action| !contracts.iter().any(|contract| contract.wraps_host_call == action.host_call))
        .map(|action| action.host_call.clone())
        .collect()
}

fn valid_nickel_contract_fixture() -> &'static str {
    "let ScriptBinding = {} in let HostContract = {} in let Budgets = {} in let RepoEvolutionPack = { allowed_host_calls | Array String, host_contracts | Array HostContract, receipt_root | String, fallback_mode | String } in {} | RepoEvolutionPack"
}

fn pack_path_allowed(path: &str) -> bool {
    path == STEEL_REPO_EVOLUTION_EXPORTED_PROFILE
        || (path.starts_with(&format!("{STEEL_REPO_EVOLUTION_PACK_ROOT}/"))
            && !path.contains("..")
            && !path.contains('\\')
            && !path.contains('\0'))
}

fn receipt_root_allowed(path: &str) -> bool {
    path == "target/steel-repo-evolution-packs" || path.starts_with("target/steel-repo-evolution-packs/")
}

fn unknown_values(values: &[String], allowed: &[String]) -> Vec<String> {
    let allowed_set = allowed.iter().map(String::as_str).collect::<BTreeSet<_>>();
    values.iter().filter(|value| !allowed_set.contains(value.as_str())).cloned().collect()
}

fn sorted(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut values = values.into_iter().collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

#[must_use]
pub fn denied_hash_marker() -> &'static str {
    DEFAULT_DENIED_HASH
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCRIPT_SOURCE: &[u8] = b"(host \"repo.propose_patch\")";

    fn valid_pack() -> SteelRepoEvolutionPack {
        SteelRepoEvolutionPack {
            schema: STEEL_REPO_EVOLUTION_PACK_SCHEMA.to_string(),
            name: "test-pack".to_string(),
            abi_version: STEEL_REPO_EVOLUTION_ABI_VERSION.to_string(),
            scripts: vec![SteelRepoEvolutionScriptBinding {
                id: "plan-evolution".to_string(),
                path: ".clankers/steel/scripts/plan-evolution.scm".to_string(),
                blake3: ArtifactHash::digest(SCRIPT_SOURCE).prefixed(),
            }],
            allowed_host_calls: vec!["repo.propose_patch".to_string(), "repo.run_gate".to_string()],
            host_contracts: vec![
                SteelRepoEvolutionHostContract {
                    name: "contract-propose-patch".to_string(),
                    wraps_host_call: "repo.propose_patch".to_string(),
                    mode: CONTRACT_MODE_HIGHER_ORDER.to_string(),
                    preconditions: vec!["typed-patch-envelope".to_string()],
                    postconditions: vec!["receipt-recorded".to_string()],
                },
                SteelRepoEvolutionHostContract {
                    name: "contract-run-gate".to_string(),
                    wraps_host_call: "repo.run_gate".to_string(),
                    mode: CONTRACT_MODE_HIGHER_ORDER.to_string(),
                    preconditions: vec!["gate-allowlisted".to_string()],
                    postconditions: vec!["gate-receipt-hash".to_string()],
                },
            ],
            budgets: SteelRepoEvolutionBudgets {
                max_source_bytes: SCRIPT_SOURCE.len() as u64,
                max_output_bytes: 4096,
                max_host_calls: 4,
            },
            gates: vec!["cargo-test-runtime".to_string()],
            receipt_root: "target/steel-repo-evolution-packs".to_string(),
            fallback_mode: SteelRepoEvolutionFallbackMode::Block,
        }
    }

    fn pack_json(pack: &SteelRepoEvolutionPack) -> String {
        serde_json::to_string(pack).expect("pack serializes")
    }

    #[test]
    fn absent_pack_is_inactive() {
        let receipt = inactive_repo_evolution_receipt();
        assert_eq!(receipt.status, SteelRepoEvolutionActivationStatus::Inactive);
        assert_eq!(receipt.reason_code, SteelRepoEvolutionActivationReason::AbsentPack);
        assert!(receipt.script_hashes.is_empty());
    }

    #[test]
    fn valid_pack_activates_with_hashes_and_host_abi() {
        let pack = valid_pack();
        let receipt = validate_repo_evolution_pack_from_export(&pack_json(&pack), |_| Some(SCRIPT_SOURCE.to_vec()));
        assert_eq!(receipt.status, SteelRepoEvolutionActivationStatus::Active);
        assert_eq!(receipt.reason_code, SteelRepoEvolutionActivationReason::Active);
        assert_eq!(receipt.script_hashes[0].hash, ArtifactHash::digest(SCRIPT_SOURCE));
        assert_eq!(receipt.abi_version.as_deref(), Some(STEEL_REPO_EVOLUTION_ABI_VERSION));
        assert!(receipt.receipt_hash().prefixed().starts_with("b3:"));
    }

    #[test]
    fn invalid_nickel_and_missing_contracts_fail_closed() {
        let pack = valid_pack();
        let pack_text = pack_json(&pack);
        let invalid_nickel = validate_repo_evolution_pack_from_sources(SteelRepoEvolutionPackSources {
            profile_text: &pack_text,
            nickel_text: "let profile = {} in profile",
            script_loader: |_path: &str| Some(SCRIPT_SOURCE.to_vec()),
        });
        assert_eq!(invalid_nickel.status, SteelRepoEvolutionActivationStatus::Denied);
        assert_eq!(invalid_nickel.reason_code, SteelRepoEvolutionActivationReason::InvalidNickelContract);

        let mut missing_contract = valid_pack();
        missing_contract.host_contracts.pop();
        let missing_contract_receipt =
            validate_repo_evolution_pack_from_export(&pack_json(&missing_contract), |_| Some(SCRIPT_SOURCE.to_vec()));
        assert_eq!(missing_contract_receipt.status, SteelRepoEvolutionActivationStatus::Denied);
        assert_eq!(missing_contract_receipt.reason_code, SteelRepoEvolutionActivationReason::MissingHostContract);
    }

    #[test]
    fn invalid_packs_fail_before_script_execution() {
        let mut cases = Vec::new();
        let mut malformed = valid_pack();
        malformed.schema = "wrong".to_string();
        cases.push((malformed, SteelRepoEvolutionActivationReason::InvalidSchema));
        let mut path_escape = valid_pack();
        path_escape.scripts[0].path = "../plan.scm".to_string();
        cases.push((path_escape, SteelRepoEvolutionActivationReason::PathEscape));
        let mut unknown_host = valid_pack();
        unknown_host.allowed_host_calls = vec!["repo.raw_shell".to_string()];
        cases.push((unknown_host, SteelRepoEvolutionActivationReason::UnknownHostCall));
        let mut over_budget = valid_pack();
        over_budget.budgets.max_source_bytes = MIN_ALLOWED_SCRIPT_BYTES;
        cases.push((over_budget, SteelRepoEvolutionActivationReason::ScriptTooLarge));

        for (pack, reason) in cases {
            let receipt = validate_repo_evolution_pack_from_export(&pack_json(&pack), |_| Some(SCRIPT_SOURCE.to_vec()));
            assert_eq!(receipt.status, SteelRepoEvolutionActivationStatus::Denied);
            assert_eq!(receipt.reason_code, reason);
        }
    }

    #[test]
    fn missing_and_hash_mismatched_scripts_fail_closed() {
        let pack = valid_pack();
        let missing = validate_repo_evolution_pack_from_export(&pack_json(&pack), |_| None);
        assert_eq!(missing.reason_code, SteelRepoEvolutionActivationReason::MissingScript);

        let mismatch = validate_repo_evolution_pack_from_export(&pack_json(&pack), |_| Some(b"other".to_vec()));
        assert_eq!(mismatch.reason_code, SteelRepoEvolutionActivationReason::ScriptHashMismatch);
    }

    #[test]
    fn plan_accepts_known_host_calls_and_gates() {
        let pack = valid_pack();
        let plan = serde_json::json!({
            "schema": STEEL_REPO_EVOLUTION_PLAN_SCHEMA,
            "intent": "improve repo gates",
            "actions": [{"host_call": "repo.propose_patch", "payload_hash": ArtifactHash::digest(b"patch").prefixed()}],
            "gates": ["cargo-test-runtime"]
        })
        .to_string();
        let receipt = evaluate_repo_evolution_plan(&pack, &plan);
        assert_eq!(receipt.status, SteelRepoEvolutionPlanStatus::Accepted);
        assert_eq!(receipt.reason_code, SteelRepoEvolutionPlanReason::Accepted);
        assert_eq!(receipt.requested_host_calls, vec!["repo.propose_patch".to_string()]);
    }

    #[test]
    fn plan_rejects_malformed_unknown_host_and_unknown_gate() {
        let pack = valid_pack();
        let malformed = evaluate_repo_evolution_plan(&pack, "not-json");
        assert_eq!(malformed.reason_code, SteelRepoEvolutionPlanReason::MalformedPayload);

        let unknown_host = serde_json::json!({
            "schema": STEEL_REPO_EVOLUTION_PLAN_SCHEMA,
            "intent": "bad",
            "actions": [{"host_call": "repo.raw_shell", "payload_hash": denied_hash_marker()}],
            "gates": ["cargo-test-runtime"]
        })
        .to_string();
        let host_receipt = evaluate_repo_evolution_plan(&pack, &unknown_host);
        assert_eq!(host_receipt.reason_code, SteelRepoEvolutionPlanReason::UnknownHostCall);
        assert_eq!(host_receipt.status, SteelRepoEvolutionPlanStatus::Blocked);

        let unknown_gate = serde_json::json!({
            "schema": STEEL_REPO_EVOLUTION_PLAN_SCHEMA,
            "intent": "bad",
            "actions": [{"host_call": "repo.propose_patch", "payload_hash": denied_hash_marker()}],
            "gates": ["skip-all"]
        })
        .to_string();
        let gate_receipt = evaluate_repo_evolution_plan(&pack, &unknown_gate);
        assert_eq!(gate_receipt.reason_code, SteelRepoEvolutionPlanReason::UnknownGate);
    }
}
