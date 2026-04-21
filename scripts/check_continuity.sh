#!/usr/bin/env bash
# Runs the continuity-canary query against an events.jsonl footer.
# Exits non-zero when any of the six canaries (grooming, play,
# mentoring, burial, courtship, mythic-texture) fires zero times.
#
# Paired with `scripts/check_canaries.sh`: canaries gate survival
# ("colony didn't starve"); continuity gates range ("colony showed
# the behavioural repertoire the design promises"). Both are hard
# gates for the substrate refactor's autoloop.
#
# Source of truth for the canary set: docs/systems/ai-substrate-refactor.md
# §11.3, propagated into refactor-plan.md Phase 1 deliverables.

set -euo pipefail

LOGFILE="${1:-logs/events.jsonl}"

if [ ! -f "$LOGFILE" ]; then
    echo "error: logfile not found: $LOGFILE" >&2
    exit 2
fi

# Extract continuity_tallies from the footer. Emits `{}` if the field
# is missing (pre-Phase-1 events.jsonl) so we can tell "no tallies yet"
# apart from "all tallies zero".
tallies=$(jq -c 'select(._footer) | .continuity_tallies // {}' "$LOGFILE" | head -1)
if [ -z "$tallies" ] || [ "$tallies" = "null" ]; then
    tallies="{}"
fi

echo "checking continuity canaries against: $LOGFILE"

fail=0

# Six canary classes. Order chosen to match the headless footer's
# print order (CLAUDE.md "broaden sideways" list).
for canary in grooming play mentoring burial courtship mythic-texture; do
    count=$(echo "$tallies" | jq -r --arg k "$canary" '.[$k] // 0')
    count="${count:-0}"
    if [ "$count" -gt 0 ]; then
        printf "  [pass] %-16s %s (target > 0)\n" "$canary" "$count"
    else
        printf "  [FAIL] %-16s %s (target > 0)\n" "$canary" "$count"
        fail=1
    fi
done

if [ "$tallies" = "{}" ]; then
    echo "  note: continuity_tallies block absent from footer — log may be pre-Phase-1" >&2
fi

exit "$fail"
