#!/usr/bin/env bash
# Create an isolated parallel-session workspace + bookmark + atomic
# ticket claim. Stage 1.3 of ticket 354.
#
# What this does, in order:
#   1. Validate the slug (bookmark-safe characters only)
#   2. flock(docs/open-work/.claim-lock) — serialize claim races
#   3. Verify no requested ticket is already status: in-progress
#   4. Create ~/clowder-sessions/<slug>/ as a jj workspace at main
#   5. Set bookmark session/<slug> at main's head
#   6. Write status: in-progress on each --ticket, regenerate the index
#   7. Write .session-info.json into the new workspace
#   8. (--print-prompt) emit a starter prompt for a new Claude session
#
# Workspace location is fixed: ~/clowder-sessions/<slug>/.
# Bookmark is fixed: session/<slug>.
#
# Usage:
#   session-new.sh <slug> [--tickets <id1,id2>] [--track <name>]
#                  [--initiative <name>] [--pick] [--print-prompt]
#
# --pick:         auto-select one ready ticket from --track queue
#                 (mutually exclusive with --tickets)
# --print-prompt: emit the starter prompt to stdout

set -euo pipefail

REPO_ROOT=$(cd "$(dirname "$0")/.." && pwd)
SESSIONS_ROOT="$HOME/clowder-sessions"
CLAIM_LOCK="$REPO_ROOT/docs/open-work/.claim-lock"

slug=""
tickets=""
track="substrate-sensitive"
initiative=""
do_pick="false"
print_prompt="false"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --tickets) tickets="$2"; shift 2 ;;
        --track) track="$2"; shift 2 ;;
        --initiative) initiative="$2"; shift 2 ;;
        --pick) do_pick="true"; shift ;;
        --print-prompt) print_prompt="true"; shift ;;
        -h|--help) sed -n '/^# Usage:/,/^$/p' "$0" | sed 's/^# \?//'; exit 0 ;;
        --*) echo "Unknown flag: $1" >&2; exit 2 ;;
        *) slug="$1"; shift ;;
    esac
done

[[ -z "$slug" ]] && { echo "ERROR: missing <slug>" >&2; exit 2; }
[[ "$slug" =~ ^[a-z0-9][a-z0-9-]*$ ]] || {
    echo "ERROR: slug must be [a-z0-9][a-z0-9-]+ (got '$slug')" >&2; exit 2; }

case "$track" in
    substrate-sensitive|coherent-block|swarm-safe) ;;
    *) echo "ERROR: --track must be one of substrate-sensitive|coherent-block|swarm-safe (got '$track')" >&2; exit 2 ;;
esac

workspace="$SESSIONS_ROOT/$slug"
[[ -e "$workspace" ]] && { echo "ERROR: $workspace already exists" >&2; exit 1; }

mkdir -p "$SESSIONS_ROOT"
touch "$CLAIM_LOCK"

# --pick: pull one ready ticket from the requested track
if [[ "$do_pick" == "true" ]]; then
    [[ -n "$tickets" ]] && { echo "ERROR: --pick and --tickets are mutually exclusive" >&2; exit 2; }
    picked=$(python3 - "$track" <<'PY'
import sys, re
from pathlib import Path
track = sys.argv[1]
tickets_dir = Path("docs/open-work/tickets")
for p in sorted(tickets_dir.glob("*.md")):
    if p.name.startswith("_"):
        continue
    fm = {}
    in_fm = False; seen = False
    with p.open() as f:
        for line in f:
            line = line.rstrip("\n")
            if line.strip() == "---":
                if not seen: seen = True; in_fm = True; continue
                break
            if in_fm:
                m = re.match(r"^([A-Za-z][\w-]*):\s*(.*)$", line)
                if m: fm[m.group(1)] = m.group(2).strip()
    if fm.get("status") != "ready":
        continue
    if fm.get("orchestration", "").strip() != track:
        continue
    print(fm.get("id", ""))
    break
PY
)
    [[ -z "$picked" ]] && { echo "ERROR: no ready ticket on --track $track" >&2; exit 1; }
    tickets="$picked"
    echo "session-new: --pick selected ticket $picked"
fi

