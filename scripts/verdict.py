#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""
One-call run validation for a Clowder soak.

Composes existing primitives — `check_canaries.sh`, `check_continuity.sh`,
`diff-constants` (jq), and a footer-vs-baseline drift summary — into a
single structured JSON envelope so a Claude Code turn can decide
pass/concern/fail in one tool call.

Replaces `just autoloop`. Reads the active baseline from
`logs/baselines/current.json` (Tier 2.2) when present; falls back to
`logs/baseline-pre-substrate-refactor/events.jsonl` for backwards compat.

Usage:
    just verdict <run-dir>
    just verdict <run-dir> --baseline <path-to-events.jsonl>
    just verdict <run-dir> --no-history    # don't append to verdict-history.jsonl
    just verdict <run-dir> --text          # human-readable summary instead of JSON

Exit codes: 0 pass, 1 concern, 2 fail.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from dataclasses import dataclass, field, asdict
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO_ROOT / "scripts"))
from _agent_call_log import append_call_history  # noqa: E402

LEGACY_BASELINE = REPO_ROOT / "logs" / "baseline-pre-substrate-refactor" / "events.jsonl"
BASELINES_DIR = REPO_ROOT / "logs" / "baselines"
HISTORY_PATH = REPO_ROOT / "logs" / "verdict-history.jsonl"

NOISE_PCT = 10.0
SIGNIFICANT_PCT = 30.0


@dataclass
class Verdict:
    run: str
    verdict: str  # pass | concern | fail
    canaries: dict[str, Any] = field(default_factory=dict)
    constants_drift_vs_baseline: str = "no-baseline"
    seed_match_vs_baseline: str = "no-baseline"  # match | mismatch | no-baseline
    footer_drift: list[dict[str, Any]] = field(default_factory=list)
    # Ticket 125: per-field numerical-delta channel for `_footer.colony_score`.
    # `None` when either side lacks the block (older baselines, or a run that
    # exited before the first ColonyScore emission). Each value is a dict of
    # `{baseline, observed, delta_pct, band}` keyed by colony-score field name.
    colony_score_drift: dict[str, dict[str, Any]] | None = None
    # Ticket 194 / P3: per-tick rate normalization for cross-run comparison
    # at unequal durations. `duration_drift_pct` is None when either side's
    # duration is unreadable; the overall verdict only escalates on rate-
    # band when this exceeds DURATION_DRIFT_PCT_THRESHOLD.
    baseline_duration_ticks: int | None = None
    observed_duration_ticks: int | None = None
    duration_drift_pct: float | None = None
    baseline: str | None = None
    commit: str | None = None
    seed: int | None = None
    next_steps: list[str] = field(default_factory=list)
    rationale: str | None = None


def find_events_log(run_dir: Path) -> Path:
    direct = run_dir / "events.jsonl"
    if direct.exists():
        return direct
    if run_dir.is_file() and run_dir.suffix == ".jsonl":
        return run_dir
    raise SystemExit(f"verdict: no events.jsonl found at {run_dir}")


def read_footer(events_path: Path) -> dict[str, Any]:
    proc = subprocess.run(
        ["jq", "-c", "select(._footer)", str(events_path)],
        capture_output=True, text=True,
    )
    if proc.returncode != 0:
        raise SystemExit(f"verdict: jq failed reading footer: {proc.stderr.strip()}")
    line = next((l for l in proc.stdout.splitlines() if l.strip()), "")
    if not line:
        return {}
    return json.loads(line)


def read_final_tick(events_path: Path) -> int | None:
    """Return the highest `tick` value in `events_path`, or None.

    The `_footer` line is always last and lacks `tick`. The second-to-last
    line is the last real event, so a tail-scan suffices in the common
    case; we widen if it doesn't carry a tick (e.g. a SystemActivation
    block right before footer).
    """
    try:
        proc = subprocess.run(
            ["bash", "-c",
             f"tail -n 200 {events_path!s} | jq -c 'select(.tick != null) | .tick' | tail -n 1"],
            capture_output=True, text=True,
        )
    except OSError:
        return None
    line = proc.stdout.strip()
    if not line:
        return None
    try:
        return int(line)
    except ValueError:
        return None


