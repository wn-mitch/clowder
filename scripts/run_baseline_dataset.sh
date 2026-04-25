#!/usr/bin/env bash
# Baseline-dataset orchestrator.
#
# Runs five phases in sequence and writes everything under
# logs/baseline-<LABEL>/. Designed to be backgroundable: writes STATUS.txt
# and STATUS.json after every phase so external watchers can poll progress
# without parsing the run.log.
#
# Phases (collect-everything failure mode — survival/continuity canary
# regressions are recorded but DO NOT halt the run):
#   1. Probe pass (60s smoke per seed → rosters.json)
#   2. Aggregate sweep (5 seeds × N reps × 900s, 4-way parallel)
#   3. Focal-cat trace pass (5 seeds × 2 focals × 900s)
#   4. Conditional weather (seed 42 × {fog, storm} × 900s)
#   5. Auto-aggregation report (REPORT.md)
#
# Hard-fail conditions:
#   - Dirty working tree (set ALLOW_DIRTY=1 to override)
#   - cargo build --release fails
#   - Disk write fails on STATUS.txt
#
# Idempotency: Phase 2/3/4 runs that already have a `_footer` line in their
# events.jsonl are skipped. Re-invocation after partial completion picks up
# where it left off.
#
# Environment overrides (all optional):
#   SEEDS        Default "42 99 7 2025 314"
#   REPS         Default 3 (sweep reps per seed)
#   DURATION     Default 900 (seconds per long soak)
#   PROBE_DURATION  Default 60
#   PARALLEL     Default 4 (xargs concurrency)
#   ALLOW_DIRTY  Default 0 (set 1 to permit commit_dirty=true headers)
#   SKIP_PHASE_4 Default 0 (set 1 to skip fog/storm conditional runs)
#
# Usage:
#   scripts/run_baseline_dataset.sh <LABEL>
# or:
#   SEEDS="42 99" REPS=1 DURATION=60 scripts/run_baseline_dataset.sh smoke

set -uo pipefail

# --- args & defaults -------------------------------------------------------