# Atomic claim under flock
claim_under_lock() {
    if [[ -n "$tickets" ]]; then
        IFS=',' read -ra ticket_ids <<< "$tickets"
        for tid in "${ticket_ids[@]}"; do
            tid_trimmed=$(echo "$tid" | tr -d ' ')
            padded=$(printf "%03d" "$tid_trimmed" 2>/dev/null || echo "$tid_trimmed")
            tfile=$(ls "$REPO_ROOT/docs/open-work/tickets/${padded}-"*.md 2>/dev/null | head -1)
            [[ -z "$tfile" ]] && tfile=$(ls "$REPO_ROOT/docs/open-work/tickets/${tid_trimmed}-"*.md 2>/dev/null | head -1)
            if [[ -z "$tfile" ]]; then
                echo "ERROR: no ticket file for id '$tid_trimmed'" >&2
                exit 1
            fi
            current_status=$(awk -F': *' '/^status:/ { print $2; exit }' "$tfile" | tr -d ' ')
            if [[ "$current_status" == "in-progress" ]]; then
                echo "ERROR: ticket $tid_trimmed is already in-progress (refusing to claim)" >&2
                exit 1
            fi
        done
        # All clear — write the claim
        for tid in "${ticket_ids[@]}"; do
            tid_trimmed=$(echo "$tid" | tr -d ' ')
            padded=$(printf "%03d" "$tid_trimmed" 2>/dev/null || echo "$tid_trimmed")
            tfile=$(ls "$REPO_ROOT/docs/open-work/tickets/${padded}-"*.md 2>/dev/null | head -1)
            [[ -z "$tfile" ]] && tfile=$(ls "$REPO_ROOT/docs/open-work/tickets/${tid_trimmed}-"*.md 2>/dev/null | head -1)
            awk '/^status:/ && !done { print "status: in-progress"; done=1; next } { print }' "$tfile" > "$tfile.tmp" && mv "$tfile.tmp" "$tfile"
        done
    fi
}

if command -v flock >/dev/null 2>&1; then
    (
        flock -x 9
        claim_under_lock
    ) 9>"$CLAIM_LOCK"
else
    # macOS doesn't ship flock by default; fall back to lockfile-style guard
    # via a sentinel + retry. Best-effort serialization rather than strict.
    sentinel="$CLAIM_LOCK.pid"
    for _ in $(seq 1 30); do
        if (set -C; echo $$ > "$sentinel") 2>/dev/null; then
            trap 'rm -f "$sentinel"' EXIT
            claim_under_lock
            rm -f "$sentinel"; trap - EXIT
            break
        fi
        sleep 0.2
    done
fi

# Workspace creation (after claim succeeds)
cd "$REPO_ROOT"
jj workspace add "$workspace" --name "$slug" >/dev/null 2>&1 || {
    echo "ERROR: jj workspace add failed" >&2; exit 1; }

# Create the session/<slug> bookmark at main's head (so refinery rebases off main)
cd "$workspace"
jj bookmark create "session/$slug" -r 'main' 2>/dev/null || true
jj edit "session/$slug" >/dev/null 2>&1 || true

# Write .session-info.json
created_at=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
cat > "$workspace/.session-info.json" <<JSON
{
  "slug": "$slug",
  "bookmark": "session/$slug",
  "track": "$track",
  "tickets": [$(echo "$tickets" | tr ',' '\n' | awk 'NF { gsub(/ /, ""); printf "%s\"%s\"", (NR>1?", ":""), $0 }')],
  "initiative": "$initiative",
  "created_at": "$created_at",
  "claimed_by_pid": $$
}
JSON

# Regenerate the open-work index since ticket statuses changed
cd "$REPO_ROOT"
just open-work-index >/dev/null 2>&1 || true

echo "session-new: $workspace ← bookmark=session/$slug track=$track tickets=${tickets:-_}"

if [[ "$print_prompt" == "true" ]]; then
    cat <<PROMPT

═══════════════════════════════════════════════════════════════════════════════
STARTER PROMPT — copy/paste into a fresh Claude session in $workspace
═══════════════════════════════════════════════════════════════════════════════

You are working on ticket(s) $tickets, $track track.
Workspace: $workspace
Bookmark: session/$slug (push here, never main)

Convention reminders:
  $(case "$track" in
    substrate-sensitive) echo "- Layer-walk required before listing fixes (CLAUDE.md \"Bugfix discipline\")"
                          echo "- Structural-option menu required (split / extend / rebind / retire)"
                          echo "- Promote [suspect] rows to [verified-*] via fresh queries" ;;
    coherent-block)       echo "- Block-level verdict — intermediates land verdict-skipped"
                          echo "- Orthogonality precondition holds (verify before declaring done)" ;;
    swarm-safe)           echo "- Atomic / mechanical work; sweep-land may auto"
                          echo "- Stick to scope; don't drift into substrate-sensitive territory" ;;
  esac)

Exit ceremony:
  /handoff
  jj git push --bookmark session/$slug --allow-new

The master session at ~/clowder will run /work to sweep-land your bookmark.
═══════════════════════════════════════════════════════════════════════════════
PROMPT
fi
