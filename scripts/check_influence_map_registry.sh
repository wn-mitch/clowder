#!/usr/bin/env bash
# Enforces the InfluenceMap registry contract (CLAUDE.md §"Conventions"
# / "InfluenceMap registry stubs are forbidden"). Spec: ticket 207.
#
# Single audit:
#   For every `impl InfluenceMap for <Type>` in src/, verify the type
#   has at least one registration in `populate_influence_map_registry`
#   (src/plugins/simulation.rs):
#     * `registry.register::<<Type>>()`         — Resource-backed maps
#     * `register_with(...)` whose closure body
#       contains a `<Type>(` constructor call    — borrow-adapter maps
#                                                  (e.g., CorruptionLens
#                                                  over &TileMap, or
#                                                  per-kind adapters
#                                                  like PerSpeciesScentRef)
#     * Allowlist entry in scripts/influence_map_registry.allowlist
#
# Closes the regression vector tickets 048 → 206 spent two landings
# closing: an InfluenceMap impl that lands without trace coverage
# silently drops the map from the L1 surface. With this lint, the
# omission fails `just check` instead of after a focal-cat soak.
#
# Allowlist format (mirrors scripts/substrate_stubs.allowlist):
#   <TypeName> <ticket-id>     # rationale
# Comments after `#` ignored.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

REGISTRATION_FILE="src/plugins/simulation.rs"
ALLOWLIST="scripts/influence_map_registry.allowlist"
SRC_GLOB="src"

# Parse allowlist: format is `<TypeName> <ticket-id>` per line, with
# `# comments` after `<ticket-id>` ignored. Empty / comment-only lines
# skipped.
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
        [ "$entry" = "$name" ] && return 0
    done
    return 1
}

# Detect every `impl InfluenceMap for <Type>` in src/. Strip:
#   - leading `impl<'a>` / `impl<T>` generic params
#   - module-path prefixes (`crate::resources::FoxScentMap` → `FoxScentMap`)
#   - trailing lifetime / generic params on the type (`CorruptionLens<'_>`
#     → `CorruptionLens`)
# Comment lines are filtered post-grep.
impls=()
while IFS= read -r match; do
    # match shape: "src/path/file.rs:LINE: impl[<...>] InfluenceMap for <Type>[<...>] {"
    # Strip path:line prefix.
    rest="${match#*:*:}"
    # Strip leading whitespace.
    rest="${rest#"${rest%%[![:space:]]*}"}"
    # Skip line/doc comments.
    case "$rest" in
        //*|///*) continue ;;
    esac
    # Extract everything after "InfluenceMap for " up to the first `{`,
    # `<`, or whitespace, then strip module path and lifetime params.
    type_name=$(printf '%s\n' "$rest" \
        | sed -E 's/.*InfluenceMap[[:space:]]+for[[:space:]]+([A-Za-z_][A-Za-z0-9_:]*).*/\1/')
    type_name="${type_name##*::}"
    impls+=("$type_name")
done < <(rg --type rust -n 'impl(<[^>]+>)?\s+InfluenceMap\s+for\s+' "$SRC_GLOB" 2>/dev/null || true)

# De-duplicate impls (a type might have multiple impl blocks in
# different cfg gates; we only need to verify it's registered once).
# `sort -u` over the array — portable across macOS bash 3.x and Linux
# bash 4+ (no `declare -A`).
unique_impls=()
if [ "${#impls[@]}" -gt 0 ]; then
    while IFS= read -r t; do
        unique_impls+=("$t")
    done < <(printf '%s\n' "${impls[@]}" | sort -u)
fi

# Pre-filter REGISTRATION_FILE: strip line comments (`//...`) so a
# commented-out `registry.register::<X>();` doesn't satisfy the audit.
# Block-comment stripping is overkill for this codebase — line
# comments are the realistic regression mode.
non_comment="$(grep -vE '^[[:space:]]*//' "$REGISTRATION_FILE" || true)"

# Verify each impl has a registration call in REGISTRATION_FILE.
offenders=()
for t in "${unique_impls[@]+"${unique_impls[@]}"}"; do
    if is_allowlisted "$t"; then
        continue
    fi
    # `register::<<Type>>()` — Resource-backed map registration.
    # `[[:<:]]` / `[[:>:]]` word boundaries prevent PreyScentMap
    # matching PreyScentMaps (and vice versa). Inside grep -E we use
    # alternation with `[^A-Za-z0-9_]` because POSIX `\b` is unreliable.
    if printf '%s\n' "$non_comment" \
        | grep -qE "(^|[^A-Za-z0-9_])register::<[[:space:]]*${t}[[:space:]]*>\(\)"; then
        continue
    fi
    # `register_with(...)` body containing a `<Type>(` constructor.
    # Coarse but effective: a non-comment line in REGISTRATION_FILE
    # matching `<Type>\s*\(` together with a `register_with` call is
    # treated as a borrow-adapter registration. The lint accepts
    # false positives (a string literal containing the type name
    # would slip through) over false negatives — the latter is the
    # regression we're closing.
    if printf '%s\n' "$non_comment" \
        | grep -qE "(^|[^A-Za-z0-9_])${t}[[:space:]]*\(" \
        && printf '%s\n' "$non_comment" | grep -q "register_with"; then
        continue
    fi
    offenders+=("$t")
done

if [ "${#offenders[@]}" -ne 0 ]; then
    echo "InfluenceMap registry stub(s) detected:" >&2
    for t in "${offenders[@]}"; do
        echo "  - ${t}: \`impl InfluenceMap for ${t}\` in src/ but no" >&2
        echo "    \`registry.register::<${t}>()\` or matching" >&2
        echo "    \`register_with(...)\` call in ${REGISTRATION_FILE}, and no" >&2
        echo "    allowlist entry in ${ALLOWLIST}." >&2
    done
    echo >&2
    echo "Fix: add a registration call in populate_influence_map_registry," >&2
    echo "or — for follow-on work landing the impl ahead of the" >&2
    echo "registration — add an allowlist entry naming the ticket that" >&2
    echo "wires it. See scripts/substrate_stubs.allowlist for the format." >&2
    exit 1
fi

count="${#unique_impls[@]}"
echo "InfluenceMap registry: ${count} impl(s), all registered."
exit 0
