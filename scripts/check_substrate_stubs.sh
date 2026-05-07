#!/usr/bin/env bash
# Enforces the substrate-stub contract (CLAUDE.md §"Conventions" /
# "Substrate stubs are forbidden"). Spec: ticket 160.
#
# Two audits in one script. Both share scripts/substrate_stubs.allowlist.
#
# Audit 1 — Marker orphans.
#   For every `pub struct <Name>;` in src/components/markers.rs (every such
#   struct in this file derives Component), verify the marker has at least
#   one reader AND at least one writer in src/ outside the marker module.
#     Reader patterns:
#       * Has<X>            or Has<markers::X>    (per-cat marker query)
#       * With<X,           or With<X>             (filter-position query;
#                                                  also catches Without<X,>)
#       * X::KEY                                   (MarkerSnapshot::has /
#                                                  EligibilityFilter::require)
#     Writer patterns (any one suffices):
#       * .insert(X)            (commands.entity().insert(X), EntityCommands,
#                                entity_mut.insert(X::default()), with optional
#                                `markers::` qualifier)
#       * .set_entity(X::KEY,   (MarkerSnapshot::set_entity — per-cat marker
#                                cached into the snapshot)
#       * .set_colony(X::KEY,   (MarkerSnapshot::set_colony — colony-scoped
#                                marker cached into the snapshot)
#   Comment lines (`//` / `///`) are filtered out post-grep.
#
# Audit 2 — Consideration string-name validity.
#   `MarkerConsideration::new(_, "<NAME>", _)` references a marker by string
#   literal. Verify each such name matches a real marker enumerated by
#   Audit 1. Catches typos and rename-without-update drift.
#
# Audit 3 — Plan-template stub-vs-curve cross-check (ticket 195).
#   Greps `src/ai/planner/{actions,goap_plan}.rs` for the structured tag
#       STUB(<dse_name>): <successor> until <X> lands.
#   For each match, opens `src/ai/dses/<dse_name>.rs` and detects whether
#   the scoring curve is the default-zero shape `Linear { slope: 0.0,
#   intercept: 0.0 }` (dormant, the stub's promise) or some non-zero
#   shape (Logistic, non-zero Linear, Composite, ScoringConstants-sourced
#   curve — "lifted"). When lifted, the comment's promise is broken and
#   the lint fails unless an allowlist entry of the form
#       STUB:<file>:<line>:<dse_name> <ticket-id>
#   names the ticket that lands the successor entity. Failure mode this
#   prevents: 185-shape regression where the curve is lifted without
#   the prerequisite plan-template successor (zone, marker, DSE).
#
# Writer regex also matches `world.spawn(X)` / `world.spawn((X, …))` so
# spawn-only markers (e.g. `ColonyState`, ticket 168) count as wired
# without an additional `.insert()`. Markers spawned and never removed
# after that are still legitimate substrate.
#
# Out of scope (per ticket 160 §"Out of scope"): orphan Components /
# Resources / Messages / plan templates. Each has different reader
# semantics; defer per-category extensions to follow-on tickets.
#
# Wired into `just check`. Exits non-zero with a list of offenders.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

MARKERS_FILE="src/components/markers.rs"
SRC_GLOB="src"
ALLOWLIST="scripts/substrate_stubs.allowlist"
CONSIDERATIONS_FILE="src/ai/considerations.rs"

# Parse allowlist: format is `<name> <ticket-id>` per line, # comments
# after `<ticket-id>` ignored. Empty lines and comment-only lines skipped.
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

# Detect whether `src/ai/dses/<dse_name>.rs` uses the default-zero curve
# `Linear { slope: 0.0, intercept: 0.0 }` (dormant) or some other shape
# (lifted). Returns "dormant" / "lifted" / "missing" via stdout.
dse_curve_state() {
    local dse_name="$1"
    local file="src/ai/dses/${dse_name}.rs"
    if [ ! -f "$file" ]; then
        echo "missing"
        return
    fi
    # Strip line/block comments so commented-out curves don't fool us.
    # rg counts non-comment lines that contain a curve constructor.
    local default_zero
    default_zero="$(rg -c \
        'Curve::Linear\s*\{\s*slope:\s*0\.0\s*,\s*intercept:\s*0\.0\s*\}' \
        "$file" 2>/dev/null || echo 0)"
    local any_curve
    any_curve="$(rg -c \
        -e 'Curve::(Linear|Logistic|Composite|Quadratic)' \
        -e 'composition::(Composition|Curve)' \
        -e 'ScoringConstants' \
        "$file" 2>/dev/null || echo 0)"
    if [ "$any_curve" -eq 0 ]; then
        # No curve construction at all (DSE may use a different shape);
        # treat as dormant for the purpose of this audit.
        echo "dormant"
        return
    fi
    if [ "$default_zero" -gt 0 ] && [ "$any_curve" -le "$default_zero" ]; then
        echo "dormant"
    else
        echo "lifted"
    fi
}

