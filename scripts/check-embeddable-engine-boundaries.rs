#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"

[dependencies]
syn = { version = "2", features = ["full", "visit"] }
walkdir = "2"
---

use std::fs;
use std::path::{Path, PathBuf};

use syn::visit::Visit;
use syn::{ItemUse, UseTree};
use walkdir::WalkDir;

const REQUIREMENT_ID: &str = "r[embeddable-agent-engine.no-inward-display-or-protocol-leaks]";
const ENGINE_ROOTS: &[&str] = &[
    "crates/clankers-engine/src",
    "crates/clankers-engine-host/src",
];
const FORBIDDEN_CRATE_PREFIXES: &[&str] = &[
    "clankers_agent",
    "clankers_config",
    "clankers_controller",
    "clankers_matrix",
    "clankers_protocol",
    "clankers_provider",
    "clankers_runtime",
    "clankers_session",
    "clankers_tui",
    "clanker_router",
    "ratatui",
    "crossterm",
    "iroh",
    "reqwest",
    "redb",
    "tokio",
    "tokio_util",
];
const FORBIDDEN_TEXT_PATTERNS: &[&str] = &[
    "crate::modes",
    "src::modes",
    "DaemonEvent",
    "SessionCommand",
    "MatrixBridge",
    "RouterProvider",
    "CredentialManager",
    "AuthStore",
    "CompletionRequest",
];

#[derive(Debug)]
struct Finding {
    path: PathBuf,
    target: String,
    reason: String,
}

fn main() {
    let mut findings = Vec::new();
    for root in ENGINE_ROOTS {
        for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
            let path = entry.path();
            if !is_rust_source(path) {
                continue;
            }
            inspect_source(path, &mut findings);
        }
    }

    if findings.is_empty() {
        println!("embeddable engine boundary check passed for {REQUIREMENT_ID}");
        return;
    }

    eprintln!("embeddable engine boundary check failed for {REQUIREMENT_ID}:");
    for finding in findings {
        eprintln!(
            "  - {} imports/mentions {} ({})",
            finding.path.display(),
            finding.target,
            finding.reason
        );
    }
    std::process::exit(1);
}

fn is_rust_source(path: &Path) -> bool {
    path.is_file() && path.extension().is_some_and(|extension| extension == "rs")
}

fn inspect_source(path: &Path, findings: &mut Vec<Finding>) {
    let source = fs::read_to_string(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let production_source = source_before_cfg_test_module(&source);
    inspect_text_patterns(path, production_source, findings);
    let syntax = syn::parse_file(production_source).unwrap_or_else(|error| panic!("parse {}: {error}", path.display()));
    ImportVisitor { path, findings }.visit_file(&syntax);
}

fn source_before_cfg_test_module(source: &str) -> &str {
    const TEST_MODULE_MARKER: &str = "#[cfg(test)]\nmod tests";
    source.split(TEST_MODULE_MARKER).next().unwrap_or(source)
}

fn inspect_text_patterns(path: &Path, source: &str, findings: &mut Vec<Finding>) {
    for pattern in FORBIDDEN_TEXT_PATTERNS {
        if source.contains(pattern) {
            findings.push(Finding {
                path: path.to_path_buf(),
                target: (*pattern).to_string(),
                reason: "forbidden shell/display/protocol/concrete-provider surface".to_string(),
            });
        }
    }
}

struct ImportVisitor<'a> {
    path: &'a Path,
    findings: &'a mut Vec<Finding>,
}

impl Visit<'_> for ImportVisitor<'_> {
    fn visit_item_use(&mut self, item_use: &ItemUse) {
        let mut imports = Vec::new();
        flatten_use_tree(String::new(), &item_use.tree, &mut imports);
        for import in imports {
            if let Some(prefix) = FORBIDDEN_CRATE_PREFIXES
                .iter()
                .find(|prefix| import == **prefix || import.starts_with(&format!("{}::", prefix)))
            {
                self.findings.push(Finding {
                    path: self.path.to_path_buf(),
                    target: import,
                    reason: format!("forbidden reusable-engine dependency owner {prefix}"),
                });
            }
        }
        syn::visit::visit_item_use(self, item_use);
    }
}

fn flatten_use_tree(prefix: String, tree: &UseTree, imports: &mut Vec<String>) {
    match tree {
        UseTree::Path(path) => {
            let next = append_segment(&prefix, &path.ident.to_string());
            flatten_use_tree(next, &path.tree, imports);
        }
        UseTree::Name(name) => imports.push(append_segment(&prefix, &name.ident.to_string())),
        UseTree::Rename(rename) => imports.push(append_segment(&prefix, &rename.ident.to_string())),
        UseTree::Glob(_) => imports.push(format!("{prefix}::*")),
        UseTree::Group(group) => {
            for item in &group.items {
                flatten_use_tree(prefix.clone(), item, imports);
            }
        }
    }
}

fn append_segment(prefix: &str, segment: &str) -> String {
    if prefix.is_empty() {
        segment.to_string()
    } else {
        format!("{prefix}::{segment}")
    }
}
