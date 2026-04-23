#!/usr/bin/env bash
# Enforces the GOAP step resolver contract (CLAUDE.md §"GOAP Step
# Resolver Contract").
#
# For every `pub fn resolve_*` in `src/steps/**/*.rs`, verifies the
# preceding `///` block contains each of the five required headings:
#   - Real-world effect
#   - Plan-level preconditions
#   - Runtime preconditions
#   - Witness
#   - Feature emission
#
# Exits non-zero with a list of offenders. Wired into `just check`.
#
# The contract exists because Phase 4c.3 (feed-kitten) and Phase 4c.4
# (tend-crops) shipped the same bug: a step returned Advance with no
# real-world effect, Feature emission was either unconditional or
# absent, and the Activation canary went blind. The type (StepOutcome<W>
# in src/steps/outcome.rs) enforces the structural shape; this lint
# enforces the documentation shape.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

STEPS_GLOB="src/steps"
ALLOWLIST="scripts/step_contracts.allowlist"

REQUIRED_HEADINGS=(
    "Real-world effect"
    "Plan-level preconditions"
    "Runtime preconditions"
    "Witness"
    "Feature emission"
)

# Files exempt while their resolvers are migrated in Phases B/C/E.
# Empty lines and # comments are ignored.
allowlist=()
if [ -f "$ALLOWLIST" ]; then
    while IFS= read -r line; do
        line="${line%%#*}"
        line="${line#"${line%%[![:space:]]*}"}"
        line="${line%"${line##*[![:space:]]}"}"
        if [ -n "$line" ]; then
            allowlist+=("$line")
        fi
    done < "$ALLOWLIST"
fi

is_allowlisted() {
    local file="$1"
    for entry in "${allowlist[@]}"; do
        if [ "$entry" = "$file" ]; then
            return 0
        fi
    done
    return 1
}

exit_code=0
offenders=()
allowlisted_hits=0

# Find every `pub fn resolve_*` with its file:line. ripgrep handles
# multi-line context naturally via -B.
while IFS= read -r match; do
    file="${match%%:*}"
    rest="${match#*:}"
    line="${rest%%:*}"

    # Collect the up-to-40-line `///` block immediately preceding the
    # fn signature. Stop at the first non-`///` line walking backwards.
    block="$(awk -v end="$line" '
        NR < end && NR >= end - 40 { lines[NR] = $0 }
        END {
            started = 0
            for (i = end - 1; i >= end - 40; i--) {
                ln = lines[i]
                if (ln ~ /^[[:space:]]*\/\/\//) {
                    doc[i] = ln
                    started = 1
                } else if (started) {
                    break
                }
            }
            for (k in doc) print doc[k]
        }
    ' "$file")"

    missing=()
    for heading in "${REQUIRED_HEADINGS[@]}"; do
        if ! grep -qF "$heading" <<<"$block"; then
            missing+=("$heading")
        fi
    done

    if [ "${#missing[@]}" -gt 0 ]; then
        if is_allowlisted "$file"; then
            allowlisted_hits=$((allowlisted_hits + 1))
        else
            offenders+=("$file:$line  (missing: $(IFS='|'; echo "${missing[*]}"))")
            exit_code=1
        fi
    fi
done < <(rg --line-number '^pub fn resolve_' "$STEPS_GLOB" || true)

if [ "$exit_code" -eq 0 ]; then
    if [ "$allowlisted_hits" -gt 0 ]; then
        echo "step-contract: ok ($allowlisted_hits allowlisted — see $ALLOWLIST)"
    else
        echo "step-contract: all resolve_* functions carry the 5-heading rustdoc preamble"
    fi
else
    echo "step-contract: offenders missing required rustdoc headings:" >&2
    for line in "${offenders[@]}"; do
        echo "  $line" >&2
    done
    echo "" >&2
    echo "See CLAUDE.md §\"GOAP Step Resolver Contract\" for the template." >&2
    echo "To silence temporarily while migrating, add the file path to $ALLOWLIST." >&2
fi

exit "$exit_code"
