#!/usr/bin/env bash
# Enforces the time-unit typing contract (CLAUDE.md "Time-unit typing"
# section + ticket 033, Phase 6 — gate hardened).
#
# Bans raw-literal `% N` and `.is_multiple_of(N)` near a `tick`
# expression in src/systems/, src/steps/, src/ai/. Field-driven
# modulos (e.g. `tick.is_multiple_of(c.evaluate_interval)`) are
# untouched — those go through the type system via
# `IntervalPerDay::fires_at`.
#
# History: the 100-ticks-per-day stragglers from the 2026-04-10
# overhaul (CoordinationConstants::evaluate_interval, AspirationConstants::
# second_slot_check_interval, FertilityConstants::update_interval_ticks)
# survived because nothing forced consumers through a converter — this
# script is the long-term backstop, complementing the typed
# `RatePerDay` / `DurationDays` / `IntervalPerDay` API in
# src/resources/time_units.rs.
#
# Phase 6 deleted the allowlist (scripts/time_units_allowlist.txt) —
# raw-literal tick modulos are now a hard fail. Any new occurrence
# must use a typed wrapper.
#
# Wired into `just check`. Mirrors scripts/check_step_contracts.sh.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

SCAN_DIRS=("src/systems" "src/steps" "src/ai")

# Pattern hits — raw integer literal in a modulo or is_multiple_of
# adjacent to `tick`. We only flag literal numerics; field accesses
# (e.g. `c.evaluate_interval`) are the migration unit, not the bug.
hits=()
for dir in "${SCAN_DIRS[@]}"; do
    while IFS= read -r match; do
        [ -z "$match" ] && continue
        # Strip leading `// ` comments — comment matches are
        # informational, not enforceable.
        body="${match#*:*:}"
        if [[ "$body" =~ ^[[:space:]]*// ]]; then
            continue
        fi
        file="${match%%:*}"
        rest="${match#*:}"
        line="${rest%%:*}"
        key="$file:$line"
        hits+=("$key  $body")
    done < <(rg --line-number --no-heading \
        -e 'tick[[:alnum:]_]*\s*%\s*[0-9]+' \
        -e 'tick[[:alnum:]_]*\.is_multiple_of\(\s*[0-9]+' \
        "$dir" 2>/dev/null || true)
done

if [ "${#hits[@]}" -eq 0 ]; then
    echo "time-units: ok (no raw-literal tick modulos)"
    exit 0
fi

echo "time-units: raw-literal tick modulo without typed-units conversion:" >&2
for h in "${hits[@]}"; do
    echo "  $h" >&2
done
echo "" >&2
echo "Replace with a typed wrapper from src/resources/time_units.rs:" >&2
echo "  - IntervalPerDay::new(N).fires_at(tick, &time_scale)  // \"fires N times per in-game day\"" >&2
echo "  - DurationDays::new(N).ticks(&time_scale)" >&2
echo "  - DurationSeasons::new(N).ticks(&time_scale)" >&2
echo "" >&2
echo "See ticket 033 / docs/systems/time-anchor.md." >&2
exit 1
