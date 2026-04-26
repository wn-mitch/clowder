#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""
Per-metric "is this run in band?" verdict against the healthy-colony
reference table.

Reads `docs/balance/healthy-colony.md`, parses the metric tables (each
`| field | expected | what it tells you |` row), and evaluates the
target run's `_footer` against the bands. Outputs a structured JSON
envelope so an agent can see at-a-glance which metrics drifted.

Bands are parsed from the doc's `mean ± stdev` notation; the doc is the
source of truth so refreshing it (after a substrate change) auto-updates
this tool. Continuity tallies use `≥ N` form. Survival caps come from
the same canary thresholds `just check-canaries` enforces (Starvation
== 0 hard, ShadowFoxAmbush ≤ 10).

Usage:
    just fingerprint logs/tuned-42
    just fingerprint logs/tuned-42 --text
    just fingerprint logs/tuned-42 --doc docs/balance/healthy-colony.md

Exit codes: 0 all in band, 1 concerns, 2 failures.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from dataclasses import dataclass, field, asdict
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parent.parent
HEALTHY_COLONY = REPO_ROOT / "docs" / "balance" / "healthy-colony.md"

# Bands tighter than ±2σ get bumped to a min absolute width so a small-stdev
# metric isn't flagged for trivial drift. 0.5 in the field's natural unit.
MIN_BAND_WIDTH = 0.5


@dataclass
class Band:
    field: str
    mean: float | None = None
    stdev: float | None = None
    floor: float | None = None    # for ≥ N continuity tallies
    cap: float | None = None      # for ≤ N survival canaries
    hard_zero: bool = False       # for `cap: 0 hard` (Starvation)


@dataclass
class FingerprintRow:
    field: str
    observed: float | None
    band: Band
    verdict: str   # in-range | low | high | below-floor | above-cap | failure | unknown
    note: str = ""


@dataclass
class Fingerprint:
    run: str
    overall: str   # pass | concern | fail
    rows: list[dict[str, Any]] = field(default_factory=list)
    next_steps: list[str] = field(default_factory=list)
    doc_source: str = str(HEALTHY_COLONY)


# ── parsing healthy-colony.md ──────────────────────────────────────────

_FIELD_PATTERN = re.compile(r"`([a-zA-Z_][\w.\-]*)`")
_MEAN_STDEV = re.compile(r"([\d.]+)\s*±\s*([\d.]+)")
_CAP = re.compile(r"cap:\s*([≤<=]+)\s*([\d.]+)|cap:\s*0\s*hard", re.IGNORECASE)
_FLOOR = re.compile(r"≥\s*([\d.]+)")


def parse_doc(path: Path) -> dict[str, Band]:
    bands: dict[str, Band] = {}
    text = path.read_text() if path.exists() else ""
    for line in text.splitlines():
        if not line.startswith("|"):
            continue
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if len(cells) < 2:
            continue
        # cells: [field, expected, ... what it tells you]
        m = _FIELD_PATTERN.search(cells[0])
        if not m:
            continue
        fld = m.group(1)
        if fld in bands:
            continue
        expected = cells[1]
        b = Band(field=fld)
        ms = _MEAN_STDEV.search(expected)
        if ms:
            b.mean = float(ms.group(1))
            b.stdev = float(ms.group(2))
        cap = _CAP.search(expected)
        if cap:
            if "hard" in expected.lower() and "0 hard" in expected.lower():
                b.hard_zero = True
                b.cap = 0.0
            elif cap.group(2):
                b.cap = float(cap.group(2))
        floor = _FLOOR.search(expected)
        if floor:
            b.floor = float(floor.group(1))
        bands[fld] = b
    return bands


# ── reading the run footer ─────────────────────────────────────────────

def find_events_log(run_dir: Path) -> Path:
    direct = run_dir / "events.jsonl"
    if direct.exists():
        return direct
    if run_dir.is_file() and run_dir.suffix == ".jsonl":
        return run_dir
    raise SystemExit(f"fingerprint: no events.jsonl found at {run_dir}")


def read_footer(events_path: Path) -> dict[str, Any]:
    proc = subprocess.run(
        ["jq", "-c", "select(._footer)", str(events_path)],
        capture_output=True, text=True,
    )
    if proc.returncode != 0:
        raise SystemExit(f"fingerprint: jq failed: {proc.stderr.strip()}")
    line = next((l for l in proc.stdout.splitlines() if l.strip()), "")
    return json.loads(line) if line else {}


