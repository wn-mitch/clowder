#!/usr/bin/env bash
# Runs the continuity-canary query against an events.jsonl footer.
# Exits non-zero when any of the five canaries (grooming, play,
# mentoring, courtship, mythic-texture) fires zero times.
#
# Paired with `scripts/check_canaries.sh`: canaries gate survival
# ("colony didn't starve"); continuity gates range ("colony showed
# the behavioural repertoire the design promises"). Both are hard
# gates wrapped by `just verdict`.
#
# Ticket 250: burial removed from the canary set (was the sixth
# class). Post-247 / 248 the substrate keeps colonies healthy enough
# that deaths (and therefore burials) are genuinely rare; treating
# zero burials as a continuity defect produced false `verdict: fail`
# results across baselines. Footer tallies still emit `burial` when
# burials happen — the demotion is purely the gate.
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

# Four canary classes (burial demoted in ticket 250; mythic-texture
# demoted in ticket 445 — its contributing events
# `EventKind::ShadowFoxBanished` and `EventKind::MythicTexture` are
# rare-legend / not-yet-wired, and the BondFormed / Adopted events
# that would carry the canary in a healthy colony are blocked on
# 403/404. The tally key stays initialized in event_log.rs so events
# still increment if any fire; only the verdict gate is retired).
# Order chosen to match the headless footer's print order (CLAUDE.md
# "broaden sideways" list).
for canary in grooming play mentoring courtship; do
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
