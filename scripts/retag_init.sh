#!/usr/bin/env bash
# Backfills `orchestration: substrate-sensitive` (the safe default) on
# every active ticket missing the field. Idempotent — re-runs skip
# already-tagged tickets.
#
# Inserts the new line immediately after `cluster:` in the frontmatter.
# If a ticket lacks `cluster:` (shouldn't happen post-clusters.md), it's
# skipped with a warning rather than getting malformed frontmatter.
#
# This is Stage 0 step 4 of the orchestration rollout (ticket 354):
# bring the corpus into compliance with the four invariants enforced
# by scripts/check_orchestration_frontmatter.sh.

set -euo pipefail

TICKETS_DIR="docs/open-work/tickets"
tagged=0
skipped=0
missing_cluster=0

for f in "$TICKETS_DIR"/*.md; do
    base=$(basename "$f")
    [[ "$base" == _* ]] && continue

    if grep -q '^orchestration:' "$f"; then
        skipped=$((skipped + 1))
        continue
    fi

    if ! grep -q '^cluster:' "$f"; then
        echo "WARN: $f: no cluster: line, skipping"
        missing_cluster=$((missing_cluster + 1))
        continue
    fi

    awk '
        /^cluster:/ && !ins { print; print "orchestration: substrate-sensitive"; ins=1; next }
        { print }
    ' "$f" > "$f.tmp" && mv "$f.tmp" "$f"

    tagged=$((tagged + 1))
done

echo "retag-init: tagged=$tagged skipped=$skipped missing_cluster=$missing_cluster"
