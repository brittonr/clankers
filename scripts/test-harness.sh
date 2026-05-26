#!/usr/bin/env bash
# Canonical local test harness for clankers.
#
# Usage:
#   ./scripts/test-harness.sh quick
#   ./scripts/test-harness.sh package <crate> [filter...]
#   ./scripts/test-harness.sh full
#   ./scripts/test-harness.sh deterministic
#   ./scripts/test-harness.sh e2e [fake|deterministic|fast|api|all|test-name]
#   ./scripts/test-harness.sh live [local-model|aspen2-qwen36|all]
#   ./scripts/test-harness.sh dogfood [bg-process-tui]
#   ./scripts/test-harness.sh vm [all|core|module|smoke|check-name]
#   ./scripts/test-harness.sh ci [extra nix args...]
#   ./scripts/test-harness.sh evidence-index
#   ./scripts/test-harness.sh list
#
# Set CLANKERS_TEST_DRY_RUN=1 to print the selected commands without running them.
set -euo pipefail

cd "$(dirname "$0")/.."

MODE="${1:-quick}"
if [[ $# -gt 0 ]]; then
    shift
fi

RESULT_DIR="${CLANKERS_TEST_RESULT_DIR:-target/test-harness}"
DRY_RUN="${CLANKERS_TEST_DRY_RUN:-0}"
RUN_ID="${CLANKERS_TEST_RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)-$$}"

case "$RUN_ID" in
    *[!A-Za-z0-9_.-]*|"")
        echo "error: CLANKERS_TEST_RUN_ID must contain only A-Z, a-z, 0-9, dot, underscore, or dash" >&2
        exit 2
        ;;
esac

RUN_DIR="$RESULT_DIR/runs/$RUN_ID"
SUMMARY_MD="$RUN_DIR/summary.md"
RESULTS_JSON="$RUN_DIR/results.json"
JUNIT_XML="$RUN_DIR/junit.xml"
LOG_DIR="$RUN_DIR/logs"
COMPAT_SUMMARY_MD="$RESULT_DIR/summary.md"
COMPAT_RESULTS_JSON="$RESULT_DIR/results.json"
COMPAT_JUNIT_XML="$RESULT_DIR/junit.xml"

mkdir -p "$LOG_DIR"

STARTED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
PAYLOAD_COMMIT="$(git rev-parse HEAD 2>/dev/null || printf 'unknown')"
PAYLOAD_BRANCH="$(git branch --show-current 2>/dev/null || true)"
if [[ -z "$PAYLOAD_BRANCH" ]]; then
    PAYLOAD_BRANCH="detached"
fi
PAYLOAD_DESCRIBE="$(git describe --tags --long --always --dirty 2>/dev/null || printf '%s' "$PAYLOAD_COMMIT")"
PAYLOAD_STATUS="$(git status --porcelain --untracked-files=no 2>/dev/null || true)"
if [[ -n "$PAYLOAD_STATUS" ]]; then
    PAYLOAD_TRACKED_DIRTY="true"
else
    PAYLOAD_TRACKED_DIRTY="false"
fi
PAYLOAD_UPSTREAM="$(git rev-parse --abbrev-ref --symbolic-full-name '@{u}' 2>/dev/null || true)"
if [[ -n "$PAYLOAD_UPSTREAM" ]]; then
    PAYLOAD_AHEAD_BEHIND="$(git rev-list --left-right --count 'HEAD...@{u}' 2>/dev/null || true)"
else
    PAYLOAD_AHEAD_BEHIND=""
fi
PASS_COUNT=0
FAIL_COUNT=0
SKIP_COUNT=0
RESULT_ITEMS=()
RESULT_NAMES=()
RESULT_STATUSES=()
RESULT_EXIT_CODES=()
RESULT_LOGS=()
RESULT_COMMANDS=()

# Keep cargo output and runtime state predictable for agent/CI runs.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target}"
export CLANKERS_NO_DAEMON="${CLANKERS_NO_DAEMON:-1}"

usage() {
    awk '
        NR == 1 { next }
        /^# ?/ {
            line = $0
            sub(/^# ?/, "", line)
            print line
            next
        }
        { exit }
    ' "$0"
}

list_profiles() {
    cat <<'EOF'
# clankers test harness profiles

## Modes

- `quick`: cargo check plus workspace nextest.
- `package <crate> [filter...]`: package-scoped cargo check plus nextest.
- `full`: fmt, check, workspace nextest, clippy, repo verify, tigerstyle, and the primary live aspen2 Qwen gate.
- `deterministic`: credential-free deterministic engine, controller, and session-resume replay fixtures.
- `e2e [fake|deterministic|fast|api|all|test-name]`: readiness E2E gates or legacy E2E selector.
- `live [local-model|aspen2-qwen36|all]`: opt-in live local-model readiness.
- `dogfood [bg-process-tui]`: local operator dogfood receipts that drive the real TUI with deterministic local stubs.
- `vm [all|core|module|smoke|check-name]`: opt-in NixOS VM readiness.
- `ci [extra nix args...]`: opt-in flake readiness adapter.
- `evidence-index`: compose Git/lifecycle state with existing local harness receipts; does not run missing readiness profiles.
- `help`: usage summary.
- `list`: this profile inventory.

## Selectors

- E2E selectors: `fake`, `deterministic`, `fast`, `api`, `all`, or a legacy test name.
- Deterministic profile: `clankers-engine` replay equivalence tests plus controller/agent replay tests and persisted session-resume replay tests with scripted provider/tool fixtures and no live credentials.
- Live selectors: `local-model`, `aspen2-qwen36`, `all`.
- Dogfood selectors: `bg-process-tui`.
- VM selectors: `all`, `core`, `module`, `smoke`.
- VM checks: `vm-smoke`, `vm-remote-daemon`, `vm-session-recovery`, `vm-plugin-runtime`, `vm-module-daemon`, `vm-module-router`, `vm-module-integration`.

## Environment

- `CLANKERS_TEST_DRY_RUN=1`: print selected commands and mark steps skipped without executing expensive gates.
- `CLANKERS_TEST_RESULT_DIR=<dir>`: override the receipt root, default `target/test-harness`.
- `CLANKERS_TEST_RUN_ID=<id>`: set a deterministic run id; allowed characters are `A-Z`, `a-z`, `0-9`, `.`, `_`, and `-`.
- `CARGO_TARGET_DIR=<dir>`: cargo target directory, default `target`.
- `CLANKERS_NO_DAEMON=1`: default harness setting for predictable local runs.

## Receipts

- Primary artifacts: `<result-dir>/runs/<run_id>/summary.md`, `<result-dir>/runs/<run_id>/results.json`, `<result-dir>/runs/<run_id>/junit.xml`.
- Primary logs: `<result-dir>/runs/<run_id>/logs/*.log`.
- Payload metadata: every `results.json` records `payload.commit`, `payload.branch`, `payload.describe`, `payload.tracked_dirty`, `payload.upstream`, and `payload.ahead_behind` captured at harness start.
- Stable compatibility artifacts: `<result-dir>/summary.md`, `<result-dir>/results.json`, `<result-dir>/junit.xml`.
EOF
}

json_escape() {
    python3 -c 'import json,sys; print(json.dumps(sys.stdin.read()))'
}

xml_escape() {
    local value="$1"
    value="${value//&/&amp;}"
    value="${value//</&lt;}"
    value="${value//>/&gt;}"
    value="${value//\"/&quot;}"
    value="${value//\'/&apos;}"
    printf '%s' "$value"
}

append_result() {
    local name="$1"
    local status="$2"
    local exit_code="$3"
    local log_file="$4"
    local command_text="$5"
    local command_json log_json
    command_json="$(printf '%s' "$command_text" | json_escape)"
    log_json="$(printf '%s' "$log_file" | json_escape)"
    RESULT_ITEMS+=("{\"name\":\"$name\",\"status\":\"$status\",\"exit_code\":$exit_code,\"command\":$command_json,\"log\":$log_json}")
    RESULT_NAMES+=("$name")
    RESULT_STATUSES+=("$status")
    RESULT_EXIT_CODES+=("$exit_code")
    RESULT_LOGS+=("$log_file")
    RESULT_COMMANDS+=("$command_text")
}

run_step() {
    local name="$1"
    shift
    local log_file="$LOG_DIR/${name//[^A-Za-z0-9_.-]/_}.log"
    local command_text="$*"

    echo "==> $name"
    echo "    $command_text"

    if [[ "$DRY_RUN" == "1" ]]; then
        printf 'DRY RUN: %s\n' "$command_text" > "$log_file"
        append_result "$name" "skipped" 0 "$log_file" "$command_text"
        SKIP_COUNT=$((SKIP_COUNT + 1))
        return 0
    fi

    set +e
    "$@" > >(tee "$log_file") 2>&1
    local exit_code=$?
    set -e

    if [[ $exit_code -eq 0 ]]; then
        append_result "$name" "passed" "$exit_code" "$log_file" "$command_text"
        PASS_COUNT=$((PASS_COUNT + 1))
    else
        append_result "$name" "failed" "$exit_code" "$log_file" "$command_text"
        FAIL_COUNT=$((FAIL_COUNT + 1))
        return "$exit_code"
    fi
}

run_shell_step() {
    local name="$1"
    shift
    run_step "$name" bash -lc "$*"
}

run_vm_check() {
    local check="$1"
    local system="$2"
    local label="${check#vm-}"
    run_step "nix vm $label" nix build ".#checks.$system.$check" --no-link -L
}

run_vm_selector() {
    local selector="${1:-all}"
    local system
    local checks=()

    system="$(nix eval --raw --impure --expr 'builtins.currentSystem')"

    case "$selector" in
        all)
            checks=(
                vm-smoke
                vm-remote-daemon
                vm-session-recovery
                vm-plugin-runtime
                vm-module-daemon
                vm-module-router
                vm-module-integration
            )
            ;;
        core)
            checks=(
                vm-smoke
                vm-remote-daemon
                vm-session-recovery
            )
            ;;
        module)
            checks=(
                vm-module-daemon
                vm-module-router
                vm-module-integration
            )
            ;;
        smoke)
            checks=(vm-smoke)
            ;;
        vm-*)
            checks=("$selector")
            ;;
        *)
            echo "error: unknown vm selector/check: $selector" >&2
            echo "known vm selectors: all, core, module, smoke" >&2
            echo "known vm checks: vm-smoke, vm-remote-daemon, vm-session-recovery, vm-plugin-runtime, vm-module-daemon, vm-module-router, vm-module-integration" >&2
            return 2
            ;;
    esac

    local check
    for check in "${checks[@]}"; do
        run_vm_check "$check" "$system"
    done
}

