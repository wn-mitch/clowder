#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = ["scipy>=1.10"]
# ///
"""
Statistical A/B comparison between two Phase 5b sweep directories.

Each sweep directory has the layout produced by `just sweep`:
    logs/sweep-<label>/<seed>-<rep>/events.jsonl

The footer (last line of events.jsonl) is the ground-truth metric source.
This script reads all runs in each directory, computes per-metric mean/stddev
and a Mann-Whitney U test between the two groups, and prints a markdown
report with pass/fail against Balance Methodology thresholds.

Usage:
    uv run scripts/sweep_compare.py logs/sweep-baseline-5b logs/sweep-fog-activation-1
    uv run scripts/sweep_compare.py BASE POST --predictions predictions.json

The predictions file (optional) is a JSON object mapping metric name to
  {"direction": "up"|"down", "magnitude_pct": N}
for formal concordance checking. Without it, the script just reports deltas.
"""

from __future__ import annotations

import argparse
import json
import math
import statistics
import sys
from pathlib import Path

from scipy import stats  # type: ignore[import-not-found]

CANARY_GATES = {
    "deaths_by_cause.Starvation": ("eq", 0),
    "deaths_by_cause.ShadowFoxAmbush": ("le", 10),
}

DRIFT_NOISE_BAND = 10.0   # |%| below this is measurement noise
DRIFT_SCRUTINY_BAND = 30.0  # |%| above this needs second-order investigation


def load_footer(events_path: Path) -> dict | None:
    """Read the `_footer` JSON object (last line) of an events.jsonl."""
    try:
        with events_path.open() as f:
            last = None
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
            return last
    except OSError:
        return None


def load_header(events_path: Path) -> dict | None:
    """Read the `_header` JSON object (first line) of an events.jsonl."""
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
                if isinstance(obj, dict) and obj.get("_header"):
                    return obj
                return None
    except OSError:
        return None


def flatten_footer(footer: dict) -> dict[str, float]:
    """Flatten the footer into metric_name → scalar. Map fields become
    dot-separated keys, with zero-fill for missing entries at compare time."""
    out: dict[str, float] = {}
    for k, v in footer.items():
        if k == "_footer":
            continue
        if isinstance(v, (int, float)):
            out[k] = float(v)
        elif isinstance(v, dict):
            for sub_k, sub_v in v.items():
                if isinstance(sub_v, (int, float)):
                    out[f"{k}.{sub_k}"] = float(sub_v)
    return out


def load_sweep_dir(sweep_dir: Path) -> tuple[list[dict], list[dict]]:
    """Collect (footers, headers) from every run directory in a sweep."""
    footers: list[dict] = []
    headers: list[dict] = []
    for run_dir in sorted(sweep_dir.iterdir()):
        if not run_dir.is_dir():
            continue
        events_path = run_dir / "events.jsonl"
        if not events_path.exists():
            print(f"  warn: {run_dir} has no events.jsonl, skipping", file=sys.stderr)
            continue
        footer = load_footer(events_path)
        header = load_header(events_path)
        if footer is None:
            print(f"  warn: {run_dir} has no _footer, skipping", file=sys.stderr)
            continue
        footer["_run_id"] = run_dir.name
        footers.append(footer)
        if header is not None:
            headers.append(header)
    return footers, headers


def aggregate(group: list[dict], metric: str) -> list[float]:
    """Extract per-run values for a metric, zero-filling when absent."""
    out: list[float] = []
    for footer in group:
        if "." in metric:
            top, sub = metric.split(".", 1)
            v = footer.get(top, {}).get(sub, 0)
        else:
            v = footer.get(metric, 0)
        out.append(float(v) if isinstance(v, (int, float)) else 0.0)
    return out


def gather_metric_names(groups: list[list[dict]]) -> list[str]:
    """Union of flattened metric names across all runs in all groups."""
    names: set[str] = set()
    for group in groups:
        for footer in group:
            names.update(flatten_footer(footer).keys())
    return sorted(names)


def pct_delta(old_mean: float, new_mean: float) -> float | None:
    if old_mean == 0 and new_mean == 0:
        return 0.0
    if old_mean == 0:
        return None  # infinite — flag with None
    return (new_mean - old_mean) / abs(old_mean) * 100.0


