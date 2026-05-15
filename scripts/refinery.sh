#!/usr/bin/env bash
# Refinery — the verdict-gated lander for parallel sessions.
# Stage 1.4 of ticket 354.
#
# Walks every session/<slug> bookmark, reports rebase / conflict status,
# and (with --land) merges a session's work into main.
#
# Modes:
#   refinery.sh                       report all session bookmarks
#   refinery.sh --json                machine-readable for /work skill
#   refinery.sh --track <name>        filter the report
#   refinery.sh --land <slug>         land one session (any track)
#
# Landing pipeline (per session):
#   1. verify session/<slug> bookmark exists + is ahead of main
#   2. verify rebase onto main is conflict-free
#   3. set main to the session's head (effectively a fast-forward / rebase merge)
#   4. forget session/<slug> bookmark
#   5. session-done.sh <slug> --no-release (tickets are 'done' via just land)
#
# --auto is intentionally NOT implemented in this commit. Verdict-gated
# auto-land needs explicit verdict integration; that lands in a follow-on.
# For now every land is per-bookmark and human-decided (via /work).

set -euo pipefail

REPO_ROOT=$(cd "$(dirname "$0")/.." && pwd)
SESSIONS_ROOT="$HOME/clowder-sessions"

mode="report"
filter_track=""
target_slug=""
emit_json="false"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --json) emit_json="true"; shift ;;
        --track) filter_track="$2"; shift 2 ;;
        --land) mode="land"; target_slug="$2"; shift 2 ;;
        --auto)
            echo "ERROR: --auto not yet implemented (verdict integration pending). Use --land <slug>." >&2
            exit 2 ;;
        -h|--help) sed -n '/^# Modes:/,/^# Landing/p' "$0" | sed 's/^# \?//'; exit 0 ;;
        --*) echo "Unknown flag: $1" >&2; exit 2 ;;
        *) echo "Unexpected positional: $1" >&2; exit 2 ;;
    esac
done

cd "$REPO_ROOT"

list_session_bookmarks() {
    jj bookmark list 2>/dev/null \
        | awk '/^session\//{print $1}' \
        | tr -d ':'
}

