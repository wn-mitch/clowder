#!/usr/bin/env bash
# Enforces the `score_actions` dispatcher contract (CLAUDE.md §"Conventions"
# / "Prefer compile-time contracts to runtime checks"). Spec: ticket 438.
#
# Two audits:
#   1. `src/ai/scoring.rs::score_actions` MUST contain exactly one
#      `score_dse_by_id` call site (the registry-iteration loop body).
#      Any additional hand-written `score_dse_by_id("<id>", ...)` branch
#      is the pre-438 antipattern — recording a parallel hand-maintained
#      list whose drift from `populate_dse_registry` (or, post-438, from
#      `CAT_DSE_REGISTRY`) was the silent-failure surface diagnosed by
#      tickets 436 / 437.
#   2. Every `impl crate::ai::dse::CatDse for <Type>` in src/ai/dses/
#      MUST be paired with at least one
#      `#[linkme::distributed_slice(crate::ai::dses::CAT_DSE_REGISTRY)]`
#      registration in the same file. Adding a CatDse impl without
#      registering it leaves the DSE constructable-but-never-dispatched
#      — the same silent-failure class, just on the auto-discovery surface.
#
# Both checks are belt-and-suspenders alongside the type-level
# guarantees (Dse::action on the CatDse sub-trait makes "registered ⇒
# dispatchable" hold at compile time; this script makes the CI surface
# explicit so a regression hits the build, not a soak).

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

SCORING_FILE="src/ai/scoring.rs"
DSES_DIR="src/ai/dses"

# -----------------------------------------------------------------------
# Audit 1 — score_actions must have exactly one `score_dse_by_id` call.
# -----------------------------------------------------------------------

# Locate the `score_actions` function body. The function is defined once
# in scoring.rs at module scope; we extract everything between its
# signature line and the next module-scope `fn`/`pub fn`/`pub type`/...
# boundary, then count `score_dse_by_id` occurrences within.
score_actions_body=$(awk '
    /^pub fn score_actions\(/ { in_fn = 1; brace = 0 }
    in_fn {
        for (i = 1; i <= length($0); i++) {
            c = substr($0, i, 1)
            if (c == "{") brace++
            else if (c == "}") {
                brace--
                if (brace == 0) { in_fn = 0; print; exit }
            }
        }
        print
    }
' "$SCORING_FILE")

# Strip `//`-led comment lines AND embedded backtick-quoted occurrences
# (`...score_dse_by_id...` references in doc-style comments and
# `// inline notes`) before counting. We want only real call sites.
count=$(printf '%s\n' "$score_actions_body" \
    | grep -vE '^[[:space:]]*//' \
    | grep -oE '[^a-zA-Z_]score_dse_by_id[[:space:]]*\(' \
    | wc -l \
    | tr -d ' ')

if [ "$count" -ne 1 ]; then
    echo "score_actions dispatcher stub detected:" >&2
    echo "  $SCORING_FILE::score_actions contains $count \`score_dse_by_id\`" >&2
    echo "  call sites; expected exactly 1 (the registry-iteration loop body)." >&2
    echo >&2
    echo "Hand-written dispatch branches re-introduce the silent-failure" >&2
    echo "class diagnosed by tickets 436 / 437 — a DSE registered in" >&2
    echo "CAT_DSE_REGISTRY without a matching branch never enters L2/L3" >&2
    echo "scoring. Iterate \`inputs.dse_registry.cat_dses\` instead and" >&2
    echo "express any outer gates via PRE_DISPATCH_GATES / POST_EVAL_HOOKS" >&2
    echo "(both keyed by DseId) in $SCORING_FILE." >&2
    exit 1
fi

# -----------------------------------------------------------------------
# Audit 2 — every `impl CatDse for <Type>` is registered in CAT_DSE_REGISTRY.
# -----------------------------------------------------------------------

# Collect every CatDse impl. Match either `impl CatDse for X` or
# `impl crate::ai::dse::CatDse for X` (the path varies by file).
impls=()
while IFS= read -r match; do
    rest="${match#*:*:}"
    rest="${rest#"${rest%%[![:space:]]*}"}"
    case "$rest" in
        //*|///*) continue ;;
    esac
    type_name=$(printf '%s\n' "$rest" \
        | sed -E 's/.*CatDse[[:space:]]+for[[:space:]]+([A-Za-z_][A-Za-z0-9_:]*).*/\1/')
    type_name="${type_name##*::}"
    file=$(printf '%s\n' "$match" | cut -d: -f1)
    impls+=("$file:$type_name")
done < <(rg --type rust -n 'impl(<[^>]+>)?\s+(crate::ai::dse::)?CatDse\s+for\s+' "$DSES_DIR" 2>/dev/null || true)

offenders=()
for entry in "${impls[@]+"${impls[@]}"}"; do
    file="${entry%%:*}"
    type_name="${entry##*:}"
    # Skip the registry-internal helper struct itself.
    [ "$type_name" = "CatDseRegistration" ] && continue
    # Does the same file emit a `#[linkme::distributed_slice(...CAT_DSE_REGISTRY)]`?
    if ! grep -qE 'distributed_slice\([^)]*CAT_DSE_REGISTRY' "$file"; then
        offenders+=("$file::$type_name")
    fi
done

if [ "${#offenders[@]}" -ne 0 ]; then
    echo "CatDse without CAT_DSE_REGISTRY registration:" >&2
    for o in "${offenders[@]}"; do
        echo "  - $o" >&2
    done
    echo >&2
    echo "Every \`impl CatDse for <T>\` MUST be paired with a" >&2
    echo "\`#[linkme::distributed_slice(crate::ai::dses::CAT_DSE_REGISTRY)]\`" >&2
    echo "entry in the same file. Without it the DSE is constructable but" >&2
    echo "never enters \`populate_dse_registry\` — same silent-failure class" >&2
    echo "ticket 438 retired the hand-written dispatcher to close." >&2
    exit 1
fi

impl_count="${#impls[@]}"
echo "score_actions dispatcher: 1 call site (registry iteration); ${impl_count} CatDse impl(s), all registered."
exit 0