def check_concordance(
    metric: str,
    base_vals: list[float],
    post_vals: list[float],
    prediction: dict | None,
) -> tuple[str, str]:
    """Return (status, reason). status in {PASS, FAIL, NOISE, NO-PRED}."""
    base_mean = statistics.fmean(base_vals) if base_vals else 0.0
    post_mean = statistics.fmean(post_vals) if post_vals else 0.0
    delta = pct_delta(base_mean, post_mean)

    if prediction is None:
        if delta is None:
            return ("NO-PRED", "new metric (base mean 0)")
        if abs(delta) < DRIFT_NOISE_BAND:
            return ("NOISE", f"|Δ|={abs(delta):.1f}% within noise band")
        return ("NO-PRED", f"Δ={delta:+.1f}% (no prediction to check)")

    predicted_dir = prediction.get("direction", "up")
    predicted_mag = float(prediction.get("magnitude_pct", 0))

    if delta is None:
        return ("FAIL", "base mean is 0 — cannot measure %")
    observed_dir = "up" if delta > 0 else "down"
    if observed_dir != predicted_dir:
        return ("FAIL", f"predicted {predicted_dir}, got {observed_dir} ({delta:+.1f}%)")

    if predicted_mag == 0:
        return ("PASS", f"Δ={delta:+.1f}% (direction only, no magnitude check)")
    ratio = abs(delta) / predicted_mag
    if 0.5 <= ratio <= 2.0:
        return ("PASS", f"|Δ|={abs(delta):.1f}% / predicted {predicted_mag}% (×{ratio:.2f})")
    return ("FAIL", f"magnitude off: |Δ|={abs(delta):.1f}% vs predicted {predicted_mag}% (×{ratio:.2f})")


def check_canaries(post: list[dict]) -> list[tuple[str, bool, str]]:
    """Apply hard-gate canaries to the post-activation group."""
    results: list[tuple[str, bool, str]] = []
    for metric, (op, threshold) in CANARY_GATES.items():
        vals = aggregate(post, metric)
        if not vals:
            results.append((metric, True, "metric not present"))
            continue
        worst = max(vals) if op == "le" else max(vals)
        if op == "eq":
            passed = all(v == threshold for v in vals)
            reason = f"max={worst:.0f} (threshold == {threshold})"
        elif op == "le":
            passed = all(v <= threshold for v in vals)
            reason = f"max={worst:.0f} (threshold ≤ {threshold})"
        else:
            passed = True
            reason = "unknown op"
        results.append((metric, passed, reason))
    return results


def detect_header_drift(base_headers: list[dict], post_headers: list[dict]) -> list[str]:
    """Sanity-check that two sweeps are behaviorally comparable."""
    problems: list[str] = []
    if not base_headers or not post_headers:
        problems.append("missing headers in one or both sweeps")
        return problems

    # All runs within a sweep should share a commit hash.
    base_commits = {h.get("commit_hash_short") for h in base_headers}
    post_commits = {h.get("commit_hash_short") for h in post_headers}
    if len(base_commits) > 1:
        problems.append(f"base sweep spans multiple commits: {sorted(base_commits)}")
    if len(post_commits) > 1:
        problems.append(f"post sweep spans multiple commits: {sorted(post_commits)}")
    if any(h.get("commit_dirty") for h in base_headers + post_headers):
        problems.append("at least one run built from a dirty tree (headers may mislead)")

    # Forced weather must match across sweeps for clean comparison.
    base_forced = {h.get("forced_weather") for h in base_headers}
    post_forced = {h.get("forced_weather") for h in post_headers}
    if base_forced != post_forced:
        problems.append(
            f"forced_weather differs: base={sorted(base_forced)} vs post={sorted(post_forced)}"
        )
    return problems


