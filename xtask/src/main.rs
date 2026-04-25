#![cfg_attr(
    dylint_lib = "tigerstyle",
    allow(
        no_unwrap,
        no_panic,
        reason = "xtask is a build tool — panics are acceptable for fatal errors"
    )
)]
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
    let mut plugins = Vec::with_capacity(32);
    assert!(root.is_dir());
    assert!(dirs.len() == 2);

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
    assert!(plugins.windows(2).all(|pair| pair[0] <= pair[1]));
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

    assert!(root.is_dir());
    assert!(u32::try_from(plugins.len()).is_ok());

    for manifest in &plugins {
        let name = match crate_name(manifest) {
            Some(n) => n,
            None => {
                eprintln!("warning: couldn't parse crate name from {}", manifest.display());
                continue;
            }
        };

        assert!(!name.is_empty());
        if let Some(f) = filter
            && !name.contains(f)
        {
            continue;
        }

        let dir = manifest.parent().unwrap();
        let wasm_stem = name.replace('-', "_");

        assert!(dir.is_dir());
        assert!(!wasm_stem.is_empty());
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

        let size_bytes = fs::metadata(&dst).map(|m| m.len()).unwrap_or(0);
        let rel = dst.strip_prefix(&root).unwrap_or(&dst);
        assert!(dst.is_file());
        assert!(rel != Path::new(""));
        println!("  ✓ {:.0}K → {}", size_bytes as f64 / 1024.0, rel.display());
        built += 1;
    }

    if built == 0 {
        eprintln!("no plugins matched{}", filter.map_or(String::new(), |f| format!(" filter '{f}'")));
        return ExitCode::FAILURE;
    }

    assert!(built > 0);
    assert!(usize::try_from(built).is_ok_and(|built_usize| built_usize <= plugins.len()));
    println!("built {built} plugin(s).");
    ExitCode::SUCCESS
}

// ── Docs generation ─────────────────────────────────────────────────────

