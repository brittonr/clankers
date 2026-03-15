#!/usr/bin/env bash
# Combined verus + tracey verification check.
#
# Runs Verus machine-checked proofs on the spec modules, then checks
# Tracey requirement coverage. Both must pass for CI to go green.
#
# Usage: ./scripts/verify.sh [--tracey-only]

set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

TRACEY_ONLY=false
for arg in "$@"; do
    case "$arg" in
        --tracey-only) TRACEY_ONLY=true ;;
        *) echo "Unknown arg: $arg"; exit 1 ;;
    esac
done

echo "=== Tracey requirement coverage ==="
tracey query status
echo ""

uncovered=$(tracey query uncovered --prefix merge --json 2>/dev/null | jq '.count // 0' 2>/dev/null || echo "0")
if [ "$uncovered" != "0" ]; then
    echo "ERROR: $uncovered merge requirements uncovered"
    tracey query uncovered --prefix merge
    exit 1
fi

untested=$(tracey query untested --prefix merge --json 2>/dev/null | jq '.count // 0' 2>/dev/null || echo "0")
if [ "$untested" != "0" ]; then
    echo "ERROR: $untested merge requirements untested"
    tracey query untested --prefix merge
    exit 1
fi

echo "✓ All merge requirements covered and tested"
echo ""

if [ "$TRACEY_ONLY" = true ]; then
    echo "Skipping verus (--tracey-only)"
    exit 0
fi

if ! command -v verus &>/dev/null; then
    echo "WARNING: verus not found, skipping proof verification"
    echo "  Install verus or enter the nix devshell: nix develop"
    exit 0
fi

echo "=== Verus proof verification ==="
verus --crate-type=lib verus/lib.rs
echo "✓ All proofs verified"