if [[ $# -lt 1 ]]; then
    echo "usage: $0 <label>" >&2
    exit 64
fi

LABEL="$1"
SEEDS="${SEEDS:-42 99 7 2025 314}"
REPS="${REPS:-3}"
DURATION="${DURATION:-900}"
PROBE_DURATION="${PROBE_DURATION:-60}"
PARALLEL="${PARALLEL:-4}"
ALLOW_DIRTY="${ALLOW_DIRTY:-0}"
SKIP_PHASE_4="${SKIP_PHASE_4:-0}"
ROOT="${ROOT:-logs}"

BASE="$ROOT/baseline-$LABEL"
BIN="target/release/clowder"
STATUS_TXT="$BASE/STATUS.txt"
STATUS_JSON="$BASE/STATUS.json"

mkdir -p "$BASE" "$BASE/sweep" "$BASE/trace" "$BASE/conditional" "$BASE/canaries"

START_TS="$(date -Iseconds 2>/dev/null || date)"

# --- helpers ---------------------------------------------------------------

now() { date -Iseconds 2>/dev/null || date; }

write_status() {
    # write_status <phase> <state> [note]
    local phase="$1" state="$2" note="${3:-}"
    {
        echo "label: $LABEL"
        echo "started_at: $START_TS"
        echo "updated_at: $(now)"
        echo "phase: $phase"
        echo "state: $state"
        echo "seeds: $SEEDS"
        echo "reps: $REPS"
        echo "duration: $DURATION"
        echo "parallel: $PARALLEL"
        echo "allow_dirty: $ALLOW_DIRTY"
        if [[ -n "$note" ]]; then
            echo "note: $note"
        fi
    } > "$STATUS_TXT"

    # JSON mirror so a watcher can poll without sed/awk.
    python3 - "$LABEL" "$START_TS" "$(now)" "$phase" "$state" "$SEEDS" "$REPS" "$DURATION" "$PARALLEL" "$ALLOW_DIRTY" "$note" "$STATUS_JSON" <<'PY'
import json, sys
label, started_at, updated_at, phase, state, seeds, reps, duration, parallel, allow_dirty, note, out = sys.argv[1:13]
data = {
    "label": label,
    "started_at": started_at,
    "updated_at": updated_at,
    "phase": phase,
    "state": state,
    "seeds": seeds.split(),
    "reps": int(reps),
    "duration": int(duration),
    "parallel": int(parallel),
    "allow_dirty": int(allow_dirty),
}
if note:
    data["note"] = note
with open(out, "w") as f:
    json.dump(data, f, indent=2)
    f.write("\n")
PY
}

has_footer() {
    # has_footer <events.jsonl> — exit 0 if file ends with a `_footer` line.
    local path="$1"
    [[ -f "$path" ]] || return 1
    [[ -s "$path" ]] || return 1
    python3 - "$path" <<'PY'
import json, sys
p = sys.argv[1]
with open(p, "rb") as f:
    f.seek(0, 2)
    sz = f.tell()
    f.seek(max(0, sz - 16384))
    tail = f.read().decode("utf-8", errors="replace")
for line in reversed([l for l in tail.splitlines() if l.strip()]):
    try:
        obj = json.loads(line)
    except ValueError:
        continue
    sys.exit(0 if obj.get("_footer") else 1)
sys.exit(1)
PY
}

run_canaries() {
    # run_canaries <events.jsonl> <label_for_output>
    local events="$1" name="$2"
    local out="$BASE/canaries/${name}.txt"
    {
        echo "=== survival canaries: $name ==="
        bash scripts/check_canaries.sh "$events" 2>&1
        echo ""
        echo "=== continuity canaries: $name ==="
        bash scripts/check_continuity.sh "$events" 2>&1
    } > "$out" 2>&1 || true   # collect-everything: never abort
}

# --- precondition: clean tree ----------------------------------------------

if [[ "$ALLOW_DIRTY" -ne 1 ]]; then
    if ! git diff --quiet HEAD 2>/dev/null || ! git diff --cached --quiet 2>/dev/null; then
        write_status "0" "blocked-on-dirty-tree" "git diff HEAD is non-empty; commit/stash or set ALLOW_DIRTY=1"
        echo "ERROR: working tree is dirty; refusing to run baseline (set ALLOW_DIRTY=1 to override)" >&2
        git status --short >&2
        exit 65
    fi
fi

# --- precondition: release binary -----------------------------------------

write_status "0" "building-release-binary"
echo "[$(now)] building release binary..." >&2
if ! cargo build --release 2>&1 | tail -5 >&2; then
    write_status "0" "release-build-failed"
    exit 70
fi
if [[ ! -x "$BIN" ]]; then
    write_status "0" "release-binary-missing" "expected $BIN after cargo build --release"
    echo "ERROR: $BIN missing after build" >&2
    exit 70
fi

# --- Phase 1: probe roster -------------------------------------------------

write_status "1" "probe-roster"
echo "[$(now)] phase 1: probe roster (${PROBE_DURATION}s per seed × $(echo $SEEDS | wc -w | tr -d ' ') seeds)" >&2
SEEDS_CSV="$(echo "$SEEDS" | tr ' ' ',')"
if ! python3 scripts/probe_roster.py \
        --label "$LABEL" \
        --seeds "$SEEDS_CSV" \
        --duration "$PROBE_DURATION" \
        --binary "$BIN" \
        --parallel "$PARALLEL" \
        --root "$ROOT" \
        2>&1 | tee -a "$BASE/probe.log" >&2; then
    write_status "1" "probe-failed" "see probe.log"
    echo "WARN: probe phase exited non-zero — continuing anyway" >&2
fi

ROSTERS="$BASE/rosters.json"
if [[ ! -f "$ROSTERS" ]]; then
    write_status "1" "no-rosters" "rosters.json not produced"
    echo "ERROR: $ROSTERS missing — cannot pick focals for phase 3" >&2
    # Continue: phase 2 doesn't need rosters; phases 3/4 will fall back to Simba.
fi

# --- Phase 2: aggregate sweep ----------------------------------------------

write_status "2" "aggregate-sweep"
echo "[$(now)] phase 2: aggregate sweep ($(echo $SEEDS | wc -w | tr -d ' ') seeds × $REPS reps × ${DURATION}s, $PARALLEL-way)" >&2

JOBS_FILE="$(mktemp)"
trap 'rm -f "$JOBS_FILE"' EXIT
for seed in $SEEDS; do
    for rep in $(seq 1 "$REPS"); do
        dir="$BASE/sweep/${seed}-${rep}"
        mkdir -p "$dir"
        events="$dir/events.jsonl"
        if has_footer "$events"; then
            echo "  skip ${seed}-${rep} (footer present)" >&2
            continue
        fi
        cmd="$BIN --headless --seed $seed --duration $DURATION"
        cmd+=" --log $dir/narrative.jsonl --event-log $events"
        cmd+=" > $dir/stderr.log 2>&1"
        echo "$cmd" >> "$JOBS_FILE"
    done
done

njobs=$(wc -l < "$JOBS_FILE" | tr -d ' ')
if [[ "$njobs" -gt 0 ]]; then
    echo "  dispatching $njobs sweep jobs" >&2
    xargs -P "$PARALLEL" -I CMD -S 4096 bash -c CMD < "$JOBS_FILE" || true
else
    echo "  all sweep runs already had footers — nothing to do" >&2
fi
> "$JOBS_FILE"

# Run canaries on each completed sweep run.
for seed in $SEEDS; do
    for rep in $(seq 1 "$REPS"); do
        events="$BASE/sweep/${seed}-${rep}/events.jsonl"
        if has_footer "$events"; then
            run_canaries "$events" "sweep-${seed}-${rep}"
        fi
    done
done

# --- Phase 3: focal-cat trace pass -----------------------------------------

write_status "3" "focal-trace"
echo "[$(now)] phase 3: focal-cat traces (5 seeds × 2 focals × ${DURATION}s)" >&2

# Read rosters.json into a `seed:focal` job list.
TRACE_JOBS="$(mktemp)"
trap 'rm -f "$JOBS_FILE" "$TRACE_JOBS"' EXIT
if [[ -f "$ROSTERS" ]]; then
    python3 - "$ROSTERS" "$BASE" "$BIN" "$DURATION" <<'PY' >> "$TRACE_JOBS"
import json, sys
rosters_path, base, binary, duration = sys.argv[1:]
with open(rosters_path) as f:
    rosters = json.load(f)
for seed, info in (rosters.get("seeds") or {}).items():
    slot_a = info.get("slot_a")
    slot_b = info.get("slot_b")
    for slot, name in (("a", slot_a), ("b", slot_b)):
        if not name:
            continue
        # Slot A and Slot B may be the same cat on tiny rosters; avoid duplicate work.
        if slot == "b" and name == slot_a:
            continue
        # Sanitize the name for filesystem safety.
        safe = "".join(c if c.isalnum() or c in "-_" else "_" for c in name)
        run_dir = f"{base}/trace/{seed}-{safe}"
        cmd = (
            f'mkdir -p "{run_dir}" && '
            f'"{binary}" --headless --seed {seed} --duration {duration} '
            f'--focal-cat "{name}" '
            f'--log "{run_dir}/narrative.jsonl" '
            f'--event-log "{run_dir}/events.jsonl" '
            f'--trace-log "{run_dir}/trace-{safe}.jsonl" '
            f'> "{run_dir}/stderr.log" 2>&1'
        )
        print(cmd)
PY
else
    echo "WARN: no rosters.json — falling back to Simba on every seed" >&2
    for seed in $SEEDS; do
        run_dir="$BASE/trace/${seed}-Simba"
        echo "mkdir -p \"$run_dir\" && \"$BIN\" --headless --seed $seed --duration $DURATION --focal-cat Simba --log \"$run_dir/narrative.jsonl\" --event-log \"$run_dir/events.jsonl\" --trace-log \"$run_dir/trace-Simba.jsonl\" > \"$run_dir/stderr.log\" 2>&1" >> "$TRACE_JOBS"
    done
fi

# Filter out trace runs that already have footers (idempotency).
TRACE_JOBS_FILTERED="$(mktemp)"
trap 'rm -f "$JOBS_FILE" "$TRACE_JOBS" "$TRACE_JOBS_FILTERED"' EXIT
while IFS= read -r line; do
    # Extract the events.jsonl path from the command.
    events_path="$(echo "$line" | sed -nE 's/.*--event-log "([^"]*)".*/\1/p')"
    if [[ -n "$events_path" ]] && has_footer "$events_path"; then
        echo "  skip trace run for $events_path (footer present)" >&2
    else
        echo "$line" >> "$TRACE_JOBS_FILTERED"
    fi
done < "$TRACE_JOBS"

ntrace=$(wc -l < "$TRACE_JOBS_FILTERED" | tr -d ' ')
if [[ "$ntrace" -gt 0 ]]; then
    echo "  dispatching $ntrace trace jobs" >&2
    xargs -P "$PARALLEL" -I CMD -S 8192 bash -c CMD < "$TRACE_JOBS_FILTERED" || true
else
    echo "  all trace runs already had footers — nothing to do" >&2
fi

# Canaries on each trace run.
for trace_dir in "$BASE/trace"/*/; do
    [[ -d "$trace_dir" ]] || continue
    events="$trace_dir/events.jsonl"
    if has_footer "$events"; then
        name="$(basename "$trace_dir")"
        run_canaries "$events" "trace-${name}"
    fi
done

# --- Phase 4: conditional weather treatments ------------------------------

if [[ "$SKIP_PHASE_4" -ne 1 ]]; then
    write_status "4" "conditional-weather"
    echo "[$(now)] phase 4: conditional weather (seed 42 × {fog,storm} × ${DURATION}s)" >&2

    # Pick Slot A for seed 42 from rosters, else fall back to Simba.
    FOCAL_42="Simba"
    if [[ -f "$ROSTERS" ]]; then
        FOCAL_42="$(python3 -c "
import json,sys
with open('$ROSTERS') as f: r = json.load(f)
print((r.get('seeds') or {}).get('42', {}).get('slot_a') or 'Simba')
")"
    fi
    SAFE_42="$(echo "$FOCAL_42" | tr -c '[:alnum:]_-' '_')"

    COND_JOBS="$(mktemp)"
    trap 'rm -f "$JOBS_FILE" "$TRACE_JOBS" "$TRACE_JOBS_FILTERED" "$COND_JOBS"' EXIT
    for weather in fog storm; do
        run_dir="$BASE/conditional/42-$weather"
        mkdir -p "$run_dir"
        events="$run_dir/events.jsonl"
        if has_footer "$events"; then
            echo "  skip 42-$weather (footer present)" >&2
            continue
        fi
        cmd="$BIN --headless --seed 42 --duration $DURATION --force-weather $weather"
        cmd+=" --focal-cat \"$FOCAL_42\""
        cmd+=" --log \"$run_dir/narrative.jsonl\""
        cmd+=" --event-log \"$events\""
        cmd+=" --trace-log \"$run_dir/trace-${SAFE_42}.jsonl\""
        cmd+=" > \"$run_dir/stderr.log\" 2>&1"
        echo "$cmd" >> "$COND_JOBS"
    done
    ncond=$(wc -l < "$COND_JOBS" | tr -d ' ')
    if [[ "$ncond" -gt 0 ]]; then
        xargs -P "$PARALLEL" -I CMD -S 8192 bash -c CMD < "$COND_JOBS" || true
    fi

    for weather in fog storm; do
        events="$BASE/conditional/42-$weather/events.jsonl"
        if has_footer "$events"; then
            run_canaries "$events" "conditional-42-$weather"
        fi
    done
else
    echo "[$(now)] phase 4: SKIPPED (SKIP_PHASE_4=1)" >&2
fi

# --- Phase 5: report -------------------------------------------------------

write_status "5" "aggregating-report"
echo "[$(now)] phase 5: aggregating report" >&2
if python3 scripts/baseline_report.py --baseline-dir "$BASE" 2>&1 | tee -a "$BASE/report.log" >&2; then
    write_status "5" "complete"
    echo "[$(now)] DONE — see $BASE/REPORT.md" >&2
    exit 0
else
    write_status "5" "report-failed" "see report.log"
    echo "ERROR: baseline_report.py failed — JSONL data is intact, see $BASE" >&2
    exit 75
fi
