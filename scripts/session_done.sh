#!/usr/bin/env bash
# Clean up a parallel-session workspace after its bookmark has landed
# (or the work has been abandoned). Stage 1.3 of ticket 354.
#
# What this does:
#   1. cargo clean inside the workspace (reclaims target/ disk)
#   2. jj workspace forget <slug>
#   3. rm -rf the workspace directory
#   4. (default) jj bookmark forget session/<slug>
#      --keep-bookmark preserves the bookmark (e.g., for a later land)
#
# Refuses if the workspace has uncommitted working-copy edits unless
# --force.
#
# Usage:
#   session-done.sh <slug> [--keep-bookmark] [--force]

set -euo pipefail

REPO_ROOT=$(cd "$(dirname "$0")/.." && pwd)
SESSIONS_ROOT="$HOME/clowder-sessions"

slug=""
keep_bookmark="false"
force="false"
no_release="false"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --keep-bookmark) keep_bookmark="true"; shift ;;
        --force) force="true"; shift ;;
        --no-release) no_release="true"; shift ;;
        -h|--help) sed -n '/^# Usage:/,/^$/p' "$0" | sed 's/^# \?//'; exit 0 ;;
        --*) echo "Unknown flag: $1" >&2; exit 2 ;;
        *) slug="$1"; shift ;;
    esac
done

[[ -z "$slug" ]] && { echo "ERROR: missing <slug>" >&2; exit 2; }

workspace="$SESSIONS_ROOT/$slug"
if [[ ! -d "$workspace" ]]; then
    echo "WARN: $workspace does not exist; cleaning bookmark + workspace state only"
fi

# Safety: check for unsnapshotted working-copy edits unless --force
if [[ -d "$workspace" && "$force" != "true" ]]; then
    pushd "$workspace" >/dev/null
    if jj status 2>&1 | grep -q '^Working copy changes:'; then
        if jj status 2>&1 | grep -qE '^[AM] '; then
            popd >/dev/null
            echo "ERROR: $workspace has uncommitted working-copy changes." >&2
            echo "  Run 'cd $workspace && jj status' to inspect, then either:" >&2
            echo "    - commit/snapshot the changes, OR" >&2
            echo "    - rerun with --force to discard them" >&2
            exit 1
        fi
    fi
    popd >/dev/null
fi

# Release any in-progress ticket claims back to ready (unless --no-release).
# Skips tickets already marked done — `just land` may have set them.
if [[ "$no_release" != "true" && -f "$workspace/.session-info.json" ]]; then
    tickets=$(python3 -c '
import json, sys
try:
    info = json.load(open(sys.argv[1]))
    print(",".join(str(t) for t in info.get("tickets", [])))
except Exception:
    pass
' "$workspace/.session-info.json")
    if [[ -n "$tickets" ]]; then
        IFS=',' read -ra tids <<< "$tickets"
        for tid in "${tids[@]}"; do
            tid_trimmed=$(echo "$tid" | tr -d ' ')
            [[ -z "$tid_trimmed" ]] && continue
            padded=$(printf "%03d" "$tid_trimmed" 2>/dev/null || echo "$tid_trimmed")
            tfile=$(ls "$REPO_ROOT/docs/open-work/tickets/${padded}-"*.md 2>/dev/null | head -1)
            [[ -z "$tfile" ]] && tfile=$(ls "$REPO_ROOT/docs/open-work/tickets/${tid_trimmed}-"*.md 2>/dev/null | head -1)
            if [[ -n "$tfile" ]]; then
                current=$(awk -F': *' '/^status:/ { print $2; exit }' "$tfile" | tr -d ' ')
                if [[ "$current" == "in-progress" ]]; then
                    awk '/^status:/ && !done { print "status: ready"; done=1; next } { print }' "$tfile" > "$tfile.tmp" && mv "$tfile.tmp" "$tfile"
                    echo "session-done: released ticket $tid_trimmed (in-progress → ready)"
                fi
            fi
        done
    fi
fi

# Reclaim target/ disk before removing the directory
if [[ -d "$workspace/target" ]]; then
    (cd "$workspace" && cargo clean 2>&1 | tail -3) || true
fi

# Forget the jj workspace
cd "$REPO_ROOT"
jj workspace forget "$slug" 2>&1 | head -3 || true

# Remove the directory
if [[ -d "$workspace" ]]; then
    rm -rf "$workspace"
fi

# Forget the bookmark unless asked to keep it
if [[ "$keep_bookmark" != "true" ]]; then
    jj bookmark forget "session/$slug" 2>&1 | head -3 || true
fi

echo "session-done: $slug cleaned (workspace removed, bookmark $([[ $keep_bookmark == true ]] && echo kept || echo forgotten))"
