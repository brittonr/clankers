#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"
---

use std::fs;
use std::path::Path;
use std::process::ExitCode;

const ERROR_EXIT: u8 = 1;
const SELECTED_CLUSTER: &str = "process-job-profile";

struct RootCluster {
    id: &'static str,
    path: &'static str,
    owner: &'static str,
    classification: &'static str,
    required: &'static [&'static str],
    forbidden: &'static [&'static str],
}

const CLUSTERS: &[RootCluster] = &[
    RootCluster {
        id: "process-root-wiring",
        path: "src/tools/process.rs",
        owner: "root process tool registration/wiring",
        classification: "root wiring and projection",
        required: &[
            "mod native;",
            "mod pueue;",
            "mod systemd;",
            "ProcessJobProfile",
            "AdoptProcessJobRequest",
            "ProcessJobProfileReceiptMetadata",
        ],
        forbidden: &[],
    },
    RootCluster {
        id: "process-native-backend-owner",
        path: "src/tools/process/native.rs",
        owner: "native process backend adapter",
        classification: "selected reusable policy owner",
        required: &["NativeProcessJobService", "AdoptProcessJobRequest", "ProcessJobProfileReceiptMetadata::from_metadata"],
        forbidden: &[],
    },
    RootCluster {
        id: "process-pueue-backend-owner",
        path: "src/tools/process/pueue.rs",
        owner: "pueue process backend adapter",
        classification: "selected reusable policy owner",
        required: &["PueueProcessJobService", "AdoptProcessJobRequest", "ProcessJobProfileReceiptMetadata::from_metadata"],
        forbidden: &[],
    },
    RootCluster {
        id: "process-systemd-backend-owner",
        path: "src/tools/process/systemd.rs",
        owner: "systemd process backend adapter",
        classification: "selected reusable policy owner",
        required: &["SystemdProcessJobService", "AdoptProcessJobRequest", "ProcessJobProfileReceiptMetadata::from_metadata"],
        forbidden: &[],
    },
    RootCluster {
        id: "process-job-profile-rail",
        path: "scripts/check-process-job-profile-kit.rs",
        owner: "process job profile kit rail",
        classification: "owner receipt and root parity rail",
        required: &[
            "ProcessJobProfileReceiptMetadata::from_metadata",
            "src/tools/process/native.rs",
            "src/tools/process/pueue.rs",
            "src/tools/process/systemd.rs",
            "profile receipt",
        ],
        forbidden: &[],
    },
    RootCluster {
        id: "root-policy-inventory",
        path: "scripts/check-behavioral-lego-rails.rs",
        owner: "behavioral lego architecture rail",
        classification: "root owner receipt rail",
        required: &["behavioral", "lego", "process"],
        forbidden: &[],
    },
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            println!("ok: root brick extraction selected cluster `{SELECTED_CLUSTER}` covers {} owners", CLUSTERS.len());
            ExitCode::SUCCESS
        }
        Err(errors) => {
            for error in errors {
                eprintln!("root brick extraction error: {error}");
            }
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    for cluster in CLUSTERS {
        validate_source(cluster, &mut errors);
    }
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

fn validate_source(cluster: &RootCluster, errors: &mut Vec<String>) {
    let path = Path::new(cluster.path);
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            errors.push(format!("{} failed to read {}: {error}", cluster.id, cluster.path));
            return;
        }
    };
    for marker in cluster.required {
        if !source.contains(marker) {
            errors.push(format!(
                "{} ({}, {}) missing marker {:?} in {}",
                cluster.id, cluster.owner, cluster.classification, marker, cluster.path
            ));
        }
    }
    for marker in cluster.forbidden {
        if source.contains(marker) {
            errors.push(format!(
                "{} ({}, {}) contains forbidden policy marker {:?} in {}",
                cluster.id, cluster.owner, cluster.classification, marker, cluster.path
            ));
        }
    }
}
