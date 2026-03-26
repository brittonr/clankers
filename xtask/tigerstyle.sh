#!/usr/bin/env bash
# Run tigerstyle lints on the clankers workspace.
#
# Builds tigerstyle from ../tigerstyle, creates the @toolchain symlink
# that cargo-dylint expects, then runs the lints.
#
# Usage:
#   ./xtask/tigerstyle.sh                       # lint entire workspace
#   ./xtask/tigerstyle.sh -p clankers-provider  # lint one crate

set -euo pipefail

TOOLCHAIN="nightly-x86_64-unknown-linux-gnu"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
TIGERSTYLE_DIR="$SCRIPT_DIR/../../tigerstyle"
DYLINT_BUILD_DIR="${CARGO_TARGET_DIR:-$HOME/.cargo-target}/dylint/libraries/$TOOLCHAIN/release"

# Build tigerstyle into the directory cargo-dylint expects
if [[ ! -f "$DYLINT_BUILD_DIR/libtigerstyle.so" ]]; then
    echo "Building tigerstyle..."
    mkdir -p "$DYLINT_BUILD_DIR"
    (cd "$TIGERSTYLE_DIR" && cargo build --release --target-dir "$DYLINT_BUILD_DIR/..")
fi

# Create the @toolchain symlink that cargo-dylint looks for
LIB_LINK="$DYLINT_BUILD_DIR/libtigerstyle@${TOOLCHAIN}.so"
if [[ ! -f "$LIB_LINK" ]]; then
    ln -sf "$DYLINT_BUILD_DIR/libtigerstyle.so" "$LIB_LINK"
fi

# Default to --workspace if no package args given
if [[ $# -eq 0 ]]; then
    set -- --workspace
fi

exec cargo dylint --all --no-build -- "$@"
