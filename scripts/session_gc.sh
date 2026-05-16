#!/usr/bin/env bash
# Batch GC for parallel-session workspaces — finds sessions whose work has
# landed on main@origin and runs `session_done.sh` against each. Composes the
# existing per-session primitive; never duplicates the cleanup logic.
#
# A session is considered "landed" when its bookmark tip is an ancestor of
# main@origin (the same ancestry rule session_done.sh uses by default).
#
# Usage:
#   session-gc                   # report + prompt per session before acting
#   session-gc --dry-run         # report only; no cleanup
#   session-gc --yes             # report + GC all landed sessions without prompting
#   session-gc --json            # machine-readable status (for /work skill)
#   session-gc --force           # pass --force to session_done.sh (uncommitted edits OK)
#
# Exit codes:
#   0  no sessions to GC, or all GCs succeeded
#   1  at least one GC failed
#   2  bad usage

set -euo pipefail

REPO_ROOT=$(cd "$(dirname "$0")/.." && pwd)
SESSIONS_ROOT="$HOME/clowder-sessions"

dry_run="false"
yes="false"
emit_json="false"
force="false"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run) dry_run="true"; shift ;;
        --yes|-y) yes="true"; shift ;;
        --json) emit_json="true"; shift ;;
        --force) force="true"; shift ;;
        -h|--help) sed -n '/^# Usage:/,/^$/p' "$0" | sed 's/^# \?//'; exit 0 ;;
        *) echo "Unknown arg: $1" >&2; exit 2 ;;
    esac
done

if [[ ! -d "$SESSIONS_ROOT" ]]; then
    if [[ "$emit_json" == "true" ]]; then
        echo "[]"
    else
        echo "session-gc: no $SESSIONS_ROOT/ — nothing to GC"
    fi
    exit 0
fi

cd "$REPO_ROOT"

# Classify each session: landed | unlanded | no-bookmark | not-a-jj-workspace
classify() {
    local slug="$1"
    local bookmark="session/$slug"
    local tip
    tip=$(jj log -r "bookmarks(\"$bookmark\")" --no-graph -T 'commit_id.short()' 2>/dev/null | head -1 || true)
    if [[ -z "$tip" ]]; then
        echo "no-bookmark"
        return
    fi
    # Is the bookmark tip an ancestor of main@origin?
    if jj log -r "$tip & ::main@origin" --no-graph -T 'commit_id.short()' 2>/dev/null | grep -q .; then
        echo "landed"
    else
        echo "unlanded"
    fi
}

# Build the report. Use parallel arrays to stay compatible with bash 3.2 (macOS).
slugs=()
session_statuses=()
for path in "$SESSIONS_ROOT"/*/; do
    [[ -d "$path" ]] || continue
    slug=$(basename "$path")
    slugs+=("$slug")
    session_statuses+=("$(classify "$slug")")
done

if [[ ${#slugs[@]} -eq 0 ]]; then
    if [[ "$emit_json" == "true" ]]; then
        echo "[]"
    else
        echo "session-gc: no sessions in flight"
    fi
    exit 0
fi

if [[ "$emit_json" == "true" ]]; then
    printf '['
    for i in "${!slugs[@]}"; do
        if [[ $i -gt 0 ]]; then
            printf ','
        fi
        printf '\n  {"slug": "%s", "status": "%s"}' "${slugs[$i]}" "${session_statuses[$i]}"
    done
    printf '\n]\n'
    exit 0
fi

printf '%-26s %s\n' "SLUG" "STATUS"
for i in "${!slugs[@]}"; do
    printf '%-26s %s\n' "${slugs[$i]}" "${session_statuses[$i]}"
done

# Collect landed sessions for GC.
to_gc=()
for i in "${!slugs[@]}"; do
    if [[ "${session_statuses[$i]}" == "landed" ]]; then
        to_gc+=("${slugs[$i]}")
    fi
done

if [[ ${#to_gc[@]} -eq 0 ]]; then
    echo
    echo "session-gc: no landed sessions to clean up"
    exit 0
fi

echo
echo "Landed sessions ready for GC: ${to_gc[*]}"

if [[ "$dry_run" == "true" ]]; then
    echo "(dry-run — not acting)"
    exit 0
fi

if [[ "$yes" != "true" ]]; then
    read -r -p "GC ${#to_gc[@]} session(s)? [y/N] " ans
    case "$ans" in
        y|Y|yes) ;;
        *) echo "session-gc: aborted"; exit 0 ;;
    esac
fi

failures=0
done_args=()
[[ "$force" == "true" ]] && done_args+=(--force)

for slug in "${to_gc[@]}"; do
    echo
    echo "=== session-gc: cleaning $slug ==="
    if ! bash "$REPO_ROOT/scripts/session_done.sh" "$slug" "${done_args[@]}"; then
        echo "session-gc: FAILED to clean $slug" >&2
        failures=$((failures + 1))
    fi
done

if [[ $failures -gt 0 ]]; then
    echo
    echo "session-gc: $failures session(s) failed to clean" >&2
    exit 1
fi

echo
echo "session-gc: cleaned ${#to_gc[@]} session(s)"
