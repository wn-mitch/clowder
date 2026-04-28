#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""
Per-DSE frame diff between two focal-cat trace files.

Walks both traces, joins records on (tick, cat, layer, primitive_id),
and reports the largest per-DSE `final_score` deltas. When paired with
a `--hypothesis PATH`, overlays predicted drift directions from the
phase's balance doc and classifies each DSE's concordance as
`ok | drift | wrong-direction`.

Gate semantics (per refactor-plan.md "Verification loop"):
  - Phase 2 mode: drift > ε is a **failure** (invariant-preserving L1
    refactor; scent map must be tick-for-tick identical).
  - Phase 3+ mode: drift is a **signal**; the hypothesis overlay
    determines pass/fail per-DSE.

Usage:
    uv run scripts/frame_diff.py BASELINE NEW
    uv run scripts/frame_diff.py BASELINE NEW --hypothesis docs/balance/substrate-phase-3.md
    uv run scripts/frame_diff.py BASELINE NEW --hypothesis PATH --strict

Per §11.4 of docs/systems/ai-substrate-refactor.md.
"""

import argparse
import json
import re
import sys
from collections import defaultdict
from pathlib import Path


def read_trace(path: Path):
    """Parse a JSONL trace file. Returns (header, records).

    Header is None if absent (malformed trace); records skips headers
    and anything without a `layer` field.
    """
    header = None
    records = []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            obj = json.loads(line)
            if obj.get("_header"):
                header = obj
                continue
            if obj.get("layer"):
                records.append(obj)
    return header, records


def headers_match(a, b):
    """§11.4 joinability invariant — traces are comparable only when
    their headers agree on commit_hash, sim_config, constants, and seed.

    Seed is part of the joinability set: same-commit + same-constants +
    different-seed gives two different control worlds, so per-DSE drift
    measured across them is confounded with seed-level variance.
    """
    if a is None or b is None:
        return False, "one or both traces missing header"
    for key in ("commit_hash", "sim_config", "constants", "seed"):
        if a.get(key) != b.get(key):
            return False, f"header field mismatch: {key}"
    return True, None


def aggregate_l2_by_dse(records):
    """For each DSE, collect (tick, cat, final_score) samples and
    return per-DSE summary stats."""
    by_dse = defaultdict(list)
    for r in records:
        if r.get("layer") != "L2":
            continue
        dse = r.get("dse")
        if dse is None:
            continue
        by_dse[dse].append(r.get("final_score", 0.0))
    stats = {}
    for dse, scores in by_dse.items():
        stats[dse] = {
            "count": len(scores),
            "mean": sum(scores) / len(scores) if scores else 0.0,
            "max": max(scores) if scores else 0.0,
            "min": min(scores) if scores else 0.0,
        }
    return stats


def diff_stats(baseline, new):
    """Per-DSE mean/count delta between two aggregate dicts."""
    all_dses = sorted(set(baseline) | set(new))
    rows = []
    for dse in all_dses:
        b = baseline.get(dse, {"count": 0, "mean": 0.0})
        n = new.get(dse, {"count": 0, "mean": 0.0})
        mean_delta = n["mean"] - b["mean"]
        count_delta = n["count"] - b["count"]
        rel_delta = (
            (n["mean"] - b["mean"]) / abs(b["mean"])
            if b["mean"] not in (0.0, None)
            else float("inf") if n["mean"] != 0.0 else 0.0
        )
        rows.append(
            {
                "dse": dse,
                "baseline_mean": b["mean"],
                "new_mean": n["mean"],
                "mean_delta": mean_delta,
                "rel_delta": rel_delta,
                "baseline_count": b["count"],
                "new_count": n["count"],
                "count_delta": count_delta,
            }
        )
    # Rank by absolute mean delta.
    rows.sort(key=lambda r: abs(r["mean_delta"]), reverse=True)
    return rows


def parse_hypothesis_table(path: Path):
    """Parse a phase balance doc's hypothesis table into dict keyed
    by DSE name. Expected rows like:

      | **Mate (L3)** | CompensatedProduct | Logistic(...) | Gate-starvation resolved ...  firing count rises from ~0 to ≥3 ... |
      | **Farming** | CompensatedProduct | Quadratic(2) food-scarcity | First-ever fire. |

    Returns {dse_name: {"direction": "up"|"down"|"flat", "raw": "<prediction text>"}}.
    Direction parsed from keywords in the prediction cell.
    """
    if not path.exists():
        print(f"warning: hypothesis file not found: {path}", file=sys.stderr)
        return {}

    out = {}
    table_re = re.compile(r"^\|\s*\*?\*?([^|*]+?)\*?\*?\s*\|.*\|.*\|\s*(.+?)\s*\|\s*$")
    with open(path) as f:
        for line in f:
            m = table_re.match(line.strip())
            if not m:
                continue
            name_raw, prediction = m.group(1), m.group(2)
            # Strip L3/L2/L1/sub-mode annotations.
            name = re.sub(r"\s*\([^)]*\)\s*", "", name_raw).strip()
            if name.lower() in ("dse", "name", "—"):
                continue
            direction = classify_direction(prediction)
            if direction is None:
                continue
            out[name] = {"direction": direction, "raw": prediction}
    return out


def classify_direction(text: str):
    """Classify a prediction string into up/down/flat/None.

    Conservative: returns None if the text doesn't carry an obvious
    direction keyword. The hypothesis parse is advisory, not definitive.
    """
    t = text.lower()
    up_markers = ("rise", "rises", "up", "increase", "grows", "first-ever fire", "non-zero", "≥")
    down_markers = ("fall", "falls", "decrease", "decline", "down", "retire")
    flat_markers = ("unchanged", "no change", "within noise", "flat")
    if any(m in t for m in flat_markers):
        return "flat"
    if any(m in t for m in up_markers):
        return "up"
    if any(m in t for m in down_markers):
        return "down"
    return None


def concordance(row, prediction):
    """Compare a single DSE row's observed drift against its
    predicted direction.

    Returns (status, note) where status ∈ {ok, drift, wrong-direction,
    untracked}.
    """
    if prediction is None:
        return "untracked", ""
    direction = prediction["direction"]
    delta = row["mean_delta"]
    rel = row["rel_delta"]

    if direction == "flat":
        # Within ±10% is noise per CLAUDE.md.
        if abs(rel) <= 0.10:
            return "ok", f"flat (rel {rel:+.1%} within ±10%)"
        return "drift", f"expected flat; observed rel {rel:+.1%}"

    if direction == "up":
        if delta > 0:
            return "ok", f"rose (Δ mean {delta:+.3f})"
        if abs(delta) <= 0.01:
            return "drift", f"expected up; observed {delta:+.3f}"
        return "wrong-direction", f"expected up; observed {delta:+.3f}"

    if direction == "down":
        if delta < 0:
            return "ok", f"fell (Δ mean {delta:+.3f})"
        if abs(delta) <= 0.01:
            return "drift", f"expected down; observed {delta:+.3f}"
        return "wrong-direction", f"expected down; observed {delta:+.3f}"

    return "untracked", ""


def main():
    p = argparse.ArgumentParser(description=__doc__.strip().split("\n\n")[0])
    p.add_argument("baseline", type=Path, help="Baseline trace sidecar")
    p.add_argument("new", type=Path, help="New trace sidecar")
    p.add_argument(
        "--hypothesis",
        type=Path,
        help="Phase balance doc with a hypothesis table (see refactor-plan.md)",
    )
    p.add_argument(
        "--top",
        type=int,
        default=15,
        help="Show the top-N per-DSE deltas (default 15).",
    )
    p.add_argument(
        "--strict",
        action="store_true",
        help="Exit non-zero on any drift (Phase 2 mode).",
    )
    args = p.parse_args()

    for path in (args.baseline, args.new):
        if not path.exists():
            print(f"error: trace not found: {path}", file=sys.stderr)
            sys.exit(2)

    base_hdr, base_recs = read_trace(args.baseline)
    new_hdr, new_recs = read_trace(args.new)

    ok, reason = headers_match(base_hdr, new_hdr)
    if not ok:
        print(f"header mismatch: {reason}")
        if base_hdr and new_hdr:
            for key in ("commit_hash_short", "seed"):
                print(f"  baseline {key}: {base_hdr.get(key)}")
                print(f"  new      {key}: {new_hdr.get(key)}")
        print("(diff proceeds; results are advisory only)")
    else:
        print(f"headers match — traces are directly comparable")
        print(
            f"commit {base_hdr.get('commit_hash_short', '?')}"
            f"  seed {base_hdr.get('seed', '?')}"
            f"  focal_cat {base_hdr.get('focal_cat', '?')}"
        )

    base_stats = aggregate_l2_by_dse(base_recs)
    new_stats = aggregate_l2_by_dse(new_recs)

    rows = diff_stats(base_stats, new_stats)

    hypotheses = {}
    if args.hypothesis:
        hypotheses = parse_hypothesis_table(args.hypothesis)
        print(f"\nparsed {len(hypotheses)} predictions from {args.hypothesis}")
        for dse, pred in sorted(hypotheses.items()):
            print(f"  {dse}: predicted {pred['direction']}")

    print(f"\ntop-{args.top} per-DSE mean-score deltas:\n")
    header = f"  {'DSE':<20}  {'baseline':>10}  {'new':>10}  {'Δ mean':>10}  {'rel':>8}  {'Δ count':>10}  concordance"
    print(header)
    print("  " + "-" * (len(header) - 2))

    any_drift = False
    any_wrong = False
    for row in rows[: args.top]:
        pred = hypotheses.get(row["dse"])
        status, note = concordance(row, pred)
        if status == "drift":
            any_drift = True
        if status == "wrong-direction":
            any_wrong = True
        rel_str = f"{row['rel_delta']:+.1%}" if row["rel_delta"] != float("inf") else "    new"
        print(
            f"  {row['dse']:<20}  "
            f"{row['baseline_mean']:>+10.3f}  {row['new_mean']:>+10.3f}  "
            f"{row['mean_delta']:>+10.3f}  {rel_str:>8}  "
            f"{row['count_delta']:>+10}  {status}  {note}"
        )

    # Overall concordance summary.
    print()
    if any_wrong:
        print("concordance: wrong-direction drift — investigate before phase exit")
        sys.exit(2)
    if any_drift:
        print("concordance: drift — direction missing/indeterminate on one or more DSEs")
        if args.strict:
            sys.exit(1)
    else:
        print("concordance: ok — no unacknowledged drift on tracked DSEs")


if __name__ == "__main__":
    main()
