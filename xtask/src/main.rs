use std::cmp::Reverse;
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        Some("build-plugins") => build_plugins(args.get(1).map(String::as_str)),
        Some("docs") => generate_docs(args.get(1).map(String::as_str) == Some("--open")),
        Some(cmd) => {
            eprintln!("unknown command: {cmd}");
            eprintln!();
            print_usage();
            ExitCode::FAILURE
        }
        None => {
            print_usage();
            ExitCode::FAILURE
        }
    }
}

fn print_usage() {
    eprintln!("usage: cargo xtask <command>");
    eprintln!();
    eprintln!("commands:");
    eprintln!("  build-plugins [filter]   Build all WASM plugins (or matching filter)");
    eprintln!("  docs [--open]            Generate docs and build the mdBook site");
}

fn repo_root() -> PathBuf {
    // Allow override for nix builds where CARGO_MANIFEST_DIR points into
    // the read-only store.
    if let Ok(root) = env::var("CLANKERS_ROOT") {
        return PathBuf::from(root);
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask must live one level below repo root")
        .to_path_buf()
}

fn discover_plugins(root: &Path) -> Vec<PathBuf> {
    let dirs = ["plugins", "examples/plugins"];
    let mut plugins = Vec::new();
    for dir in dirs {
        let base = root.join(dir);
        if let Ok(entries) = fs::read_dir(&base) {
            for entry in entries.flatten() {
                let manifest = entry.path().join("Cargo.toml");
                if manifest.exists() {
                    plugins.push(manifest);
                }
            }
        }
    }
    plugins.sort();
    plugins
}

fn crate_name(manifest: &Path) -> Option<String> {
    let contents = fs::read_to_string(manifest).ok()?;
    for line in contents.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("name") {
            let rest = rest.trim().strip_prefix('=')?.trim();
            return Some(rest.trim_matches('"').to_string());
        }
    }
    None
}

fn build_plugins(filter: Option<&str>) -> ExitCode {
    let root = repo_root();
    let plugins = discover_plugins(&root);
    let mut built = 0u32;

    for manifest in &plugins {
        let name = match crate_name(manifest) {
            Some(n) => n,
            None => {
                eprintln!("warning: couldn't parse crate name from {}", manifest.display());
                continue;
            }
        };

        if let Some(f) = filter
            && !name.contains(f)
        {
            continue;
        }

        let dir = manifest.parent().unwrap();
        let wasm_stem = name.replace('-', "_");

        println!("building {name} → wasm32-unknown-unknown (release)…");

        let status = Command::new("cargo")
            .args([
                "build",
                "--manifest-path",
                &manifest.to_string_lossy(),
                "--target",
                "wasm32-unknown-unknown",
                "--release",
                "-Zbuild-std=std,panic_abort",
            ])
            .status();

        match status {
            Ok(s) if s.success() => {}
            Ok(s) => {
                eprintln!("cargo build failed for {name} (exit {})", s.code().unwrap_or(-1));
                return ExitCode::FAILURE;
            }
            Err(e) => {
                eprintln!("failed to run cargo: {e}");
                return ExitCode::FAILURE;
            }
        }

        let src = dir.join("target/wasm32-unknown-unknown/release").join(format!("{wasm_stem}.wasm"));
        let dst = dir.join(format!("{wasm_stem}.wasm"));

        if let Err(e) = fs::copy(&src, &dst) {
            eprintln!("failed to copy {}: {e}", src.display());
            return ExitCode::FAILURE;
        }

        let size = fs::metadata(&dst).map(|m| m.len()).unwrap_or(0);
        let rel = dst.strip_prefix(&root).unwrap_or(&dst);
        println!("  ✓ {:.0}K → {}", size as f64 / 1024.0, rel.display());
        built += 1;
    }

    if built == 0 {
        eprintln!("no plugins matched{}", filter.map_or(String::new(), |f| format!(" filter '{f}'")));
        return ExitCode::FAILURE;
    }

    println!("built {built} plugin(s).");
    ExitCode::SUCCESS
}

// ── Docs generation ─────────────────────────────────────────────────────

