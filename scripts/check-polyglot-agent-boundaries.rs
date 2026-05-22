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
use std::process::ExitCode;

use syn::visit::Visit;
use syn::{ItemUse, UseTree};
use walkdir::WalkDir;

const ERROR_EXIT: u8 = 1;
const REQUIREMENT_ID: &str = "r[polyglot-agent-architecture.verification-rails.dependency-boundary]";
const GENERIC_CRATE_ROOTS: &[&str] = &[
    "crates/clankers-core",
    "crates/clankers-engine",
    "crates/clankers-engine-host",
    "crates/clankers-agent-defs",
    "crates/clanker-message",
    "crates/clankers-runtime",
];
const FORBIDDEN_DEPENDENCIES: &[&str] = &[
    "steel",
    "steel-core",
    "steel-interpreter",
    "nickel-lang",
    "nickel-lang-core",
    "wasmtime",
    "wasmer",
    "wasmi",
    "wasm3",
    "wasi-cap-std-sync",
];
const FORBIDDEN_IMPORT_PREFIXES: &[&str] = &[
    "steel",
    "steel_core",
    "steel_interpreter",
    "nickel_lang",
    "nickel_lang_core",
    "wasmtime",
    "wasmer",
    "wasmi",
    "wasm3",
    "wasi_cap_std_sync",
];

#[derive(Debug)]
struct Finding {
    path: PathBuf,
    target: String,
    reason: &'static str,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            println!("polyglot agent boundary check passed for {REQUIREMENT_ID}");
            ExitCode::SUCCESS
        }
        Err(findings) => {
            eprintln!("polyglot agent boundary check failed for {REQUIREMENT_ID}:");
            for finding in findings {
                eprintln!(
                    "  - {} references {} ({})",
                    finding.path.display(),
                    finding.target,
                    finding.reason
                );
            }
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<(), Vec<Finding>> {
    let mut findings = Vec::new();
    for root in GENERIC_CRATE_ROOTS {
        let root_path = Path::new(root);
        inspect_manifest(root_path, &mut findings);
        inspect_sources(root_path, &mut findings);
    }
    if findings.is_empty() { Ok(()) } else { Err(findings) }
}

fn inspect_manifest(root: &Path, findings: &mut Vec<Finding>) {
    let manifest_path = root.join("Cargo.toml");
    let Ok(manifest) = fs::read_to_string(&manifest_path) else {
        return;
    };
    for dependency in FORBIDDEN_DEPENDENCIES {
        if manifest_declares_dependency(&manifest, dependency) {
            findings.push(Finding {
                path: manifest_path.clone(),
                target: (*dependency).to_string(),
                reason: "generic crate must not depend directly on Steel, live Nickel, or Wasm runtime internals",
            });
        }
    }
}

fn manifest_declares_dependency(manifest: &str, dependency: &str) -> bool {
    let bare = format!("{dependency} =");
    let quoted = format!("\"{dependency}\"");
    manifest.lines().any(|line| {
        let trimmed = line.trim_start();
        !trimmed.starts_with('#') && (trimmed.starts_with(&bare) || trimmed.contains(&quoted))
    })
}

fn inspect_sources(root: &Path, findings: &mut Vec<Finding>) {
    let src = root.join("src");
    if !src.exists() {
        return;
    }
    for entry in WalkDir::new(&src).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|extension| extension == "rs") {
            inspect_rust_source(path, findings);
        }
    }
}

fn inspect_rust_source(path: &Path, findings: &mut Vec<Finding>) {
    let source = fs::read_to_string(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let production_source = source_before_cfg_test_module(&source);
    let syntax = syn::parse_file(production_source).unwrap_or_else(|error| panic!("parse {}: {error}", path.display()));
    ImportVisitor { path, findings }.visit_file(&syntax);
}

fn source_before_cfg_test_module(source: &str) -> &str {
    const TEST_MODULE_MARKER: &str = "#[cfg(test)]\nmod tests";
    source.split(TEST_MODULE_MARKER).next().unwrap_or(source)
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
            if let Some(prefix) = FORBIDDEN_IMPORT_PREFIXES
                .iter()
                .find(|prefix| import == **prefix || import.starts_with(&format!("{}::", prefix)))
            {
                self.findings.push(Finding {
                    path: self.path.to_path_buf(),
                    target: import,
                    reason: "generic crate must use typed DTO seams instead of direct interpreter/runtime imports",
                });
                eprintln!("blocked direct polyglot runtime import prefix {prefix}");
            }
        }
        syn::visit::visit_item_use(self, item_use);
    }
}

fn flatten_use_tree(prefix: String, tree: &UseTree, imports: &mut Vec<String>) {
    match tree {
        UseTree::Path(path) => flatten_use_tree(append_segment(&prefix, &path.ident.to_string()), &path.tree, imports),
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
    if prefix.is_empty() { segment.to_string() } else { format!("{prefix}::{segment}") }
}
