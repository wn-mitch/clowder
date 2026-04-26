#!/usr/bin/env bash
# Build logs/sensitivity-map.json: for each leaf in SimConstants, perturb
# ±20%, run a 3-seed sweep, record per-knob → per-metric Spearman rho.
#
# Cost: O(N × 2 × 3) sweeps where N ≈ 100 SimConstants leaves; each sweep
# is a 60s release run. ~5–10 hours wall on a quiet machine — run on a
# weekend, commit the output, refresh quarterly.
#
# Output schema (logs/sensitivity-map.json):
#   {
#     "<dotted.path>": [
#       {"metric": "deaths_by_cause.Starvation", "rho": -0.84, "n": 6},
#       ...
#     ],
#     ...
#   }
#
# Usage:
#   just rebuild-sensitivity-map                 # all leaves
#   just rebuild-sensitivity-map magic.*         # only magic.* leaves (glob)
#   just rebuild-sensitivity-map --duration 30   # cheaper smoke
#
# Implementation defers to scripts/build_sensitivity_map.py for the
# Spearman computation; this shell entry-point handles the sweep loop
# and per-knob orchestration.

set -uo pipefail

DURATION=60
SEEDS="42 99 7"
GLOB=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --duration) DURATION="$2"; shift 2 ;;
    --seeds)    SEEDS="$2"; shift 2 ;;
    -h|--help)  sed -n '2,28p' "$0" >&2; exit 0 ;;
    -*) echo "build-sensitivity-map: unknown flag $1" >&2; exit 2 ;;
    *)
      if [[ -z "$GLOB" ]]; then GLOB="$1"; else echo "too many args" >&2; exit 2; fi
      shift ;;
  esac
done

# Build release binary up-front so each sweep doesn't pay rebuild cost.
if [[ ! -x target/release/clowder ]]; then
  echo "build-sensitivity-map: building release binary..." >&2
  cargo build --release || exit 2
fi

# Discover every leaf path. `just explain --list` does the right thing
# but requires a recent events.jsonl with a constants block — fall back
# to a quick probe run if none exists.
PROBE_LOG=""
if ! uv run scripts/explain_constant.py --list >/tmp/sm-leaves 2>/dev/null; then
  PROBE_LOG=logs/sensitivity-probe-$$
  mkdir -p "$PROBE_LOG"
  ./target/release/clowder --headless --seed 42 --duration 5 \
    --log "$PROBE_LOG/narrative.jsonl" \
    --event-log "$PROBE_LOG/events.jsonl" >&2
  uv run scripts/explain_constant.py --list --run "$PROBE_LOG/events.jsonl" \
    > /tmp/sm-leaves
  rm -rf "$PROBE_LOG"
fi

if [[ -n "$GLOB" ]]; then
  pattern="${GLOB/\*/.*}"
  grep -E "^${pattern}$" /tmp/sm-leaves > /tmp/sm-leaves-filtered
  mv /tmp/sm-leaves-filtered /tmp/sm-leaves
fi

total=$(wc -l < /tmp/sm-leaves | tr -d ' ')
echo "build-sensitivity-map: $total leaves × 2 levels × $(echo $SEEDS | wc -w | tr -d ' ') seeds, ${DURATION}s each" >&2

work=logs/sensitivity-build
mkdir -p "$work"

# For each leaf:
#   1. Read default value from a baseline header.
#   2. Run -20% sweep and +20% sweep with CLOWDER_OVERRIDES patch.
#   3. Hand the bundle to build_sensitivity_map.py to compute rho.
DEFAULTS=$(uv run scripts/explain_constant.py --list 2>/dev/null | head -1 || true)
i=0
while IFS= read -r path; do
  i=$((i+1))
  echo "  [$i/$total] $path" >&2
  uv run scripts/explain_constant.py "$path" --text 2>/dev/null \
    | awk '/value:/ { print $2; exit }' > /tmp/sm-val
  default=$(cat /tmp/sm-val)
  if [[ -z "$default" || "$default" == "None" ]]; then
    echo "    skip (no value)" >&2
    continue
  fi
  for level in down up; do
    if [[ "$level" == "down" ]]; then
      patch_val=$(awk -v v="$default" 'BEGIN { print v * 0.8 }')
    else
      patch_val=$(awk -v v="$default" 'BEGIN { print v * 1.2 }')
    fi
    # Build nested JSON for the dotted path.
    overrides=$(uv run python3 -c "
import json, sys
path = '$path'.split('.')
val = $patch_val
out = val
for p in reversed(path):
  out = {p: out}
print(json.dumps(out))
")
    out="$work/${path//./_}-$level"
    mkdir -p "$out"
    for seed in $SEEDS; do
      CLOWDER_OVERRIDES="$overrides" \
        ./target/release/clowder --headless --seed "$seed" --duration "$DURATION" \
        --log "$out/${seed}-narrative.jsonl" \
        --event-log "$out/${seed}-events.jsonl" >/dev/null 2>&1 \
        || { echo "    skip seed $seed (run failed)" >&2; continue; }
    done
  done
done < /tmp/sm-leaves

uv run scripts/build_sensitivity_map.py "$work" \
  --output logs/sensitivity-map.json
echo "build-sensitivity-map: wrote logs/sensitivity-map.json" >&2