def lookup_dotted(d: dict[str, Any], dotted: str) -> Any:
    cur: Any = d
    for part in dotted.split("."):
        if isinstance(cur, dict) and part in cur:
            cur = cur[part]
        else:
            return None
    return cur


# ── verdict logic ──────────────────────────────────────────────────────

def evaluate(b: Band, observed: Any) -> tuple[str, str]:
    if observed is None or not isinstance(observed, (int, float)):
        return "unknown", "metric not in footer"
    o = float(observed)

    if b.hard_zero:
        if o == 0:
            return "in-range", ""
        return "failure", f"hard cap is 0; observed {o}"

    if b.cap is not None and o > b.cap:
        return "above-cap", f"cap is {b.cap}; observed {o}"

    if b.floor is not None and o < b.floor:
        return "below-floor", f"floor is {b.floor}; observed {o}"

    if b.mean is not None and b.stdev is not None:
        width = max(2 * b.stdev, MIN_BAND_WIDTH)
        lo = max(0.0, b.mean - width)
        hi = b.mean + width
        if o < lo:
            return "low", f"expected {b.mean:.2f} ± {b.stdev:.2f}; observed {o}"
        if o > hi:
            return "high", f"expected {b.mean:.2f} ± {b.stdev:.2f}; observed {o}"

    return "in-range", ""


def derive_overall(rows: list[FingerprintRow]) -> str:
    if any(r.verdict in ("failure", "above-cap") for r in rows):
        return "fail"
    if any(r.verdict in ("below-floor", "high", "low") for r in rows):
        return "concern"
    return "pass"


def derive_next_steps(rows: list[FingerprintRow], run_dir: Path) -> list[str]:
    steps: list[str] = []
    for r in rows:
        if r.verdict == "failure":
            if r.field.startswith("deaths_by_cause."):
                cause = r.field.split(".", 1)[1]
                steps.append(f"just q deaths {run_dir} --cause={cause}")
        elif r.verdict == "below-floor" and r.field.startswith("continuity_tallies."):
            steps.append(f"just q anomalies {run_dir}")
            break
    if any(r.verdict == "above-cap" for r in rows):
        steps.append(f"just verdict {run_dir}")
    return list(dict.fromkeys(steps))


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("run_dir")
    ap.add_argument("--doc", default=str(HEALTHY_COLONY))
    ap.add_argument("--text", action="store_true")
    args = ap.parse_args(argv)

    run_dir = Path(args.run_dir)
    events = find_events_log(run_dir)
    footer = read_footer(events)
    bands = parse_doc(Path(args.doc))
    if not bands:
        sys.stderr.write(f"fingerprint: no bands parsed from {args.doc}\n")
        return 2

    rows: list[FingerprintRow] = []
    for fld, b in bands.items():
        observed = lookup_dotted(footer, fld)
        verdict, note = evaluate(b, observed)
        rows.append(FingerprintRow(field=fld, observed=observed, band=b,
                                   verdict=verdict, note=note))

    overall = derive_overall(rows)
    fp = Fingerprint(
        run=str(run_dir),
        overall=overall,
        rows=[asdict(r) for r in rows],
        next_steps=derive_next_steps(rows, run_dir),
    )

    if args.text:
        sys.stdout.write(f"fingerprint: {overall.upper()}  ({run_dir})\n")
        groups: dict[str, list[FingerprintRow]] = {}
        for r in rows:
            groups.setdefault(r.verdict, []).append(r)
        for v in ("failure", "above-cap", "below-floor", "low", "high",
                  "in-range", "unknown"):
            group = groups.get(v, [])
            if not group:
                continue
            sys.stdout.write(f"  [{v}] ({len(group)})\n")
            for r in group[:8]:
                obs = r.observed if r.observed is not None else "?"
                detail = f" — {r.note}" if r.note else ""
                sys.stdout.write(f"    {r.field}  observed={obs}{detail}\n")
        if fp.next_steps:
            sys.stdout.write("  next:\n")
            for s in fp.next_steps:
                sys.stdout.write(f"    $ {s}\n")
    else:
        sys.stdout.write(json.dumps(asdict(fp), indent=2) + "\n")

    return {"pass": 0, "concern": 1, "fail": 2}[overall]


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