def env_multiplier_diff(base_headers: list[dict], post_headers: list[dict]) -> list[str]:
    """Return a list of 'A.B.C: x -> y' lines for every sensory-env multiplier
    that differs between the sweeps. Empty list means structural-only change.
    An unexpectedly long diff means an activation changed more knobs than
    claimed — investigate before trusting the concordance verdict."""
    if not base_headers or not post_headers:
        return []
    base_mx = base_headers[0].get("sensory_env_multipliers") or {}
    post_mx = post_headers[0].get("sensory_env_multipliers") or {}
    diffs: list[str] = []

    def walk(path: list[str], a, b) -> None:
        if isinstance(a, dict) and isinstance(b, dict):
            for k in sorted(set(a) | set(b)):
                walk(path + [k], a.get(k), b.get(k))
            return
        if a != b:
            diffs.append(f"{'.'.join(path)}: {a} → {b}")

    walk([], base_mx, post_mx)
    return diffs


def mwu_pvalue(base: list[float], post: list[float]) -> float | None:
    """Mann-Whitney U two-sided p-value, or None if undefined."""
    if len(base) < 2 or len(post) < 2:
        return None
    # All-equal values give NaN; scipy raises in some versions.
    if len(set(base + post)) == 1:
        return 1.0
    try:
        result = stats.mannwhitneyu(base, post, alternative="two-sided")
        p = float(result.pvalue)
        return p if not math.isnan(p) else None
    except (ValueError, TypeError):
        return None


def seed_of(footer: dict) -> str:
    """Extract the seed prefix from the run id (e.g. '42' from '42-1')."""
    run_id = footer.get("_run_id", "")
    return run_id.split("-", 1)[0] if "-" in run_id else run_id


def group_by_seed(footers: list[dict]) -> dict[str, list[dict]]:
    out: dict[str, list[dict]] = {}
    for f in footers:
        out.setdefault(seed_of(f), []).append(f)
    return out


def paired_seed_analysis(
    base: list[dict], post: list[dict], metric: str
) -> tuple[list[tuple[str, float, float, float | None]], float | None]:
    """For each seed present in both sweeps, compute per-seed mean(base),
    mean(post), and pct delta. Return the per-seed rows plus the Wilcoxon
    signed-rank p-value on the vector of absolute deltas."""
    base_by_seed = group_by_seed(base)
    post_by_seed = group_by_seed(post)
    rows: list[tuple[str, float, float, float | None]] = []
    for seed in sorted(set(base_by_seed) & set(post_by_seed)):
        b_vals = aggregate(base_by_seed[seed], metric)
        p_vals = aggregate(post_by_seed[seed], metric)
        if not b_vals or not p_vals:
            continue
        b_mean = statistics.fmean(b_vals)
        p_mean = statistics.fmean(p_vals)
        delta = pct_delta(b_mean, p_mean)
        rows.append((seed, b_mean, p_mean, delta))

    # Signed-rank test on absolute differences (not percent — avoids
    # exploding at small base means).
    diffs = [p_mean - b_mean for _, b_mean, p_mean, _ in rows]
    if len([d for d in diffs if d != 0]) < 3:
        return rows, None
    try:
        result = stats.wilcoxon(diffs)
        p = float(result.pvalue)
        return rows, p if not math.isnan(p) else None
    except (ValueError, TypeError):
        return rows, None


