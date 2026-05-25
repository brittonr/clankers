#!/usr/bin/env bash
# Run tigerstyle lints on the clankers workspace.
#
# Fetches pinned Tiger Style from Octet on GitHub, builds a cached lint
# library plus cargo-tigerstyle runner, then runs the first-class tigerstyle
# consumer command against this workspace.
#
# Usage:
#   ./xtask/tigerstyle.sh                       # lint entire workspace
#   ./xtask/tigerstyle.sh -p clankers-provider  # lint one crate

set -euo pipefail

TOOLCHAIN="nightly-x86_64-unknown-linux-gnu"
TIGERSTYLE_REPO="github:OnixResearch/octet"
TIGERSTYLE_REV="bbf5fbb60679668ca8c42593fd617db2d0f89b43"
TIGERSTYLE_DYLINT_REV="0e0a71eefe6f01563d11acc6e1d7af1d505934a9"
TIGERSTYLE_CACHE_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/clankers/octet-tigerstyle"
LINT_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cargo-target}/dylint/octet-tigerstyle/$TIGERSTYLE_REV/$TOOLCHAIN"
LINT_BUILD_DIR="$LINT_TARGET_DIR/release"
RUNNER_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cargo-target}/cargo-tigerstyle/$TIGERSTYLE_REV"
RUNNER_BIN="$RUNNER_TARGET_DIR/release/cargo-tigerstyle"
LINT_LINK="$LINT_BUILD_DIR/libtigerstyle@${TOOLCHAIN}.so"

prefer_tigerstyle_toolchain() {
    local pinned_bin
    pinned_bin="$(find /nix/store -maxdepth 1 -type d -name '*rust-default-1.97.0-nightly-2026-04-16' -print -quit 2>/dev/null)/bin"
    if [[ -x "$pinned_bin/rustc" && -x "$pinned_bin/cargo" ]]; then
        export PATH="$pinned_bin:$PATH"
    fi
}

tigerstyle_git_url() {
    case "$TIGERSTYLE_REPO" in
        github:*)
            printf 'ssh://git@github.com/%s.git\n' "${TIGERSTYLE_REPO#github:}"
            ;;
        *)
            printf '%s\n' "$TIGERSTYLE_REPO"
            ;;
    esac
}

sync_tigerstyle() {
    local tigerstyle_git_repo
    tigerstyle_git_repo="$(tigerstyle_git_url)"

    mkdir -p "$(dirname "$TIGERSTYLE_CACHE_DIR")"
    if [[ ! -d "$TIGERSTYLE_CACHE_DIR/.git" ]]; then
        rm -rf "$TIGERSTYLE_CACHE_DIR"
        git clone "$tigerstyle_git_repo" "$TIGERSTYLE_CACHE_DIR"
    fi

    (
        cd "$TIGERSTYLE_CACHE_DIR"
        git remote set-url origin "$tigerstyle_git_repo"
        git fetch origin "$TIGERSTYLE_REV"
        git checkout --detach "$TIGERSTYLE_REV"
    )
}

build_tigerstyle() {
    if [[ -f "$LINT_BUILD_DIR/libtigerstyle.so" ]]; then
        mkdir -p "$LINT_BUILD_DIR"
        ln -sf "$LINT_BUILD_DIR/libtigerstyle.so" "$LINT_LINK"
        return
    fi

    echo "Building tigerstyle $TIGERSTYLE_REV..."
    mkdir -p "$LINT_BUILD_DIR"
    (
        cd "$TIGERSTYLE_CACHE_DIR"
        cargo build --release --target-dir "$LINT_TARGET_DIR" -p tigerstyle --lib
    )
    ln -sf "$LINT_BUILD_DIR/libtigerstyle.so" "$LINT_LINK"
}

build_runner() {
    if [[ -x "$RUNNER_BIN" ]]; then
        return
    fi

    echo "Building cargo-tigerstyle $TIGERSTYLE_REV..."
    mkdir -p "$(dirname "$RUNNER_BIN")"
    cargo build \
        --manifest-path "$TIGERSTYLE_CACHE_DIR/cargo-tigerstyle/Cargo.toml" \
        --release \
        --target-dir "$RUNNER_TARGET_DIR"
}

nix_cargo_tigerstyle() {
    if ! command -v nix >/dev/null 2>&1; then
        return 1
    fi

    nix build "$TIGERSTYLE_REPO/$TIGERSTYLE_REV#cargo-tigerstyle" --no-link --print-out-paths 2>/dev/null
}

install_rustup_shim() {
    local shim_dir
    shim_dir="$(mktemp -d)"
    cat > "$shim_dir/rustup" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

if [[ ${1:-} == +* ]]; then
    shift
fi

case "${1:-} ${2:-}" in
    "show active-toolchain")
        echo "${RUSTUP_TOOLCHAIN:-nightly-x86_64-unknown-linux-gnu} (xtask shim)"
        ;;
    "which rustc")
        command -v rustc
        ;;
    "which cargo")
        command -v cargo
        ;;
    "run "*)
        shift
        if [[ ${1:-} == +* || ${1:-} == *-unknown-linux-gnu ]]; then
            shift
        fi
        exec "$@"
        ;;
    "rustc "*)
        exec "$@"
        ;;
    "cargo "*)
        exec "$@"
        ;;
    *)
        echo "xtask rustup shim: unsupported command: $*" >&2
        exit 1
        ;;
esac
EOF
    chmod +x "$shim_dir/rustup"
    export PATH="$shim_dir:$PATH"
    trap 'rm -rf "$shim_dir"' EXIT
}

# Default to no explicit scope so cargo-tigerstyle can use
# [workspace.metadata.tigerstyle].
if [[ $# -eq 0 ]]; then
    set --
fi

nix_runner="$(nix_cargo_tigerstyle || true)"
if [[ -n "$nix_runner" && -x "$nix_runner/bin/cargo-tigerstyle" ]]; then
    exec "$nix_runner/bin/cargo-tigerstyle" check "$@"
fi

prefer_tigerstyle_toolchain
sync_tigerstyle
build_tigerstyle
build_runner
install_rustup_shim

export TIGERSTYLE_TOOLCHAIN="$TOOLCHAIN"
export TIGERSTYLE_DYLINT_REV="$TIGERSTYLE_DYLINT_REV"
export TIGERSTYLE_LINT_LIB="$LINT_LINK"
export RUSTC="$(command -v rustc)"

exec "$RUNNER_BIN" check "$@"