fn generate_docs(open: bool) -> ExitCode {
    let root = repo_root();
    let gen_dir = root.join("docs/src/generated");
    fs::create_dir_all(&gen_dir).expect("create generated dir");

    let crates = discover_crates(&root);

    println!("generating crate reference…");
    let crate_ref = gen_crate_reference(&root, &crates);
    fs::write(gen_dir.join("crates.md"), crate_ref).expect("write crates.md");

    println!("generating architecture map…");
    let arch = gen_architecture(&root, &crates);
    fs::write(gen_dir.join("architecture.md"), arch).expect("write architecture.md");

    println!("generating project stats…");
    let stats = gen_stats(&root, &crates);
    fs::write(gen_dir.join("stats.md"), stats).expect("write stats.md");

    println!("building mdbook…");
    let status = Command::new("mdbook")
        .arg("build")
        .current_dir(root.join("docs"))
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("docs built → docs/book/");
            if open {
                let index = root.join("docs/book/index.html");
                let _ = Command::new("xdg-open")
                    .arg(&index)
                    .status()
                    .or_else(|_| Command::new("open").arg(&index).status());
            }
            ExitCode::SUCCESS
        }
        Ok(s) => {
            eprintln!("mdbook build failed (exit {})", s.code().unwrap_or(-1));
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("failed to run mdbook: {e}");
            eprintln!("install with: cargo install mdbook");
            ExitCode::FAILURE
        }
    }
}

struct CrateInfo {
    name: String,
    description: String,
    loc: usize,
    test_count: usize,
    public_items: Vec<String>,
    deps: Vec<String>,
}

fn discover_crates(root: &Path) -> Vec<CrateInfo> {
    let crates_dir = root.join("crates");
    let mut infos = Vec::new();

    let mut entries: Vec<_> = fs::read_dir(&crates_dir)
        .expect("read crates/")
        .flatten()
        .filter(|e| e.path().join("Cargo.toml").exists())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        let description = extract_crate_doc(&path);
        let loc = count_lines(&path);
        let test_count = count_tests(&path);
        let public_items = extract_public_api(&path);
        let deps = extract_workspace_deps(&path);

        infos.push(CrateInfo {
            name,
            description,
            loc,
            test_count,
            public_items,
            deps,
        });
    }

    infos
}

/// Pull the first `//!` doc comment block from lib.rs.
fn extract_crate_doc(crate_path: &Path) -> String {
    let lib_rs = crate_path.join("src/lib.rs");
    let contents = match fs::read_to_string(&lib_rs) {
        Ok(c) => c,
        Err(_) => return String::new(),
    };

    let mut lines = Vec::new();
    for line in contents.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("//!") {
            lines.push(rest.strip_prefix(' ').unwrap_or(rest).to_string());
        } else if !trimmed.is_empty() {
            break;
        }
    }

    // Take just the first paragraph.
    let mut result = Vec::new();
    for line in &lines {
        if line.trim().is_empty() {
            break;
        }
        // Skip lines that look like markdown headers inside doc comments.
        if line.starts_with('#') {
            continue;
        }
        result.push(line.as_str());
    }
    result.join(" ").trim().to_string()
}

/// Count non-blank lines of .rs files (excluding target/).
fn count_lines(crate_path: &Path) -> usize {
    let mut total = 0;
    walk_rs_files(crate_path, &mut |path| {
        if let Ok(contents) = fs::read_to_string(path) {
            total += contents.lines().filter(|l| !l.trim().is_empty()).count();
        }
    });
    total
}

/// Count `#[test]` attributes.
fn count_tests(crate_path: &Path) -> usize {
    let mut total = 0;
    walk_rs_files(crate_path, &mut |path| {
        if let Ok(contents) = fs::read_to_string(path) {
            total += contents.matches("#[test]").count();
            total += contents.matches("#[tokio::test]").count();
        }
    });
    total
}

