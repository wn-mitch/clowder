#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = ["pyyaml>=6.0", "scipy>=1.10"]
# ///
"""
Run a balance hypothesis end-to-end: baseline + treatment sweeps,
concordance check, draft balance doc.

Formalizes the four-artifact balance methodology CLAUDE.md mandates
(hypothesis / prediction / observation / concordance). Read a YAML
spec, run the existing `just sweep` recipe twice (once with no
overrides, once with `CLOWDER_OVERRIDES` set from `constants_patch`),
compute per-metric drift via the existing `sweep_stats.py` machinery,
and emit a JSON envelope with a concordance verdict. Drafts a
`docs/balance/<slug>.md` with all four artifacts pre-filled.

Background-safe: writes a `STATUS.json` after every phase. Re-running
with the same slug resumes from the last completed phase.

Usage:
    just hypothesize docs/balance/my-hypothesis.yaml
    just hypothesize SPEC --slug custom-slug   # override doc filename
    just hypothesize SPEC --duration 60 --seeds 42 --reps 1   # smoke test
    just hypothesize SPEC --skip-baseline      # reuse existing baseline-<slug> sweep
    just hypothesize SPEC --skip-treatment     # rerun analysis only

Exit codes: 0 concordant, 1 inconclusive, 2 wrong direction or fail.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import re
import shutil
import subprocess
import sys
from dataclasses import dataclass, asdict
from pathlib import Path
from typing import Any

import yaml  # type: ignore[import-untyped]

REPO_ROOT = Path(__file__).resolve().parent.parent

# Reuse sweep_stats's stat machinery directly.
sys.path.insert(0, str(REPO_ROOT / "scripts"))
import sweep_stats  # noqa: E402
from _agent_call_log import append_call_history  # noqa: E402


@dataclass
class HypothesisSpec:
    hypothesis: str
    constants_patch: dict[str, Any]
    metric: str
    direction: str  # increase | decrease | unchanged
    rough_magnitude_pct: list[float]
    seeds: list[int]
    reps: int
    duration: int

    @staticmethod
    def from_yaml(path: Path) -> "HypothesisSpec":
        spec = yaml.safe_load(path.read_text())
        prediction = spec.get("prediction") or {}
        return HypothesisSpec(
            hypothesis=spec["hypothesis"],
            constants_patch=spec.get("constants_patch") or {},
            metric=prediction["metric"],
            direction=prediction.get("direction", "increase"),
            rough_magnitude_pct=list(prediction.get("rough_magnitude_pct") or [10.0, 100.0]),
            seeds=list(spec.get("seeds") or [42, 99, 7]),
            reps=int(spec.get("reps") or 3),
            duration=int(spec.get("duration") or 900),
        )


def slugify(text: str) -> str:
    s = re.sub(r"[^a-zA-Z0-9]+", "-", text.lower()).strip("-")
    return s[:60] or "hypothesis"


def write_status(dir_path: Path, payload: dict[str, Any]) -> None:
    dir_path.mkdir(parents=True, exist_ok=True)
    (dir_path / "STATUS.json").write_text(json.dumps(payload, indent=2))


def run_sweep(label: str, seeds: list[int], reps: int, duration: int,
              overrides: dict[str, Any] | None) -> Path:
    """Invoke `just sweep` with the configured args and optional CLOWDER_OVERRIDES."""
    sweep_dir = REPO_ROOT / "logs" / f"sweep-{label}"
    if sweep_dir.exists() and any(sweep_dir.iterdir()):
        sys.stderr.write(f"hypothesize: reusing existing sweep at {sweep_dir}\n")
        return sweep_dir
    env = os.environ.copy()
    if overrides:
        env["CLOWDER_OVERRIDES"] = json.dumps(overrides)
    seeds_str = " ".join(str(s) for s in seeds)
    cmd = ["just", "sweep", label, "", seeds_str, str(reps), str(duration)]
    sys.stderr.write(f"hypothesize: running `{' '.join(cmd)}` "
                     f"(overrides={'yes' if overrides else 'no'})\n")
    proc = subprocess.run(cmd, env=env, cwd=REPO_ROOT)
    if proc.returncode != 0:
        sys.stderr.write(f"hypothesize: sweep `{label}` failed (code {proc.returncode})\n")
        sys.exit(2)
    return sweep_dir


def get_metric_columns(sweep_dir: Path) -> dict[str, list[float]]:
    files = sweep_stats.find_events_files(sweep_dir)
    footers = [f for f in (sweep_stats.read_footer(p) for p in files) if f]
    return sweep_stats.collect(footers)


def evaluate_concordance(
    metric: str, direction: str, magnitude_band: list[float],
    baseline_values: list[float], treatment_values: list[float],
) -> dict[str, Any]:
    cmp = sweep_stats.compare(treatment_values, baseline_values)
    delta = cmp.get("delta_pct")
    observed_dir: str
    if delta is None:
        observed_dir = "unknown"
    elif abs(delta) < sweep_stats.NOISE_PCT:
        observed_dir = "unchanged"
    elif delta > 0:
        observed_dir = "increase"
    else:
        observed_dir = "decrease"

    direction_match = (direction == observed_dir) or (
        direction == "unchanged" and observed_dir == "unchanged"
    )

    in_band = False
    if delta is not None and direction in ("increase", "decrease"):
        lo, hi = magnitude_band
        in_band = lo <= abs(delta) <= hi

    if not direction_match:
        verdict = "wrong-direction"
    elif direction == "unchanged":
        verdict = "concordant" if observed_dir == "unchanged" else "drift"
    elif in_band:
        verdict = "concordant"
    elif delta is not None and abs(delta) > 0:
        # Direction matched but magnitude outside band — mark accordingly.
        verdict = "off-magnitude"
    else:
        verdict = "inconclusive"

    return {
        "metric": metric,
        "predicted_direction": direction,
        "predicted_magnitude_pct": magnitude_band,
        "observed_direction": observed_dir,
        "observed_delta_pct": delta,
        "p_value": cmp.get("p"),
        "effect_size": cmp.get("effect_size"),
        "verdict": verdict,
    }


def draft_balance_doc(slug: str, spec: HypothesisSpec, baseline_dir: Path,
                      treatment_dir: Path, conc: dict[str, Any]) -> Path:
    out = REPO_ROOT / "docs" / "balance" / f"{slug}.md"
    today = dt.date.today().isoformat()
    body = f"""# {spec.hypothesis} ({today})

