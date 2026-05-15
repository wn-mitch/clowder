#!/usr/bin/env bash
# Surface orphaned bookmarks + commits whose work is NOT reachable from
# main@origin. Ticket 362 — companion to session_done.sh's bookmark-
# preservation default; this is the recovery path for orphans that
# already exist (either pre-fix legacy or new ones from an explicit
# --orphan-ok cleanup).
#
# Two passes:
#   1. Local bookmarks. Walks every `session/*` bookmark; flags those
#      whose tip is NOT an ancestor of main@origin.
#   2. Reachable-but-detached commits. Walks the jj op-log for
#      `feat:`/`fix:`/`land:` commits with messages matching active
#      ticket ids; flags any whose commits aren't on main@origin and
#      aren't reachable from any bookmark.
#
# Usage:
#   orphan-scan [--json]

set -euo pipefail

REPO_ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$REPO_ROOT"

json_mode="false"
while [[ $# -gt 0 ]]; do
    case "$1" in
        --json) json_mode="true"; shift ;;
        -h|--help) sed -n '/^# Usage:/,/^$/p' "$0" | sed 's/^# \?//'; exit 0 ;;
        *) echo "Unknown arg: $1" >&2; exit 2 ;;
    esac
done

base_ref="main@origin"
if ! jj log -r "$base_ref" --no-graph -T 'commit_id' --limit 1 >/dev/null 2>&1; then
    base_ref="main"
fi

# ---- Pass 1: local session bookmarks ----------------------------------

declare -a orphan_bookmarks=()
while IFS= read -r bm; do
    [[ -z "$bm" ]] && continue
    # Only the bookmark name (strip the colon + commit metadata).
    name="${bm%%:*}"
    name="${name## }"
    [[ "$name" =~ ^session/ ]] || continue
    if [[ -z "$(jj log -r "$name & ::$base_ref" --no-graph -T 'commit_id' 2>/dev/null)" ]]; then
        tip=$(jj log -r "$name" --no-graph -T 'commit_id.shortest(8) ++ " " ++ description.first_line()' --limit 1 2>/dev/null || echo "?")
        orphan_bookmarks+=("$name|$tip")
    fi
done < <(jj bookmark list 2>/dev/null | grep -E '^session/' || true)

# ---- Pass 2: detached commits matching active ticket numbers ---------

# Collect every known ticket id — both active (tickets/) and landed
# (landed/). Orphans matching a landed id are particularly load-bearing:
# the ticket is "supposed to be done" but the work is unreachable.
known_ids=$( { ls "$REPO_ROOT/docs/open-work/tickets/" 2>/dev/null;
               ls "$REPO_ROOT/docs/open-work/landed/" 2>/dev/null; } | \
    awk -F'-' '/^[0-9]+/ { print $1 }' | sort -u)

declare -a orphan_commits=()
# Walk every visible commit in the op-log NOT reachable from main@origin
# AND NOT reachable from any extant bookmark.
detached_revset="all() ~ ::$base_ref ~ ::bookmarks() ~ empty()"
while IFS=$'\t' read -r cid msg; do
    [[ -z "$cid" ]] && continue
    # Match against known ticket numbers. To dodge false positives from
    # SHA hex substrings (e.g. `…158a57` matching id 057), require the
    # match to be anchored at a word boundary AND prefixed by an explicit
    # ticket-reference marker: `#NNN`, `ticket NNN`, `NNN —`, or
    # `NNN/NNN ` (the multi-ticket combo header like `332/333`).
    # Match against known ticket numbers using anchored word-boundary
    # regex. The number must be the WHOLE token — surrounded by either
    # a non-digit OR start/end of string. This dodges false positives
    # like `#3` matching inside `#336`. Iterate longest-id-first so the
    # most-specific match wins when both a short and long id match.
    matched_id=""
    for id in $(echo "$known_ids" | awk '{ print length, $0 }' | sort -rn | awk '{ print $2 }'); do
        unpadded="$(echo "$id" | sed 's/^0*//')"
        [[ -z "$unpadded" ]] && unpadded="0"
        # `\b` doesn't work in basic POSIX regex consistently across
        # macOS/Linux; use explicit non-digit boundary alternatives via
        # extended regex with `(^|[^0-9])` and `([^0-9]|$)`.
        if echo "$msg" | grep -qE "(^|[^0-9])(${id}|${unpadded})([^0-9]|$)"; then
            matched_id="$id"
            break
        fi
    done
    [[ -z "$matched_id" ]] && continue
    orphan_commits+=("$cid|$matched_id|$msg")
done < <(jj log -r "$detached_revset" --no-graph \
    -T 'commit_id.shortest(8) ++ "\t" ++ description.first_line() ++ "\n"' 2>/dev/null || true)

# ---- Output -----------------------------------------------------------

if [[ "$json_mode" == "true" ]]; then
    printf '{\n  "base_ref": "%s",\n  "orphan_bookmarks": [\n' "$base_ref"
    first="true"
    for entry in "${orphan_bookmarks[@]:-}"; do
        [[ -z "$entry" ]] && continue
        name="${entry%%|*}"; tip="${entry#*|}"
        cid="${tip%% *}"; line="${tip#* }"
        [[ "$first" == "true" ]] || printf ',\n'
        printf '    {"bookmark": "%s", "tip": "%s", "subject": %s}' \
            "$name" "$cid" "$(printf '%s' "$line" | python3 -c 'import json,sys;print(json.dumps(sys.stdin.read().strip()))')"
        first="false"
    done
    printf '\n  ],\n  "orphan_commits": [\n'
    first="true"
    for entry in "${orphan_commits[@]:-}"; do
        [[ -z "$entry" ]] && continue
        cid="${entry%%|*}"; rest="${entry#*|}"
        ticket="${rest%%|*}"; msg="${rest#*|}"
        [[ "$first" == "true" ]] || printf ',\n'
        printf '    {"commit": "%s", "ticket": "%s", "subject": %s}' \
            "$cid" "$ticket" "$(printf '%s' "$msg" | python3 -c 'import json,sys;print(json.dumps(sys.stdin.read().strip()))')"
        first="false"
    done
    printf '\n  ]\n}\n'
else
    echo "orphan-scan: base = $base_ref"
    echo ""
    if [[ ${#orphan_bookmarks[@]} -eq 0 ]] || [[ -z "${orphan_bookmarks[0]:-}" ]]; then
        echo "Local session/* bookmarks not on main: none"
    else
        echo "Local session/* bookmarks NOT on $base_ref:"
        for entry in "${orphan_bookmarks[@]}"; do
            [[ -z "$entry" ]] && continue
            name="${entry%%|*}"; tip="${entry#*|}"
            echo "  $name -> $tip"
        done
    fi
    echo ""
    if [[ ${#orphan_commits[@]} -eq 0 ]] || [[ -z "${orphan_commits[0]:-}" ]]; then
        echo "Detached commits matching active tickets: none"
    else
        echo "Detached commits (not reachable from $base_ref or any bookmark) matching active ticket ids:"
        for entry in "${orphan_commits[@]}"; do
            [[ -z "$entry" ]] && continue
            cid="${entry%%|*}"; rest="${entry#*|}"
            ticket="${rest%%|*}"; msg="${rest#*|}"
            echo "  $cid  (ticket $ticket)  $msg"
        done
        echo ""
        echo "To rescue: 'jj duplicate <hex> -d main' (resolve conflicts in-place), then continue."
    fi
fi
