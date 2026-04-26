#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = ["scipy>=1.10"]
# ///
"""
Compute Spearman rho between SimConstants leaves and footer metrics.

Backing tool for `scripts/build_sensitivity_map.sh`. Reads the per-knob
sweep bundle written by that script (`logs/sensitivity-build/<knob>-(up|down)/<seed>-events.jsonl`)
and emits `logs/sensitivity-map.json` keyed by the dotted constant path.

Output schema:
    {
      "<dotted.path>": [
        {"metric": "deaths_by_cause.Starvation", "rho": -0.84, "n": 6},
        ...
      ]
    }

Each rho is computed across the 6 runs (3 seeds × 2 perturbation levels)
for that knob. Top 5 metrics by |rho| per knob.

Usage (typically invoked from build_sensitivity_map.sh):
    uv run scripts/build_sensitivity_map.py logs/sensitivity-build \
        --output logs/sensitivity-map.json
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

from scipy import stats  # type: ignore[import-not-found]


REPO_ROOT = Path(__file__).resolve().parent.parent

# Reuse footer-flatten + reader from sweep_stats.
sys.path.insert(0, str(REPO_ROOT / "scripts"))
import sweep_stats  # noqa: E402


def discover_knob_dirs(work: Path) -> dict[str, list[Path]]:
    """Group `<knob>-up` / `<knob>-down` directories by reconstructed dotted path."""
    out: dict[str, list[Path]] = {}
    for child in sorted(work.iterdir()):
        if not child.is_dir():
            continue
        m = re.match(r"^(.+)-(up|down)$", child.name)
        if not m:
            continue
        # Reconstruct dotted path from `<knob_with_underscores>` — the
        # build script substituted `.` → `_` once. Heuristic recovery is
        # ambiguous for fields whose names contain `_`. A future variant
        # could record the exact path in a sidecar file alongside each
        # bundle; for now we accept the ambiguity and store the underscore
        # form, leaving the user/explain_constant.py to disambiguate.
        knob = m.group(1).replace("__", ".")
        out.setdefault(knob, []).append(child)
    return out


def collect_runs(dirs: list[Path]) -> list[dict[str, float]]:
    """Read every per-seed events.jsonl footer in the given directories."""
    rows: list[dict[str, float]] = []
    for d in dirs:
        for p in sorted(d.glob("*-events.jsonl")):
            footer = sweep_stats.read_footer(p)
            if footer:
                rows.append(sweep_stats.flatten(footer))
    return rows


def spearman_for_knob(rows: list[dict[str, float]], knob_values: list[float]) -> list[dict[str, Any]]:
    """For each metric across rows, compute Spearman rho vs knob_values."""
    if len(rows) < 3:
        return []
    metrics: set[str] = set()
    for r in rows:
        metrics.update(r.keys())
    out: list[dict[str, Any]] = []
    for m in sorted(metrics):
        vals = [r.get(m, 0.0) for r in rows]
        if len(set(vals)) < 2:
            continue  # constant column — no signal
        try:
            rho, p = stats.spearmanr(knob_values, vals)
        except Exception:
            continue
        if rho is None or (isinstance(rho, float) and (rho != rho)):
            continue
        out.append({"metric": m, "rho": round(float(rho), 3),
                    "p": round(float(p), 4) if p == p else None,
                    "n": len(vals)})
    out.sort(key=lambda r: -abs(r["rho"]))
    return out[:5]


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("work_dir", help="logs/sensitivity-build/")
    ap.add_argument("--output", default="logs/sensitivity-map.json")
    args = ap.parse_args(argv)

    work = Path(args.work_dir)
    if not work.is_dir():
        sys.stderr.write(f"build_sensitivity_map: no such dir: {work}\n")
        return 2

    knob_dirs = discover_knob_dirs(work)
    out: dict[str, list[dict[str, Any]]] = {}

    for knob, dirs in knob_dirs.items():
        # `up` runs map to perturb=+1, `down` to -1; this is a coarse
        # ordinal code that's enough for Spearman.
        knob_values: list[float] = []
        rows: list[dict[str, float]] = []
        for d in dirs:
            sign = 1.0 if d.name.endswith("-up") else -1.0
            seeds_in_dir = sorted(d.glob("*-events.jsonl"))
            for p in seeds_in_dir:
                footer = sweep_stats.read_footer(p)
                if footer:
                    rows.append(sweep_stats.flatten(footer))
                    knob_values.append(sign)
        rho_rows = spearman_for_knob(rows, knob_values)
        if rho_rows:
            out[knob] = rho_rows

    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(out, indent=2))
    sys.stdout.write(f"build_sensitivity_map: wrote {output} ({len(out)} knobs)\n")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
