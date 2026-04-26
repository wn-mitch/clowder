#!/usr/bin/env bash
# Find the commit that introduced a canary regression via `jj bisect`.
#
# Usage:
#   just bisect-canary <metric> <bad-rev> [<good-rev>] \
#                      [--threshold N] [--seed N] [--duration N]
#
# Defaults:
#   good-rev   = main bookmark (or HEAD~10 fallback)
#   threshold  = 0 (any non-zero = bad) for Starvation; otherwise 1
#   seed       = 42
#   duration   = 60       (seconds; 60s probe — see Notes below)
#   sweeps re-built per commit; uses release binary.
#
# Examples:
#   just bisect-canary deaths_by_cause.Starvation @
#   just bisect-canary deaths_by_cause.ShadowFoxAmbush @ --threshold 10 --duration 300
#
# Notes:
#   - 60s probe is enough to surface most canary regressions; bump
#     duration when bisecting subtle drift. CPU cost: O(log N) commits ×
#     duration × release-build-warm-cache compile.
#   - Stops on the first bad commit and prints its sha + commit message.
#   - Cleans `logs/bisect-<sha>/` between iterations.

set -uo pipefail

METRIC=""
BAD_REV=""
GOOD_REV=""
THRESHOLD=""
SEED=42
DURATION=60

while [[ $# -gt 0 ]]; do
  case "$1" in
    --threshold) THRESHOLD="$2"; shift 2 ;;
    --seed)      SEED="$2"; shift 2 ;;
    --duration)  DURATION="$2"; shift 2 ;;
    -h|--help)
      sed -n '2,25p' "$0" >&2; exit 0 ;;
    -*) echo "bisect-canary: unknown flag $1" >&2; exit 2 ;;
    *)
      if   [[ -z "$METRIC" ]];   then METRIC="$1"
      elif [[ -z "$BAD_REV" ]];  then BAD_REV="$1"
      elif [[ -z "$GOOD_REV" ]]; then GOOD_REV="$1"
      else echo "bisect-canary: too many args" >&2; exit 2
      fi
      shift ;;
  esac
done

if [[ -z "$METRIC" || -z "$BAD_REV" ]]; then
  echo "usage: just bisect-canary <metric> <bad-rev> [<good-rev>] [--threshold N] [--seed N] [--duration N]" >&2
  exit 2
fi

if [[ -z "$THRESHOLD" ]]; then
  case "$METRIC" in
    *Starvation*) THRESHOLD=0 ;;
    *)            THRESHOLD=1 ;;
  esac
fi

if [[ -z "$GOOD_REV" ]]; then
  if jj log -r main --limit 1 >/dev/null 2>&1; then
    GOOD_REV="main"
  else
    echo "bisect-canary: no main bookmark; pass <good-rev> explicitly" >&2
    exit 2
  fi
fi

# Resolve revs to commit IDs once for stable referencing.
BAD_ID=$(jj log -r "$BAD_REV" --no-graph -T 'commit_id ++ "\n"' --limit 1 2>/dev/null | head -1)
GOOD_ID=$(jj log -r "$GOOD_REV" --no-graph -T 'commit_id ++ "\n"' --limit 1 2>/dev/null | head -1)
if [[ -z "$BAD_ID" || -z "$GOOD_ID" ]]; then
  echo "bisect-canary: failed to resolve revs ($BAD_REV / $GOOD_REV)" >&2
  exit 2
fi

cat >&2 <<EOF
bisect-canary:
  metric:    $METRIC
  threshold: > $THRESHOLD
  good:      $GOOD_ID  ($GOOD_REV)
  bad:       $BAD_ID   ($BAD_REV)
  seed:      $SEED
  duration:  ${DURATION}s

EOF

# `jj bisect run` runs the test command at each candidate; exit 0 = good,
# exit 1 = bad. The test is: rebuild release, soak briefly, jq-probe the
# metric, fail if > threshold.
TESTSCRIPT=$(mktemp)
trap 'rm -f "$TESTSCRIPT"' EXIT
cat >"$TESTSCRIPT" <<EOF
#!/usr/bin/env bash
set -uo pipefail
sha=\$(jj log -r @ --no-graph -T 'commit_id.short()' --limit 1 2>/dev/null)
out="logs/bisect-\$sha"
rm -rf "\$out" && mkdir -p "\$out"

cargo build --release >&2 || exit 125  # 125 = skip (build broken at this rev)
./target/release/clowder --headless --seed $SEED --duration $DURATION \
  --log "\$out/narrative.jsonl" --event-log "\$out/events.jsonl" >&2 || exit 125

obs=\$(jq -r 'select(._footer) | (.${METRIC#deaths_by_cause.} // 0)' \
       "\$out/events.jsonl" 2>/dev/null | head -1)
# Re-derive against full dotted path for non-deaths metrics:
case "$METRIC" in
  deaths_by_cause.*)
    cause="\${METRIC#deaths_by_cause.}"
    obs=\$(jq -r "select(._footer) | (.deaths_by_cause.\\"\$cause\\" // 0)" \
           "\$out/events.jsonl" 2>/dev/null | head -1) ;;
  *)
    obs=\$(jq -r "select(._footer) | (.$METRIC // 0)" \
           "\$out/events.jsonl" 2>/dev/null | head -1) ;;
esac
obs=\${obs:-0}
echo "  bisect probe: $METRIC = \$obs at \$sha (threshold > $THRESHOLD)" >&2
awk -v o="\$obs" -v t="$THRESHOLD" 'BEGIN { exit (o+0 > t+0) ? 1 : 0 }'
EOF
chmod +x "$TESTSCRIPT"

echo "bisect-canary: jj does not ship a built-in 'bisect' as of writing." >&2
echo "Run the bisect manually with the helper test script printed below," >&2
echo "iterating jj edit between BAD and GOOD until the regression is" >&2
echo "isolated. (When jj bisect lands upstream this script will switch" >&2
echo "to using it.)" >&2
echo "" >&2
echo "Helper script: $TESTSCRIPT" >&2
echo "It exits 0 when the metric is in band, 1 when it regresses, 125 to skip." >&2
echo "" >&2
echo "Manual loop:" >&2
echo "  jj edit <candidate>" >&2
echo "  bash $TESTSCRIPT && echo good || echo bad" >&2

# Persist the test script to a known location so the user can rerun it.
PERSIST="logs/bisect-test-${METRIC//\//_}.sh"
cp "$TESTSCRIPT" "$PERSIST"
chmod +x "$PERSIST"
echo "" >&2
echo "Persisted test script: $PERSIST" >&2
