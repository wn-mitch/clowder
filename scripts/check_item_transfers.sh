#!/usr/bin/env bash
# Enforces the items-are-real coding contract (CLAUDE.md §"items are
# real"; ticket 175). Spec: src/components/item_transfer.rs module
# rustdoc.
#
# Items in Clowder are real `Entity`s with an `Item` component. Moving
# an item between containers (Stores ↔ cat Inventory ↔ ground) is a
# *transfer*; the cardinal rule is **no transfer may silently destroy
# the item**. Pre-175, several resolvers ran the sequence
# `stored.remove(...) → inventory.add_*(...) → commands.entity(_).despawn()`
# and discarded the `add_*` return value, silently destroying real
# entities when inventory was full.
#
# This lint flags files that contain BOTH `stored.remove(` and
# `commands.entity(...).despawn()` somewhere in the file but do NOT
# also reference `transfer_item_stores_to_inventory` (the typed
# primitive in `src/components/item_transfer.rs`). The file-level
# granularity is coarse but reliable: every audited resolver in 175
# either uses the primitive or doesn't pair the two patterns at all.
#
# Allowlist: `scripts/item_transfers.allowlist`. Format is one entry
# per line: `<file> <ticket-id>` — `<file>` is a path relative to
# repo root. Comment lines start with `#`. The ticket id is required
# so reviewers know why each entry exists.
#
# Out of scope: arbitrary entity-despawn patterns unrelated to a
# stored `remove`. Cat death cleanup that despawns the cat entity
# itself is not a transfer; the lint isn't triggered unless the same
# file ALSO calls `stored.remove(`.
#
# Wired into `just check`. Exits non-zero with a list of offenders.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

ALLOWLIST="scripts/item_transfers.allowlist"
SRC_GLOB="src"
CONTRACT_MODULE="src/components/item_transfer.rs"

# Parse allowlist into a flat array of file paths.
# (macOS bash 3.2 lacks `declare -A` — linear scan is fine for a
# small allowlist.)
allow=()
if [ -f "$ALLOWLIST" ]; then
    while IFS= read -r line; do
        line="${line%%#*}"
        line="${line#"${line%%[![:space:]]*}"}"
        line="${line%"${line##*[![:space:]]}"}"
        [ -z "$line" ] && continue
        # Each line: `<file> <ticket-id>`; we only need the
        # first whitespace-separated token for matching.
        key="${line%% *}"
        allow+=("$key")
    done < "$ALLOWLIST"
fi

is_allowed() {
    local needle="$1"
    [ ${#allow[@]} -eq 0 ] && return 1
    local entry
    for entry in "${allow[@]}"; do
        [ "$entry" = "$needle" ] && return 0
    done
    return 1
}

offenders=()
# Files that mention `stored.remove(`. Iterate and check whether
# they also pair with a `.despawn()` and lack the contract import.
while IFS= read -r file; do
    rel="${file#"$REPO_ROOT/"}"

    # The contract module itself is the home of the typed primitive
    # and is allowed to call destructive ops directly.
    if [ "$rel" = "$CONTRACT_MODULE" ]; then
        continue
    fi

    # File-level allowlist.
    if is_allowed "$rel"; then
        continue
    fi

    # Both `stored.remove(` and `.despawn()` present?
    if ! grep -q '\.despawn()' "$file"; then
        continue
    fi

    # Routes through the typed primitive?
    if grep -q 'transfer_item_stores_to_inventory\|transfer_item_inventory_to_stores' "$file"; then
        continue
    fi

    offenders+=("$rel")
done < <(grep -lR --include='*.rs' 'stored\.remove(' "$SRC_GLOB" 2>/dev/null || true)

if [ ${#offenders[@]} -gt 0 ]; then
    echo "FAIL: items-are-real contract violations (ticket 175)" >&2
    echo >&2
    echo "Each file below pairs 'stored.remove(' with '.despawn()' but does" >&2
    echo "NOT route the transfer through the typed primitive in" >&2
    echo "src/components/item_transfer.rs. Pre-175 this pattern silently" >&2
    echo "destroyed real item entities when the inventory was full." >&2
    echo >&2
    for o in "${offenders[@]}"; do
        echo "  $o" >&2
    done
    echo >&2
    echo "Fix: replace the manual 'stored.remove + add + despawn' sequence" >&2
    echo "with 'transfer_item_stores_to_inventory(...)'. See" >&2
    echo "src/steps/disposition/retrieve_raw_food_from_stores.rs for the" >&2
    echo "reference migration." >&2
    echo >&2
    echo "If this file is genuinely OK to bypass the contract (e.g. cat-death" >&2
    echo "cleanup that despawns the carrier, not a transfer), add an entry" >&2
    echo "to scripts/item_transfers.allowlist with the ticket id." >&2
    exit 1
fi

echo "items-are-real contract: OK"
