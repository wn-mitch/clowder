#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = ["scipy>=1.10"]
# ///
"""
Statistical summary of a sweep directory, optionally comparing two sweeps.

Replaces the retired `balance-report`, `score-diff`, and `sweep_compare.py`.
Reads `_footer` records from every `<seed>-<rep>/events.jsonl` (or
`<seed>/events.jsonl` for older layouts) under a sweep directory, then
emits per-metric mean / stdev / 95% CI / sample size. With `--vs
<baseline-sweep-dir>`, also computes Welch's two-sample t / Cohen's d /
effect-size band against that baseline.

Output is a structured JSON envelope on stdout; pass `--text` for a
human-readable summary or `--charts` for opt-in matplotlib trend PNGs
(under `logs/charts/<sweep-name>/`).

Bands:
  significant — |delta_pct| ≥ 30% and p < 0.05 and |Cohen's d| > 0.5
  drift       — 10% ≤ |delta_pct| < 30% (worth investigating)
  noise       — |delta_pct| < 10%
  inconclusive — direction-only (one side missing or zero)

Usage:
    just sweep-stats logs/sweep-baseline-5b
    just sweep-stats logs/sweep-fog-activation-1 --vs logs/sweep-baseline-5b
    just sweep-stats logs/sweep-X --vs logs/sweep-Y --text
    just sweep-stats logs/sweep-X --charts
"""

from __future__ import annotations

import argparse
import json
import math
import statistics
import sys
from dataclasses import dataclass, field, asdict
from pathlib import Path
from typing import Any, Iterable

from scipy import stats  # type: ignore[import-not-found]


REPO_ROOT = Path(__file__).resolve().parent.parent

NOISE_PCT = 10.0
SIGNIFICANT_PCT = 30.0
P_THRESHOLD = 0.05
EFFECT_THRESHOLD = 0.5  # |Cohen's d| above this counts as a real shift


@dataclass
class SweepReport:
    sweep: str
    n: int
    runs: list[str] = field(default_factory=list)
    metrics: list[dict[str, Any]] = field(default_factory=list)
    vs_baseline: str | None = None
    # Seed bookkeeping. Listed sorted; `seed_sets_match` only meaningful
    # when comparing against a baseline. When False, every per-metric
    # delta is confounded with seed-level variance and the bands are
    # inflated — the caller should rebuild the comparison sweep on the
    # baseline's seed set.
    seeds: list[int] = field(default_factory=list)
    baseline_seeds: list[int] | None = None
    seed_sets_match: bool | None = None


# ── reading ─────────────────────────────────────────────────────────────────

def find_events_files(sweep_dir: Path) -> list[Path]:
    out: list[Path] = []
    for child in sorted(sweep_dir.iterdir()):
        if not child.is_dir():
            continue
        e = child / "events.jsonl"
        if e.exists() and e.stat().st_size > 0:
            out.append(e)
    if not out:
        for e in sorted(sweep_dir.rglob("events.jsonl")):
            if e.stat().st_size > 0:
                out.append(e)
    return out


def read_footer(events_path: Path) -> dict[str, Any] | None:
    last: dict[str, Any] | None = None
    try:
        with events_path.open() as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                try:
                    obj = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if isinstance(obj, dict) and obj.get("_footer"):
                    last = obj
    except OSError:
        return None
    return last


def read_header_seed(events_path: Path) -> int | None:
    """Best-effort seed extraction from line 1's `_header` record."""
    try:
        with events_path.open() as f:
            line = f.readline().strip()
            if not line:
                return None
            obj = json.loads(line)
            if isinstance(obj, dict) and obj.get("_header"):
                seed = obj.get("seed")
                return seed if isinstance(seed, int) else None
    except (OSError, json.JSONDecodeError):
        return None
    return None


def collect_seeds(files: list[Path]) -> list[int]:
    seeds: set[int] = set()
    for p in files:
        s = read_header_seed(p)
        if s is not None:
            seeds.add(s)
    return sorted(seeds)


# ── flattening ──────────────────────────────────────────────────────────────

def flatten(d: dict[str, Any], prefix: str = "") -> dict[str, float]:
    out: dict[str, float] = {}
    for k, v in d.items():
        if k.startswith("_"):
            continue
        path = f"{prefix}{k}"
        if isinstance(v, bool):
            continue
        if isinstance(v, (int, float)):
            out[path] = float(v)
        elif isinstance(v, dict):
            out.update(flatten(v, prefix=f"{path}."))
    return out


def collect(footers: Iterable[dict[str, Any]]) -> dict[str, list[float]]:
    flats = [flatten(f) for f in footers]
    keys: set[str] = set()
    for f in flats:
        keys.update(f.keys())
    return {k: [f.get(k, 0.0) for f in flats] for k in keys}


