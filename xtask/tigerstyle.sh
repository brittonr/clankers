#!/usr/bin/env bash
# Run tigerstyle lints on the clankers workspace.
#
# Prerequisites (one-time, inside nix develop):
#   cd ../tigerstyle && cargo build --release
#   ln -sf libtigerstyle.so ~/.cargo-target/release/libtigerstyle@nightly-x86_64-unknown-linux-gnu.so
#
# Usage:
#   ./xtask/tigerstyle.sh                  # lint entire workspace
#   ./xtask/tigerstyle.sh -p clankers-provider  # lint one crate

set -euo pipefail

TOOLCHAIN="nightly-x86_64-unknown-linux-gnu"
LIB_DIR="${CARGO_TARGET_DIR:-$HOME/.cargo-target}/release"
LIB_PATH="$LIB_DIR/libtigerstyle@${TOOLCHAIN}.so"
LIB_SRC="$LIB_DIR/libtigerstyle.so"

# Build tigerstyle if needed
if [[ ! -f "$LIB_SRC" ]]; then
    echo "Building tigerstyle..."
    (cd "$(dirname "$0")/../../tigerstyle" && cargo build --release)
fi

# Create @toolchain symlink if missing
if [[ ! -f "$LIB_PATH" ]]; then
    ln -sf "$LIB_SRC" "$LIB_PATH"
fi

# Default to --workspace if no package args given
if [[ $# -eq 0 ]]; then
    set -- --workspace
fi

exec cargo dylint --lib-path "$LIB_PATH" -- "$@"