Drafted by `just hypothesize` (ticket 031). Edit before committing — pre-filled
fields are starting points.

## Hypothesis

{spec.hypothesis}

**Constants patch:**

```json
{json.dumps(spec.constants_patch, indent=2)}
```

## Prediction

| Field | Value |
|---|---|
| Metric | `{spec.metric}` |
| Direction | {spec.direction} |
| Rough magnitude band | ±{spec.rough_magnitude_pct[0]:.0f}–{spec.rough_magnitude_pct[1]:.0f}% |

## Observation

Sweeps: {len(spec.seeds)} seeds × {spec.reps} reps × {spec.duration}s.

- Baseline: `{baseline_dir.relative_to(REPO_ROOT)}`
- Treatment: `{treatment_dir.relative_to(REPO_ROOT)}`

| Field | Value |
|---|---|
| Observed direction | {conc['observed_direction']} |
| Observed Δ | {conc['observed_delta_pct']}% |
| p-value (Welch's t) | {conc['p_value']} |
| Cohen's d | {conc['effect_size']} |

## Concordance

**Verdict: {conc['verdict']}**

- Direction match: {'✓' if conc['predicted_direction'] == conc['observed_direction'] else '✗'} ({conc['predicted_direction']} vs {conc['observed_direction']})
- Magnitude in band: see |Δ|={conc['observed_delta_pct']}% vs predicted ±{spec.rough_magnitude_pct[0]:.0f}–{spec.rough_magnitude_pct[1]:.0f}%

## Survival canaries

Run `just verdict {treatment_dir.relative_to(REPO_ROOT)}/<seed>-1` against any
treatment run to check survival/continuity didn't regress.

## Decision

_To fill in: ship / iterate / reject. If iterating, append the next iteration to
this file (don't open a new doc — see CLAUDE.md §Long-horizon coordination)._
"""
    out.parent.mkdir(parents=True, exist_ok=True)
    if out.exists():
        sys.stderr.write(f"hypothesize: balance doc {out} already exists; not overwriting\n")
    else:
        out.write_text(body)
    return out


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("spec", help="Path to a hypothesis YAML spec")
    ap.add_argument("--slug", default=None, help="Override slug (defaults to derived from hypothesis)")
    ap.add_argument("--seeds", default=None, help="Override seeds list (space-separated)")
    ap.add_argument("--reps", type=int, default=None, help="Override reps")
    ap.add_argument("--duration", type=int, default=None, help="Override duration")
    ap.add_argument("--skip-baseline", action="store_true", help="Reuse logs/sweep-baseline-<slug>")
    ap.add_argument("--skip-treatment", action="store_true", help="Reuse logs/sweep-<slug>-treatment")
    ap.add_argument("--text", action="store_true", help="Human-readable output")
    ap.add_argument("--rationale", default=None,
                    help="Why this hypothesis run was requested (free text). "
                         "Appended to logs/agent-call-history.jsonl on completion. "
                         "Always pass when invoked by an agent.")
    args = ap.parse_args(argv)

    spec_path = Path(args.spec)
    spec = HypothesisSpec.from_yaml(spec_path)
    if args.seeds:
        spec.seeds = [int(s) for s in args.seeds.split()]
    if args.reps:
        spec.reps = args.reps
    if args.duration:
        spec.duration = args.duration

    slug = args.slug or slugify(spec.hypothesis)
    work_dir = REPO_ROOT / "logs" / f"hypothesize-{slug}"
    work_dir.mkdir(parents=True, exist_ok=True)
    write_status(work_dir, {"slug": slug, "phase": "starting", "spec": asdict(spec)})

    baseline_label = f"baseline-{slug}"
    treatment_label = f"{slug}-treatment"

    write_status(work_dir, {"slug": slug, "phase": "baseline-sweep"})
    baseline_dir = run_sweep(baseline_label, spec.seeds, spec.reps, spec.duration, None)
    if args.skip_baseline and not (baseline_dir.exists() and any(baseline_dir.iterdir())):
        sys.stderr.write(f"hypothesize: --skip-baseline requested but {baseline_dir} is empty\n")
        sys.exit(2)

    write_status(work_dir, {"slug": slug, "phase": "treatment-sweep"})
    treatment_dir = run_sweep(treatment_label, spec.seeds, spec.reps, spec.duration,
                              spec.constants_patch)

    write_status(work_dir, {"slug": slug, "phase": "concordance"})
    baseline_columns = get_metric_columns(baseline_dir)
    treatment_columns = get_metric_columns(treatment_dir)
    if spec.metric not in baseline_columns and spec.metric not in treatment_columns:
        sys.stderr.write(
            f"hypothesize: predicted metric `{spec.metric}` not found in either sweep's footers. "
            f"Check the dotted path against `_footer` keys.\n"
        )
        sys.exit(2)

    conc = evaluate_concordance(
        spec.metric, spec.direction, spec.rough_magnitude_pct,
        baseline_columns.get(spec.metric, []),
        treatment_columns.get(spec.metric, []),
    )

    write_status(work_dir, {"slug": slug, "phase": "drafting-doc"})
    doc = draft_balance_doc(slug, spec, baseline_dir, treatment_dir, conc)

    envelope = {
        "slug": slug,
        "spec": asdict(spec),
        "baseline_dir": str(baseline_dir.relative_to(REPO_ROOT)),
        "treatment_dir": str(treatment_dir.relative_to(REPO_ROOT)),
        "concordance": conc,
        "balance_doc": str(doc.relative_to(REPO_ROOT)),
        "next_steps": [
            f"just verdict {treatment_dir.relative_to(REPO_ROOT)}/{spec.seeds[0]}-1",
            f"edit {doc.relative_to(REPO_ROOT)} and commit",
        ],
    }

    write_status(work_dir, {"slug": slug, "phase": "done", "concordance": conc})

    if args.text:
        v = conc["verdict"]
        sys.stdout.write(f"hypothesize: {v.upper()}  ({slug})\n")
        sys.stdout.write(f"  metric:    {spec.metric}\n")
        sys.stdout.write(f"  predicted: {spec.direction} ±{spec.rough_magnitude_pct[0]:.0f}–{spec.rough_magnitude_pct[1]:.0f}%\n")
        sys.stdout.write(f"  observed:  {conc['observed_direction']} {conc['observed_delta_pct']}%  "
                         f"p={conc['p_value']}  d={conc['effect_size']}\n")
        sys.stdout.write(f"  doc:       {doc.relative_to(REPO_ROOT)}\n")
    else:
        sys.stdout.write(json.dumps(envelope, indent=2) + "\n")

    exit_code = {
        "concordant": 0,
        "inconclusive": 1,
        "off-magnitude": 1,
        "drift": 1,
        "wrong-direction": 2,
    }.get(conc["verdict"], 1)
    append_call_history(tool="hypothesize", subtool=None, args=args,
                        rationale=args.rationale, exit_code=exit_code)
    return exit_code


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