/// Extract `pub fn`, `pub struct`, `pub enum`, `pub trait` from src/ files.
fn extract_public_api(crate_path: &Path) -> Vec<String> {
    let src = crate_path.join("src");
    let mut items = Vec::new();
    walk_rs_files(&src, &mut |path| {
        if let Ok(contents) = fs::read_to_string(path) {
            for line in contents.lines() {
                let trimmed = line.trim();
                let kind = if trimmed.starts_with("pub struct ") {
                    Some("struct")
                } else if trimmed.starts_with("pub enum ") {
                    Some("enum")
                } else if trimmed.starts_with("pub trait ") {
                    Some("trait")
                } else if trimmed.starts_with("pub fn ")
                    || trimmed.starts_with("pub async fn ")
                {
                    Some("fn")
                } else {
                    None
                };

                if let Some(k) = kind {
                    let name = extract_item_name(trimmed, k);
                    if !name.is_empty() {
                        items.push(format!("{k} {name}"));
                    }
                }
            }
        }
    });
    items.sort();
    items.dedup();
    items
}

fn extract_item_name(line: &str, kind: &str) -> String {
    let after = match kind {
        "fn" => line
            .strip_prefix("pub fn ")
            .or_else(|| line.strip_prefix("pub async fn ")),
        "struct" => line.strip_prefix("pub struct "),
        "enum" => line.strip_prefix("pub enum "),
        "trait" => line.strip_prefix("pub trait "),
        _ => None,
    };
    match after {
        Some(rest) => rest
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .next()
            .unwrap_or("")
            .to_string(),
        None => String::new(),
    }
}

/// Extract workspace dependency names from Cargo.toml.
fn extract_workspace_deps(crate_path: &Path) -> Vec<String> {
    let manifest = crate_path.join("Cargo.toml");
    let contents = match fs::read_to_string(&manifest) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut deps = Vec::new();
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("clankers-")
            && let Some(name) = trimmed.split(|c: char| !c.is_alphanumeric() && c != '-').next()
            && name.starts_with("clankers-")
        {
            deps.push(name.to_string());
        }
    }
    deps.sort();
    deps.dedup();
    deps
}

fn walk_rs_files(dir: &Path, f: &mut dyn FnMut(&Path)) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n != "target") {
                walk_rs_files(&path, f);
            }
        } else if path.extension().is_some_and(|e| e == "rs") {
            f(&path);
        }
    }
}

// ── Markdown generators ─────────────────────────────────────────────────

fn gen_warning() -> &'static str {
    "<!-- This file is auto-generated by `cargo xtask docs`. Do not edit. -->\n\n\
     <div class=\"generated-warning\">\n\
     ⚡ Auto-generated from source. Run <code>cargo xtask docs</code> to refresh.\n\
     </div>\n\n"
}

fn gen_crate_reference(root: &Path, crates: &[CrateInfo]) -> String {
    let mut out = String::from(gen_warning());
    out.push_str("# Crate Reference\n\n");
    out.push_str("Each crate in the `crates/` workspace directory.\n\n");

    for c in crates {
        let _ = write!(out, "## {}\n\n", c.name);

        if !c.description.is_empty() {
            let _ = write!(out, "{}\n\n", c.description);
        }

        let _ = write!(out, "**{}** lines of Rust · **{}** tests\n\n", c.loc, c.test_count);

        if !c.deps.is_empty() {
            out.push_str("**Workspace deps:** ");
            let joined = c
                .deps
                .iter()
                .map(|d| format!("`{d}`"))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&joined);
            out.push_str("\n\n");
        }

        if !c.public_items.is_empty() {
            out.push_str("<details><summary>Public API</summary>\n\n");
            out.push_str("```\n");
            for item in &c.public_items {
                let _ = writeln!(out, "{item}");
            }
            out.push_str("```\n\n");
            out.push_str("</details>\n\n");
        }

        // Link to source.
        let src_path = format!("crates/{}/src/", c.name);
        if root.join(&src_path).exists() {
            let _ = write!(
                out,
                "[Source](https://github.com/brittonr/clankers/tree/main/{src_path})\n\n",
            );
        }

        out.push_str("---\n\n");
    }

    out
}

