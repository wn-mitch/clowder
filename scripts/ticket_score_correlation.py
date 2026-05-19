#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = [
#   "duckdb>=1.0",
#   "pandas>=2.0",
#   "pyyaml>=6.0",
# ]
# ///
"""Correlate ticket clusters / initiatives with seed-42 score boosts.

Method:
  1. Read every landed/<NNN>-*.md frontmatter for (cluster, initiative, landed-at).
  2. Resolve landed-at short SHAs to commit_time via `git show`.
  3. Order all seed-42 footer-complete runs by commit_time.
  4. For each pair of consecutive runs (A, B), compute Δ = score(B) - score(A);
     attribute Δ / N to each ticket whose landed_at commit sits in (A, B].
  5. Aggregate per cluster and per initiative; print ranked tables.

The attribution divides Δ equally among co-landed tickets, which is the
honest move when you can't separate causes within one inter-soak window.
"""
from __future__ import annotations

import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

import duckdb  # type: ignore[import-not-found]
import pandas as pd  # type: ignore[import-not-found]
import yaml  # type: ignore[import-not-found]

REPO = Path(__file__).resolve().parent.parent
LANDED_DIR = REPO / "docs" / "open-work" / "landed"
DB = REPO / "logs" / "runs.duckdb"

FRONTMATTER_RE = re.compile(r"^---\n(.*?)\n---", re.DOTALL)


def parse_frontmatter(path: Path) -> dict | None:
    text = path.read_text(encoding="utf-8", errors="replace")
    m = FRONTMATTER_RE.match(text)
    if not m:
        return None
    try:
        return yaml.safe_load(m.group(1)) or {}
    except yaml.YAMLError:
        return None


def resolve_commit_time(sha: str) -> str | None:
    """Return ISO 8601 commit time for a (possibly short) SHA, or None."""
    try:
        out = subprocess.run(
            ["git", "show", "--no-patch", "--format=%cI", sha],
            cwd=REPO, check=True, capture_output=True, text=True,
        )
        return out.stdout.strip() or None
    except subprocess.CalledProcessError:
        return None


def load_tickets() -> pd.DataFrame:
    rows: list[dict] = []
    skipped_no_sha = 0
    skipped_unresolved = 0
    for p in sorted(LANDED_DIR.glob("*.md")):
        if p.name == "README.md":
            continue
        fm = parse_frontmatter(p)
        if not fm:
            continue
        sha = fm.get("landed-at")
        if not sha or sha == "null" or sha == "pending":
            skipped_no_sha += 1
            continue
        commit_time = resolve_commit_time(str(sha))
        if not commit_time:
            skipped_unresolved += 1
            continue
        initiatives = fm.get("initiative") or []
        if isinstance(initiatives, str):
            initiatives = [initiatives]
        rows.append({
            "ticket_id": fm.get("id"),
            "title": fm.get("title"),
            "cluster": fm.get("cluster") or "(no-cluster)",
            "initiatives": initiatives,
            "landed_at": str(sha),
            "landed_on": fm.get("landed-on"),
            "commit_time": pd.to_datetime(commit_time, utc=True),
            "path": str(p.relative_to(REPO)),
        })
    print(
        f"tickets: {len(rows)} resolved, "
        f"{skipped_no_sha} missing SHA, {skipped_unresolved} unresolved SHA",
        file=sys.stderr,
    )
    return pd.DataFrame(rows).sort_values("commit_time").reset_index(drop=True)


def load_seed42_soaks() -> pd.DataFrame:
    con = duckdb.connect(str(DB), read_only=True)
    df = con.execute("""
        SELECT
            r.commit_hash,
            r.commit_hash_short,
            r.commit_time,
            r.archive,
            f.final_aggregate
        FROM runs r
        JOIN run_footers f USING (run_id)
        WHERE r.seed = 42 AND f.final_aggregate IS NOT NULL
        ORDER BY r.commit_time
    """).fetchdf()
    con.close()
    df["commit_time"] = pd.to_datetime(df["commit_time"], utc=True)
    # Collapse multiple soaks at the same commit by taking the median.
    collapsed = (
        df.groupby(["commit_hash", "commit_time"], as_index=False)
        .agg(final_aggregate=("final_aggregate", "median"),
             n_soaks=("final_aggregate", "size"))
        .sort_values("commit_time")
        .reset_index(drop=True)
    )
    return collapsed


