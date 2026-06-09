#!/usr/bin/env bash
# Promote a soak directory to a named first-class baseline.
#
# Writes logs/baselines/<label>.json describing the run + a snapshot of
# the footer metrics so `just verdict` can diff against it without
# re-reading the original events.jsonl. Updates logs/baselines/current.json
# (a regular file pointing at the active baseline; symlink semantics are
# kept simple so it works on any FS).
#
# Examples:
#   just promote logs/tuned-42-state-trio post-state-trio
#   just promote logs/tuned-42-state-trio post-state-trio --no-current   # don't activate
#   just promote logs/tuned-42-state-trio post-state-trio --force        # overwrite
#   just promote ... --accept-throughput-regression  # bypass the perf ratchet
#
# Perf epic 480 — throughput ratchet: refuses to promote a candidate
# >15% slower than the active baseline (same duration budget), so the
# baseline lineage cannot quietly absorb stairstep perf regressions.
# `--accept-throughput-regression` overrides deliberately (distinct from
# `--force`, which only means "overwrite the label file").

set -euo pipefail

NO_CURRENT=0
FORCE=0
ACCEPT_TP_REGRESSION=0
RUN_DIR=""
LABEL=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --no-current) NO_CURRENT=1; shift ;;
    --force) FORCE=1; shift ;;
    --accept-throughput-regression) ACCEPT_TP_REGRESSION=1; shift ;;
    -h|--help)
      sed -n '2,15p' "$0" >&2
      exit 0 ;;
    -*) echo "promote: unknown flag $1" >&2; exit 2 ;;
    *)
      if [[ -z "$RUN_DIR" ]]; then RUN_DIR="$1"
      elif [[ -z "$LABEL" ]]; then LABEL="$1"
      else echo "promote: too many positional args" >&2; exit 2
      fi
      shift ;;
  esac
done

if [[ -z "$RUN_DIR" || -z "$LABEL" ]]; then
  echo "usage: just promote <run-dir> <label> [--no-current] [--force]" >&2
  exit 2
fi

if [[ ! "$LABEL" =~ ^[a-zA-Z0-9._-]+$ ]]; then
  echo "promote: label must be [a-zA-Z0-9._-]+ (got: $LABEL)" >&2
  exit 2
fi

EVENTS="$RUN_DIR/events.jsonl"
if [[ ! -s "$EVENTS" ]]; then
  echo "promote: $EVENTS missing or empty" >&2
  exit 2
fi

BASELINES_DIR="logs/baselines"
mkdir -p "$BASELINES_DIR"
TARGET="$BASELINES_DIR/$LABEL.json"
if [[ -e "$TARGET" && "$FORCE" -eq 0 ]]; then
  echo "promote: $TARGET already exists; pass --force to overwrite" >&2
  exit 2
fi

COMMIT=$(jq -r 'select(._header) | .commit_hash_short // ""' "$EVENTS" | head -1)
COMMIT_DIRTY=$(jq -r 'select(._header) | .commit_dirty // false' "$EVENTS" | head -1)
SEED=$(jq -r 'select(._header) | .seed // 0' "$EVENTS" | head -1)
DURATION=$(jq -r 'select(._header) | .duration_secs // 0' "$EVENTS" | head -1)
FOOTER=$(jq -c 'select(._footer)' "$EVENTS" | head -1)

# Perf epic 480 — throughput ratchet against the active baseline.
# Metric preference mirrors scripts/verdict.py::throughput_drift:
# ticks_per_sec when both footers carry it, else elapsed_ticks when the
# duration budgets match, else incomparable (ratchet skips).
CURRENT_JSON="$BASELINES_DIR/current.json"
if [[ -s "$CURRENT_JSON" && "$ACCEPT_TP_REGRESSION" -eq 0 ]]; then
  CUR_DURATION=$(jq -r '.duration_secs // 0' "$CURRENT_JSON")
  CUR_TPS=$(jq -r '.footer_snapshot.ticks_per_sec // 0' "$CURRENT_JSON")
  CUR_ET=$(jq -r '.footer_snapshot.elapsed_ticks // 0' "$CURRENT_JSON")
  CAND_TPS=$(jq -r 'select(._footer) | .ticks_per_sec // 0' "$EVENTS" | head -1)
  CAND_ET=$(jq -r 'select(._footer) | .elapsed_ticks // 0' "$EVENTS" | head -1)
  METRIC="" ; CUR_VAL=0 ; CAND_VAL=0
  if awk -v a="${CUR_TPS:-0}" -v b="${CAND_TPS:-0}" 'BEGIN{exit !(a>0 && b>0)}'; then
    METRIC="ticks_per_sec" ; CUR_VAL="$CUR_TPS" ; CAND_VAL="$CAND_TPS"
  elif [[ "$CUR_DURATION" == "$DURATION" ]] && \
       awk -v a="${CUR_ET:-0}" -v b="${CAND_ET:-0}" 'BEGIN{exit !(a>0 && b>0)}'; then
    METRIC="elapsed_ticks" ; CUR_VAL="$CUR_ET" ; CAND_VAL="$CAND_ET"
  fi
  if [[ -n "$METRIC" ]]; then
    DELTA_PCT=$(awk -v cur="$CUR_VAL" -v cand="$CAND_VAL" \
      'BEGIN{printf "%.1f", (cand - cur) / cur * 100.0}')
    if awk -v d="$DELTA_PCT" 'BEGIN{exit !(d < -15.0)}'; then
      echo "promote: REFUSED — candidate is ${DELTA_PCT}% slower than the active baseline" >&2
      echo "  metric: $METRIC  baseline: $CUR_VAL  candidate: $CAND_VAL" >&2
      echo "  Rule out CPU contention (re-run idle), or pass" >&2
      echo "  --accept-throughput-regression to promote deliberately." >&2
      exit 3
    fi
  fi
fi

PROMOTED_AT=$(date -u +%Y-%m-%dT%H:%M:%SZ)
ABS_RUN_DIR=$(cd "$RUN_DIR" && pwd)
ABS_EVENTS="$ABS_RUN_DIR/events.jsonl"

jq -n \
  --arg label "$LABEL" \
  --arg run_dir "$ABS_RUN_DIR" \
  --arg events_path "$ABS_EVENTS" \
  --arg commit "$COMMIT" \
  --argjson commit_dirty "$COMMIT_DIRTY" \
  --argjson seed "$SEED" \
  --argjson duration_secs "$DURATION" \
  --arg promoted_at "$PROMOTED_AT" \
  --argjson footer_snapshot "${FOOTER:-null}" \
  '{
     label: $label,
     run_dir: $run_dir,
     events_path: $events_path,
     commit_hash_short: $commit,
     commit_dirty: $commit_dirty,
     seed: $seed,
     duration_secs: $duration_secs,
     promoted_at: $promoted_at,
     footer_snapshot: $footer_snapshot
   }' > "$TARGET"

echo "promote: wrote $TARGET (commit $COMMIT, seed $SEED)"

if [[ "$NO_CURRENT" -eq 0 ]]; then
  CURRENT="$BASELINES_DIR/current.json"
  cp "$TARGET" "$CURRENT"
  echo "promote: $CURRENT now points at $LABEL"
fi
