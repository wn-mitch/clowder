#!/usr/bin/env bash
# Enforces marker-snapshot population coverage for DSE eligibility
# filters (ticket 217).
#
# The substrate-stub lint (`scripts/check_substrate_stubs.sh`) catches
# whether a marker has a writer and ANY reader. This lint catches a
# distinct failure mode: a marker has a writer (e.g.
# `commands.entity(e).insert(M)` in a capability-author system) AND a
# reader via `Has<M>` (the same writer reading its own state to avoid
# archetype churn), BUT the marker is NOT plumbed into
# `MarkerSnapshot` via `set_entity(M::KEY, ...)` or
# `set_colony(M::KEY, ...)` in `goap.rs::evaluate_and_plan`. The DSE's
# `EligibilityFilter::require(M::KEY)` then reads the snapshot, finds
# the marker absent, and silently fails eligibility — for the entire
# soak.
#
# Precedents:
# - Ticket 209 (2026-05-07): GroomOther's HasGroomingCandidate authored
#   on the ECS but never copied into MarkerSnapshot. Cost ~25min and
#   one soak loop to localize.
# - Ticket 084 Commit 3 (2026-05-19): CanWardFromSupply authored by
#   `capabilities.rs::update_capability_markers` but never populated
#   into MarkerSnapshot. Caused 0 Thornward placements on seed-42
#   (4 pre-baseline) and required a sub-soak investigation to localize.
#
# The lint:
#   1. Greps every `.require(markers::<Name>::KEY)` reference under
#      `src/ai/dses/**/*.rs` (cat DSEs that participate in the
#      MarkerSnapshot scoring surface).
#   2. For each required marker, confirms a
#      `set_entity(markers::<Name>::KEY` OR
#      `set_colony(markers::<Name>::KEY` populator call appears in
#      `src/systems/goap.rs` (the canonical populator).
#   3. Reports any mismatch with the specific marker name.
#
# Out of scope:
#   - Fox / hawk / snake DSEs (use separate populators in
#     `*_goap.rs` files).
#   - Plain `.require("string")` literals — currently no DSEs use
#     literal strings; all go through `markers::X::KEY`.
#
# Wired into `just check`.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

DSE_GLOB="src/ai/dses"
POPULATOR_FILE="src/systems/goap.rs"
ALLOWLIST="scripts/substrate_stubs.allowlist"

if [ ! -f "$POPULATOR_FILE" ]; then
    echo "marker-snapshot-wiring: missing populator file $POPULATOR_FILE" >&2
    exit 1
fi

# Reuse the substrate-stub allowlist — markers like `HideEligible`
# that are intentionally dormant (no authoring system, by design)
# are already named there with the wiring ticket.
allowlist=()
if [ -f "$ALLOWLIST" ]; then
    while IFS= read -r line; do
        line="${line%%#*}"
        line="${line#"${line%%[![:space:]]*}"}"
        line="${line%"${line##*[![:space:]]}"}"
        if [ -n "$line" ]; then
            name="${line%% *}"
            allowlist+=("$name")
        fi
    done < "$ALLOWLIST"
fi

is_allowlisted() {
    local name="$1"
    for entry in "${allowlist[@]+"${allowlist[@]}"}"; do
        if [ "$entry" = "$name" ]; then
            return 0
        fi
    done
    return 1
}

# Collect the set of populated marker names from goap.rs.
# Matches:
#   .set_entity(markers::Name::KEY, ...)
#   .set_colony(markers::Name::KEY, ...)
# Captures just the `Name` segment. `rg -o -r '$1'` outputs only the
# capture group, one per line.
populated=$(
    rg --type rust --multiline -o --no-filename --no-line-number \
        --replace '$1' \
        '\.set_(?:entity|colony)\(\s*markers::([A-Z][A-Za-z0-9]+)::KEY' \
        "$POPULATOR_FILE" 2>/dev/null \
        | sort -u || true
)

# Collect required markers from DSE eligibility filters.
# Pattern: `.require(markers::Name::KEY)` — captures just `Name`.
# Excludes fox / hawk / snake DSEs — those use separate populators
# in `fox_goap.rs` / `hawk_goap.rs` / `snake_goap.rs`. If a wildlife
# populator needs the same discipline, this lint can be extended
# with per-populator scanning.
required=$(
    rg --type rust -o --no-filename --no-line-number \
        --replace '$1' \
        --glob '!**/fox_*.rs' \
        --glob '!**/hawk_*.rs' \
        --glob '!**/snake_*.rs' \
        '\.require\(\s*markers::([A-Z][A-Za-z0-9]+)::KEY' \
        "$DSE_GLOB" 2>/dev/null \
        | sort -u || true
)

# Diff: required minus populated == offenders.
offenders=()
allowlisted_hits=0
while IFS= read -r marker; do
    [ -z "$marker" ] && continue
    if grep -qFx "$marker" <<< "$populated"; then
        continue
    fi
    if is_allowlisted "$marker"; then
        allowlisted_hits=$((allowlisted_hits + 1))
        continue
    fi
    # Find the citation site(s) for the error message.
    cite=$(rg --type rust -n \
        --glob '!**/fox_*.rs' \
        --glob '!**/hawk_*.rs' \
        --glob '!**/snake_*.rs' \
        "\.require\(\s*markers::${marker}::KEY" \
        "$DSE_GLOB" 2>/dev/null \
        | grep -vE ':[0-9]+:[[:space:]]*//' \
        | head -1)
    offenders+=("$cite  // marker '$marker' required by a DSE filter but never set_entity/set_colony in $POPULATOR_FILE")
done <<< "$required"

if [ "${#offenders[@]}" -gt 0 ]; then
    echo "marker-snapshot-wiring: DSE eligibility filters require markers that aren't populated into MarkerSnapshot" >&2
    echo "  (ticket 217 — see precedents 084 / 209)" >&2
    for line in "${offenders[@]}"; do
        echo "  $line" >&2
    done
    echo "" >&2
    echo "Fix: in src/systems/goap.rs::evaluate_and_plan, add a populator call:" >&2
    echo "  markers.set_entity(markers::<Name>::KEY, entity, <bool from Has<...> query>);  // per-cat" >&2
    echo "  markers.set_colony(markers::<Name>::KEY, <bool>);                              // colony-scoped" >&2
    echo "Also mirror the call in src/systems/disposition.rs::evaluate_dispositions if the marker is per-cat." >&2
    echo "Or — if the marker is intentionally dormant (no authoring system) — add an entry to" >&2
    echo "  $ALLOWLIST naming the ticket that lands the authoring system." >&2
    exit 1
fi

if [ "$allowlisted_hits" -gt 0 ]; then
    echo "marker-snapshot-wiring: ok ($allowlisted_hits allowlisted — see $ALLOWLIST)"
else
    echo "marker-snapshot-wiring: all DSE-required markers populated in $POPULATOR_FILE"
fi
exit 0