def attribute_deltas(tickets: pd.DataFrame, soaks: pd.DataFrame) -> pd.DataFrame:
    """For each (soak_A, soak_B) pair, divide ΔScore among tickets landed in (A, B]."""
    rows = []
    for i in range(len(soaks) - 1):
        a = soaks.iloc[i]
        b = soaks.iloc[i + 1]
        delta = b["final_aggregate"] - a["final_aggregate"]
        in_range = tickets[
            (tickets["commit_time"] > a["commit_time"])
            & (tickets["commit_time"] <= b["commit_time"])
        ]
        n = len(in_range)
        if n == 0:
            continue
        share = delta / n
        for _, t in in_range.iterrows():
            rows.append({
                "ticket_id": t["ticket_id"],
                "cluster": t["cluster"],
                "initiatives": t["initiatives"],
                "delta_share": share,
                "delta_total_in_window": delta,
                "co_landed_count": n,
                "pre_commit": a["commit_hash_short"] if "commit_hash_short" in a.index
                              else a["commit_hash"][:8],
                "post_commit": b["commit_hash_short"] if "commit_hash_short" in b.index
                              else b["commit_hash"][:8],
                "pre_time": a["commit_time"],
                "post_time": b["commit_time"],
            })
    return pd.DataFrame(rows)


def main() -> int:
    tickets = load_tickets()
    soaks = load_seed42_soaks()
    print(
        f"soaks: {len(soaks)} unique-commit seed-42 soaks "
        f"between {soaks['commit_time'].min()} and {soaks['commit_time'].max()}",
        file=sys.stderr,
    )

    attributed = attribute_deltas(tickets, soaks)
    if attributed.empty:
        print("no overlap between landed tickets and seed-42 soaks", file=sys.stderr)
        return 1

    print(
        f"\nattributed: {len(attributed)} ticket-deltas across "
        f"{attributed[['pre_commit', 'post_commit']].drop_duplicates().shape[0]} "
        f"inter-soak windows",
        file=sys.stderr,
    )

    # --- Per-cluster aggregation
    by_cluster = (
        attributed.groupby("cluster")
        .agg(
            total_delta=("delta_share", "sum"),
            mean_delta=("delta_share", "mean"),
            ticket_count=("ticket_id", "count"),
        )
        .sort_values("total_delta", ascending=False)
        .reset_index()
    )
    print("\n=== by cluster (total attributed Δ aggregate, ranked) ===")
    with pd.option_context(
        "display.max_rows", None, "display.width", 200,
        "display.float_format", "{:+.1f}".format,
    ):
        print(by_cluster.to_string(index=False))

    # --- Per-initiative aggregation (explode multi-tag)
    initiative_rows = []
    for _, row in attributed.iterrows():
        for tag in row["initiatives"] or ["(no-initiative)"]:
            initiative_rows.append({"initiative": tag, "delta_share": row["delta_share"],
                                    "ticket_id": row["ticket_id"]})
    by_init = pd.DataFrame(initiative_rows)
    if not by_init.empty:
        agg = (
            by_init.groupby("initiative")
            .agg(
                total_delta=("delta_share", "sum"),
                mean_delta=("delta_share", "mean"),
                ticket_count=("ticket_id", "count"),
            )
            .sort_values("total_delta", ascending=False)
            .reset_index()
        )
        print("\n=== by initiative (total attributed Δ aggregate, ranked) ===")
        with pd.option_context(
            "display.max_rows", None, "display.width", 200,
            "display.float_format", "{:+.1f}".format,
        ):
            print(agg.to_string(index=False))

    # --- Per-ticket top-10 movers (positive and negative)
    top_pos = attributed.nlargest(10, "delta_share")[
        ["ticket_id", "cluster", "delta_share", "delta_total_in_window",
         "co_landed_count", "post_commit", "post_time"]
    ]
    top_neg = attributed.nsmallest(10, "delta_share")[
        ["ticket_id", "cluster", "delta_share", "delta_total_in_window",
         "co_landed_count", "post_commit", "post_time"]
    ]
    print("\n=== top 10 positive ticket-deltas ===")
    with pd.option_context("display.width", 200, "display.float_format", "{:+.1f}".format):
        print(top_pos.to_string(index=False))
    print("\n=== top 10 negative ticket-deltas ===")
    with pd.option_context("display.width", 200, "display.float_format", "{:+.1f}".format):
        print(top_neg.to_string(index=False))

    return 0


if __name__ == "__main__":
    sys.exit(main())
