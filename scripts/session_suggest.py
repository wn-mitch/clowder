#!/usr/bin/env python3
"""Suggest one or more next-session candidates: {slug, tickets, track, rationale}.

Composes `just open-work-by-track --json` to find ready tickets and proposes
sensible bundles per track:
  - swarm-safe: single-ticket suggestions (atomic by definition)
  - substrate-sensitive: 1-2 tickets from same cluster (high adjacency)
  - coherent-block: single-ticket suggestions, preferring blocks with the
    most remaining ready siblings (so we make progress on a block)

Slug is derived from cluster + ticket id(s), kebab-cased.
Rationale names which signal drove the pairing.

Usage:
    session-suggest                        # default 3 candidates, mixed tracks
    session-suggest --track swarm-safe     # filter to one track
    session-suggest --n 1                  # one candidate
    session-suggest --json                 # machine-readable for /work skill
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from collections import defaultdict


def slugify(s: str) -> str:
    s = s.lower()
    s = re.sub(r"[^a-z0-9]+", "-", s)
    return s.strip("-") or "session"


def gather_ready() -> dict:
    out = subprocess.run(
        ["python3", "scripts/open_work_by_track.py", "--json"],
        capture_output=True, text=True, check=False,
    )
    if out.returncode != 0:
        print(f"ERROR: open-work-by-track failed: {out.stderr}", file=sys.stderr)
        sys.exit(out.returncode or 1)
    return json.loads(out.stdout)


def suggest_swarm_safe(rows: list[dict]) -> list[dict]:
    suggestions = []
    for t in rows:
        slug = f"swarm-{t['id']}-{slugify(t['cluster'])}"[:48]
        suggestions.append({
            "slug": slug,
            "tickets": [t["id"]],
            "track": "swarm-safe",
            "rationale": f"atomic swarm-safe in cluster '{t['cluster']}'",
        })
    return suggestions


def suggest_substrate_sensitive(rows: list[dict]) -> list[dict]:
    by_cluster: dict[str, list[dict]] = defaultdict(list)
    for t in rows:
        by_cluster[t["cluster"]].append(t)

    suggestions = []
    seen: set[str] = set()
    # First pass: pair adjacent tickets from same cluster
    for cluster, ts in sorted(by_cluster.items(), key=lambda kv: -len(kv[1])):
        for i in range(0, len(ts) - 1, 2):
            a, b = ts[i], ts[i + 1]
            if a["id"] in seen or b["id"] in seen:
                continue
            seen.add(a["id"])
            seen.add(b["id"])
            slug = f"sub-{a['id']}-{b['id']}-{slugify(cluster)}"[:48]
            suggestions.append({
                "slug": slug,
                "tickets": [a["id"], b["id"]],
                "track": "substrate-sensitive",
                "rationale": f"adjacent pair in cluster '{cluster}' ({len(ts)} ready)",
            })
    # Second pass: lone-ticket clusters
    for t in rows:
        if t["id"] in seen:
            continue
        slug = f"sub-{t['id']}-{slugify(t['cluster'])}"[:48]
        suggestions.append({
            "slug": slug,
            "tickets": [t["id"]],
            "track": "substrate-sensitive",
            "rationale": f"sole ready ticket in cluster '{t['cluster']}'",
        })
    return suggestions


def suggest_coherent_block(blocks: dict[str, list[dict]]) -> list[dict]:
    suggestions = []
    # Prefer blocks with most remaining ready siblings
    for block, rows in sorted(blocks.items(), key=lambda kv: -len(kv[1])):
        for t in rows:
            slug = f"block-{block}-{t['id']}"[:48]
            suggestions.append({
                "slug": slug,
                "tickets": [t["id"]],
                "track": "coherent-block",
                "block": block,
                "rationale": f"block '{block}' has {len(rows)} ready leg(s)",
            })
    return suggestions


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--track", choices=("substrate-sensitive", "coherent-block", "swarm-safe"))
    parser.add_argument("--n", type=int, default=3, help="max candidates (default 3)")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()

    ready = gather_ready()

    pool: list[dict] = []
    if args.track in (None, "swarm-safe"):
        pool.extend(suggest_swarm_safe(ready.get("swarm-safe", [])))
    if args.track in (None, "substrate-sensitive"):
        pool.extend(suggest_substrate_sensitive(ready.get("substrate-sensitive", [])))
    if args.track in (None, "coherent-block"):
        pool.extend(suggest_coherent_block(ready.get("coherent-block", {})))

    candidates = pool[: args.n]

    if args.json:
        print(json.dumps(candidates, indent=2))
        return 0

    if not candidates:
        print("session-suggest: no candidates (queue empty for the requested track)")
        return 0

    for c in candidates:
        print(f"=== {c['slug']} ({c['track']}) ===")
        print(f"  tickets:   {','.join(c['tickets'])}")
        if "block" in c:
            print(f"  block:     {c['block']}")
        print(f"  rationale: {c['rationale']}")
        cmd = (
            f"  command:   just session-new {c['slug']} "
            f"--tickets {','.join(c['tickets'])} --track {c['track']} --print-prompt"
        )
        print(cmd)
        print()
    return 0


if __name__ == "__main__":
    sys.exit(main())
