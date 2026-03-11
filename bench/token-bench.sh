#!/usr/bin/env bash
# token-bench.sh — A/B token usage comparison: pi vs clankers
#
# Runs identical prompts through both agents in headless mode,
# captures per-turn and cumulative token usage, then prints a
# side-by-side comparison table.
#
# Usage:
#   ./bench/token-bench.sh                     # run all prompts
#   ./bench/token-bench.sh --prompt 2          # run prompt #2 only
#   ./bench/token-bench.sh --suite read-only   # run one suite
#   ./bench/token-bench.sh --model sonnet      # override model
#   ./bench/token-bench.sh --no-thinking       # disable thinking
#   ./bench/token-bench.sh --runs 3            # repeat N times
#   ./bench/token-bench.sh --pi-only           # skip clankers
#   ./bench/token-bench.sh --clankers-only     # skip pi

set -euo pipefail

# ── Defaults ──────────────────────────────────────────────────────────
SCRIPT_DIR="$(unset CDPATH; cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(dirname "$SCRIPT_DIR")"
RESULTS_BASE="/tmp/token-bench"
CLANKERS_BIN="${CLANKERS_BIN:-${CARGO_TARGET_DIR:-$REPO_DIR/target}/debug/clankers}"
PI_BIN="${PI_BIN:-pi}"
MODEL="${BENCH_MODEL:-claude-sonnet-4-20250514}"
THINKING="${BENCH_THINKING:-off}"
RUNS=1
SUITE=""
PROMPT_NUM=""
PI_ONLY=false
CLANKERS_ONLY=false
WORKDIR="${BENCH_WORKDIR:-$REPO_DIR}"
TIMEOUT=120

# ── Colors ────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
DIM='\033[2m'
RESET='\033[0m'

# ── Parse args ────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case $1 in
        --model)      MODEL="$2"; shift 2 ;;
        --no-thinking) THINKING="off"; shift ;;
        --thinking)   THINKING="$2"; shift 2 ;;
        --runs)       RUNS="$2"; shift 2 ;;
        --suite)      SUITE="$2"; shift 2 ;;
        --prompt)     PROMPT_NUM="$2"; shift 2 ;;
        --pi-only)    PI_ONLY=true; shift ;;
        --clankers-only) CLANKERS_ONLY=true; shift ;;
        --workdir)    WORKDIR="$2"; shift 2 ;;
        --timeout)    TIMEOUT="$2"; shift 2 ;;
        --help|-h)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --model MODEL        Model to use (default: claude-sonnet-4-20250514)"
            echo "  --thinking LEVEL     Thinking level: off, low, medium, high (default: off)"
            echo "  --no-thinking        Disable thinking (same as --thinking off)"
            echo "  --runs N             Repeat each prompt N times (default: 1)"
            echo "  --suite NAME         Run only prompts in named suite"
            echo "  --prompt N           Run only prompt #N"
            echo "  --pi-only            Only run pi"
            echo "  --clankers-only      Only run clankers"
            echo "  --workdir DIR        Working directory for agents (default: repo root)"
            echo "  --timeout SECS       Per-prompt timeout (default: 120)"
            echo ""
            echo "Environment:"
            echo "  CLANKERS_BIN         Path to clankers binary"
            echo "  PI_BIN               Path to pi binary"
            echo "  BENCH_MODEL          Default model"
            echo "  BENCH_WORKDIR        Default workdir"
            exit 0
            ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

# ── Prompt suites ─────────────────────────────────────────────────────
# Format: SUITE|PROMPT
# Suites: read-only, search, edit, multi-tool, reasoning
PROMPTS=(
    "read-only|Read the file Cargo.toml and list the workspace members"
    "read-only|What license does this project use? Answer in one sentence."
    "search|Find all files that contain the word 'Usage' and list them"
    "search|How many Rust source files are in the crates/ directory?"
    "edit|Add a comment '// token-bench marker' to the top of src/main.rs then remove it"
    "multi-tool|List the 3 largest crates by line count (use wc -l on each)"
    "multi-tool|Read README.md, count its words with wc -w, and report the count"
    "reasoning|In one paragraph, explain the relationship between clankers-provider and clankers-router"
    "reasoning|What are the main differences between SessionEntry variants? List them briefly."
    "no-tools|Write a haiku about Rust programming"
)