fn gen_architecture(_root: &Path, crates: &[CrateInfo]) -> String {
    let mut out = String::from(gen_warning());
    out.push_str("# Architecture Map\n\n");

    out.push_str("## Dependency graph\n\n");
    out.push_str("Workspace crate dependencies (auto-extracted from Cargo.toml files).\n\n");
    out.push_str("```mermaid\ngraph TD\n");

    for c in crates {
        let short = c.name.strip_prefix("clankers-").unwrap_or(&c.name);
        for dep in &c.deps {
            let dep_short = dep.strip_prefix("clankers-").unwrap_or(dep);
            let _ = writeln!(out, "    {short} --> {dep_short}");
        }
    }
    out.push_str("```\n\n");

    // Layer grouping.
    out.push_str("## Layers\n\n");
    let layers: &[(&str, &[&str])] = &[
        (
            "User interface",
            &["clankers-tui", "clankers-tui-types", "clankers-zellij"],
        ),
        (
            "Agent core",
            &[
                "clankers-agent",
                "clankers-agent-defs",
                "clankers-controller",
                "clankers-loop",
            ],
        ),
        (
            "LLM routing",
            &[
                "clankers-provider",
                "clankers-router",
                "clankers-model-selection",
            ],
        ),
        (
            "Infrastructure",
            &[
                "clankers-actor",
                "clankers-protocol",
                "clankers-session",
                "clankers-db",
                "clankers-config",
            ],
        ),
        (
            "Extensions",
            &[
                "clankers-plugin",
                "clankers-plugin-sdk",
                "clankers-skills",
                "clankers-hooks",
                "clankers-specs",
            ],
        ),
        (
            "Networking",
            &["clankers-auth", "clankers-matrix"],
        ),
        (
            "Utilities",
            &[
                "clankers-message",
                "clankers-prompts",
                "clankers-merge",
                "clankers-scheduler",
                "clankers-procmon",
                "clankers-util",
            ],
        ),
    ];

    for (layer, members) in layers {
        let _ = write!(out, "### {layer}\n\n");
        out.push_str("| Crate | Lines | Tests | Description |\n");
        out.push_str("|-------|------:|------:|-------------|\n");
        for &member in *members {
            if let Some(c) = crates.iter().find(|c| c.name == member) {
                let short = c.name.strip_prefix("clankers-").unwrap_or(&c.name);
                let _ = writeln!(out, "| `{short}` | {} | {} | {} |", c.loc, c.test_count, c.description);
            }
        }
        out.push('\n');
    }

    out
}

fn gen_stats(root: &Path, crates: &[CrateInfo]) -> String {
    let mut out = String::from(gen_warning());
    out.push_str("# Project Stats\n\n");

    let total_loc: usize = crates.iter().map(|c| c.loc).sum();
    let total_tests: usize = crates.iter().map(|c| c.test_count).sum();
    let total_public: usize = crates.iter().map(|c| c.public_items.len()).sum();

    // Also count src/ (the main binary crate).
    let main_loc = count_lines(&root.join("src"));
    let main_tests = count_tests(&root.join("src"));

    out.push_str("## Overview\n\n");
    out.push_str("| Metric | Count |\n");
    out.push_str("|--------|------:|\n");
    let _ = writeln!(out, "| Workspace crates | {} |", crates.len());
    let _ = writeln!(out, "| Lines of Rust (crates/) | {} |", fmt_num(total_loc));
    let _ = writeln!(out, "| Lines of Rust (src/) | {} |", fmt_num(main_loc));
    let _ = writeln!(out, "| **Total lines of Rust** | **{}** |", fmt_num(total_loc + main_loc));
    let _ = writeln!(out, "| Tests (crates/) | {} |", fmt_num(total_tests));
    let _ = writeln!(out, "| Tests (src/) | {} |", fmt_num(main_tests));
    let _ = writeln!(out, "| **Total tests** | **{}** |", fmt_num(total_tests + main_tests));
    let _ = writeln!(out, "| Public API items | {} |", fmt_num(total_public));

    out.push_str("\n## Crates by size\n\n");
    out.push_str("| Crate | Lines | Tests |\n");
    out.push_str("|-------|------:|------:|\n");

    // Add main binary.
    let mut sorted: Vec<(&str, usize, usize)> = crates
        .iter()
        .map(|c| (c.name.as_str(), c.loc, c.test_count))
        .collect();
    sorted.push(("src/ (binary)", main_loc, main_tests));
    sorted.sort_by_key(|&(_, loc, _)| Reverse(loc));

    for (name, loc, tests) in &sorted {
        let _ = writeln!(out, "| `{name}` | {} | {} |", fmt_num(*loc), tests);
    }
    out.push('\n');

    out
}

fn fmt_num(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}