def run_duration_ticks(events_path: Path) -> int | None:
    """`final_tick - start_tick`, or None if either is unreadable."""
    start = read_header_field(events_path, ".start_tick")
    final = read_final_tick(events_path)
    if not isinstance(start, int) or not isinstance(final, int):
        return None
    delta = final - start
    return delta if delta > 0 else None


def read_header_field(events_path: Path, jq_expr: str) -> Any:
    proc = subprocess.run(
        ["jq", "-c", f"select(._header) | {jq_expr}", str(events_path)],
        capture_output=True, text=True,
    )
    if proc.returncode != 0:
        return None
    line = next((l for l in proc.stdout.splitlines() if l.strip()), "")
    if not line or line == "null":
        return None
    try:
        return json.loads(line)
    except json.JSONDecodeError:
        return line


def run_canary_script(script: Path, events_path: Path) -> tuple[str, str]:
    """Returns (status, raw_output). status ∈ {pass, fail, error}."""
    if not script.exists():
        return ("error", f"missing script: {script}")
    proc = subprocess.run(
        ["bash", str(script), str(events_path)],
        capture_output=True, text=True,
    )
    out = (proc.stdout + proc.stderr).strip()
    if proc.returncode == 0:
        return ("pass", out)
    return ("fail", out)


def resolve_baseline(explicit: str | None) -> Path | None:
    if explicit:
        p = Path(explicit)
        return p if p.exists() else None
    current = BASELINES_DIR / "current.json"
    if current.exists():
        try:
            ref = json.loads(current.read_text())
            p = Path(ref.get("events_path") or ref.get("path", ""))
            if p.exists():
                return p
        except (json.JSONDecodeError, OSError):
            pass
    if LEGACY_BASELINE.exists():
        return LEGACY_BASELINE
    return None


def constants_drift(baseline: Path, observed: Path) -> str:
    proc = subprocess.run(
        ["bash", "-c",
         f"diff <(jq -c 'select(._header) | .constants' {baseline!s}) "
         f"<(jq -c 'select(._header) | .constants' {observed!s})"],
        capture_output=True, text=True,
    )
    return "clean" if proc.returncode == 0 else "drift"


def seed_match(baseline: Path, observed: Path) -> tuple[str, int | None, int | None]:
    """Compare seeds between baseline and observed `events.jsonl` headers.

    Returns (status, baseline_seed, observed_seed). status ∈ {match, mismatch}.
    Seed mismatch means the per-metric drift readout is confounded with
    seed-level variance and the comparison is not a valid regression
    measurement — the caller should re-run on the baseline's seed.
    """
    b_seed = read_header_field(baseline, ".seed")
    o_seed = read_header_field(observed, ".seed")
    b_seed = b_seed if isinstance(b_seed, int) else None
    o_seed = o_seed if isinstance(o_seed, int) else None
    if b_seed is None or o_seed is None:
        return ("match", b_seed, o_seed)  # missing field — don't block
    return (("match" if b_seed == o_seed else "mismatch"), b_seed, o_seed)


_NUMERIC_FIELDS = (
    "wards_placed_total", "wards_despawned_total", "ward_count_final",
    "ward_avg_strength_final", "shadow_foxes_avoided_ward_total",
    "ward_siege_started_total", "shadow_fox_spawn_total",
    "anxiety_interrupt_total", "positive_features_active",
    "negative_events_total", "neutral_features_active",
)

# Ticket 194 / P3: count-style fields are rate-normalized per 10k ticks
# when run-durations differ; instantaneous fields (*_final / *_active /
# *_avg_*) are point-in-time, so a per-tick rate is meaningless on them.
# `deaths_by_cause.*` rows are always counts. The categorization is by
# field-name suffix so it stays in sync as new metrics land.
def _is_rate_normalizable(field_name: str) -> bool:
    if field_name.startswith("deaths_by_cause."):
        return True
    return not (
        field_name.endswith("_final")
        or field_name.endswith("_active")
        or "_avg_" in field_name
    )

DURATION_DRIFT_PCT_THRESHOLD = 10.0


def band(delta_pct: float) -> str:
    a = abs(delta_pct)
    if a < NOISE_PCT:
        return "noise"
    if a >= SIGNIFICANT_PCT:
        return "significant"
    return "drift"


