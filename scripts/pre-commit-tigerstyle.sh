#!/usr/bin/env sh
# Pre-commit Tigerstyle gate for Clankers.
#
# Install locally with:
#   cp scripts/pre-commit-tigerstyle.sh .git/hooks/pre-commit
#   chmod +x .git/hooks/pre-commit

set -eu

repo_root=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
cd "$repo_root"

export TMPDIR=${TMPDIR:-/home/brittonr/.cargo-target/tmp}
export RUSTC_WRAPPER=${RUSTC_WRAPPER:-}

exec ./xtask/tigerstyle.sh -- --keep-going
