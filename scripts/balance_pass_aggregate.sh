#!/usr/bin/env bash
# Aggregate phase of the pre-alpha balance-pass GHA workflow.
#
# Reads downloaded matrix artifacts from an input directory, rearranges
# them into the layout `scripts/baseline_report.py` expects
# (`logs/baseline-<LABEL>/{sweep,trace,conditional,long-soak}/<key>/...`),
# and runs the analysis pipeline:
#
#   1. Per-run `scripts/verdict.py` against every sweep run; collect
#      pass/concern/fail tallies.
#   2. `scripts/sweep_stats.py` over the sweep directory, comparing
#      against an optional published baseline.
#   3. `scripts/sweep_stats.py` over the long-soak directory standalone
#      (separate band — 2700s runs don't share envelope with 900s).
#   4. `scripts/baseline_report.py` → REPORT.md (uses the canonical
#      sweep/trace/conditional layout).
#   5. Composes a small baseline-pack JSON (per-metric mean/stdev/p50/p95
#      across the sweep) ready for `just promote` to consume.
#
# Designed for GHA: tolerates missing matrix cells (a single failed seed
# shouldn't abort the report) and prints what it processed to stderr so
# the workflow log is self-explanatory.
#
# Usage:
#   scripts/balance_pass_aggregate.sh <input-dir> <label> [--vs <baseline-sweep-dir>]
#
# Where <input-dir> is the path GHA downloaded artifacts into; each
# matrix-cell artifact unpacks as a subdirectory named
# `sweep-<label>-<seed>` / `trace-<label>-<seed>-<focal>` /
# `conditional-<label>-<seed>-<weather>` / `long-soak-<label>-<seed>`.

set -uo pipefail

# --- args ------------------------------------------------------------------

if [[ $# -lt 2 ]]; then
    echo "usage: $0 <input-dir> <label> [--vs <baseline-sweep-dir>]" >&2
    exit 64
fi

INPUT_DIR="$1"
LABEL="$2"
shift 2

VS_BASELINE=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --vs) VS_BASELINE="$2"; shift 2 ;;
        *) echo "error: unknown arg $1" >&2; exit 64 ;;
    esac
done

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT_BASE="$REPO_ROOT/logs/baseline-$LABEL"
REPORT_DIR="$REPO_ROOT/logs/balance-pass-$LABEL"

mkdir -p "$OUT_BASE/sweep" "$OUT_BASE/trace" "$OUT_BASE/conditional" "$OUT_BASE/long-soak"
mkdir -p "$REPORT_DIR"

echo "[aggregate] input=$INPUT_DIR label=$LABEL" >&2
echo "[aggregate] output baseline-dir=$OUT_BASE" >&2
echo "[aggregate] output report-dir=$REPORT_DIR" >&2

# --- helpers ---------------------------------------------------------------

# Decompress an events.jsonl.zst if present and absent in source form.
decompress_events() {
    local dir="$1"
    if [[ -f "$dir/events.jsonl.zst" && ! -f "$dir/events.jsonl" ]]; then
        if command -v zstd >/dev/null 2>&1; then
            zstd -d -q --rm "$dir/events.jsonl.zst" -o "$dir/events.jsonl" 2>/dev/null \
                || zstd -d -q "$dir/events.jsonl.zst" -o "$dir/events.jsonl"
        else
            echo "[aggregate] WARN: zstd not installed; cannot decompress $dir/events.jsonl.zst" >&2
        fi
    fi
}

# Move a single artifact subdir's contents into the canonical layout slot.
# Args: <artifact-subdir> <dest-slot-dir>
move_run() {
    local src="$1" dest="$2"
    if [[ ! -d "$src" ]]; then
        return 0
    fi
    mkdir -p "$dest"
    # Use cp -a to keep permissions and to be tolerant of source removal.
    # We don't move because GHA download dirs are sometimes reused.
    cp -a "$src/." "$dest/"
    decompress_events "$dest"
}

# --- phase a: arrange artifacts -------------------------------------------

echo "[aggregate] arranging matrix artifacts into baseline layout..." >&2

# Sweep: artifact name `sweep-<label>-<seed>` → sweep/<seed>-1/
for src in "$INPUT_DIR"/sweep-"$LABEL"-*; do
    [[ -d "$src" ]] || continue
    name="$(basename "$src")"
    seed="${name#sweep-${LABEL}-}"
    move_run "$src" "$OUT_BASE/sweep/${seed}-1"
done

# Trace: artifact name `trace-<label>-<seed>-<focal>` → trace/<seed>-<focal>/
for src in "$INPUT_DIR"/trace-"$LABEL"-*; do
    [[ -d "$src" ]] || continue
    name="$(basename "$src")"
    rest="${name#trace-${LABEL}-}"
    move_run "$src" "$OUT_BASE/trace/${rest}"
done