def _rate_columns(field_name: str, b: float, o: float,
                  baseline_dur: int | None, observed_dur: int | None) -> dict[str, Any]:
    """Compute per-10kt rate delta for a count-style field. Empty dict when
    either duration is unknown or the field is instantaneous (point-in-time).
    """
    if not _is_rate_normalizable(field_name):
        return {}
    if not baseline_dur or not observed_dur:
        return {}
    rb = b / baseline_dur * 10_000.0
    ro = o / observed_dur * 10_000.0
    if rb == 0 and ro == 0:
        return {"rate_baseline": 0.0, "rate_observed": 0.0,
                "delta_pct_rate": 0.0, "band_rate": "noise"}
    if rb == 0:
        return {"rate_baseline": 0.0, "rate_observed": round(ro, 3),
                "delta_pct_rate": None, "band_rate": "new-nonzero"}
    delta_rate = (ro - rb) / rb * 100.0
    return {"rate_baseline": round(rb, 3), "rate_observed": round(ro, 3),
            "delta_pct_rate": round(delta_rate, 1), "band_rate": band(delta_rate)}


def footer_drift(baseline: dict[str, Any], observed: dict[str, Any],
                 baseline_dur: int | None = None,
                 observed_dur: int | None = None) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for field_name in _NUMERIC_FIELDS:
        b = baseline.get(field_name)
        o = observed.get(field_name)
        if not isinstance(b, (int, float)) or not isinstance(o, (int, float)):
            continue
        if b == 0 and o == 0:
            continue
        if b == 0:
            row = {"field": field_name, "baseline": b, "observed": o,
                   "delta_pct": None, "band": "new-nonzero"}
        else:
            delta = (o - b) / b * 100.0
            row = {"field": field_name, "baseline": b, "observed": o,
                   "delta_pct": round(delta, 1), "band": band(delta)}
        row.update(_rate_columns(field_name, float(b), float(o),
                                 baseline_dur, observed_dur))
        rows.append(row)

    for cause in set((baseline.get("deaths_by_cause") or {}).keys()
                     | (observed.get("deaths_by_cause") or {}).keys()):
        b = (baseline.get("deaths_by_cause") or {}).get(cause, 0)
        o = (observed.get("deaths_by_cause") or {}).get(cause, 0)
        if b == 0 and o == 0:
            continue
        field_name = f"deaths_by_cause.{cause}"
        delta = None if b == 0 else round((o - b) / b * 100.0, 1)
        row = {
            "field": field_name,
            "baseline": b, "observed": o,
            "delta_pct": delta,
            "band": "new-nonzero" if b == 0 else band(delta),
        }
        row.update(_rate_columns(field_name, float(b), float(o),
                                 baseline_dur, observed_dur))
        rows.append(row)

    rows.sort(key=lambda r: -abs(r["delta_pct"]) if r["delta_pct"] is not None else -1e9)
    return rows[:20]


def derive_overall(canary_survival: str, canary_continuity: str,
                   constants: str, drift: list[dict[str, Any]],
                   colony_score: dict[str, dict[str, Any]] | None,
                   duration_drift_pct: float | None = None) -> str:
    if canary_survival == "fail":
        return "fail"
    if canary_continuity == "fail" or constants == "drift":
        return "concern"
    if any(r["band"] == "significant" for r in drift):
        return "concern"
    # Ticket 194 / P3: when durations diverge enough that raw counts are
    # misleading, escalate on the rate band too. The raw band stays
    # primary so equal-duration runs keep behaving as before.
    if (duration_drift_pct is not None
            and duration_drift_pct > DURATION_DRIFT_PCT_THRESHOLD
            and any(r.get("band_rate") == "significant" for r in drift)):
        return "concern"
    # Ticket 125: aggregate-only drift escalates to concern but never to
    # fail — canaries gate hard, this is a continuous-health lens. The
    # gap this closes is "all canaries green but aggregate moved 30%."
    if colony_score:
        for axis in ("aggregate", "welfare"):
            row = colony_score.get(axis)
            if row and row.get("band") in ("concern", "fail"):
                return "concern"
    return "pass"


