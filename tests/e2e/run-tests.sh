#!/usr/bin/env bash
# Compatibility wrapper for Clankers credential-free E2E readiness.
#
# The release-readiness assertions live in Rust integration tests so they are
# discoverable by `cargo nextest`. This wrapper only maps the historical shell
# selectors onto nextest filters.
#
# Usage: ./tests/e2e/run-tests.sh [all|fast|api|fake|deterministic|test-name]
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" >/dev/null && pwd)
REPO_ROOT=$(cd -- "$SCRIPT_DIR/../.." >/dev/null && pwd)
cd "$REPO_ROOT"

selector="${1:-fake}"
case "$selector" in
    all|fake|deterministic)
        filter='test(/^readiness_e2e_/)' ;;
    fast)
        filter='test(readiness_e2e_version_help_config_and_auth_are_credential_free)' ;;
    api)
        filter='test(readiness_e2e_fake_provider_print_bash_read_find_and_json) | test(readiness_e2e_fake_provider_write_edit_read_round_trip)' ;;
    version|help|auth|config|paths|session)
        filter='test(readiness_e2e_version_help_config_and_auth_are_credential_free)' ;;
    print|tools|read|ls|grep|find|json)
        filter='test(readiness_e2e_fake_provider_print_bash_read_find_and_json)' ;;
    write|edit|read-write|write-edit)
        filter='test(readiness_e2e_fake_provider_write_edit_read_round_trip)' ;;
    *)
        filter="test(readiness_e2e_${selector})" ;;
esac

exec cargo nextest run -p clankers --test readiness_e2e --no-fail-fast -E "$filter"
