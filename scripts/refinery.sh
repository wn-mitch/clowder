#!/usr/bin/env bash
# Refinery — the gated lander for parallel sessions.
# Stage 1.4 (manual landing) + Stage 2.1 (--auto gate) of ticket 354.
#
# Walks every session/<slug> bookmark, reports rebase / conflict status,
# and (with --land or --auto) merges a session's work into main.
#
# Modes:
#   refinery.sh                       report all session bookmarks
#   refinery.sh --json                machine-readable for /work + /foreman
#   refinery.sh --track <name>        filter the report (any track)
#   refinery.sh --land <slug>         land one session (any track, manual gate)
#   refinery.sh --auto [--dry-run]    drain swarm-safe queue, gated on
#                                     working-copy clean + just check && just test
#
# Landing pipeline (per session, both --land and --auto):
#   1. verify session/<slug> bookmark exists + is ahead of main
#   2. rebase onto main if needed (refuses on conflict)
#   3. set main to the session's head (effectively a fast-forward)
#   4. forget session/<slug> bookmark
#   5. session_done.sh <slug> --no-release (tickets are 'done' via just land)
#
# --auto gate (run per session BEFORE step 1):
#   a. track == swarm-safe (whitelist enforced HERE and at flag-parse)
#   b. jj status shows no uncommitted working-copy edits in the workspace
#   c. cd <workspace> && just check && just test exits 0
# Sessions that fail the gate are reported (gate-fail) but not landed.
# --dry-run runs the gate but skips steps 1-5.

set -euo pipefail

REPO_ROOT=$(cd "$(dirname "$0")/.." && pwd)
SESSIONS_ROOT="$HOME/clowder-sessions"
AUTO_WHITELIST_TRACK="swarm-safe"

mode="report"
filter_track=""
target_slug=""
emit_json="false"
dry_run="false"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --json) emit_json="true"; shift ;;
        --track)
            filter_track="$2"; shift 2 ;;
        --land) mode="land"; target_slug="$2"; shift 2 ;;
        --auto) mode="auto"; shift ;;
        --dry-run) dry_run="true"; shift ;;
        -h|--help) sed -n '/^# Modes:/,/^# --dry-run/p' "$0" | sed 's/^# \?//'; exit 0 ;;
        --*) echo "Unknown flag: $1" >&2; exit 2 ;;
        *) echo "Unexpected positional: $1" >&2; exit 2 ;;
    esac
done

# Whitelist enforcement, layer 1: refuse --auto --track <not-swarm-safe>
# (The per-row filter inside auto() is layer 2 — even without --track, only
# swarm-safe rows land.)
if [[ "$mode" == "auto" && -n "$filter_track" && "$filter_track" != "$AUTO_WHITELIST_TRACK" ]]; then
    echo "ERROR: --auto is whitelisted to track=$AUTO_WHITELIST_TRACK only (got --track $filter_track)." >&2
    echo "       Substrate-sensitive and coherent-block sessions land via --land <slug>." >&2
    exit 2
fi

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