# Conditional: artifact name `conditional-<label>-<seed>-<weather>` → conditional/<seed>-<weather>/
for src in "$INPUT_DIR"/conditional-"$LABEL"-*; do
    [[ -d "$src" ]] || continue
    name="$(basename "$src")"
    rest="${name#conditional-${LABEL}-}"
    move_run "$src" "$OUT_BASE/conditional/${rest}"
done

# Long-soak: artifact name `long-soak-<label>-<seed>` → long-soak/<seed>-1/
for src in "$INPUT_DIR"/long-soak-"$LABEL"-*; do
    [[ -d "$src" ]] || continue
    name="$(basename "$src")"
    seed="${name#long-soak-${LABEL}-}"
    move_run "$src" "$OUT_BASE/long-soak/${seed}-1"
done

n_sweep=$(find "$OUT_BASE/sweep" -mindepth 1 -maxdepth 1 -type d | wc -l | tr -d ' ')
n_trace=$(find "$OUT_BASE/trace" -mindepth 1 -maxdepth 1 -type d | wc -l | tr -d ' ')
n_cond=$(find "$OUT_BASE/conditional" -mindepth 1 -maxdepth 1 -type d | wc -l | tr -d ' ')
n_long=$(find "$OUT_BASE/long-soak" -mindepth 1 -maxdepth 1 -type d | wc -l | tr -d ' ')
echo "[aggregate] arranged: sweep=$n_sweep trace=$n_trace conditional=$n_cond long-soak=$n_long" >&2

if [[ "$n_sweep" -eq 0 ]]; then
    echo "[aggregate] ERROR: no sweep runs found; refusing to produce empty report" >&2
    exit 65
fi

# --- phase b: per-run verdict ---------------------------------------------