run_live_selector() {
    local selector="${1:-local-model}"

    case "$selector" in
        all|local-model|aspen2-qwen36)
            run_step "live readiness $selector" env CLANKERS_RUN_LIVE_READINESS=1 CLANKERS_LIVE_READINESS_SELECTOR="$selector" cargo nextest run -p clankers --test readiness_opt_in --no-fail-fast -E 'test(readiness_live_local_model_aspen2_qwen36_nextest_opt_in)'
            ;;
        *)
            echo "error: unknown live selector: $selector" >&2
            echo "known live selectors: local-model, aspen2-qwen36, all" >&2
            return 2
            ;;
    esac
}

run_dogfood_selector() {
    local selector="${1:-bg-process-tui}"

    case "$selector" in
        bg-process-tui)
            run_step "dogfood bg-process-tui" ./scripts/check-bg-process-tui-dogfood.rs
            ;;
        *)
            echo "error: unknown dogfood selector: $selector" >&2
            echo "known dogfood selectors: bg-process-tui" >&2
            return 2
            ;;
    esac
}

write_reports() {
    local finished_at items_json run_dir_json payload_branch_json payload_commit_json payload_describe_json payload_upstream_json payload_ahead_behind_json
    finished_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    run_dir_json="$(printf '%s' "$RUN_DIR" | json_escape)"
    payload_commit_json="$(printf '%s' "$PAYLOAD_COMMIT" | json_escape)"
    payload_branch_json="$(printf '%s' "$PAYLOAD_BRANCH" | json_escape)"
    payload_describe_json="$(printf '%s' "$PAYLOAD_DESCRIBE" | json_escape)"
    if [[ -n "$PAYLOAD_UPSTREAM" ]]; then
        payload_upstream_json="$(printf '%s' "$PAYLOAD_UPSTREAM" | json_escape)"
    else
        payload_upstream_json="null"
    fi
    if [[ -n "$PAYLOAD_AHEAD_BEHIND" ]]; then
        payload_ahead_behind_json="$(printf '%s' "$PAYLOAD_AHEAD_BEHIND" | json_escape)"
    else
        payload_ahead_behind_json="null"
    fi
    if [[ ${#RESULT_ITEMS[@]} -eq 0 ]]; then
        items_json="[]"
    else
        items_json="[$(IFS=,; echo "${RESULT_ITEMS[*]}")]"
    fi

    cat > "$RESULTS_JSON" <<JSON
{
  "mode": "$MODE",
  "run_id": "$RUN_ID",
  "run_dir": $run_dir_json,
  "started_at": "$STARTED_AT",
  "finished_at": "$finished_at",
  "payload": {
    "commit": $payload_commit_json,
    "branch": $payload_branch_json,
    "describe": $payload_describe_json,
    "tracked_dirty": $PAYLOAD_TRACKED_DIRTY,
    "upstream": $payload_upstream_json,
    "ahead_behind": $payload_ahead_behind_json
  },
  "passed": $PASS_COUNT,
  "failed": $FAIL_COUNT,
  "skipped": $SKIP_COUNT,
  "steps": $items_json
}
JSON

    {
        echo "# clankers test harness summary"
        echo
        echo "- mode: \`$MODE\`"
        echo "- run_id: \`$RUN_ID\`"
        echo "- run_dir: \`$RUN_DIR\`"
        echo "- started: \`$STARTED_AT\`"
        echo "- finished: \`$finished_at\`"
        echo "- payload_commit: \`$PAYLOAD_COMMIT\`"
        echo "- payload_tracked_dirty: \`$PAYLOAD_TRACKED_DIRTY\`"
        echo "- passed: $PASS_COUNT"
        echo "- failed: $FAIL_COUNT"
        echo "- skipped: $SKIP_COUNT"
        echo
        echo "## Steps"
        for item in "${RESULT_ITEMS[@]}"; do
            ITEM_JSON="$item" python3 - <<'PY'
import json
import os

item = json.loads(os.environ["ITEM_JSON"])
print(f"- {item['status']}: `{item['name']}` — `{item['command']}` ({item['log']})")
PY
        done
    } > "$SUMMARY_MD"

    {
        local total_count
        total_count="${#RESULT_NAMES[@]}"
        printf '<testsuites>\n'
        printf '  <testsuite name="%s" tests="%s" failures="%s" skipped="%s" timestamp="%s">\n' \
            "$(xml_escape "clankers test-harness $MODE")" \
            "$total_count" \
            "$FAIL_COUNT" \
            "$SKIP_COUNT" \
            "$(xml_escape "$STARTED_AT")"
        local index
        for index in "${!RESULT_NAMES[@]}"; do
            local name status exit_code log_file command_text
            name="${RESULT_NAMES[$index]}"
            status="${RESULT_STATUSES[$index]}"
            exit_code="${RESULT_EXIT_CODES[$index]}"
            log_file="${RESULT_LOGS[$index]}"
            command_text="${RESULT_COMMANDS[$index]}"
            printf '    <testcase classname="%s" name="%s">\n' \
                "$(xml_escape "test-harness.$MODE")" \
                "$(xml_escape "$name")"
            if [[ "$status" == "failed" ]]; then
                printf '      <failure message="%s">command: %s&#10;log: %s&#10;exit_code: %s</failure>\n' \
                    "$(xml_escape "exit code $exit_code")" \
                    "$(xml_escape "$command_text")" \
                    "$(xml_escape "$log_file")" \
                    "$(xml_escape "$exit_code")"
            elif [[ "$status" == "skipped" ]]; then
                printf '      <skipped message="%s"/>\n' \
                    "$(xml_escape "dry run")"
            fi
            printf '    </testcase>\n'
        done
        printf '  </testsuite>\n'
        printf '</testsuites>\n'
    } > "$JUNIT_XML"

    cp "$SUMMARY_MD" "$COMPAT_SUMMARY_MD"
    cp "$RESULTS_JSON" "$COMPAT_RESULTS_JSON"
    cp "$JUNIT_XML" "$COMPAT_JUNIT_XML"

    echo "run_id:  $RUN_ID"
    echo "run_dir: $RUN_DIR"
    echo "summary: $SUMMARY_MD"
    echo "json:    $RESULTS_JSON"
    echo "junit:   $JUNIT_XML"
    echo "latest summary: $COMPAT_SUMMARY_MD"
    echo "latest json:    $COMPAT_RESULTS_JSON"
    echo "latest junit:   $COMPAT_JUNIT_XML"
}

main() {
    case "$MODE" in
        quick)
            run_step "cargo check tests" cargo check --tests
            run_step "cargo nextest workspace" cargo nextest run --workspace --no-fail-fast
            ;;
        package)
            local package="${1:-}"
            if [[ -z "$package" ]]; then
                echo "error: package mode requires a crate/package name" >&2
                usage >&2
                return 2
            fi
            shift || true
            run_step "cargo check $package" cargo check --tests -p "$package"
            if [[ $# -gt 0 ]]; then
                run_step "cargo nextest $package filtered" cargo nextest run -p "$package" --no-fail-fast "$@"
            else
                run_step "cargo nextest $package" cargo nextest run -p "$package" --no-fail-fast
            fi
            ;;
        full)
            run_step "cargo fmt check" cargo fmt --check
            run_step "cargo check tests" cargo check --tests
            run_step "cargo nextest workspace" cargo nextest run --workspace --no-fail-fast
            run_step "cargo clippy" cargo clippy --workspace --all-targets -- -D warnings
            run_step "repo verify" ./scripts/verify.sh
            run_step "tigerstyle" ./xtask/tigerstyle.sh
            run_live_selector aspen2-qwen36
            ;;
        deterministic)
            run_step "deterministic engine replay" cargo nextest run -p clankers-engine --test deterministic_turn_replay --no-fail-fast
            run_step "deterministic controller replay" cargo nextest run -p clankers --test controller_deterministic_replay --no-fail-fast
            run_step "deterministic session resume replay" cargo nextest run -p clankers --test session_resume_deterministic_replay --no-fail-fast
            ;;
        e2e)
            local selector="${1:-fake}"
            case "$selector" in
                all|fake|deterministic)
                    run_step "e2e $selector" cargo nextest run -p clankers --test readiness_e2e --no-fail-fast -E 'test(/^readiness_e2e_/)' ;;
                fast)
                    run_step "e2e $selector" cargo nextest run -p clankers --test readiness_e2e --no-fail-fast -E 'test(readiness_e2e_version_help_config_and_auth_are_credential_free)' ;;
                api)
                    run_step "e2e $selector" cargo nextest run -p clankers --test readiness_e2e --no-fail-fast -E 'test(readiness_e2e_fake_provider_print_bash_read_find_and_json) | test(readiness_e2e_fake_provider_write_edit_read_round_trip)' ;;
                *)
                    run_step "e2e $selector" ./tests/e2e/run-tests.sh "$selector" ;;
            esac
            ;;
        live)
            run_live_selector "${1:-local-model}"
            ;;
        dogfood)
            run_dogfood_selector "${1:-bg-process-tui}"
            ;;
        vm)
            run_step "vm readiness ${1:-all}" env CLANKERS_RUN_VM_READINESS=1 CLANKERS_VM_READINESS_SELECTOR="${1:-all}" cargo nextest run -p clankers --test readiness_opt_in --no-fail-fast -E 'test(readiness_vm_required_nixos_checks_nextest_opt_in)'
            ;;
        ci)
            run_step "flake readiness" env CLANKERS_RUN_FLAKE_READINESS=1 cargo nextest run -p clankers --test readiness_opt_in --no-fail-fast -E 'test(readiness_flake_ci_nextest_opt_in)'
            ;;
        evidence-index)
            run_step "current head release evidence index" ./scripts/check-current-head-release-evidence.rs --result-dir "$RESULT_DIR" --out-dir target/release-evidence/current-head
            ;;
        list|profiles)
            list_profiles
            ;;
        help|-h|--help)
            usage
            ;;
        *)
            echo "error: unknown mode: $MODE" >&2
            usage >&2
            return 2
            ;;
    esac
}

trap write_reports EXIT
main "$@"