session_status() {
    local bm="$1"
    local slug="${bm#session/}"
    local workspace="$SESSIONS_ROOT/$slug"

    local info_file="$workspace/.session-info.json"
    local track="unknown"
    local tickets="—"
    if [[ -f "$info_file" ]]; then
        track=$(python3 -c "import json,sys; print(json.load(open(sys.argv[1])).get('track', 'unknown'))" "$info_file" 2>/dev/null || echo "unknown")
        tickets=$(python3 -c "
import json, sys
info = json.load(open(sys.argv[1]))
print(','.join(str(t) for t in info.get('tickets', [])) or '—')
" "$info_file" 2>/dev/null || echo "—")
    fi

    # Count commits ahead of main + behind
    local ahead behind
    ahead=$(jj log -r "main..bookmarks(\"$bm\")" --no-graph -T 'change_id ++ "\n"' 2>/dev/null | grep -c . || true)
    behind=$(jj log -r "bookmarks(\"$bm\")..main" --no-graph -T 'change_id ++ "\n"' 2>/dev/null | grep -c . || true)

    local rebase_state action
    if (( ahead == 0 )); then
        rebase_state="already-on-main"
        action="forget-bookmark"
    elif (( behind == 0 )); then
        rebase_state="clean-fast-forward"
        action="landable-manual"
    else
        # Diverged from main — conflict status discovered at land-time
        # (jj 0.39 has no dry-run rebase; the land path attempts it).
        rebase_state="needs-rebase"
        action="landable-manual"
    fi

    # Coherent-block intermediates require anchor verdict — never landable-auto
    # (and currently never landable-manual without explicit override).
    if [[ "$track" == "coherent-block" ]]; then
        if [[ "$action" == "landable-manual" ]]; then
            action="awaiting-anchor"
        fi
    fi

    printf '%s\t%s\t%s\t%s\t%s\t%d\t%d\n' "$bm" "$slug" "$track" "$tickets" "$rebase_state:$action" "$ahead" "$behind"
}

report() {
    local rows=()
    while read -r bm; do
        [[ -z "$bm" ]] && continue
        local row
        row=$(session_status "$bm")
        if [[ -n "$filter_track" ]]; then
            local row_track
            row_track=$(echo "$row" | cut -f3)
            [[ "$row_track" != "$filter_track" ]] && continue
        fi
        rows+=("$row")
    done < <(list_session_bookmarks)

    if [[ "$emit_json" == "true" ]]; then
        echo "["
        local i
        for i in "${!rows[@]}"; do
            IFS=$'\t' read -r bm slug track tickets action ahead behind <<< "${rows[$i]}"
            local comma=","
            (( i == ${#rows[@]} - 1 )) && comma=""
            printf '  {"bookmark":"%s","slug":"%s","track":"%s","tickets":"%s","status":"%s","ahead":%d,"behind":%d}%s\n' \
                "$bm" "$slug" "$track" "$tickets" "$action" "$ahead" "$behind" "$comma"
        done
        echo "]"
        return
    fi

    if (( ${#rows[@]} == 0 )); then
        echo "refinery: no session/* bookmarks (no sessions to land)"
        return
    fi

    printf '%-28s %-22s %-12s %-26s %5s %5s\n' "BOOKMARK" "TRACK" "TICKETS" "STATUS" "AHEAD" "BEHIND"
    for row in "${rows[@]}"; do
        IFS=$'\t' read -r bm slug track tickets action ahead behind <<< "$row"
        printf '%-28s %-22s %-12s %-26s %5d %5d\n' "$bm" "$track" "$tickets" "$action" "$ahead" "$behind"
    done
}

land() {
    local slug="$1"
    local bm="session/$slug"

    if ! jj bookmark list 2>/dev/null | awk '{print $1}' | tr -d ':' | grep -qx "$bm"; then
        echo "ERROR: bookmark '$bm' not found" >&2
        exit 1
    fi

    local ahead behind
    ahead=$(jj log -r "main..bookmarks(\"$bm\")" --no-graph -T 'change_id ++ "\n"' 2>/dev/null | grep -c . || true)
    behind=$(jj log -r "bookmarks(\"$bm\")..main" --no-graph -T 'change_id ++ "\n"' 2>/dev/null | grep -c . || true)

    if (( ahead == 0 )); then
        echo "refinery: $bm has no new commits over main — forgetting bookmark only"
        jj bookmark forget "$bm"
        if [[ -d "$SESSIONS_ROOT/$slug" ]]; then
            bash "$REPO_ROOT/scripts/session_done.sh" "$slug" --no-release || true
        fi
        return
    fi

    if (( behind > 0 )); then
        echo "refinery: $bm is $behind commit(s) behind main — rebasing onto main first"
        jj rebase -r "bookmarks(\"$bm\")" -d main || {
            echo "ERROR: rebase failed (manual conflict resolution required)" >&2
            exit 1
        }
    fi

    # Move main to the session's head (no merge commit — fast-forward style)
    jj bookmark set main -r "bookmarks(\"$bm\")" --allow-backwards >/dev/null 2>&1 || {
        echo "ERROR: failed to advance main to $bm" >&2
        exit 1
    }
    echo "refinery: main advanced to $bm head"

    # Forget the session bookmark
    jj bookmark forget "$bm"

    # Clean up the workspace
    if [[ -d "$SESSIONS_ROOT/$slug" ]]; then
        bash "$REPO_ROOT/scripts/session_done.sh" "$slug" --no-release || true
    fi

    echo "refinery: landed $slug → main"
}

case "$mode" in
    report) report ;;
    land) land "$target_slug" ;;
esac