echo "[aggregate] running verdict.py on each sweep run..." >&2
VERDICT_JSONL="$REPORT_DIR/verdicts.jsonl"
: > "$VERDICT_JSONL"
pass=0; concern=0; fail=0; unprovable=0; missing=0
for run in "$OUT_BASE/sweep"/*/; do
    [[ -d "$run" ]] || continue
    if [[ ! -s "$run/events.jsonl" ]]; then
        echo "{\"run\": \"$run\", \"verdict\": \"missing-events\"}" >> "$VERDICT_JSONL"
        missing=$((missing + 1))
        continue
    fi
    # Use --no-history so we don't append CI runs to the dev verdict-history.jsonl.
    if v=$(python3 "$REPO_ROOT/scripts/verdict.py" "$run" --no-history 2>/dev/null); then
        ec=0
    else
        ec=$?
    fi
    echo "$v" >> "$VERDICT_JSONL"
    case "$ec" in
        0) pass=$((pass + 1)) ;;
        1) concern=$((concern + 1)) ;;
        2) fail=$((fail + 1)) ;;
        *) unprovable=$((unprovable + 1)) ;;
    esac
done
echo "[aggregate] verdict tallies: pass=$pass concern=$concern fail=$fail unprovable=$unprovable missing=$missing" >&2

# --- phase c: sweep-stats -------------------------------------------------

echo "[aggregate] running sweep_stats.py over sweep directory..." >&2
if [[ -n "$VS_BASELINE" && -d "$VS_BASELINE" ]]; then
    python3 "$REPO_ROOT/scripts/sweep_stats.py" "$OUT_BASE/sweep" --vs "$VS_BASELINE" \
        > "$REPORT_DIR/sweep_stats.json" 2> "$REPORT_DIR/sweep_stats.log" || true
    python3 "$REPO_ROOT/scripts/sweep_stats.py" "$OUT_BASE/sweep" --vs "$VS_BASELINE" --text \
        > "$REPORT_DIR/sweep_stats.txt" 2>> "$REPORT_DIR/sweep_stats.log" || true
else
    python3 "$REPO_ROOT/scripts/sweep_stats.py" "$OUT_BASE/sweep" \
        > "$REPORT_DIR/sweep_stats.json" 2> "$REPORT_DIR/sweep_stats.log" || true
    python3 "$REPO_ROOT/scripts/sweep_stats.py" "$OUT_BASE/sweep" --text \
        > "$REPORT_DIR/sweep_stats.txt" 2>> "$REPORT_DIR/sweep_stats.log" || true
fi

if [[ "$n_long" -gt 0 ]]; then
    echo "[aggregate] running sweep_stats.py over long-soak directory (separate band)..." >&2
    python3 "$REPO_ROOT/scripts/sweep_stats.py" "$OUT_BASE/long-soak" \
        > "$REPORT_DIR/long_soak_stats.json" 2> "$REPORT_DIR/long_soak_stats.log" || true
    python3 "$REPO_ROOT/scripts/sweep_stats.py" "$OUT_BASE/long-soak" --text \
        > "$REPORT_DIR/long_soak_stats.txt" 2>> "$REPORT_DIR/long_soak_stats.log" || true
fi

# --- phase d: baseline_report.py REPORT.md --------------------------------

echo "[aggregate] generating REPORT.md..." >&2
python3 "$REPO_ROOT/scripts/baseline_report.py" --baseline-dir "$OUT_BASE" \
    --output "$REPORT_DIR/REPORT.md" \
    --json-sidecar "$REPORT_DIR/REPORT.json" \
    2> "$REPORT_DIR/baseline_report.log" \
    || echo "[aggregate] WARN: baseline_report.py exited non-zero; partial REPORT.md may exist" >&2

# --- phase e: baseline pack JSON ------------------------------------------

echo "[aggregate] composing baseline pack JSON..." >&2
python3 - "$OUT_BASE" "$LABEL" "$REPORT_DIR/sweep_stats.json" "$REPORT_DIR/REPORT.json" "$REPORT_DIR/baseline_pack.json" <<'PY'
"""Compose a small baseline pack ready for `just promote` consumption.

Reads the sweep_stats.json envelope (per-metric mean/stdev/p50/p95 across
all sweep runs) plus the first sweep run's header (commit_hash + constants
hash) and packages a JSON file that mirrors `logs/baselines/<label>.json`
schema.
"""
import json
import sys
from pathlib import Path

base = Path(sys.argv[1])
label = sys.argv[2]
sweep_stats_path = Path(sys.argv[3])
report_sidecar_path = Path(sys.argv[4])
out_path = Path(sys.argv[5])

# Pull a sample run's header to capture commit_hash + the SimConstants
# bundle that the campaign was run against. Picks the first sweep run.
sample_header = None
sample_seed = None
for run in sorted(base.joinpath("sweep").iterdir()):
    events = run / "events.jsonl"
    if not events.exists() or events.stat().st_size == 0:
        continue
    with events.open() as f:
        first = f.readline()
        try:
            obj = json.loads(first)
            if obj.get("_header"):
                sample_header = obj
                sample_seed = run.name
                break
        except ValueError:
            continue

sweep_stats: dict = {}
if sweep_stats_path.exists():
    try:
        sweep_stats = json.loads(sweep_stats_path.read_text())
    except ValueError:
        pass

report_sidecar: dict = {}
if report_sidecar_path.exists():
    try:
        report_sidecar = json.loads(report_sidecar_path.read_text())
    except ValueError:
        pass

pack = {
    "label": label,
    "kind": "balance-pass",
    "sample_seed_dir": sample_seed,
    "commit_hash": (sample_header or {}).get("commit_hash"),
    "commit_dirty": (sample_header or {}).get("commit_dirty"),
    "start_tick": (sample_header or {}).get("start_tick"),
    "constants": (sample_header or {}).get("constants"),
    # sweep_stats.py writes the per-metric envelope under "metrics"; the
    # pack field keeps its established "per_metric" name for downstream
    # readers but now actually populates.
    "per_metric": sweep_stats.get("metrics") or [],
    "n_runs": sweep_stats.get("n"),
    "sweep_dir": str(base / "sweep"),
    # Tier A signals from baseline_report.py's JSON sidecar. Each key is
    # null when the underlying source isn't present (e.g. no trace
    # sidecars → per_dse_l2 is []).
    "per_dse_l2": report_sidecar.get("per_dse_l2") or [],
}

with out_path.open("w") as f:
    json.dump(pack, f, indent=2)
    f.write("\n")
print(f"[aggregate] wrote baseline pack → {out_path}", file=sys.stderr)
PY

# --- phase f: summary index -----------------------------------------------

cat > "$REPORT_DIR/SUMMARY.md" <<EOF
# Balance pass: $LABEL

Run on $(date -u +%Y-%m-%dT%H:%M:%SZ).

## Tallies
- sweep runs: $n_sweep
- trace runs: $n_trace
- conditional weather runs: $n_cond
- long-soak runs: $n_long
- verdict — pass: $pass, concern: $concern, fail: $fail, unprovable: $unprovable, missing-events: $missing

## Artifacts in this bundle
- \`REPORT.md\` — full per-run summary (mirrors \`logs/baseline-<L>/REPORT.md\` from baseline-dataset)
- \`sweep_stats.json\` / \`sweep_stats.txt\` — per-metric mean/stdev/p50/p95 across the sweep
$( [[ "$n_long" -gt 0 ]] && echo "- \`long_soak_stats.json\` / \`long_soak_stats.txt\` — separate band for 2700s runs" )
- \`verdicts.jsonl\` — per-run \`verdict.py\` output (one JSON object per line)
- \`baseline_pack.json\` — promotable baseline pack (commit hash + constants + per-metric stats)

## Next steps locally
\`\`\`sh
gh run download <run-id> --dir logs/balance-pass-$LABEL
# Review REPORT.md and sweep_stats.txt; if happy:
cp logs/balance-pass-$LABEL/baseline_pack.json logs/baselines/$LABEL.json
ln -sf $LABEL.json logs/baselines/current.json
\`\`\`
EOF

echo "[aggregate] done. Bundle at $REPORT_DIR" >&2
ls -la "$REPORT_DIR" >&2

exit 0