# Collect every marker name + declaration line.
markers=()
marker_lines=()
while IFS= read -r match; do
    line="${match%%:*}"
    rest="${match#*:}"
    name="${rest#pub struct }"
    name="${name%;*}"
    markers+=("$name")
    marker_lines+=("$line")
done < <(rg -n '^pub struct ([A-Z][A-Za-z0-9]+);' "$MARKERS_FILE" --replace 'pub struct $1;' --no-heading || true)

# Helper: count code-line occurrences of a regex outside the marker module.
# Returns count to stdout.
count_code_hits() {
    local pattern="$1"
    rg --type rust -c "$pattern" "$SRC_GLOB" \
        --glob '!src/components/markers.rs' 2>/dev/null \
    | awk -F: '{sum += $2} END {print sum + 0}'
}

# Has<X>, With<X,>, Without<X,>, X::KEY — combined as one alternation per
# marker keeps the grep cost linear in the marker count.
reader_hit() {
    local name="$1"
    # Three patterns ORed for a single rg pass. Filter out comment lines.
    rg --type rust -n \
        -e "\\bHas<\\s*(markers::)?${name}\\s*>" \
        -e "\\bWith(out)?<\\s*(markers::)?${name}\\s*[,>]" \
        -e "\\b${name}::KEY\\b" \
        "$SRC_GLOB" --glob '!src/components/markers.rs' 2>/dev/null \
    | grep -vE '^[^:]*:[0-9]+:[[:space:]]*//' \
    | head -1
}

writer_hit() {
    local name="$1"
    # Path qualifier alternation — markers can be referenced bare (`use`d),
    # `markers::X`, `m::X` (alias), or `crate::components::markers::X`.
    local qual='(markers::|m::|crate::components::markers::)?'
    # Writer patterns (any one suffices):
    #   * `.insert(X)` / `.insert(markers::X)` — direct component insert.
    #   * `.remove::<X>()` — lifecycle remove (implies the marker IS managed).
    #   * `.set_entity(...X::KEY...)` / `.set_colony(...X::KEY...)` —
    #     MarkerSnapshot setter (arg list may span lines; --multiline
    #     handles the multi-line case via `[^)]*?`).
    #   * `toggle(...X)` / `toggle(...markers::X)` — generic helper that
    #     internally insert/remove the marker (defined in a few places
    #     under src/systems/ and src/ai/capabilities.rs).
    rg --type rust --multiline -n \
        -e "\\.insert\\(\\s*${qual}${name}[\\s,)]" \
        -e "\\.remove::<\\s*${qual}${name}\\s*>" \
        -e "\\.(set_entity|set_colony)\\([^)]*?${name}::KEY" \
        -e "\\btoggle\\([^)]*?${qual}${name}[\\s,)]" \
        -e "\\.spawn\\(\\s*\\(?\\s*${qual}${name}[\\s,)]" \
        "$SRC_GLOB" --glob '!src/components/markers.rs' 2>/dev/null \
    | grep -vE '^[^:]*:[0-9]+:[[:space:]]*//' \
    | head -1
}

# Audit 1.
marker_offenders=()
allowlisted_marker_hits=0
for i in "${!markers[@]}"; do
    name="${markers[$i]}"
    line="${marker_lines[$i]}"

    has_reader=0
    has_writer=0
    [ -n "$(reader_hit "$name")" ] && has_reader=1
    [ -n "$(writer_hit "$name")" ] && has_writer=1

    if [ $has_reader -eq 1 ] && [ $has_writer -eq 1 ]; then
        continue
    fi

    if [ $has_reader -eq 0 ] && [ $has_writer -eq 0 ]; then
        mode="fully-orphan"
    elif [ $has_reader -eq 0 ]; then
        mode="write-only"
    else
        mode="read-only"
    fi

    if is_allowlisted "$name"; then
        allowlisted_marker_hits=$((allowlisted_marker_hits + 1))
    else
        marker_offenders+=("$MARKERS_FILE:$line  $name  ($mode)")
    fi
done

