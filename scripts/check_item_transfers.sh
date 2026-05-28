#!/usr/bin/env bash
# Enforces the items-are-real coding contract (CLAUDE.md "items are
# real"; tickets 175 + 429). Spec: src/components/item_transfer.rs +
# src/components/item_gate.rs module rustdoc + docs/systems/items-are-real.md.
#
# Items in Clowder are real `Entity`s with an `Item` component. Moving
# an item between containers — Stores ↔ cat Inventory ↔ ground — passes
# through one of three named gates:
#
#   - Source   — item enters world (forage, hunt, den-raid, craft output).
#                Lives in `src/components/item_gate.rs` + submodules under
#                `item_gate/sources/`; trait `ItemSource` carries the
#                kind / modifiers / FEATURE invariant by construction.
#   - Transfer — item moves without form change (handoff, deposit, pick
#                up, drop). Lives in `src/components/item_transfer.rs`
#                primitives + the `src/steps/disposition/` resolvers that
#                call them.
#   - Sink     — item exits world or Inventory (eat, feed-a-kitten, bury,
#                craft consumption). Lives in `src/steps/disposition/`
#                resolvers (function-shape today; trait retrofit is a
#                429 follow-on).
#
# This lint flags:
#
#   175 surface — files that pair `stored.remove(` with `.despawn()`
#   without going through `transfer_item_stores_to_inventory` /
#   `transfer_item_inventory_to_stored`.
#
#   429 strict surface — Inventory slot mutations outside the gate-author
#   surface. The patterns:
#       inventory.pouch.{push,swap_remove,retain,remove}
#       *.pouch.{push,swap_remove,retain,remove}
#       inventory.{take_food,add_food*,add_item*}
#       Item::{new,with_modifiers}  (spawning a new world-Item entity)
#
# Allowed-source surface (auto-skipped):
#   src/components/item_transfer.rs    — Transfer primitive layer
#   src/components/item_gate.rs        — Source trait + default impl
#   src/components/item_gate/**        — ItemSource impls
#   src/components/items.rs            — Item struct + tests
#   src/components/magic.rs            — Inventory struct's own methods
#                                        (`take_food`, `add_food`,
#                                        `add_item_with_modifiers` are
#                                        the primitive layer; mirrors
#                                        item_transfer.rs's same role)
#   src/steps/**                       — resolver layer (Sink + Transfer)
#   src/scenarios/**                   — test-harness setup (not production economy)
#   src/world_gen/**                   — initial-world spawn (Source-class but
#                                        one-time generation, not per-tick economy)
#   src/rendering/**                   — UI / rendering test harness
#
# Per-file `#[cfg(test)] mod tests {` boundaries inside otherwise-prod
# files are auto-detected — anything below the `mod tests {` line is
# treated as test code and skipped.
#
# Allowlist: `scripts/item_transfers.allowlist`. Format is one entry
# per line: `<file> <ticket-id>` — `<file>` is a path relative to
# repo root. Comment lines start with `#`. The ticket id is required
# so reviewers know why each entry exists.
#
# Wired into `just check`. Exits non-zero with a list of offenders.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

ALLOWLIST="scripts/item_transfers.allowlist"
SRC_GLOB="src"

