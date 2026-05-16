#!/usr/bin/env bash
# Run `just verdict` against the soak log of a coherent-block's anchor session.
#
# Composes `just block-info <block> --json` (to find the anchor ticket),
# `just ticket-info <anchor-id> --json` (to find the holding session), and
# `just verdict <log-dir>` (to do the actual gate).
#
# Refuses if the block has no anchor, the anchor is not held by any session,
# or the holding session has not yet produced a soak log directory.
#
# Usage:
#   block-verdict.sh <block-id>            # uses logs/tuned-42 by default
#   block-verdict.sh <block-id> <seed>     # uses logs/tuned-<seed>
#
# Exit codes:
#   0  verdict ran (its own exit code is preserved)
#   2  pre-flight failed (no anchor, no session, no soak log)

set -euo pipefail

REPO_ROOT=$(cd "$(dirname "$0")/.." && pwd)
SESSIONS_ROOT="$HOME/clowder-sessions"

block="${1:-}"
seed="${2:-42}"

if [[ -z "$block" ]]; then
    echo "Usage: block-verdict <block-id> [seed]" >&2
    exit 2
fi

cd "$REPO_ROOT"

block_json=$(just block-info "$block" --json 2>/dev/null) || {
    echo "ERROR: block-info failed for '$block' — does the block exist?" >&2
    exit 2
}

anchor_id=$(echo "$block_json" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('anchor') or '')")

if [[ -z "$anchor_id" ]]; then
    echo "ERROR: block '$block' has no verdict-anchor set." >&2
    echo "  Set one with: just block-anchor $block <ticket-id>" >&2
    exit 2
fi

ticket_json=$(just ticket-info "$anchor_id" --json 2>/dev/null) || {
    echo "ERROR: ticket-info failed for anchor $anchor_id" >&2
    exit 2
}

holding_session=$(echo "$ticket_json" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('holding_session') or '')")

if [[ -z "$holding_session" ]]; then
    echo "ERROR: anchor ticket $anchor_id is not held by any active session." >&2
    echo "  Start one with: just session-new <slug> --tickets $anchor_id --track coherent-block" >&2
    exit 2
fi

session_dir="$SESSIONS_ROOT/$holding_session"
log_dir="$session_dir/logs/tuned-$seed"

if [[ ! -d "$log_dir" ]]; then
    echo "ERROR: no soak log at $log_dir" >&2
    echo "  Run a soak in the session first: cd $session_dir && just soak-trace $seed <focal-cat>" >&2
    exit 2
fi

echo "block-verdict: block=$block anchor=$anchor_id session=$holding_session log=$log_dir"
cd "$session_dir"
exec just verdict "logs/tuned-$seed"
