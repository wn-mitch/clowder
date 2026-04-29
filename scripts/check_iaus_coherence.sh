#!/usr/bin/env bash
# Enforces the IAUS coherence discipline (CLAUDE.md §"Architecture",
# tickets 071/079).
#
# Greps `src/ai/dses/*.rs` for the "MacGyvered pin" anti-pattern:
#
#     if <expr> { return 1.0; }
#     if <expr> { return 0.0; }
#
# (single-line or 3-line block form). These shapes subvert the IAUS
# score economy — a hard-coded override that bypasses the
# Consideration × Curve pipeline, the §3.5.1 Modifier stack, and the
# EligibilityFilter gate. The "machined gears" doctrine in ticket 071
# is that every defense lands inside one of those three engine
# primitives, never as a post-hoc resolver-body pin.
#
# Exits 1 with a clear error pointing at file:line and the rationale.
#
# Allowlist marker:
#   Add `// IAUS-COHERENCE-EXEMPT: <reason>` on the line immediately
#   preceding the offending `if` to silence the check. Reserved for
#   genuinely-out-of-economy cases (narrative-injected events,
#   constructor-body gate semantics, etc.).
#
# Wired into `just check` alongside `scripts/check_step_contracts.sh`.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

DSE_GLOB="src/ai/dses"
EXEMPT_MARKER="IAUS-COHERENCE-EXEMPT"

RATIONALE='This pattern bypasses the IAUS score economy. Express the override as a Consideration with a Curve, a Modifier in src/ai/modifier.rs, or an EligibilityFilter — not a post-hoc return. See docs/open-work/tickets/071-planning-substrate-hardening.md for the machined-gears doctrine. To exempt a genuine out-of-economy case, prepend `// IAUS-COHERENCE-EXEMPT: <one-line reason>` on the line immediately above.'

exit_code=0
offenders=()
exempted=0

# Returns 0 (true) if the line immediately above `lineno` in `file`
# carries the exemption marker. The marker line may itself be indented.
is_exempted() {
    local file="$1"
    local lineno="$2"
    if [ "$lineno" -le 1 ]; then
        return 1
    fi
    local prev=$((lineno - 1))
    local prev_line
    prev_line="$(sed -n "${prev}p" "$file")"
    if [[ "$prev_line" == *"$EXEMPT_MARKER"* ]]; then
        return 0
    fi
    return 1
}

# --- Pass 1: single-line form  `if X { return 1.0; }` -----------------
while IFS= read -r match; do
    [ -z "$match" ] && continue
    file="${match%%:*}"
    rest="${match#*:}"
    line="${rest%%:*}"
    if is_exempted "$file" "$line"; then
        exempted=$((exempted + 1))
    else
        offenders+=("$file:$line  (single-line if-return override)")
        exit_code=1
    fi
done < <(rg --line-number --no-heading \
    '^\s*if\s+[^{]+\{\s*return\s+(1\.0|0\.0)\s*;?\s*\}' \
    "$DSE_GLOB" 2>/dev/null || true)

# --- Pass 2: 3-line block form ----------------------------------------
#   if X {
#       return 1.0;
#   }
#
# ripgrep's -U + --multiline-dotall lets a single regex span lines.
# The reported line number is the start of the match (the `if` line),
# which is also where the exemption marker would precede.
while IFS= read -r match; do
    [ -z "$match" ] && continue
    file="${match%%:*}"
    rest="${match#*:}"
    line="${rest%%:*}"
    if is_exempted "$file" "$line"; then
        exempted=$((exempted + 1))
    else
        offenders+=("$file:$line  (block if-return override)")
        exit_code=1
    fi
done < <(rg --line-number --no-heading -U --multiline-dotall \
    '^\s*if\s+[^{]+\{[\s]*\n[\s]*return\s+(1\.0|0\.0)\s*;[\s]*\n[\s]*\}' \
    "$DSE_GLOB" 2>/dev/null \
    | rg '^[^:]+:[0-9]+:\s*if\s' || true)

# Deduplicate (a single match could in principle be reported twice if
# both passes catch it; today they're disjoint, but belt-and-braces).
# `mapfile` is bash 4+; macOS ships bash 3.2, so loop in portable form.
if [ "${#offenders[@]+x}" = "x" ] && [ "${#offenders[@]}" -gt 0 ]; then
    deduped=()
    while IFS= read -r entry; do
        deduped+=("$entry")
    done < <(printf '%s\n' "${offenders[@]}" | awk '!seen[$0]++')
    offenders=("${deduped[@]}")
fi

if [ "$exit_code" -eq 0 ]; then
    if [ "$exempted" -gt 0 ]; then
        echo "iaus-coherence: ok ($exempted exempted via // $EXEMPT_MARKER marker)"
    else
        echo "iaus-coherence: no MacGyvered pins in $DSE_GLOB/*.rs"
    fi
else
    echo "iaus-coherence: MacGyvered pin(s) detected in $DSE_GLOB/*.rs:" >&2
    for entry in "${offenders[@]}"; do
        echo "  $entry" >&2
    done
    echo "" >&2
    echo "$RATIONALE" >&2
fi

exit "$exit_code"