# ── Filter prompts ────────────────────────────────────────────────────
FILTERED_PROMPTS=()
for i in "${!PROMPTS[@]}"; do
    entry="${PROMPTS[$i]}"
    suite="${entry%%|*}"
    prompt="${entry#*|}"
    idx=$((i + 1))

    if [[ -n "$PROMPT_NUM" && "$idx" != "$PROMPT_NUM" ]]; then
        continue
    fi
    if [[ -n "$SUITE" && "$suite" != "$SUITE" ]]; then
        continue
    fi
    FILTERED_PROMPTS+=("$entry")
done

if [[ ${#FILTERED_PROMPTS[@]} -eq 0 ]]; then
    echo -e "${RED}No prompts matched filters (suite=$SUITE, prompt=$PROMPT_NUM)${RESET}"
    exit 1
fi

# ── Setup results directory ───────────────────────────────────────────
TIMESTAMP=$(date +%Y%m%d-%H%M%S)
RESULTS_DIR="$RESULTS_BASE/$TIMESTAMP"
mkdir -p "$RESULTS_DIR"

# ── Verify binaries ──────────────────────────────────────────────────
echo -e "${BOLD}Token Usage Benchmark: pi vs clankers${RESET}"
echo -e "${DIM}────────────────────────────────────────────${RESET}"
echo -e "  Model:     ${CYAN}$MODEL${RESET}"
echo -e "  Thinking:  ${CYAN}$THINKING${RESET}"
echo -e "  Prompts:   ${CYAN}${#FILTERED_PROMPTS[@]}${RESET}"
echo -e "  Runs:      ${CYAN}$RUNS${RESET}"
echo -e "  Workdir:   ${CYAN}$WORKDIR${RESET}"
echo -e "  Results:   ${CYAN}$RESULTS_DIR${RESET}"

if ! $CLANKERS_ONLY; then
    if ! command -v "$PI_BIN" &>/dev/null; then
        echo -e "${RED}pi not found at $PI_BIN${RESET}"
        exit 1
    fi
    echo -e "  pi:        ${GREEN}$(command -v "$PI_BIN")${RESET}"
fi

if ! $PI_ONLY; then
    if [[ ! -x "$CLANKERS_BIN" ]]; then
        echo -e "${YELLOW}clankers not found at $CLANKERS_BIN, building...${RESET}"
        (cd "$REPO_DIR" && cargo build 2>/dev/null)
    fi
    echo -e "  clankers:  ${GREEN}$CLANKERS_BIN${RESET}"
fi
echo -e "${DIM}────────────────────────────────────────────${RESET}"
echo ""

# ── Extract usage from pi JSON output ────────────────────────────────
# Pi emits message_end events with usage in the assistant message.
# We want the final assistant message_end per prompt.
extract_pi_usage() {
    local jsonl_file="$1"
    python3 - "$jsonl_file" <<'PYEOF'
import json, sys

path = sys.argv[1]
turns = 0
total = {"input": 0, "output": 0, "cache_read": 0, "cache_write": 0, "total_tokens": 0, "cost": 0.0}
turn_data = []

def extract_usage(u):
    """Extract usage from a pi usage dict (handles both camelCase and snake_case)"""
    inp = u.get("input", u.get("input_tokens", 0))
    out = u.get("output", u.get("output_tokens", 0))
    cr = u.get("cacheRead", u.get("cache_read", u.get("cache_read_tokens", 0)))
    cw = u.get("cacheWrite", u.get("cache_write", u.get("cache_write_tokens", 0)))
    tt = u.get("totalTokens", u.get("total_tokens", inp + out))
    cost_obj = u.get("cost", {})
    cost = cost_obj.get("total", 0) if isinstance(cost_obj, dict) else (cost_obj if isinstance(cost_obj, (int, float)) else 0)
    return inp, out, cr, cw, tt, cost

with open(path) as f:
    for line in f:
        line = line.strip()
        if not line:
            continue
        try:
            evt = json.loads(line)
        except json.JSONDecodeError:
            continue

        evt_type = evt.get("type", "")

        # Pi message_end with assistant role and usage
        if evt_type == "message_end":
            msg = evt.get("message", {})
            if msg.get("role") == "assistant":
                u = msg.get("usage", {})
                if u:
                    turns += 1
                    inp, out, cr, cw, tt, cost = extract_usage(u)
                    total["input"] += inp
                    total["output"] += out
                    total["cache_read"] += cr
                    total["cache_write"] += cw
                    total["total_tokens"] += tt
                    total["cost"] += cost
                    turn_data.append({
                        "turn": turns, "input": inp, "output": out,
                        "cache_read": cr, "cache_write": cw,
                        "total_tokens": tt, "cost": round(cost, 6)
                    })

        # Also check result events (pi sometimes wraps usage differently)
        elif evt_type == "result":
            u = evt.get("usage", {})
            if u:
                turns += 1
                inp, out, cr, cw, tt, cost = extract_usage(u)
                total["input"] += inp
                total["output"] += out
                total["cache_read"] += cr
                total["cache_write"] += cw
                total["total_tokens"] += tt
                total["cost"] += cost
                turn_data.append({
                    "turn": turns, "input": inp, "output": out,
                    "cache_read": cr, "cache_write": cw,
                    "total_tokens": tt, "cost": round(cost, 6)
                })

print(json.dumps({
    "agent": "pi",
    "turns": turns,
    "cumulative": {
        "input_tokens": total["input"],
        "output_tokens": total["output"],
        "cache_read_tokens": total["cache_read"],
        "cache_write_tokens": total["cache_write"],
        "total_tokens": total["total_tokens"],
        "cost_usd": round(total["cost"], 6)
    },
    "per_turn": turn_data
}, indent=2))
PYEOF
}

# ── Extract usage from clankers JSON output ───────────────────────────
# Clankers emits {"type": "usage", "turn": {...}, "cumulative": {...}}
extract_clankers_usage() {
    local jsonl_file="$1"
    python3 - "$jsonl_file" <<'PYEOF'
import json, sys

path = sys.argv[1]
turns = 0
last_cumulative = None
turn_data = []

with open(path) as f:
    for line in f:
        line = line.strip()
        if not line:
            continue
        try:
            evt = json.loads(line)
        except json.JSONDecodeError:
            continue

        if evt.get("type") == "usage":
            turns += 1
            turn = evt.get("turn", {})
            cumul = evt.get("cumulative", {})
            last_cumulative = cumul
            turn_data.append({
                "turn": turns,
                "input": turn.get("input_tokens", 0),
                "output": turn.get("output_tokens", 0),
                "cache_read": turn.get("cache_read_tokens", 0),
                "cache_create": turn.get("cache_create_tokens", 0),
            })

if last_cumulative is None:
    last_cumulative = {"input_tokens": 0, "output_tokens": 0, "cache_read_tokens": 0, "cache_create_tokens": 0}

print(json.dumps({
    "agent": "clankers",
    "turns": turns,
    "cumulative": {
        "input_tokens": last_cumulative.get("input_tokens", 0),
        "output_tokens": last_cumulative.get("output_tokens", 0),
        "cache_read_tokens": last_cumulative.get("cache_read_tokens", 0),
        "cache_create_tokens": last_cumulative.get("cache_create_tokens", 0),
        "total_tokens": last_cumulative.get("input_tokens", 0) + last_cumulative.get("output_tokens", 0),
    },
    "per_turn": turn_data
}, indent=2))
PYEOF
}

# ── Run a single prompt through pi ────────────────────────────────────
run_pi() {
    local prompt="$1"
    local outfile="$2"
    local suite="$3"
    local tools_flag=""

    if [[ "$suite" == "no-tools" ]]; then
        tools_flag="--no-tools"
    fi

    local thinking_flag=""
    if [[ "$THINKING" != "off" ]]; then
        thinking_flag="--thinking $THINKING"
    fi

    timeout "$TIMEOUT" "$PI_BIN" \
        --mode json \
        -p "$prompt" \
        --model "$MODEL" \
        --no-skills --no-extensions --no-prompt-templates \
        --no-session \
        $tools_flag \
        $thinking_flag \
        2>/dev/null > "$outfile" || true
}

# ── Run a single prompt through clankers ──────────────────────────────
run_clankers() {
    local prompt="$1"
    local outfile="$2"
    local suite="$3"
    local tools_flag=""

    if [[ "$suite" == "no-tools" ]]; then
        tools_flag="--tools none"
    fi

    local thinking_flag=""
    if [[ "$THINKING" != "off" ]]; then
        thinking_flag="--thinking"
    fi

    timeout "$TIMEOUT" "$CLANKERS_BIN" \
        -p "$prompt" \
        --mode json \
        --model "$MODEL" \
        --auto-approve \
        --max-iterations 10 \
        $tools_flag \
        $thinking_flag \
        2>/dev/null > "$outfile" || true
}

# ── Comparison table ──────────────────────────────────────────────────
print_comparison() {
    local pi_json="$1"
    local ck_json="$2"

    python3 - "$pi_json" "$ck_json" <<'PYEOF'
import json, sys

pi_path = sys.argv[1]
ck_path = sys.argv[2]

try:
    with open(pi_path) as f:
        pi = json.load(f)
except (json.JSONDecodeError, FileNotFoundError):
    pi = None

try:
    with open(ck_path) as f:
        ck = json.load(f)
except (json.JSONDecodeError, FileNotFoundError):
    ck = None

def fmt(n):
    if n is None:
        return "—"
    return f"{n:,}"

def fmt_cost(c):
    if c is None:
        return "—"
    return f"${c:.4f}"

def pct_diff(a, b):
    if a is None or b is None or b == 0:
        return "—"
    diff = ((a - b) / b) * 100
    if diff > 0:
        return f"\033[0;31m+{diff:.0f}%\033[0m"
    elif diff < 0:
        return f"\033[0;32m{diff:.0f}%\033[0m"
    return "0%"

def get(data, *keys):
    if data is None:
        return None
    d = data
    for k in keys:
        if isinstance(d, dict):
            d = d.get(k)
        else:
            return None
    return d

pi_in = get(pi, "cumulative", "input_tokens")
pi_out = get(pi, "cumulative", "output_tokens")
pi_cr = get(pi, "cumulative", "cache_read_tokens")
pi_cw = get(pi, "cumulative", "cache_write_tokens")
pi_total = get(pi, "cumulative", "total_tokens")
pi_cost = get(pi, "cumulative", "cost_usd")
pi_turns = get(pi, "turns")

ck_in = get(ck, "cumulative", "input_tokens")
ck_out = get(ck, "cumulative", "output_tokens")
ck_cr = get(ck, "cumulative", "cache_read_tokens")
ck_cw = get(ck, "cumulative", "cache_create_tokens") or get(ck, "cumulative", "cache_write_tokens")
ck_total = get(ck, "cumulative", "total_tokens")
ck_turns = get(ck, "turns")

print(f"  {'Metric':<22} {'pi':>12} {'clankers':>12} {'Δ':>10}")
print(f"  {'─' * 22} {'─' * 12} {'─' * 12} {'─' * 10}")
print(f"  {'Turns':<22} {fmt(pi_turns):>12} {fmt(ck_turns):>12} {pct_diff(pi_turns, ck_turns):>10}")
print(f"  {'Input tokens':<22} {fmt(pi_in):>12} {fmt(ck_in):>12} {pct_diff(pi_in, ck_in):>10}")
print(f"  {'Output tokens':<22} {fmt(pi_out):>12} {fmt(ck_out):>12} {pct_diff(pi_out, ck_out):>10}")
print(f"  {'Cache read':<22} {fmt(pi_cr):>12} {fmt(ck_cr):>12} {pct_diff(pi_cr, ck_cr):>10}")
print(f"  {'Cache write':<22} {fmt(pi_cw):>12} {fmt(ck_cw):>12} {pct_diff(pi_cw, ck_cw):>10}")
print(f"  {'Total tokens':<22} {fmt(pi_total):>12} {fmt(ck_total):>12} {pct_diff(pi_total, ck_total):>10}")
if pi_cost is not None:
    print(f"  {'Est. cost':<22} {fmt_cost(pi_cost):>12} {'—':>12} {'—':>10}")
PYEOF
}

# ── Grand summary ────────────────────────────────────────────────────
print_grand_summary() {
    local results_dir="$1"

    python3 - "$results_dir" <<'PYEOF'
import json, sys, os, glob

results_dir = sys.argv[1]

pi_totals = {"input": 0, "output": 0, "cache_read": 0, "cache_write": 0, "total": 0, "cost": 0.0, "turns": 0}
ck_totals = {"input": 0, "output": 0, "cache_read": 0, "cache_write": 0, "total": 0, "turns": 0}
pi_count = 0
ck_count = 0

for f in sorted(glob.glob(os.path.join(results_dir, "*_pi_usage.json"))):
    try:
        with open(f) as fh:
            data = json.load(fh)
        c = data.get("cumulative", {})
        pi_totals["input"] += c.get("input_tokens", 0)
        pi_totals["output"] += c.get("output_tokens", 0)
        pi_totals["cache_read"] += c.get("cache_read_tokens", 0)
        pi_totals["cache_write"] += c.get("cache_write_tokens", 0)
        pi_totals["total"] += c.get("total_tokens", 0)
        pi_totals["cost"] += c.get("cost_usd", 0)
        pi_totals["turns"] += data.get("turns", 0)
        pi_count += 1
    except Exception:
        pass

for f in sorted(glob.glob(os.path.join(results_dir, "*_ck_usage.json"))):
    try:
        with open(f) as fh:
            data = json.load(fh)
        c = data.get("cumulative", {})
        ck_totals["input"] += c.get("input_tokens", 0)
        ck_totals["output"] += c.get("output_tokens", 0)
        ck_totals["cache_read"] += c.get("cache_read_tokens", 0)
        ck_totals["cache_write"] += c.get("cache_write_tokens", 0) or c.get("cache_create_tokens", 0)
        ck_totals["total"] += c.get("total_tokens", 0)
        ck_totals["turns"] += data.get("turns", 0)
        ck_count += 1
    except Exception:
        pass

def fmt(n):
    return f"{n:,}"

def pct(a, b):
    if b == 0:
        return "—"
    diff = ((a - b) / b) * 100
    if diff > 0:
        return f"\033[0;31m+{diff:.0f}%\033[0m"
    elif diff < 0:
        return f"\033[0;32m{diff:.0f}%\033[0m"
    return "0%"

print(f"\n\033[1m{'═' * 60}\033[0m")
print(f"\033[1m  GRAND TOTALS\033[0m  (pi: {pi_count} runs, clankers: {ck_count} runs)")
print(f"\033[1m{'═' * 60}\033[0m")

if pi_count > 0 and ck_count > 0:
    print(f"  {'Metric':<22} {'pi':>12} {'clankers':>12} {'Δ':>10}")
    print(f"  {'─' * 22} {'─' * 12} {'─' * 12} {'─' * 10}")
    print(f"  {'Turns':<22} {fmt(pi_totals['turns']):>12} {fmt(ck_totals['turns']):>12} {pct(pi_totals['turns'], ck_totals['turns']):>10}")
    print(f"  {'Input tokens':<22} {fmt(pi_totals['input']):>12} {fmt(ck_totals['input']):>12} {pct(pi_totals['input'], ck_totals['input']):>10}")
    print(f"  {'Output tokens':<22} {fmt(pi_totals['output']):>12} {fmt(ck_totals['output']):>12} {pct(pi_totals['output'], ck_totals['output']):>10}")
    print(f"  {'Cache read':<22} {fmt(pi_totals['cache_read']):>12} {fmt(ck_totals['cache_read']):>12} {pct(pi_totals['cache_read'], ck_totals['cache_read']):>10}")
    print(f"  {'Cache write':<22} {fmt(pi_totals['cache_write']):>12} {fmt(ck_totals['cache_write']):>12} {pct(pi_totals['cache_write'], ck_totals['cache_write']):>10}")
    print(f"  {'Total tokens':<22} {fmt(pi_totals['total']):>12} {fmt(ck_totals['total']):>12} {pct(pi_totals['total'], ck_totals['total']):>10}")
    if pi_totals['cost'] > 0:
        print(f"  {'Est. cost':<22} {'${:.4f}'.format(pi_totals['cost']):>12} {'—':>12} {'—':>10}")

    # Per-prompt averages
    print(f"\n  \033[2mAverages per prompt:\033[0m")
    print(f"  {'  pi input/prompt':<22} {fmt(pi_totals['input'] // max(pi_count, 1)):>12}")
    print(f"  {'  ck input/prompt':<22} {fmt(ck_totals['input'] // max(ck_count, 1)):>12}")
    print(f"  {'  pi output/prompt':<22} {fmt(pi_totals['output'] // max(pi_count, 1)):>12}")
    print(f"  {'  ck output/prompt':<22} {fmt(ck_totals['output'] // max(ck_count, 1)):>12}")
elif pi_count > 0:
    print(f"  pi only: {fmt(pi_totals['total'])} total tokens across {pi_count} prompts (${pi_totals['cost']:.4f})")
elif ck_count > 0:
    print(f"  clankers only: {fmt(ck_totals['total'])} total tokens across {ck_count} prompts")
else:
    print("  No results collected.")

# Save summary JSON
summary = {
    "pi": {"count": pi_count, **pi_totals},
    "clankers": {"count": ck_count, **ck_totals}
}
summary_path = os.path.join(results_dir, "summary.json")
with open(summary_path, "w") as f:
    json.dump(summary, f, indent=2)
print(f"\n  Summary saved to: {summary_path}")
PYEOF
}

# ── Main loop ─────────────────────────────────────────────────────────
total_prompts=${#FILTERED_PROMPTS[@]}
prompt_idx=0

for run in $(seq 1 "$RUNS"); do
    if [[ "$RUNS" -gt 1 ]]; then
        echo -e "\n${BOLD}━━━ Run $run / $RUNS ━━━${RESET}"
    fi

    for entry in "${FILTERED_PROMPTS[@]}"; do
        suite="${entry%%|*}"
        prompt="${entry#*|}"
        prompt_idx=$((prompt_idx + 1))
        tag=$(printf "%03d_r%d" "$prompt_idx" "$run")

        echo -e "\n${BOLD}[$prompt_idx/$((total_prompts * RUNS))]${RESET} ${YELLOW}[$suite]${RESET} $prompt"

        # ── Run pi ────────────────────────────────────────────────
        if ! $CLANKERS_ONLY; then
            echo -ne "  ${BLUE}pi${RESET}       ... "
            pi_raw="$RESULTS_DIR/${tag}_pi_raw.jsonl"
            pi_usage="$RESULTS_DIR/${tag}_pi_usage.json"
            start_time=$(date +%s%N)
            (cd "$WORKDIR" && run_pi "$prompt" "$pi_raw" "$suite")
            end_time=$(date +%s%N)
            elapsed_ms=$(( (end_time - start_time) / 1000000 ))

            if [[ -s "$pi_raw" ]]; then
                extract_pi_usage "$pi_raw" > "$pi_usage"
                pi_turns=$(python3 -c "import json; print(json.load(open('$pi_usage')).get('turns', 0))")
                pi_total=$(python3 -c "import json; print(json.load(open('$pi_usage')).get('cumulative', {}).get('total_tokens', 0))")
                echo -e "${GREEN}done${RESET} (${elapsed_ms}ms, ${pi_turns} turns, ${pi_total} tokens)"
            else
                echo "{}" > "$pi_usage"
                echo -e "${RED}no output${RESET} (${elapsed_ms}ms)"
            fi
        fi

        # ── Run clankers ──────────────────────────────────────────
        if ! $PI_ONLY; then
            echo -ne "  ${BLUE}clankers${RESET} ... "
            ck_raw="$RESULTS_DIR/${tag}_ck_raw.jsonl"
            ck_usage="$RESULTS_DIR/${tag}_ck_usage.json"
            start_time=$(date +%s%N)
            (cd "$WORKDIR" && run_clankers "$prompt" "$ck_raw" "$suite")
            end_time=$(date +%s%N)
            elapsed_ms=$(( (end_time - start_time) / 1000000 ))

            if [[ -s "$ck_raw" ]]; then
                extract_clankers_usage "$ck_raw" > "$ck_usage"
                ck_turns=$(python3 -c "import json; print(json.load(open('$ck_usage')).get('turns', 0))")
                ck_total=$(python3 -c "import json; print(json.load(open('$ck_usage')).get('cumulative', {}).get('total_tokens', 0))")
                echo -e "${GREEN}done${RESET} (${elapsed_ms}ms, ${ck_turns} turns, ${ck_total} tokens)"
            else
                echo "{}" > "$ck_usage"
                echo -e "${RED}no output${RESET} (${elapsed_ms}ms)"
            fi
        fi

        # ── Per-prompt comparison ─────────────────────────────────
        if ! $PI_ONLY && ! $CLANKERS_ONLY; then
            pi_usage="$RESULTS_DIR/${tag}_pi_usage.json"
            ck_usage="$RESULTS_DIR/${tag}_ck_usage.json"
            if [[ -s "$pi_usage" && -s "$ck_usage" ]]; then
                echo ""
                print_comparison "$pi_usage" "$ck_usage"
            fi
        fi
    done
done

# ── Grand summary ────────────────────────────────────────────────────
print_grand_summary "$RESULTS_DIR"
echo ""
echo -e "${DIM}Raw data: $RESULTS_DIR${RESET}"
