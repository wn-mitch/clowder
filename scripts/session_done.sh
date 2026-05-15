#!/usr/bin/env bash
# Clean up a parallel-session workspace after its bookmark has landed
# (or the work has been abandoned). Stage 1.3 of ticket 354; hardened
# against bookmark-orphaning by ticket 362.
#
# What this does:
#   1. (default) release any in-progress ticket-claims from .session-info.json
#      back to 'ready' unless --no-release
#   2. cargo clean inside the workspace (reclaims target/ disk)
#   3. jj workspace forget <slug>
#   4. rm -rf the workspace directory
#   5. Bookmark disposition (ticket 362 — was unsafe by default before):
#        - default: forget IFF the bookmark's tip is an ancestor of
#          origin/main (the work landed); otherwise preserve it (work is
#          unpushed and would be orphaned by forget)
#        - --keep-bookmark: preserve regardless (back-compat; was the
#          old explicit opt-in for the safe behavior)
#        - --forget-bookmark: explicit force-forget; pair with
#          --orphan-ok to forget a bookmark whose tip is NOT on main
#          (you're knowingly discarding unlanded work — `just orphan-scan`
#          should not surface these later)
#
# Refuses if the workspace has uncommitted working-copy edits unless
# --force.
#
# Usage:
#   session-done.sh <slug> [--keep-bookmark | --forget-bookmark [--orphan-ok]] \
#                          [--force] [--no-release]

set -euo pipefail

REPO_ROOT=$(cd "$(dirname "$0")/.." && pwd)
SESSIONS_ROOT="$HOME/clowder-sessions"

slug=""
bookmark_mode="auto"  # auto | keep | forget
orphan_ok="false"
force="false"
no_release="false"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --keep-bookmark) bookmark_mode="keep"; shift ;;
        --forget-bookmark) bookmark_mode="forget"; shift ;;
        --orphan-ok) orphan_ok="true"; shift ;;
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

# Ticket 362 bookmark-orphaning precondition: decide whether the bookmark
# is safe to forget. "Safe" means the bookmark's tip is reachable from
# origin/main (the work has landed via `just refinery --land`). If we
# can't determine reachability, fall back to PRESERVING the bookmark —
# the safe-by-default direction. The user can override with explicit
# --forget-bookmark + --orphan-ok if they know the work is junk.
bookmark_name="session/$slug"
bookmark_landed="false"
bookmark_exists="false"

cd "$REPO_ROOT"
if jj bookmark list "$bookmark_name" 2>/dev/null | grep -q "^$bookmark_name"; then
    bookmark_exists="true"
    # Resolve the bookmark to a commit and ask whether it's an ancestor
    # of main@origin (remote main; the durable landing target). Falls
    # back to local `main` if no remote tracking is configured.
    base_ref="main@origin"
    if ! jj log -r "$base_ref" --no-graph -T 'commit_id' --limit 1 >/dev/null 2>&1; then
        base_ref="main"
    fi
    # `bookmark & ::base_ref` is the intersection of the bookmark with
    # the ancestors of base (inclusive). Non-empty iff the bookmark's
    # tip is reachable from main — i.e., the work has landed. Empty
    # means the bookmark sits on a divergent path; forgetting it
    # would orphan its commits.
    if [[ -n "$(jj log -r "$bookmark_name & ::$base_ref" --no-graph -T 'commit_id ++ \"\n\"' 2>/dev/null)" ]]; then
        bookmark_landed="true"
    fi
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
            unpadded=$(echo "$tid_trimmed" | sed 's/^0*//'); [[ -z "$unpadded" ]] && unpadded="0"
            padded=$(printf "%03d" "$unpadded" 2>/dev/null || echo "$tid_trimmed")
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

# Bookmark disposition. Default mode = auto: forget IFF landed.
should_forget="false"
forget_reason=""
case "$bookmark_mode" in
    keep)
        should_forget="false"
        forget_reason="--keep-bookmark"
        ;;
    forget)
        if [[ "$bookmark_landed" == "true" || "$orphan_ok" == "true" ]]; then
            should_forget="true"
            if [[ "$bookmark_landed" == "true" ]]; then
                forget_reason="--forget-bookmark (tip on main)"
            else
                forget_reason="--forget-bookmark --orphan-ok (knowingly discarding unlanded work)"
            fi
        else
            echo "ERROR: --forget-bookmark refused: bookmark $bookmark_name has commits not on main." >&2
            echo "  This would orphan the work (ticket 362 protection)." >&2
            echo "  Options:" >&2
            echo "    - run 'just orphan-scan' to inspect the unlanded commits" >&2
            echo "    - 'jj duplicate <hex> -d main' to rescue, then re-run" >&2
            echo "    - re-run with --orphan-ok to discard the work explicitly" >&2
            exit 1
        fi
        ;;
    auto)
        if [[ "$bookmark_exists" != "true" ]]; then
            should_forget="false"
            forget_reason="bookmark does not exist"
        elif [[ "$bookmark_landed" == "true" ]]; then
            should_forget="true"
            forget_reason="auto-forget (tip on main)"
        else
            should_forget="false"
            forget_reason="auto-preserve (tip NOT on main; would orphan)"
            echo "session-done: preserving bookmark $bookmark_name — its tip is not on main." >&2
            echo "  Run 'just orphan-scan' to triage. Pass --forget-bookmark --orphan-ok to discard." >&2
        fi
        ;;
esac

if [[ "$should_forget" == "true" && "$bookmark_exists" == "true" ]]; then
    jj bookmark forget "$bookmark_name" 2>&1 | head -3 || true
fi

bookmark_status="kept ($forget_reason)"
[[ "$should_forget" == "true" ]] && bookmark_status="forgotten ($forget_reason)"
echo "session-done: $slug cleaned (workspace removed, bookmark $bookmark_status)"