auto_gate() {
    # Runs the swarm-safe auto-land gate for one session bookmark.
    # Prints a single tab-separated outcome line:
    #   <slug>\t<outcome>\t<detail>
    # where outcome ∈ {gate-pass, wrong-track, not-fast-forward, dirty-working-copy,
    #                  no-workspace, check-fail, test-fail}
    local bm="$1"
    local slug="${bm#session/}"
    local workspace="$SESSIONS_ROOT/$slug"

    local info_file="$workspace/.session-info.json"
    local track="unknown"
    if [[ -f "$info_file" ]]; then
        track=$(python3 -c "import json,sys; print(json.load(open(sys.argv[1])).get('track', 'unknown'))" "$info_file" 2>/dev/null || echo "unknown")
    fi

    # Layer 2 of the whitelist: per-row track filter.
    if [[ "$track" != "$AUTO_WHITELIST_TRACK" ]]; then
        printf '%s\t%s\t%s\n' "$slug" "wrong-track" "track=$track (auto allowed only on $AUTO_WHITELIST_TRACK)"
        return 0
    fi

    # Must be fast-forwardable (no rebase needed for auto-land; conflict resolution
    # is human-only).
    local ahead behind
    ahead=$(jj log -r "main..bookmarks(\"$bm\")" --no-graph -T 'change_id ++ "\n"' 2>/dev/null | grep -c . || true)
    behind=$(jj log -r "bookmarks(\"$bm\")..main" --no-graph -T 'change_id ++ "\n"' 2>/dev/null | grep -c . || true)
    if (( ahead == 0 )); then
        printf '%s\t%s\t%s\n' "$slug" "no-changes" "bookmark has no commits ahead of main"
        return 0
    fi
    if (( behind > 0 )); then
        printf '%s\t%s\t%s\n' "$slug" "not-fast-forward" "behind=$behind (rebase required; manual --land only)"
        return 0
    fi

    if [[ ! -d "$workspace" ]]; then
        printf '%s\t%s\t%s\n' "$slug" "no-workspace" "$workspace missing"
        return 0
    fi

    # Working-copy clean preflight: no uncommitted [AM] edits.
    if (cd "$workspace" && jj status 2>/dev/null) | grep -qE '^[AM] '; then
        printf '%s\t%s\t%s\n' "$slug" "dirty-working-copy" "uncommitted edits in $workspace"
        return 0
    fi

    # Gate: just check && just test inside the workspace. Output redirected to a
    # per-session log so the master report stays scannable.
    local gate_log="$workspace/.refinery-gate.log"
    : > "$gate_log"
    if ! (cd "$workspace" && just check >> "$gate_log" 2>&1); then
        printf '%s\t%s\t%s\n' "$slug" "check-fail" "see $gate_log"
        return 0
    fi
    if ! (cd "$workspace" && just test >> "$gate_log" 2>&1); then
        printf '%s\t%s\t%s\n' "$slug" "test-fail" "see $gate_log"
        return 0
    fi

    printf '%s\t%s\t%s\n' "$slug" "gate-pass" "just check && just test passed in $workspace"
    return 0
}

auto() {
    local rows=()
    while read -r bm; do
        [[ -z "$bm" ]] && continue
        rows+=("$(auto_gate "$bm")")
    done < <(list_session_bookmarks)

    local rows_out=()
    local landed_count=0
    if (( ${#rows[@]} > 0 )); then
        for row in "${rows[@]}"; do
            IFS=$'\t' read -r slug outcome detail <<< "$row"
            if [[ "$outcome" == "gate-pass" && "$dry_run" == "false" ]]; then
                if land "$slug" >/dev/null 2>&1; then
                    row="$slug"$'\t'"landed"$'\t'"main advanced to session/$slug head"
                    landed_count=$((landed_count + 1))
                else
                    row="$slug"$'\t'"land-failed"$'\t'"land step errored after gate passed (rerun manually with --land $slug)"
                fi
            fi
            rows_out+=("$row")
        done
    fi

    if [[ "$emit_json" == "true" ]]; then
        echo "["
        local i n
        n=${#rows_out[@]}
        for ((i=0; i<n; i++)); do
            IFS=$'\t' read -r slug outcome detail <<< "${rows_out[$i]}"
            local comma=","
            (( i == n - 1 )) && comma=""
            local detail_json
            detail_json=$(python3 -c 'import json,sys; print(json.dumps(sys.argv[1]))' "$detail")
            printf '  {"slug":"%s","outcome":"%s","detail":%s}%s\n' \
                "$slug" "$outcome" "$detail_json" "$comma"
        done
        echo "]"
        return
    fi

    if (( ${#rows_out[@]} == 0 )); then
        echo "refinery --auto: no session/* bookmarks (queue empty)"
        return
    fi

    if [[ "$dry_run" == "true" ]]; then
        echo "refinery --auto --dry-run: gate-only (no landings)"
    fi
    printf '%-28s %-22s %s\n' "SLUG" "OUTCOME" "DETAIL"
    for row in "${rows_out[@]}"; do
        IFS=$'\t' read -r slug outcome detail <<< "$row"
        printf '%-28s %-22s %s\n' "$slug" "$outcome" "$detail"
    done

    if (( landed_count > 0 )); then
        echo
        echo "refinery --auto: landed $landed_count session(s) → main"
    fi
}

case "$mode" in
    report) report ;;
    land) land "$target_slug" ;;
    auto) auto ;;
esac