# Ticket 125: per-field numerical-delta surface for `_footer.colony_score`.
# Bucket boundaries differ from `footer_drift`'s NOISE/SIGNIFICANT bands
# because aggregate is a continuous health signal, not a count metric:
# small drift is normal noise, mid drift wants a hypothesis, large drift
# is a regression signal worth surfacing even with green canaries.
COLONY_SCORE_FIELDS: tuple[str, ...] = (
    "aggregate", "welfare",
    "shelter", "nourishment", "health", "happiness", "fulfillment",
    "seasons_survived", "peak_population",
    "kittens_born", "kittens_surviving",
    "structures_built", "bonds_formed",
    "deaths_starvation", "deaths_old_age", "deaths_injury",
)
COLONY_SCORE_PASS_PCT = 5.0
COLONY_SCORE_CONCERN_PCT = 15.0


def colony_score_band(delta_pct: float) -> str:
    a = abs(delta_pct)
    if a <= COLONY_SCORE_PASS_PCT:
        return "pass"
    if a <= COLONY_SCORE_CONCERN_PCT:
        return "concern"
    return "fail"


def colony_score_drift(baseline: dict[str, Any],
                       observed: dict[str, Any]) -> dict[str, dict[str, Any]] | None:
    """Per-field numerical drift on `_footer.colony_score`.

    Returns `None` if either side lacks the block (older baseline, or a
    run that exited before first emission). Returns an empty dict only
    when both blocks exist but contain no comparable numeric fields.
    """
    b_block = baseline.get("colony_score")
    o_block = observed.get("colony_score")
    if not isinstance(b_block, dict) or not isinstance(o_block, dict):
        return None

    rows: dict[str, dict[str, Any]] = {}
    for field_name in COLONY_SCORE_FIELDS:
        b = b_block.get(field_name)
        o = o_block.get(field_name)
        if not isinstance(b, (int, float)) or not isinstance(o, (int, float)):
            continue
        if b == 0 and o == 0:
            rows[field_name] = {"baseline": b, "observed": o,
                                "delta_pct": 0.0, "band": "pass"}
            continue
        if b == 0:
            rows[field_name] = {"baseline": b, "observed": o,
                                "delta_pct": None, "band": "new-nonzero"}
            continue
        delta = (o - b) / b * 100.0
        rows[field_name] = {
            "baseline": b, "observed": o,
            "delta_pct": round(delta, 1),
            "band": colony_score_band(delta),
        }
    return rows


def derive_next_steps(v: Verdict, run_dir: Path, footer: dict[str, Any]) -> list[str]:
    steps: list[str] = []
    if v.canaries.get("survival") == "fail":
        causes = list((footer.get("deaths_by_cause") or {}).keys())
        if causes:
            steps.append(f"just q deaths {run_dir} --cause={causes[0]}")
        else:
            steps.append(f"just q deaths {run_dir}")
    if v.canaries.get("continuity") == "fail":
        steps.append(f"just q anomalies {run_dir}")
    if v.constants_drift_vs_baseline == "drift":
        if v.baseline:
            steps.append(f"just diff-constants {v.baseline} {run_dir}/events.jsonl")
    sig = [r for r in v.footer_drift if r["band"] == "significant"]
    if sig:
        steps.append(f"just q events {run_dir} --type=Death")
    # Ticket 194 / P3: when run-durations diverge enough that raw counts
    # are misleading, point the caller at the rate-band view (which is
    # already in the JSON envelope per row as `delta_pct_rate`).
    if (v.duration_drift_pct is not None
            and v.duration_drift_pct > DURATION_DRIFT_PCT_THRESHOLD):
        rate_sig = [r for r in v.footer_drift
                    if r.get("band_rate") == "significant"]
        if rate_sig:
            top = rate_sig[0]
            steps.append(
                f"durations differ {v.duration_drift_pct:.1f}% — compare on "
                f"rate: {top['field']} {top.get('delta_pct_rate', 0):+.1f}% "
                "per 10kt (raw delta is duration-confounded)"
            )
    # Ticket 125: name colony_score axes that moved out of band so the
    # caller can decide whether the drift is intentional (file a hypothesis)
    # or a regression (bisect-canary on the moved axis).
    if v.colony_score_drift:
        notable = [
            (axis, row) for axis, row in v.colony_score_drift.items()
            if row.get("band") in ("concern", "fail")
        ]
        if notable:
            notable.sort(key=lambda kv: -abs(kv[1].get("delta_pct") or 0.0))
            top = ", ".join(
                f"{axis} {row['delta_pct']:+.1f}%" for axis, row in notable[:3]
            )
            steps.append(
                f"colony_score drift: {top} — file a hypothesis if intentional, "
                f"`just bisect-canary <axis>` if not"
            )
    return steps