# ── stats ──────────────────────────────────────────────────────────────────

def summarize(values: list[float]) -> tuple[float, float, list[float], float, float]:
    if not values:
        return 0.0, 0.0, [0.0, 0.0], 0.0, 0.0
    mean = statistics.fmean(values)
    sd = statistics.stdev(values) if len(values) > 1 else 0.0
    if len(values) > 1:
        t = stats.t.ppf(0.975, df=len(values) - 1)
        margin = t * sd / math.sqrt(len(values))
        ci = [mean - margin, mean + margin]
    else:
        ci = [mean, mean]
    return mean, sd, ci, min(values), max(values)


def compare(observed: list[float], baseline: list[float]) -> dict[str, Any]:
    o_mean = statistics.fmean(observed) if observed else 0.0
    b_mean = statistics.fmean(baseline) if baseline else 0.0
    if not observed or not baseline:
        return {"delta_pct": None, "p": None, "effect_size": None, "band": "inconclusive",
                "baseline_mean": round(b_mean, 4)}
    if b_mean == 0 and o_mean == 0:
        return {"delta_pct": 0.0, "p": 1.0, "effect_size": 0.0, "band": "noise",
                "baseline_mean": 0.0}

    delta_pct = None if b_mean == 0 else (o_mean - b_mean) / b_mean * 100.0

    if len(observed) > 1 and len(baseline) > 1 and (
        statistics.stdev(observed) > 0 or statistics.stdev(baseline) > 0
    ):
        _, p = stats.ttest_ind(observed, baseline, equal_var=False)
        p = float(p) if not math.isnan(p) else 1.0
    else:
        p = 0.0 if o_mean != b_mean else 1.0

    pooled_sd = math.sqrt(
        (statistics.variance(observed) if len(observed) > 1 else 0.0)
        + (statistics.variance(baseline) if len(baseline) > 1 else 0.0)
    ) / math.sqrt(2)
    effect = (o_mean - b_mean) / pooled_sd if pooled_sd > 0 else (
        float("inf") if o_mean != b_mean else 0.0
    )

    if delta_pct is None:
        band = "inconclusive"
    elif abs(delta_pct) < NOISE_PCT:
        band = "noise"
    elif abs(delta_pct) >= SIGNIFICANT_PCT and p < P_THRESHOLD and abs(effect) > EFFECT_THRESHOLD:
        band = "significant"
    else:
        band = "drift"

    return {
        "delta_pct": None if delta_pct is None else round(delta_pct, 1),
        "p": round(p, 4) if math.isfinite(p) else None,
        "effect_size": round(effect, 2) if math.isfinite(effect) else None,
        "band": band,
        "baseline_mean": round(b_mean, 4),
    }


# ── charts (opt-in) ─────────────────────────────────────────────────────────

def emit_charts(sweep_name: str, columns: dict[str, list[float]]) -> Path | None:
    try:
        import matplotlib.pyplot as plt  # type: ignore[import-not-found]
    except ImportError:
        sys.stderr.write("sweep-stats: --charts requires matplotlib; skipping charts.\n")
        return None
    out_dir = REPO_ROOT / "logs" / "charts" / sweep_name
    out_dir.mkdir(parents=True, exist_ok=True)
    headline = ["deaths_by_cause.Starvation", "deaths_by_cause.ShadowFoxAmbush",
                "wards_placed_total", "kitten_born_total"]
    for field_name in headline:
        if field_name not in columns:
            continue
        values = columns[field_name]
        plt.figure(figsize=(6, 3))
        plt.boxplot(values, vert=False)
        plt.title(f"{sweep_name}: {field_name}")
        plt.xlabel(field_name)
        plt.tight_layout()
        plt.savefig(out_dir / f"{field_name.replace('.', '_')}.png", dpi=120)
        plt.close()
    return out_dir


# ── main ───────────────────────────────────────────────────────────────────