def fmt_mean_std(vals: list[float]) -> str:
    if not vals:
        return "—"
    m = statistics.fmean(vals)
    if len(vals) < 2:
        return f"{m:.2f}"
    s = statistics.stdev(vals)
    return f"{m:.2f} ± {s:.2f}"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("base", type=Path, help="baseline sweep directory")
    parser.add_argument("post", type=Path, help="post-activation sweep directory")
    parser.add_argument(
        "--predictions",
        type=Path,
        help="JSON file: {metric: {direction: up|down, magnitude_pct: N}}",
    )
    parser.add_argument(
        "--top",
        type=int,
        default=30,
        help="only show the N metrics with largest |%Δ| (default: 30)",
    )
    parser.add_argument(
        "--out",
        type=Path,
        help="write markdown report to this path (also printed to stdout)",
    )
    args = parser.parse_args()

    base_footers, base_headers = load_sweep_dir(args.base)
    post_footers, post_headers = load_sweep_dir(args.post)

    if not base_footers or not post_footers:
        print(f"Error: found {len(base_footers)} base footers, {len(post_footers)} post footers", file=sys.stderr)
        return 1

    predictions: dict = {}
    if args.predictions:
        try:
            predictions = json.loads(args.predictions.read_text())
        except (OSError, json.JSONDecodeError) as e:
            print(f"Error reading predictions: {e}", file=sys.stderr)
            return 1

    lines: list[str] = []

    def out(s: str = "") -> None:
        lines.append(s)
        print(s)

    out(f"# Sweep comparison: `{args.base.name}` → `{args.post.name}`")
    out()
    out(f"- base runs: {len(base_footers)}")
    out(f"- post runs: {len(post_footers)}")

    drift = detect_header_drift(base_headers, post_headers)
    if drift:
        out()
        out("## ⚠ Header drift")
        for d in drift:
            out(f"- {d}")

    env_diff = env_multiplier_diff(base_headers, post_headers)
    out()
    out("## Sensory env-multiplier changes")
    if not env_diff:
        out("- (none — structural change only)")
    else:
        for d in env_diff:
            out(f"- `{d}`")

    # Canaries
    out()
    out("## Canaries (hard gates)")
    canary_results = check_canaries(post_footers)
    all_canaries_passed = all(p for _, p, _ in canary_results)
    for metric, passed, reason in canary_results:
        marker = "✓" if passed else "✗"
        out(f"- {marker} `{metric}` — {reason}")

    # Metric table
    metrics = gather_metric_names([base_footers, post_footers])
    rows: list[tuple[float, str]] = []  # (abs_pct_for_sort, markdown_row)

    for metric in metrics:
        base_vals = aggregate(base_footers, metric)
        post_vals = aggregate(post_footers, metric)
        if all(v == 0 for v in base_vals + post_vals):
            continue
        base_mean = statistics.fmean(base_vals) if base_vals else 0.0
        post_mean = statistics.fmean(post_vals) if post_vals else 0.0
        delta = pct_delta(base_mean, post_mean)
        delta_str = "—" if delta is None else f"{delta:+.1f}%"
        p = mwu_pvalue(base_vals, post_vals)
        p_str = "—" if p is None else f"{p:.3f}"
        status, reason = check_concordance(
            metric, base_vals, post_vals, predictions.get(metric)
        )
        row = (
            f"| `{metric}` | {fmt_mean_std(base_vals)} | {fmt_mean_std(post_vals)} "
            f"| {delta_str} | {p_str} | {status} | {reason} |"
        )
        sort_key = abs(delta) if delta is not None else float("inf")
        rows.append((sort_key, row))

    rows.sort(key=lambda r: -r[0])
    if args.top > 0:
        rows = rows[: args.top]

    out()
    out("## Top movers (sorted by |Δ|)")
    out()
    out("| Metric | Base mean ± sd | Post mean ± sd | Δ%  | MWU p | Status | Notes |")
    out("|---|---|---|---|---|---|---|")
    for _, row in rows:
        out(row)

    # Per-seed paired analysis for metrics with predictions — the noise
    # structure in this sim is seed-driven, so pooled tests blur signal.
    if predictions:
        out()
        out("## Per-seed paired deltas (predicted metrics only)")
        for metric in predictions:
            seed_rows, wilcoxon_p = paired_seed_analysis(base_footers, post_footers, metric)
            if not seed_rows:
                continue
            out()
            out(f"### `{metric}`")
            out("| seed | base mean | post mean | Δ%  |")
            out("|---|---|---|---|")
            for seed, b_mean, p_mean, delta in seed_rows:
                delta_str = "—" if delta is None else f"{delta:+.1f}%"
                out(f"| {seed} | {b_mean:.2f} | {p_mean:.2f} | {delta_str} |")
            wp = "—" if wilcoxon_p is None else f"{wilcoxon_p:.3f}"
            out(f"- Wilcoxon signed-rank p (base vs post, across seeds): {wp}")

    # Summary
    out()
    out("## Summary")
    fails = sum(1 for _, row in rows if "| FAIL |" in row)
    passes = sum(1 for _, row in rows if "| PASS |" in row)
    out(f"- canaries: {'all passed' if all_canaries_passed else 'FAILED'}")
    if predictions:
        out(f"- concordance: {passes} PASS, {fails} FAIL (within top {args.top})")

    if args.out:
        args.out.write_text("\n".join(lines) + "\n")
        print(f"\nReport written to {args.out}", file=sys.stderr)

    return 0 if all_canaries_passed and fails == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
