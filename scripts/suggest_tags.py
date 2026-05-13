#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = [
#     "numpy>=1.26",
#     "pyyaml>=6.0",
# ]
# ///
"""
Suggest cluster + initiative tags for untagged tickets via embedding-neighbor voting.

For each ticket missing `cluster:` (or with `initiative: []`), this script:
  1. Loads the local embedding index (`logs/.embeddings/`).
  2. Builds a per-ticket centroid by averaging that ticket's chunk vectors.
  3. Finds the top-K nearest neighbor *tickets* (not chunks) via cosine sim.
  4. Tallies neighbor votes:
     - cluster: plurality vote (excluding null / "—"); confidence = top fraction
     - initiative: per-initiative independent yes/no — held by ≥ density_threshold
       fraction of neighbors → proposed
  5. Emits proposals to a YAML file the user can `approve: true` and apply via
     `scripts/apply_tags.py`.

Usage:
    uv run scripts/suggest_tags.py active   # over docs/open-work/tickets/
    uv run scripts/suggest_tags.py landed   # over docs/open-work/landed/

Output:
    logs/tag-proposals/{active,landed}.yaml — every proposal, with a
    `confidence` band and a `auto_apply: true|false` hint based on the
    high-confidence heuristic.

Workflow:
    1. Run this script (active or landed).
    2. Edit the proposals YAML — flip `approve: true` on entries you accept,
       `approve: false` on rejects. Auto-applied entries default to true.
    3. `uv run scripts/apply_tags.py logs/tag-proposals/active.yaml`
    4. Rebuild embedding index (auto on next `just similar` invocation).
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter, defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np

REPO_ROOT = Path(__file__).resolve().parent.parent
INDEX_NPZ = REPO_ROOT / "logs" / ".embeddings" / "index.npz"
INDEX_META = REPO_ROOT / "logs" / ".embeddings" / "index.meta.json"

# Confidence thresholds for auto-approve. Tunable via CLI.
DEFAULT_CLUSTER_CONFIDENCE = 0.55
DEFAULT_INITIATIVE_DENSITY = 0.40
DEFAULT_TOP_K = 12


@dataclass
class TicketEmbed:
    ticket_id: str           # zero-padded 3-digit
    source_kind: str         # tickets | landed
    cluster: str | None      # may be "—" if untagged
    initiative: list[str]    # may be empty
    title: str
    status: str
    path: str                # repo-relative
    centroid: np.ndarray     # (dim,) float32, L2-normalized


def _normalize_id(raw: Any) -> str:
    """Pad numeric id to 3 digits; preserve letter suffix (027b)."""
    s = str(raw)
    if s.isdigit():
        return f"{int(s):03d}"
    if len(s) > 1 and s[:-1].isdigit() and s[-1].isalpha():
        return f"{int(s[:-1]):03d}{s[-1]}"
    return s


def _l2_normalize(v: np.ndarray) -> np.ndarray:
    n = np.linalg.norm(v)
    return v / n if n > 0 else v


def load_ticket_embeddings() -> list[TicketEmbed]:
    """Build per-ticket centroids from the chunk-level index.

    Centroid = mean of that ticket's chunk vectors, then re-normalized so
    cosine similarity stays in [-1, 1] under dot product.
    """
    if not INDEX_NPZ.exists() or not INDEX_META.exists():
        raise SystemExit(
            f"embedding index not found at {INDEX_NPZ.parent} — "
            "run `just similar-build` first"
        )
    vectors = np.load(INDEX_NPZ, allow_pickle=True)["vectors"].astype(np.float32)
    meta = json.loads(INDEX_META.read_text(encoding="utf-8"))
    chunks = meta["chunks"]
    assert vectors.shape[0] == len(chunks), "vector / chunk count mismatch"

    by_ticket: dict[tuple[str, str], list[int]] = defaultdict(list)
    chunk_meta: dict[tuple[str, str], dict[str, Any]] = {}
    for i, c in enumerate(chunks):
        kind = c.get("source_kind")
        if kind not in ("tickets", "landed"):
            continue
        md = c.get("metadata", {})
        tid = _normalize_id(md.get("ticket_id"))
        key = (kind, tid)
        by_ticket[key].append(i)
        if key not in chunk_meta:
            chunk_meta[key] = {
                "cluster": md.get("cluster"),
                "initiative": md.get("initiative") or [],
                "title": md.get("title", ""),
                "status": md.get("status", ""),
                "path": c.get("source_path", ""),
            }

    embeds: list[TicketEmbed] = []
    for (kind, tid), row_ixs in by_ticket.items():
        centroid = _l2_normalize(vectors[row_ixs].mean(axis=0))
        md = chunk_meta[(kind, tid)]
        embeds.append(
            TicketEmbed(
                ticket_id=tid,
                source_kind=kind,
                cluster=md["cluster"],
                initiative=md["initiative"],
                title=md["title"],
                status=md["status"],
                path=md["path"],
                centroid=centroid,
            )
        )
    return embeds


def is_untagged_cluster(emb: TicketEmbed) -> bool:
    """A ticket needs a cluster proposal if its current value is null or
    the chunk-header sentinel '—' (which is what the chunker emits when
    `meta.get('cluster')` is None or missing)."""
    return emb.cluster is None or emb.cluster == "—" or emb.cluster == ""


def find_neighbors(
    seed: TicketEmbed,
    pool: list[TicketEmbed],
    top_k: int,
) -> list[tuple[TicketEmbed, float]]:
    """Return top-K most cosine-similar tickets from pool, excluding seed."""
    others = [e for e in pool if e.ticket_id != seed.ticket_id or e.source_kind != seed.source_kind]
    sims = np.array([float(seed.centroid @ o.centroid) for o in others])
    top_idx = np.argsort(-sims)[:top_k]
    return [(others[i], float(sims[i])) for i in top_idx]


def propose_cluster(
    neighbors: list[tuple[TicketEmbed, float]],
    min_confidence: float,
) -> tuple[str | None, float, list[tuple[str, int]]]:
    """Plurality vote on cluster, excluding untagged neighbors.

    Returns (proposed_cluster, confidence, ranked_candidates).
    confidence = top_cluster_count / total_voting_neighbors.
    """
    tagged_clusters = [
        n.cluster for n, _ in neighbors
        if n.cluster and n.cluster not in ("—", "")
    ]
    if not tagged_clusters:
        return None, 0.0, []
    counter = Counter(tagged_clusters)
    ranked = counter.most_common()
    top_cluster, top_count = ranked[0]
    confidence = top_count / len(tagged_clusters)
    if confidence < min_confidence:
        return None, confidence, ranked
    return top_cluster, confidence, ranked


def propose_initiatives(
    neighbors: list[tuple[TicketEmbed, float]],
    density_threshold: float,
) -> tuple[list[str], dict[str, float]]:
    """Per-initiative independent yes/no based on neighbor density.

    For each initiative tag present in any neighbor, compute the fraction of
    neighbors that carry it. Propose any tag above density_threshold.
    """
    n_neighbors = len(neighbors)
    if n_neighbors == 0:
        return [], {}
    counts: Counter[str] = Counter()
    for n, _ in neighbors:
        for init in n.initiative:
            counts[init] += 1
    densities = {init: c / n_neighbors for init, c in counts.items()}
    proposed = sorted(
        [init for init, d in densities.items() if d >= density_threshold],
        key=lambda i: -densities[i],
    )
    return proposed, densities


def suggest_for_ticket(
    seed: TicketEmbed,
    pool: list[TicketEmbed],
    top_k: int,
    cluster_min: float,
    initiative_min: float,
) -> dict[str, Any]:
    """Build a YAML-shaped proposal dict for one ticket."""
    needs_cluster = is_untagged_cluster(seed)
    needs_initiative = not seed.initiative

    if not needs_cluster and not needs_initiative:
        return {}  # caller skips

    neighbors = find_neighbors(seed, pool, top_k)

    proposal: dict[str, Any] = {
        "ticket_id": seed.ticket_id,
        "title": seed.title,
        "status": seed.status,
        "path": seed.path,
        "current_cluster": seed.cluster,
        "current_initiative": seed.initiative,
    }

    auto = True

    if needs_cluster:
        c, conf, ranked = propose_cluster(neighbors, cluster_min)
        proposal["proposed_cluster"] = c
        proposal["cluster_confidence"] = round(conf, 3)
        proposal["cluster_candidates"] = [
            {"cluster": cc, "votes": vv} for cc, vv in ranked[:5]
        ]
        if c is None:
            auto = False  # no high-confidence cluster proposal

    if needs_initiative:
        inits, densities = propose_initiatives(neighbors, initiative_min)
        proposal["proposed_initiatives"] = inits
        proposal["initiative_densities"] = {
            k: round(v, 3) for k, v in sorted(densities.items(), key=lambda kv: -kv[1])[:6]
        }
        # initiative is optional — empty proposed list is fine and stays auto-approvable

    proposal["top_neighbors"] = [
        {
            "id": n.ticket_id,
            "kind": n.source_kind,
            "score": round(s, 3),
            "cluster": n.cluster,
            "initiative": n.initiative,
            "title": n.title[:80],
        }
        for n, s in neighbors[:5]
    ]
    proposal["auto_apply"] = auto
    proposal["approve"] = auto  # default: high-confidence → auto-approved
    return proposal


def _format_yaml(proposals: list[dict[str, Any]]) -> str:
    """Render proposals as YAML. Vendored emitter — keeps the dep surface
    small (the existing `create_ticket.py` doesn't pull pyyaml either)."""
    import yaml
    return yaml.safe_dump(
        {"proposals": proposals},
        sort_keys=False,
        allow_unicode=True,
        default_flow_style=False,
    )


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("scope", choices=["active", "landed", "both"],
                    help="Which corpus to tag-suggest over")
    ap.add_argument("--top-k", type=int, default=DEFAULT_TOP_K)
    ap.add_argument("--cluster-confidence", type=float, default=DEFAULT_CLUSTER_CONFIDENCE,
                    help="Min plurality fraction to auto-approve a cluster proposal")
    ap.add_argument("--initiative-density", type=float, default=DEFAULT_INITIATIVE_DENSITY,
                    help="Min neighbor-density to propose an initiative tag")
    ap.add_argument("--out-dir", type=Path, default=REPO_ROOT / "logs" / "tag-proposals")
    args = ap.parse_args(argv)

    embeds = load_ticket_embeddings()
    print(f"loaded {len(embeds)} ticket-level embeddings", file=sys.stderr)

    pool = embeds  # consider both active + landed as voting neighbors
    out_dir = args.out_dir
    out_dir.mkdir(parents=True, exist_ok=True)

    scopes = ["tickets", "landed"] if args.scope == "both" else (["tickets"] if args.scope == "active" else ["landed"])

    for scope_kind in scopes:
        seeds = [e for e in embeds if e.source_kind == scope_kind]
        candidates = [s for s in seeds if is_untagged_cluster(s) or not s.initiative]
        print(
            f"  scope={scope_kind}: {len(candidates)}/{len(seeds)} candidates (untagged cluster or initiative)",
            file=sys.stderr,
        )
        proposals: list[dict[str, Any]] = []
        for seed in sorted(candidates, key=lambda s: s.ticket_id):
            p = suggest_for_ticket(
                seed, pool, args.top_k,
                args.cluster_confidence, args.initiative_density,
            )
            if p:
                proposals.append(p)

        label = "active" if scope_kind == "tickets" else "landed"
        out_path = out_dir / f"{label}.yaml"
        out_path.write_text(_format_yaml(proposals), encoding="utf-8")

        auto_n = sum(1 for p in proposals if p.get("auto_apply"))
        manual_n = len(proposals) - auto_n
        cluster_proposed = sum(1 for p in proposals if p.get("proposed_cluster"))
        init_proposed = sum(1 for p in proposals if p.get("proposed_initiatives"))
        print(
            f"  wrote {out_path.relative_to(REPO_ROOT)}: "
            f"{len(proposals)} proposals "
            f"({auto_n} auto-approve, {manual_n} need review) — "
            f"{cluster_proposed} with cluster, {init_proposed} with initiatives",
            file=sys.stderr,
        )

    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