# Audit 2 — consideration string-name validity.
consideration_offenders=()
allowlisted_consideration_hits=0
if [ -f "$CONSIDERATIONS_FILE" ]; then
    # Find the first `#[cfg(test)]` line; skip matches at or after it
    # (placeholder marker names like "X" appear in unit tests for the
    # MarkerConsideration arithmetic and aren't real references).
    test_mod_start="$(rg -n '^#\[cfg\(test\)\]' "$CONSIDERATIONS_FILE" --no-heading 2>/dev/null \
        | head -1 | cut -d: -f1)"
    test_mod_start="${test_mod_start:-99999999}"

    # `rg -o -r '$1'` outputs ONLY the capture group, not the whole matched
    # line. With `-n` the format is `<lineno>:<capture>` (no file prefix
    # because we pass a single file).
    while IFS= read -r match; do
        lineno="${match%%:*}"
        name="${match#*:}"
        # Trim whitespace defensively.
        name="${name#"${name%%[![:space:]]*}"}"
        name="${name%"${name##*[![:space:]]}"}"

        # Skip test-mod content.
        if [ "$lineno" -ge "$test_mod_start" ]; then
            continue
        fi

        # Check name against marker set.
        found=0
        for m in "${markers[@]}"; do
            if [ "$m" = "$name" ]; then
                found=1
                break
            fi
        done

        if [ $found -eq 0 ]; then
            if is_allowlisted "$name"; then
                allowlisted_consideration_hits=$((allowlisted_consideration_hits + 1))
            else
                consideration_offenders+=("$CONSIDERATIONS_FILE:$lineno  consideration \"$name\" references no marker")
            fi
        fi
    done < <(rg -n -o -r '$1' \
                'MarkerConsideration::new\(\s*"[^"]*"\s*,\s*"([^"]+)"' \
                "$CONSIDERATIONS_FILE" --no-heading || true)
fi

# Audit 3 — Plan-template stub-vs-curve cross-check (ticket 195).
stub_offenders=()
allowlisted_stub_hits=0
PLANNER_FILES=("src/ai/planner/actions.rs" "src/ai/planner/goap_plan.rs")
for planner_file in "${PLANNER_FILES[@]}"; do
    [ -f "$planner_file" ] || continue
    while IFS= read -r match; do
        lineno="${match%%:*}"
        rest="${match#*:}"
        # Capture: STUB(<dse_name>): ...
        # Trim leading `// ` or `/// `.
        rest="${rest#"${rest%%[![:space:]]*}"}"
        # Extract dse_name between `STUB(` and `):`.
        dse_name="${rest#*STUB(}"
        dse_name="${dse_name%%):*}"
        # Defensive trim.
        dse_name="${dse_name#"${dse_name%%[![:space:]]*}"}"
        dse_name="${dse_name%"${dse_name##*[![:space:]]}"}"

        state="$(dse_curve_state "$dse_name")"
        if [ "$state" = "dormant" ]; then
            # Curve is default-zero — stub is honoring its promise.
            continue
        fi
        if [ "$state" = "missing" ]; then
            stub_offenders+=("$planner_file:$lineno  STUB($dse_name) names a DSE file that doesn't exist (src/ai/dses/${dse_name}.rs)")
            continue
        fi
        # state == "lifted" — require an allowlist entry.
        allowlist_key="STUB:${planner_file}:${lineno}:${dse_name}"
        if is_allowlisted "$allowlist_key"; then
            allowlisted_stub_hits=$((allowlisted_stub_hits + 1))
        else
            stub_offenders+=("$planner_file:$lineno  STUB($dse_name) lifted without successor — add allowlist entry \"$allowlist_key <ticket-id>\" or rewrite the comment")
        fi
    done < <(rg -n '\bSTUB\(' "$planner_file" --no-heading 2>/dev/null || true)
done

# Report.
exit_code=0
total_allowlisted=$((allowlisted_marker_hits + allowlisted_consideration_hits + allowlisted_stub_hits))

if [ "${#marker_offenders[@]}" -gt 0 ] || [ "${#consideration_offenders[@]}" -gt 0 ] || [ "${#stub_offenders[@]}" -gt 0 ]; then
    exit_code=1
    if [ "${#marker_offenders[@]}" -gt 0 ]; then
        echo "substrate-stubs: orphan markers in $MARKERS_FILE" >&2
        for line in "${marker_offenders[@]}"; do
            echo "  $line" >&2
        done
    fi
    if [ "${#consideration_offenders[@]}" -gt 0 ]; then
        echo "substrate-stubs: invalid consideration string-name references" >&2
        for line in "${consideration_offenders[@]}"; do
            echo "  $line" >&2
        done
    fi
    if [ "${#stub_offenders[@]}" -gt 0 ]; then
        echo "substrate-stubs: plan-template stubs out-of-sync with their DSE curves" >&2
        for line in "${stub_offenders[@]}"; do
            echo "  $line" >&2
        done
    fi
    echo "" >&2
    echo "See docs/open-work/pre-existing/substrate-stub-catalogue.md for the catalogue." >&2
    echo "To allowlist a known orphan pending follow-on work, add an entry to $ALLOWLIST" >&2
    echo "  with the ticket id that wires it." >&2
else
    if [ "$total_allowlisted" -gt 0 ]; then
        echo "substrate-stubs: ok ($total_allowlisted allowlisted — see $ALLOWLIST)"
    else
        echo "substrate-stubs: all markers wired and all consideration string-names match"
    fi
fi

exit "$exit_code"
