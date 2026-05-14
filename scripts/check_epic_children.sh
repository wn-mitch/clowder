#!/usr/bin/env bash
# Enforces the epic-dashboard anti-staleness rule (ticket 318).
#
# For every epic dashboard listed in EPICS, runs
# `uv run scripts/epic_children.py <id> --quiet` and propagates non-zero
# exit codes. Exit 1 ⇒ at least one roster row diverges from its child
# ticket's frontmatter; run `just epic-children <id>` for the per-row
# breakdown, or `just epic-children <id> --fix` to auto-rewrite the
# mechanical drift kinds (status-mismatch, blocker-mismatch,
# landed-but-marked-active, landed-but-sha-stale).
#
# Wired into `just check`; expand EPICS as additional epic dashboards
# adopt the same roster-table shape.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

EPICS=("060")

failed=()
for id in "${EPICS[@]}"; do
    if ! uv run scripts/epic_children.py "$id" --quiet; then
        echo "[FAIL] epic-$id roster drift; run \`just epic-children $id\` for details" >&2
        failed+=("$id")
    fi
done

if [ "${#failed[@]}" -ne 0 ]; then
    exit 1
fi

echo "epic-children: ${#EPICS[@]} dashboard(s) consistent — ${EPICS[*]}"
exit 0