def build_report(sweep_dir: Path, baseline_dir: Path | None) -> SweepReport:
    files = find_events_files(sweep_dir)
    if not files:
        sys.stderr.write(f"sweep-stats: no events.jsonl found under {sweep_dir}\n")
        sys.exit(2)
    footers = [f for f in (read_footer(p) for p in files) if f]
    if not footers:
        sys.stderr.write(f"sweep-stats: no `_footer` records in {sweep_dir}\n")
        sys.exit(2)

    columns = collect(footers)
    seeds = collect_seeds(files)

    baseline_columns: dict[str, list[float]] | None = None
    baseline_seeds: list[int] | None = None
    seed_sets_match: bool | None = None
    if baseline_dir:
        b_files = find_events_files(baseline_dir)
        b_footers = [f for f in (read_footer(p) for p in b_files) if f]
        if not b_footers:
            sys.stderr.write(f"sweep-stats: no `_footer` records in baseline {baseline_dir}\n")
            sys.exit(2)
        baseline_columns = collect(b_footers)
        baseline_seeds = collect_seeds(b_files)
        seed_sets_match = (set(seeds) == set(baseline_seeds)) if (seeds and baseline_seeds) else None
        if seed_sets_match is False:
            sys.stderr.write(
                f"sweep-stats: WARNING — seed sets differ between observed {seeds} and "
                f"baseline {baseline_seeds}; per-metric deltas are confounded with seed-level "
                "variance. Re-run the observed sweep on the baseline's seed set for a clean "
                "comparison.\n"
            )

    rows: list[dict[str, Any]] = []
    for path in sorted(columns):
        values = columns[path]
        mean, sd, ci, mn, mx = summarize(values)
        if mean == 0 and sd == 0 and (baseline_columns is None
                                      or sum(baseline_columns.get(path, [])) == 0):
            continue
        row: dict[str, Any] = {
            "field": path,
            "mean": round(mean, 4),
            "stdev": round(sd, 4),
            "ci95": [round(ci[0], 4), round(ci[1], 4)],
            "min": round(mn, 4),
            "max": round(mx, 4),
            "n": len(values),
        }
        if baseline_columns is not None:
            row["vs_baseline"] = compare(values, baseline_columns.get(path, []))
        rows.append(row)

    if baseline_columns is not None:
        rows.sort(key=lambda r: -abs(r["vs_baseline"]["delta_pct"] or 0))

    return SweepReport(
        sweep=str(sweep_dir),
        n=len(footers),
        runs=[str(p.relative_to(sweep_dir)) for p in files],
        metrics=rows,
        vs_baseline=str(baseline_dir) if baseline_dir else None,
        seeds=seeds,
        baseline_seeds=baseline_seeds,
        seed_sets_match=seed_sets_match,
    )


def render_text(r: SweepReport) -> str:
    lines = [f"sweep: {r.sweep}  (n={r.n})"]
    if r.seeds:
        lines.append(f"  seeds: {r.seeds}")
    if r.vs_baseline:
        lines.append(f"  vs baseline: {r.vs_baseline}")
        if r.seed_sets_match is False:
            lines.append(f"  ⚠ seed mismatch — baseline={r.baseline_seeds}; deltas are confounded")
    bands: dict[str, list[dict[str, Any]]] = {
        "significant": [], "drift": [], "noise": [], "inconclusive": [],
    }
    for row in r.metrics:
        b = (row.get("vs_baseline") or {}).get("band")
        if b in bands:
            bands[b].append(row)
    if r.vs_baseline:
        for band_name in ("significant", "drift", "inconclusive", "noise"):
            rows = bands.get(band_name, [])
            if not rows:
                continue
            lines.append("")
            lines.append(f"  [{band_name}]  ({len(rows)} fields)")
            for row in rows[:10]:
                v = row["vs_baseline"]
                d = v["delta_pct"]
                d_s = "  new" if d is None else f"{d:+6.1f}%"
                p_s = "    -" if v["p"] is None else f"{v['p']:.4f}"
                e_s = "    -" if v["effect_size"] is None else f"{v['effect_size']:+.2f}"
                bm = v.get("baseline_mean", "?")
                lines.append(
                    f"    {d_s}  p={p_s}  d={e_s}  "
                    f"{row['field']}  ({bm} → {row['mean']})"
                )
    else:
        lines.append("")
        for row in r.metrics[:20]:
            lines.append(
                f"  {row['mean']:>10.3f}  ±{row['stdev']:.3f}  n={row['n']}  {row['field']}"
            )
    return "\n".join(lines)


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("sweep_dir", help="Path to a sweep directory (e.g. logs/sweep-foo)")
    ap.add_argument("--vs", default=None, help="Baseline sweep directory for two-sample comparison")
    ap.add_argument("--text", action="store_true", help="Human-readable output")
    ap.add_argument("--charts", action="store_true", help="Emit matplotlib boxplots under logs/charts/")
    args = ap.parse_args(argv)

    sweep_dir = Path(args.sweep_dir)
    baseline_dir = Path(args.vs) if args.vs else None
    report = build_report(sweep_dir, baseline_dir)

    if args.charts:
        files = find_events_files(sweep_dir)
        footers = [f for f in (read_footer(p) for p in files) if f]
        emit_charts(sweep_dir.name, collect(footers))

    if args.text:
        sys.stdout.write(render_text(report) + "\n")
    else:
        sys.stdout.write(json.dumps(asdict(report), indent=2) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
