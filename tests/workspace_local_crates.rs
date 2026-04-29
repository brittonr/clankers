use std::fs;
use std::path::Path;
use std::path::PathBuf;

const ROOT_CARGO_TOML: &str = include_str!("../Cargo.toml");
const FLAKE_NIX: &str = include_str!("../flake.nix");

const WORKSPACE_LOCAL_FIRST_PARTY_CRATES: [&str; 5] = [
    "clanker-auth",
    "clanker-message",
    "clanker-plugin-sdk",
    "clanker-router",
    "clanker-tui-types",
];

#[test]
fn first_party_crates_are_workspace_local_path_dependencies() {
    for crate_name in WORKSPACE_LOCAL_FIRST_PARTY_CRATES {
        let member = format!("\"crates/{crate_name}\"");
        assert!(ROOT_CARGO_TOML.contains(&member), "root workspace members must include {member}");

        let dep = format!("{crate_name} = {{ path = \"crates/{crate_name}\" }}");
        assert!(
            ROOT_CARGO_TOML.contains(&dep),
            "root [workspace.dependencies] must keep {crate_name} as a workspace-local path dependency"
        );
    }
}

#[test]
fn no_legacy_vendor_or_sibling_first_party_paths_exist() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for crate_name in WORKSPACE_LOCAL_FIRST_PARTY_CRATES {
        assert!(
            !root.join("vendor").join(crate_name).exists(),
            "do not reintroduce vendor/{crate_name}; it is workspace-local under crates/"
        );
    }

    let offenders = cargo_tomls_with_non_workspace_first_party_paths(&root);
    assert!(
        offenders.is_empty(),
        "first-party clanker-* path deps should point at crates/<name>, not sibling/vendor checkouts: {offenders:#?}"
    );
}

#[test]
fn flake_does_not_reintroduce_first_party_crate_inputs() {
    for crate_name in WORKSPACE_LOCAL_FIRST_PARTY_CRATES {
        let input_prefix = format!("inputs.{crate_name}");
        let github_ref = format!("github:brittonr/{crate_name}");
        assert!(
            !FLAKE_NIX.contains(&input_prefix),
            "flake.nix should not add {crate_name} as a flake input; use ./crates/{crate_name}"
        );
        assert!(
            !FLAKE_NIX.contains(&github_ref),
            "flake.nix should not fetch {crate_name} from GitHub; use ./crates/{crate_name}"
        );
    }
}

fn cargo_tomls_with_non_workspace_first_party_paths(root: &Path) -> Vec<String> {
    let mut offenders = Vec::new();
    for manifest in cargo_tomls(root) {
        let text = fs::read_to_string(&manifest).expect("Cargo.toml should be readable");
        for crate_name in WORKSPACE_LOCAL_FIRST_PARTY_CRATES {
            let dep_prefix = format!("{crate_name} = {{ path = ");
            for line in text.lines().filter(|line| line.trim_start().starts_with(&dep_prefix)) {
                let normalized = line.replace('\\', "/");
                if !normalized.contains(&format!("crates/{crate_name}")) {
                    offenders.push(format!("{}: {}", manifest.display(), line.trim()));
                }
            }
        }
    }
    offenders
}

fn cargo_tomls(root: &Path) -> Vec<PathBuf> {
    let mut manifests = Vec::new();
    collect_cargo_tomls(root, &mut manifests);
    manifests
}

fn collect_cargo_tomls(dir: &Path, manifests: &mut Vec<PathBuf>) {
    let ignored = [".git", "target", "docs/book"];
    let relative = dir.strip_prefix(env!("CARGO_MANIFEST_DIR")).unwrap_or(dir).to_string_lossy();
    if ignored.iter().any(|ignored| relative == *ignored || relative.starts_with(&format!("{ignored}/"))) {
        return;
    }

    let entries = fs::read_dir(dir).unwrap_or_else(|error| panic!("failed to read {}: {error}", dir.display()));
    for entry in entries {
        let entry = entry.expect("directory entry should be readable");
        let path = entry.path();
        if path.is_dir() {
            collect_cargo_tomls(&path, manifests);
        } else if path.file_name().is_some_and(|name| name == "Cargo.toml") {
            manifests.push(path);
        }
    }
}
