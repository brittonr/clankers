#!/usr/bin/env bash
# Run tigerstyle lints on the clankers workspace.
#
# Fetches pinned tigerstyle-rs from GitHub over SSH, builds a cached lint
# library plus cargo-tigerstyle runner, then runs the first-class tigerstyle
# consumer command against this workspace.
#
# Usage:
#   ./xtask/tigerstyle.sh                       # lint entire workspace
#   ./xtask/tigerstyle.sh -p clankers-provider  # lint one crate

set -euo pipefail

TOOLCHAIN="nightly-x86_64-unknown-linux-gnu"
TIGERSTYLE_REPO="ssh://git@github.com/brittonr/tigerstyle-rs.git"
TIGERSTYLE_REV="bbf5fbb60679668ca8c42593fd617db2d0f89b43"
TIGERSTYLE_DYLINT_REV="0e0a71eefe6f01563d11acc6e1d7af1d505934a9"
TIGERSTYLE_CACHE_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/clankers/tigerstyle-rs"
LINT_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cargo-target}/dylint/tigerstyle-rs/$TIGERSTYLE_REV/$TOOLCHAIN"
LINT_BUILD_DIR="$LINT_TARGET_DIR/release"
RUNNER_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cargo-target}/cargo-tigerstyle/$TIGERSTYLE_REV"
RUNNER_BIN="$RUNNER_TARGET_DIR/release/cargo-tigerstyle"
LINT_LINK="$LINT_BUILD_DIR/libtigerstyle@${TOOLCHAIN}.so"

sync_tigerstyle() {
    mkdir -p "$(dirname "$TIGERSTYLE_CACHE_DIR")"
    if [[ ! -d "$TIGERSTYLE_CACHE_DIR/.git" ]]; then
        rm -rf "$TIGERSTYLE_CACHE_DIR"
        git clone "$TIGERSTYLE_REPO" "$TIGERSTYLE_CACHE_DIR"
    fi

    (
        cd "$TIGERSTYLE_CACHE_DIR"
        git remote set-url origin "$TIGERSTYLE_REPO"
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

sync_tigerstyle
build_tigerstyle
build_runner
install_rustup_shim

# Default to no explicit scope so cargo-tigerstyle can use
# [workspace.metadata.tigerstyle].
if [[ $# -eq 0 ]]; then
    set --
fi

export TIGERSTYLE_TOOLCHAIN="$TOOLCHAIN"
export TIGERSTYLE_DYLINT_REV="$TIGERSTYLE_DYLINT_REV"
export TIGERSTYLE_LINT_LIB="$LINT_LINK"

exec "$RUNNER_BIN" check "$@"