def append_history(v: Verdict) -> None:
    HISTORY_PATH.parent.mkdir(parents=True, exist_ok=True)
    with HISTORY_PATH.open("a") as f:
        f.write(json.dumps(asdict(v), default=str) + "\n")


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("run_dir", help="Path to a run directory (containing events.jsonl) or events.jsonl directly")
    ap.add_argument("--baseline", default=None, help="Override baseline events.jsonl path")
    ap.add_argument("--no-history", action="store_true", help="Don't append to logs/verdict-history.jsonl")
    ap.add_argument("--text", action="store_true", help="Human-readable output (JSON is default)")
    ap.add_argument("--rationale", default=None,
                    help="Why this verdict was requested (free text). Appended to "
                         "logs/agent-call-history.jsonl alongside the verdict; lets "
                         "future review surface patterns of what callers were trying "
                         "to figure out. Always pass when invoked by an agent.")
    args = ap.parse_args(argv)

    run_dir = Path(args.run_dir)
    events_path = find_events_log(run_dir)
    footer = read_footer(events_path)
    if not footer:
        v = Verdict(
            run=str(run_dir), verdict="fail",
            canaries={"survival": "fail", "continuity": "skip", "detail": "no footer line"},
            next_steps=[f"check that {events_path} contains a `_footer` JSONL line"],
            rationale=args.rationale,
        )
        _emit(v, args.text)
        append_call_history(tool="verdict", subtool=None, args=args,
                            rationale=args.rationale, exit_code=2,
                            commit=v.commit)
        return 2

    surv_status, _ = run_canary_script(REPO_ROOT / "scripts" / "check_canaries.sh", events_path)
    cont_status, cont_out = run_canary_script(REPO_ROOT / "scripts" / "check_continuity.sh", events_path)

    cont_detail: list[str] = []
    if cont_status == "fail":
        for line in cont_out.splitlines():
            if "[FAIL]" in line:
                parts = line.split()
                if len(parts) >= 3:
                    cont_detail.append(f"{parts[1]}={parts[2]}")

    baseline_path = resolve_baseline(args.baseline)
    constants_status = "no-baseline"
    seed_status = "no-baseline"
    baseline_seed: int | None = None
    drift_rows: list[dict[str, Any]] = []
    cs_drift: dict[str, dict[str, Any]] | None = None
    baseline_dur: int | None = None
    observed_dur = run_duration_ticks(events_path)
    duration_drift_pct: float | None = None
    if baseline_path:
        constants_status = constants_drift(baseline_path, events_path)
        seed_status, baseline_seed, _ = seed_match(baseline_path, events_path)
        baseline_footer = read_footer(baseline_path)
        baseline_dur = run_duration_ticks(baseline_path)
        if baseline_footer:
            drift_rows = footer_drift(baseline_footer, footer,
                                      baseline_dur, observed_dur)
            cs_drift = colony_score_drift(baseline_footer, footer)
        if baseline_dur and observed_dur:
            duration_drift_pct = round(
                abs(observed_dur - baseline_dur) / baseline_dur * 100.0, 1)

    overall = derive_overall(surv_status, cont_status, constants_status,
                             drift_rows, cs_drift, duration_drift_pct)
    # Seed mismatch is a comparability failure: the drift table is bogus
    # because we're comparing different control worlds. Downgrade the
    # verdict (but never below the survival/continuity verdict) and let
    # `derive_next_steps` surface the re-run instruction.
    if seed_status == "mismatch" and overall == "pass":
        overall = "concern"
    commit = read_header_field(events_path, ".commit_hash_short")
    observed_seed = read_header_field(events_path, ".seed")
    observed_seed = observed_seed if isinstance(observed_seed, int) else None

    v = Verdict(
        run=str(run_dir),
        verdict=overall,
        canaries={
            "survival": surv_status,
            "continuity": cont_status if not cont_detail else f"fail:{','.join(cont_detail)}",
        },
        constants_drift_vs_baseline=constants_status,
        seed_match_vs_baseline=seed_status,
        footer_drift=drift_rows,
        colony_score_drift=cs_drift,
        baseline_duration_ticks=baseline_dur,
        observed_duration_ticks=observed_dur,
        duration_drift_pct=duration_drift_pct,
        baseline=str(baseline_path) if baseline_path else None,
        commit=commit if isinstance(commit, str) else None,
        seed=observed_seed,
        rationale=args.rationale,
    )
    v.next_steps = derive_next_steps(v, run_dir, footer)
    if seed_status == "mismatch" and baseline_seed is not None and observed_seed is not None:
        v.next_steps.insert(
            0,
            f"baseline seed={baseline_seed} but run seed={observed_seed}; re-run with --seed {baseline_seed} or pass an explicit baseline that matches",
        )

    if not args.no_history:
        append_history(v)

    _emit(v, args.text)

    exit_code = {"pass": 0, "concern": 1, "fail": 2}[overall]
    append_call_history(tool="verdict", subtool=None, args=args,
                        rationale=args.rationale, exit_code=exit_code,
                        commit=v.commit)
    return exit_code


