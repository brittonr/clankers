#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"
---

use std::fs;

fn main() {
    let agent = fs::read_to_string("crates/clankers-agent/src/turn/mod.rs").expect("read agent turn source");
    let fcis = fs::read_to_string("crates/clankers-controller/tests/fcis_shell_boundaries.rs").expect("read fcis source");
    let joined = format!("{agent}\n{fcis}");
    let required = [
        "ShellAdapterParityCase",
        "MatrixEntrypoint",
        "MatrixPromptSource",
        "MatrixStoreMode",
        "MatrixConfirmationOutcome",
        "MatrixDisabledToolPolicy",
        "MatrixToolResultClass",
        "MatrixModelResultClass",
        "MatrixEventTranslation",
        "shell_adapter_parity_matrix_names_required_axes",
        "standalone_agent_shell_adapter_parity_cases_preserve_engine_inputs_and_terminal_outcomes",
        "shell_adapter_parity_matrix_evidence_is_present_and_source_bounded",
        "StandaloneAgent",
        "ControllerDaemonAdapter",
        "EmbeddedBatchAdapter",
        "HostSupplied",
        "ResumeSeed",
        "DeniedByCapabilityGate",
        "MissingTool",
        "DaemonTranslated",
        "EmbeddedSemantic",
    ];
    let missing: Vec<_> = required.iter().copied().filter(|needle| !joined.contains(needle)).collect();
    if !missing.is_empty() {
        eprintln!("shell adapter parity matrix freshness failed:");
        for item in missing { eprintln!("  - missing {item}"); }
        std::process::exit(1);
    }
}
