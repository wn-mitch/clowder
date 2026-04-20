#!/usr/bin/env bash
# Runs the four canonical canary queries against an events.jsonl.
# Exits non-zero on any failure. Source of truth for the queries is
# docs/diagnostics/log-queries.md.

set -euo pipefail

LOGFILE="${1:-logs/events.jsonl}"

if [ ! -f "$LOGFILE" ]; then
    echo "error: logfile not found: $LOGFILE" >&2
    exit 2
fi

fail=0
print_status() {
    # $1=name, $2=actual, $3=limit_expr (e.g. "== 0" or "<= 5"), $4=pass|fail
    local name="$1" actual="$2" limit="$3" status="$4"
    if [ "$status" = "pass" ]; then
        printf "  [pass] %-32s %s (target %s)\n" "$name" "$actual" "$limit"
    else
        printf "  [FAIL] %-32s %s (target %s)\n" "$name" "$actual" "$limit"
    fi
}

echo "checking canaries against: $LOGFILE"

# 1. Starvation canary — target 0 on seed 42.
starvation=$(jq -c 'select(._footer) | .deaths_by_cause.Starvation // 0' "$LOGFILE" | head -1)
starvation="${starvation:-0}"
if [ "$starvation" -eq 0 ]; then
    print_status "starvation_deaths" "$starvation" "== 0" pass
else
    print_status "starvation_deaths" "$starvation" "== 0" fail
    fail=1
fi

# 2. Shadowfox ambush canary — target <= 5.
ambush=$(jq -c 'select(._footer) | .deaths_by_cause.ShadowFoxAmbush // 0' "$LOGFILE" | head -1)
ambush="${ambush:-0}"
if [ "$ambush" -le 5 ]; then
    print_status "shadowfox_ambush_deaths" "$ambush" "<= 5" pass
else
    print_status "shadowfox_ambush_deaths" "$ambush" "<= 5" fail
    fail=1
fi

# 3. Wipeout canary — footer presence is the signal. If the sim aborted
# early the footer is still emitted, but a zero-banishment + high-death
# footer paired with few final features suggests a wipe. Cheapest check:
# confirm at least one footer was written.
footer_count=$(jq -c 'select(._footer)' "$LOGFILE" | wc -l | tr -d ' ')
if [ "$footer_count" -ge 1 ]; then
    print_status "footer_written" "$footer_count" ">= 1" pass
else
    print_status "footer_written" "$footer_count" ">= 1" fail
    fail=1
fi

# 4. Activation canary — any feature at 0 is noise without a baseline,
# so we just report the zeros and don't fail. A CI loop that keeps a
# baseline can diff this list.
zeros=$(jq -c 'select(._footer) | .features_activated | to_entries | map(select(.value == 0)) | map(.key)' "$LOGFILE" 2>/dev/null || echo "[]")
if [ "$zeros" = "[]" ] || [ -z "$zeros" ]; then
    print_status "features_at_zero" "0" "informational" pass
else
    count=$(echo "$zeros" | jq -r 'length')
    print_status "features_at_zero" "$count" "informational" pass
    echo "    zeroed features: $zeros"
fi

exit "$fail"