def _emit(v: Verdict, text_mode: bool) -> None:
    if text_mode:
        sys.stdout.write(_text(v) + "\n")
    else:
        sys.stdout.write(json.dumps(asdict(v), indent=2, default=str) + "\n")


def _text(v: Verdict) -> str:
    lines = [f"verdict: {v.verdict.upper()}  ({v.run})"]
    if v.commit:
        lines.append(f"  commit:    {v.commit}")
    lines.append(f"  survival:  {v.canaries.get('survival', '?')}")
    lines.append(f"  continuity: {v.canaries.get('continuity', '?')}")
    lines.append(f"  constants: {v.constants_drift_vs_baseline}"
                 + (f"  (baseline={v.baseline})" if v.baseline else ""))
    if v.seed_match_vs_baseline != "no-baseline":
        seed_disp = "" if v.seed is None else f"  (seed={v.seed})"
        lines.append(f"  seed:      {v.seed_match_vs_baseline}{seed_disp}")
    if v.duration_drift_pct is not None and v.baseline_duration_ticks and v.observed_duration_ticks:
        lines.append(
            f"  duration:  {v.observed_duration_ticks:,} vs {v.baseline_duration_ticks:,} ticks "
            f"({v.duration_drift_pct:+.1f}%)"
        )
    show_rate = (v.duration_drift_pct is not None
                 and v.duration_drift_pct > DURATION_DRIFT_PCT_THRESHOLD)
    if v.footer_drift:
        lines.append("  footer drift (top):")
        for r in v.footer_drift[:5]:
            d = r["delta_pct"]
            d_s = "  new" if d is None else f"{d:+5.1f}%"
            line = f"    {r['band']:11s} {d_s}  {r['field']} ({r['baseline']} → {r['observed']})"
            if show_rate and r.get("delta_pct_rate") is not None:
                line += f"  [rate {r['delta_pct_rate']:+5.1f}% / 10kt]"
            lines.append(line)
    if v.colony_score_drift:
        # Headline two axes (aggregate + welfare) plus the top out-of-band
        # axis if any. Keeps the text mode terse; full per-field readout is
        # in the JSON envelope.
        lines.append("  colony_score drift:")
        for axis in ("aggregate", "welfare"):
            row = v.colony_score_drift.get(axis)
            if row:
                d = row["delta_pct"]
                d_s = "  new" if d is None else f"{d:+5.1f}%"
                lines.append(f"    {row['band']:8s} {d_s}  {axis} ({row['baseline']} → {row['observed']})")
        notable = [
            (a, r) for a, r in v.colony_score_drift.items()
            if a not in ("aggregate", "welfare") and r.get("band") in ("concern", "fail")
        ]
        if notable:
            notable.sort(key=lambda kv: -abs(kv[1].get("delta_pct") or 0.0))
            for axis, row in notable[:2]:
                d = row["delta_pct"]
                d_s = "  new" if d is None else f"{d:+5.1f}%"
                lines.append(f"    {row['band']:8s} {d_s}  {axis} ({row['baseline']} → {row['observed']})")
    if v.next_steps:
        lines.append("  next:")
        for s in v.next_steps:
            lines.append(f"    $ {s}")
    return "\n".join(lines)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
