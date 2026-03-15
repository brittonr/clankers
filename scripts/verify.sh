#!/usr/bin/env bash
set -euo pipefail

echo "=== Verus: verifying formal proofs ==="
verus --crate-type=lib verus/lib.rs
echo "  ✓ All proofs verified"

echo ""
echo "=== Tracey: checking requirement coverage ==="
tracey query status
echo ""

uncovered=$(tracey query uncovered 2>&1)
if echo "$uncovered" | grep -q "0 uncovered"; then
    echo "  ✓ All requirements covered"
else
    echo "  ✗ Uncovered requirements found:"
    echo "$uncovered"
    exit 1
fi

untested=$(tracey query untested 2>&1)
if echo "$untested" | grep -q "0 untested"; then
    echo "  ✓ All implementations verified"
else
    echo "  ✗ Untested implementations found:"
    echo "$untested"
    exit 1
fi

echo ""
echo "=== All checks passed ==="