# Files whose path matches one of these prefixes is part of the gate
# authoring surface and auto-skipped. (macOS bash 3.2 lacks `=~ ^pattern$`
# arrays of the form we'd want; linear scan via case-glob is fine.)
is_allowed_surface() {
    local rel="$1"
    case "$rel" in
        src/components/item_transfer.rs) return 0 ;;
        src/components/item_gate.rs) return 0 ;;
        src/components/item_gate/*) return 0 ;;
        src/components/items.rs) return 0 ;;
        src/components/magic.rs) return 0 ;;
        src/steps/*) return 0 ;;
        src/scenarios/*) return 0 ;;
        src/world_gen/*) return 0 ;;
        src/rendering/*) return 0 ;;
    esac
    return 1
}

# Per-file test-boundary detection. Returns the 1-based line number
# where `mod tests {` begins, or 0 if absent. Lines >= boundary are
# test code and skipped by the 429 check.
test_boundary_line() {
    local file="$1"
    grep -n '^mod tests\b\|^#\[cfg(test)\]' "$file" 2>/dev/null \
        | head -n1 | cut -d: -f1
}

# Parse allowlist into a flat array of file paths.
allow=()
if [ -f "$ALLOWLIST" ]; then
    while IFS= read -r line; do
        line="${line%%#*}"
        line="${line#"${line%%[![:space:]]*}"}"
        line="${line%"${line##*[![:space:]]}"}"
        [ -z "$line" ] && continue
        key="${line%% *}"
        # Strip any `:symbol` suffix — 175-format entries may include
        # `<file>:<symbol>`. The 429 lint operates at file granularity.
        key="${key%%:*}"
        allow+=("$key")
    done < "$ALLOWLIST"
fi

is_allowlisted() {
    local needle="$1"
    [ ${#allow[@]} -eq 0 ] && return 1
    local entry
    for entry in "${allow[@]}"; do
        [ "$entry" = "$needle" ] && return 0
    done
    return 1
}

# ----------------------------------------------------------------------
# 175 file-level check — `stored.remove(` paired with `.despawn()`.
# ----------------------------------------------------------------------

offenders_175=()
while IFS= read -r file; do
    rel="${file#"$REPO_ROOT/"}"
    if is_allowed_surface "$rel"; then continue; fi
    if is_allowlisted "$rel"; then continue; fi
    if ! grep -q '\.despawn()' "$file"; then continue; fi
    if grep -q 'transfer_item_stores_to_inventory\|transfer_item_inventory_to_stored' "$file"; then
        continue
    fi
    offenders_175+=("$rel")
done < <(grep -lR --include='*.rs' 'stored\.remove(' "$SRC_GLOB" 2>/dev/null || true)

# ----------------------------------------------------------------------
# 429 strict per-line check — Inventory mutation outside gate surface.
# Strip comment-only lines (those whose only non-whitespace content is
# `//` or `///` prefixed) to avoid flagging doc-comment references to
# the patterns. A trailing `// …` comment after a real call still matches
# the real call, which is the intended behavior.
# ----------------------------------------------------------------------

PATTERN_429='(\binventory\.pouch\.(push|swap_remove|retain|remove)\b'
PATTERN_429+='|\.pouch\.(push|swap_remove|retain|remove)\b'
PATTERN_429+='|\binventory\.take_food\b'
PATTERN_429+='|\binventory\.add_food[a-z_]*\b'
PATTERN_429+='|\binventory\.add_item[a-z_]*\b'
PATTERN_429+='|\bItem::with_modifiers\b'
PATTERN_429+='|\bItem::new\b)'

offenders_429=()
# Cache test-boundary lines per file so we only run `grep` once per file.
declare -A boundary_cache 2>/dev/null || true
while IFS= read -r match; do
    file="${match%%:*}"
    rest="${match#*:}"
    rel="${file#"$REPO_ROOT/"}"
    if is_allowed_surface "$rel"; then continue; fi
    if is_allowlisted "$rel"; then continue; fi
    # Skip lines past the file's `mod tests` / `#[cfg(test)]` boundary.
    line_no="${rest%%:*}"
    boundary="$(test_boundary_line "$file")"
    if [ -n "$boundary" ] && [ "$boundary" != "0" ] && [ "$line_no" -ge "$boundary" ]; then
        continue
    fi
    # Skip pure-comment lines (everything before `//` is whitespace).
    # `rest` is `<line-number>:<line-content>`.
    line_content="${rest#*:}"
    leading="${line_content%%//*}"
    leading_trimmed="${leading#"${leading%%[![:space:]]*}"}"
    leading_trimmed="${leading_trimmed%"${leading_trimmed##*[![:space:]]}"}"
    if [ -z "$leading_trimmed" ]; then continue; fi
    offenders_429+=("$rel:$rest")
done < <(grep -rE --include='*.rs' -n "$PATTERN_429" "$SRC_GLOB" 2>/dev/null || true)

# ----------------------------------------------------------------------
# Report
# ----------------------------------------------------------------------

fail=0

if [ ${#offenders_175[@]} -gt 0 ]; then
    echo "FAIL: items-are-real Transfer contract (ticket 175)" >&2
    echo >&2
    echo "Each file below pairs 'stored.remove(' with '.despawn()' but does" >&2
    echo "NOT route the transfer through the typed primitive in" >&2
    echo "src/components/item_transfer.rs. Pre-175 this pattern silently" >&2
    echo "destroyed real item entities when the inventory was full." >&2
    echo >&2
    for o in "${offenders_175[@]}"; do echo "  $o" >&2; done
    echo >&2
    echo "Fix: replace the manual 'stored.remove + add + despawn' sequence" >&2
    echo "with 'transfer_item_stores_to_inventory(...)'. See" >&2
    echo "src/steps/disposition/retrieve_raw_food_from_stores.rs for the" >&2
    echo "reference migration. If this file is genuinely OK to bypass the" >&2
    echo "contract (e.g. cat-death cleanup that despawns the carrier), add" >&2
    echo "an entry to scripts/item_transfers.allowlist with the ticket id." >&2
    echo >&2
    fail=1
fi

if [ ${#offenders_429[@]} -gt 0 ]; then
    echo "FAIL: items-are-real Source/Sink contract (ticket 429)" >&2
    echo >&2
    echo "The line(s) below mutate Inventory slots or spawn a new Item" >&2
    echo "entity outside the gate-author surface (item_transfer.rs," >&2
    echo "item_gate/**, items.rs, magic.rs, or src/steps/**). Every item" >&2
    echo "state-transition must pass through a named gate — see" >&2
    echo "docs/systems/items-are-real.md." >&2
    echo >&2
    for o in "${offenders_429[@]}"; do echo "  $o" >&2; done
    echo >&2
    echo "Fix: route the mutation through an existing Source / Transfer /" >&2
    echo "Sink gate, OR add a new ItemSource impl under" >&2
    echo "src/components/item_gate/sources/ if it's a new origin. If the" >&2
    echo "bypass is genuinely an internal Inventory primitive method (the" >&2
    echo "kind that lives in src/components/magic.rs alongside take_food /" >&2
    echo "add_item), file should be added to the allowed-source list in" >&2
    echo "scripts/check_item_transfers.sh. If it's a pending follow-on" >&2
    echo "promotion (e.g., HarvestCarcass / ForageIngredient / Preservation" >&2
    echo "output / wildlife loot drops), add an entry to" >&2
    echo "scripts/item_transfers.allowlist with the follow-on ticket id." >&2
    echo >&2
    fail=1
fi

if [ "$fail" -ne 0 ]; then
    exit 1
fi

echo "items-are-real contract: OK"