fn generate_docs(open: bool) -> ExitCode {
    let root = repo_root();
    let gen_dir = root.join("docs/src/generated");

    assert!(root.is_dir());
    assert!(root.join("docs/book.toml").is_file());
    fs::create_dir_all(&gen_dir).expect("create generated dir");
    assert!(gen_dir.is_dir());

    let crates = discover_crates(&root);

    println!("generating crate reference…");
    let crate_ref = gen_crate_reference(&root, &crates);
    assert!(crate_ref.contains("# Crate Reference"));
    fs::write(gen_dir.join("crates.md"), crate_ref).expect("write crates.md");

    println!("generating architecture map…");
    let arch = gen_architecture(&root, &crates);
    assert!(arch.contains("```mermaid"));
    fs::write(gen_dir.join("architecture.md"), arch).expect("write architecture.md");

    println!("generating project stats…");
    let stats = gen_stats(&root, &crates);
    assert!(stats.contains("# Project Stats"));
    fs::write(gen_dir.join("stats.md"), stats).expect("write stats.md");

    println!("building mdbook…");
    let status = Command::new("mdbook").arg("build").current_dir(root.join("docs")).status();

    match status {
        Ok(s) if s.success() => {
            println!("docs built → docs/book/");
            if open {
                let index = root.join("docs/book/index.html");
                Command::new("xdg-open")
                    .arg(&index)
                    .status()
                    .or_else(|_| Command::new("open").arg(&index).status())
                    .ok();
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
    assert!(crates_dir.is_dir());

    let mut entries: Vec<_> = fs::read_dir(&crates_dir)
        .expect("read crates/")
        .flatten()
        .filter(|e| e.path().join("Cargo.toml").exists())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    let mut infos = Vec::with_capacity(entries.len());
    assert!(entries.windows(2).all(|pair| pair[0].file_name() <= pair[1].file_name()));

    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        assert!(path.join("Cargo.toml").is_file());
        assert!(!name.is_empty());
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

    assert!(infos.len() <= usize::MAX / 2);
    assert!(infos.iter().all(|info| !info.name.is_empty()));
    infos
}

/// Pull the first `//!` doc comment block from lib.rs.
fn extract_crate_doc(crate_path: &Path) -> String {
    let lib_rs = crate_path.join("src/lib.rs");
    assert!(crate_path.is_dir());
    assert!(lib_rs.ends_with("src/lib.rs"));

    let contents = match fs::read_to_string(&lib_rs) {
        Ok(c) => c,
        Err(_) => return String::new(),
    };

    let mut lines = Vec::with_capacity(contents.lines().count());
    for line in contents.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("//!") {
            lines.push(rest.strip_prefix(' ').unwrap_or(rest).to_string());
        } else if !trimmed.is_empty() {
            break;
        }
    }

    let mut result = Vec::with_capacity(lines.len());
    for line in &lines {
        if line.trim().is_empty() {
            break;
        }
        if line.starts_with('#') {
            continue;
        }
        result.push(line.as_str());
    }

    let doc = result.join(" ").trim().to_string();
    assert!(!doc.contains('\0'));
    assert!(!doc.starts_with('#'));
    doc
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

#[derive(Clone, Copy)]
enum ItemKind {
    Function,
    Struct,
    Enum,
    Trait,
}

impl ItemKind {
    fn from_public_decl(line: &str) -> Option<Self> {
        if line.starts_with("pub struct ") {
            Some(Self::Struct)
        } else if line.starts_with("pub enum ") {
            Some(Self::Enum)
        } else if line.starts_with("pub trait ") {
            Some(Self::Trait)
        } else if line.starts_with("pub fn ") || line.starts_with("pub async fn ") {
            Some(Self::Function)
        } else {
            None
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Function => "fn",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Trait => "trait",
        }
    }

    fn strip_prefix(self, line: &str) -> Option<&str> {
        match self {
            Self::Function => line.strip_prefix("pub fn ").or_else(|| line.strip_prefix("pub async fn ")),
            Self::Struct => line.strip_prefix("pub struct "),
            Self::Enum => line.strip_prefix("pub enum "),
            Self::Trait => line.strip_prefix("pub trait "),
        }
    }
}

/// Extract `pub fn`, `pub struct`, `pub enum`, `pub trait` from src/ files.
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(
        nested_conditionals,
        reason = "complex control flow — extracting helpers would obscure logic"
    )
)]
fn extract_public_api(crate_path: &Path) -> Vec<String> {
    let src = crate_path.join("src");
    let mut items = Vec::with_capacity(64);

    assert!(crate_path.is_dir());
    assert!(src.is_dir());
    walk_rs_files(&src, &mut |path| {
        if let Ok(contents) = fs::read_to_string(path) {
            for line in contents.lines() {
                let trimmed = line.trim();
                if let Some(item_kind) = ItemKind::from_public_decl(trimmed) {
                    let name = extract_item_name(trimmed, item_kind);
                    if !name.is_empty() {
                        items.push(format!("{} {name}", item_kind.label()));
                    }
                }
            }
        }
    });

    items.sort();
    items.dedup();
    assert!(items.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(items.iter().all(|item| item.contains(' ')));
    items
}

fn extract_item_name(line: &str, item_kind: ItemKind) -> String {
    let after = item_kind.strip_prefix(line);
    match after {
        Some(rest) => rest.split(|c: char| !c.is_alphanumeric() && c != '_').next().unwrap_or("").to_string(),
        None => String::new(),
    }
}

const WORKSPACE_DEP_PREFIXES: [&str; 2] = ["clankers-", "clanker-"];

fn parse_workspace_dep_name(line: &str) -> Option<String> {
    let candidate = line.split(|c: char| !c.is_alphanumeric() && c != '-').next()?;
    if WORKSPACE_DEP_PREFIXES.iter().any(|prefix| candidate.starts_with(prefix)) {
        return Some(candidate.to_string());
    }
    None
}

/// Extract workspace dependency names from Cargo.toml.
fn extract_workspace_deps(crate_path: &Path) -> Vec<String> {
    let manifest = crate_path.join("Cargo.toml");
    assert!(crate_path.is_dir());
    assert!(manifest.ends_with("Cargo.toml"));

    let contents = match fs::read_to_string(&manifest) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut deps = Vec::with_capacity(contents.lines().count());
    for line in contents.lines() {
        let trimmed = line.trim();
        if let Some(name) = parse_workspace_dep_name(trimmed) {
            deps.push(name);
        }
    }

    deps.sort();
    deps.dedup();
    assert!(deps.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(deps.iter().all(|dep| WORKSPACE_DEP_PREFIXES.iter().any(|prefix| dep.starts_with(prefix))));
    deps
}

fn walk_rs_files(dir: &Path, f: &mut dyn FnMut(&Path)) {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n != "target") {
                    stack.push(path);
                }
            } else if path.extension().is_some_and(|e| e == "rs") {
                f(&path);
            }
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
    assert!(root.is_dir());
    assert!(crates.iter().all(|crate_info| !crate_info.name.is_empty()));

    out.push_str("# Crate Reference\n\n");
    out.push_str("Each crate in the `crates/` workspace directory.\n\n");

    for crate_info in crates {
        write!(out, "## {}\n\n", crate_info.name).ok();

        if !crate_info.description.is_empty() {
            write!(out, "{}\n\n", crate_info.description).ok();
        }

        write!(out, "**{}** lines of Rust · **{}** tests\n\n", crate_info.loc, crate_info.test_count).ok();

        if !crate_info.deps.is_empty() {
            out.push_str("**Workspace deps:** ");
            let joined = crate_info.deps.iter().map(|dep| format!("`{dep}`")).collect::<Vec<_>>().join(", ");
            out.push_str(&joined);
            out.push_str("\n\n");
        }

        if !crate_info.public_items.is_empty() {
            out.push_str("<details><summary>Public API</summary>\n\n");
            out.push_str("```\n");
            for item in &crate_info.public_items {
                writeln!(out, "{item}").ok();
            }
            out.push_str("```\n\n");
            out.push_str("</details>\n\n");
        }

        let src_path = format!("crates/{}/src/", crate_info.name);
        if root.join(&src_path).exists() {
            write!(out, "[Source](https://github.com/brittonr/clankers/tree/main/{src_path})\n\n",).ok();
        }

        out.push_str("---\n\n");
    }

    assert!(out.contains("# Crate Reference"));
    assert!(out.starts_with("<!-- This file is auto-generated"));
    out
}

fn gen_architecture(_root: &Path, crates: &[CrateInfo]) -> String {
    let mut out = String::from(gen_warning());
    let layers: &[(&str, &[&str])] = &[
        ("User interface", &["clanker-tui-types", "clankers-tui", "clankers-zellij", "clankers-tts"]),
        ("Agent core", &[
            "clanker-message",
            "clankers-agent",
            "clankers-agent-defs",
            "clankers-core",
            "clankers-engine",
            "clankers-controller",
        ]),
        ("LLM routing", &[
            "clanker-router",
            "clankers-provider",
            "clankers-model-selection",
            "clankers-prompts",
        ]),
        ("Infrastructure", &[
            "clankers-config",
            "clankers-db",
            "clankers-hooks",
            "clankers-nix",
            "clankers-protocol",
            "clankers-session",
        ]),
        ("Networking & security", &["clanker-auth", "clankers-matrix", "clankers-ucan"]),
        ("Extensions & tooling", &[
            "clanker-plugin-sdk",
            "clankers-plugin",
            "clankers-skills",
            "clankers-procmon",
        ]),
        ("Utilities", &["clankers-util"]),
    ];

    assert!(!crates.is_empty());
    assert!(!layers.is_empty());
    out.push_str("# Architecture Map\n\n");
    out.push_str("## Dependency graph\n\n");
    out.push_str("Workspace crate dependencies (auto-extracted from Cargo.toml files).\n\n");
    out.push_str("```mermaid\ngraph TD\n");

    for crate_info in crates {
        let short = crate_info.name.strip_prefix("clankers-").unwrap_or(&crate_info.name);
        for dep in &crate_info.deps {
            let dep_short = dep.strip_prefix("clankers-").unwrap_or(dep);
            writeln!(out, "    {short} --> {dep_short}").ok();
        }
    }
    out.push_str("```\n\n");

    out.push_str("## Layers\n\n");
    for (layer, members) in layers {
        write!(out, "### {layer}\n\n").ok();
        out.push_str("| Crate | Lines | Tests | Description |\n");
        out.push_str("|-------|------:|------:|-------------|\n");
        for &member in *members {
            if let Some(crate_info) = crates.iter().find(|crate_info| crate_info.name == member) {
                let short = crate_info.name.strip_prefix("clankers-").unwrap_or(&crate_info.name);
                writeln!(
                    out,
                    "| `{short}` | {} | {} | {} |",
                    crate_info.loc, crate_info.test_count, crate_info.description
                )
                .ok();
            }
        }
        out.push('\n');
    }

    assert!(out.contains("```mermaid"));
    assert!(out.contains("## Layers"));
    out
}

fn gen_stats(root: &Path, crates: &[CrateInfo]) -> String {
    let mut out = String::from(gen_warning());
    let total_loc: usize = crates.iter().map(|crate_info| crate_info.loc).sum();
    let total_tests: usize = crates.iter().map(|crate_info| crate_info.test_count).sum();
    let total_public: usize = crates.iter().map(|crate_info| crate_info.public_items.len()).sum();
    let main_loc = count_lines(&root.join("src"));
    let main_tests = count_tests(&root.join("src"));
    let total_loc_all = total_loc.saturating_add(main_loc);
    let total_tests_all = total_tests.saturating_add(main_tests);

    assert!(root.is_dir());
    assert!(total_loc_all >= total_loc);
    assert!(total_tests_all >= total_tests);
    out.push_str("# Project Stats\n\n");
    out.push_str("## Overview\n\n");
    out.push_str("| Metric | Count |\n");
    out.push_str("|--------|------:|\n");
    writeln!(out, "| Workspace crates | {} |", crates.len()).ok();
    writeln!(out, "| Lines of Rust (crates/) | {} |", fmt_num(total_loc)).ok();
    writeln!(out, "| Lines of Rust (src/) | {} |", fmt_num(main_loc)).ok();
    writeln!(out, "| **Total lines of Rust** | **{}** |", fmt_num(total_loc_all)).ok();
    writeln!(out, "| Tests (crates/) | {} |", fmt_num(total_tests)).ok();
    writeln!(out, "| Tests (src/) | {} |", fmt_num(main_tests)).ok();
    writeln!(out, "| **Total tests** | **{}** |", fmt_num(total_tests_all)).ok();
    writeln!(out, "| Public API items | {} |", fmt_num(total_public)).ok();

    out.push_str("\n## Crates by size\n\n");
    out.push_str("| Crate | Lines | Tests |\n");
    out.push_str("|-------|------:|------:|\n");

    let mut sorted: Vec<(&str, usize, usize)> = crates
        .iter()
        .map(|crate_info| (crate_info.name.as_str(), crate_info.loc, crate_info.test_count))
        .collect();
    sorted.push(("src/ (binary)", main_loc, main_tests));
    sorted.sort_by_key(|&(_, loc, _)| Reverse(loc));

    for (name, loc, tests) in &sorted {
        writeln!(out, "| `{name}` | {} | {} |", fmt_num(*loc), tests).ok();
    }
    out.push('\n');

    assert!(out.contains("# Project Stats"));
    assert!(out.contains("## Crates by size"));
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
