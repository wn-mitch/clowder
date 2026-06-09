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
    verdict: str  # pass | unprovable | concern | fail
    canaries: dict[str, Any] = field(default_factory=dict)
    constants_drift_vs_baseline: str = "no-baseline"
    seed_match_vs_baseline: str = "no-baseline"  # match | mismatch | no-baseline
    footer_drift: list[dict[str, Any]] = field(default_factory=list)
    # Ticket 125: per-field numerical-delta channel for `_footer.colony_score`.
    # `None` when either side lacks the block (older baselines, or a run that
    # exited before the first ColonyScore emission). Each value is a dict of
    # `{baseline, observed, delta_pct, band}` keyed by colony-score field name.
    colony_score_drift: dict[str, dict[str, Any]] | None = None
    # Which footer block backed `colony_score_drift`. "checkpoint" = the
    # TPS-invariant fixed-elapsed-tick capture (preferred); "end_of_run" =
    # legacy surface, confounded with binary throughput on wall-clock
    # soaks. None when there is no drift readout at all.
    colony_score_surface: str | None = None
    # Ticket 490: founder-dispersion windows (post-spawn) whose mean
    # distance-to-centroid fell below the absolute floor — the cuddle-
    # puddle spatial signature. `None` when the footer lacks the block
    # (pre-490 archives); `[]` when present and healthy. Absolute-floor
    # (not baseline-relative) so the canary is live before any baseline
    # carries the field.
    founder_dispersion_low: list[dict[str, Any]] | None = None
    # Perf epic 480: throughput-vs-baseline channel. `None` when the runs
    # are incomparable (different duration budgets AND either side lacks
    # the `ticks_per_sec` footer field). Degradation-only gating: bands
    # are wide because single-run throughput is noisy under parallel-
    # session CPU contention; escalates to concern at most, never fail.
    throughput_drift: dict[str, Any] | None = None
    # Ticket 194 / P3: per-tick rate normalization for cross-run comparison
    # at unequal durations. `duration_drift_pct` is None when either side's
    # duration is unreadable; the overall verdict only escalates on rate-
    # band when this exceeds DURATION_DRIFT_PCT_THRESHOLD.
    baseline_duration_ticks: int | None = None
    observed_duration_ticks: int | None = None
    duration_drift_pct: float | None = None
    # Ticket 196: per-Feature fire counts when the caller passed
    # `--require-feature <name>`. A run that's otherwise pass but has a
    # required Feature at 0 is "unprovable" — the run cannot evaluate the
    # hypothesis the caller is asking about.
    features_fired: dict[str, int] | None = None
    # Ticket 396: rows from `plan_failure_canary()` — keys whose per-tick
    # failure rate jumped sharply vs baseline. Empty when no baseline,
    # durations unreadable, or no key crosses the thresholds.
    plan_failure_canary: list[dict[str, Any]] = field(default_factory=list)
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
    footer = json.loads(line)
    # Footer back-compat: legacy footers serialized `kittens_surviving`;
    # current ones emit `kittens_matured`. Normalize so drift logic + any
    # other consumer sees the new name regardless of vintage.
    cs = footer.get("colony_score")
    if isinstance(cs, dict) and "kittens_matured" not in cs and "kittens_surviving" in cs:
        cs["kittens_matured"] = cs["kittens_surviving"]
    return footer


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
    """Elapsed ticks for rate-per-tick arithmetic.

    Prefers the footer's `elapsed_ticks` field (ticket 410 — self-
    contained, no header round-trip required). Falls back to
    `(final_tick - start_tick)` via header lookup + event scan so older
    archives written before the footer enrichment landed continue to
    work.

    **Invariant for callers:** rate = count / this value. Never divide
    by `final_tick` alone — runs start at absolute tick ≈ 1,200,000
    (`60 * ticks_per_season`, see `project_clowder_tick_origin` /
    CLAUDE.md "ticks on disk are absolute"). For a typical 15-min soak
    `final_tick ≈ 1,295,000` and `elapsed_ticks ≈ 95,000` — dividing by
    `final_tick` under-counts the rate by ~13.6× and looks like a real
    regression. Ticket 410 surfaced this footer-rate "freakout" pattern.
    """
    footer = read_footer(events_path)
    elapsed = footer.get("elapsed_ticks")
    if isinstance(elapsed, int) and elapsed > 0:
        return elapsed
    start = read_header_field(events_path, ".start_tick")
    final = read_final_tick(events_path)
    if not isinstance(start, int) or not isinstance(final, int):
        return None
    delta = final - start
    return delta if delta > 0 else None


def read_last_system_activation(events_path: Path) -> dict[str, Any] | None:
    """Return the last `SystemActivation` event in `events_path`, or None.

    SystemActivation events emit periodic cumulative counts of every
    `Feature::*` variant (split into `positive` / `negative` / `neutral`).
    The last one before the footer is the run-total — the cheapest
    structured place to read per-Feature counts, since the footer only
    surfaces aggregates and `never_fired_expected_positives`.

    Reads the file in chunks from the end so we touch only ~16 KB
    instead of the multi-MB whole file. (No `tac` on macOS.)
    """
    try:
        size = events_path.stat().st_size
    except OSError:
        return None
    if size == 0:
        return None
    needle = b'"type":"SystemActivation"'
    chunk_size = 16 * 1024
    try:
        with events_path.open("rb") as f:
            tail = b""
            while size > 0:
                read_at = max(0, size - chunk_size)
                f.seek(read_at)
                buf = f.read(size - read_at) + tail
                size = read_at
                idx = buf.rfind(needle)
                if idx == -1:
                    # Keep at most one full line of overlap so a needle
                    # split across chunks still matches next iteration.
                    nl = buf.find(b"\n")
                    tail = buf[: nl + 1] if 0 <= nl < len(needle) else buf[: len(needle)]
                    continue
                line_start = buf.rfind(b"\n", 0, idx) + 1
                line_end = buf.find(b"\n", idx)
                if line_end == -1:
                    line = buf[line_start:]
                else:
                    line = buf[line_start:line_end]
                try:
                    return json.loads(line.decode("utf-8"))
                except (UnicodeDecodeError, json.JSONDecodeError):
                    return None
    except OSError:
        return None
    return None


def feature_counts_for(activation: dict[str, Any], names: list[str]) -> dict[str, int]:
    """Look up `names` in a SystemActivation event's count maps.

    Each Feature lives in exactly one of `positive` / `negative` /
    `neutral` — the maps don't overlap. A name that doesn't appear in
    any map returns 0 (Feature might not be classified, or might be
    legitimately rare and never fired).
    """
    out: dict[str, int] = {}
    sources = (
        activation.get("positive") or {},
        activation.get("negative") or {},
        activation.get("neutral") or {},
    )
    for name in names:
        count = 0
        for src in sources:
            value = src.get(name)
            if isinstance(value, (int, float)):
                count = int(value)
                break
        out[name] = count
    return out


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

# Ticket 396: plan-failure rate canary. Iterates `plan_failures_by_reason`,
# `planning_failures_by_reason`, and `interrupts_by_reason` (footer dicts
# the existing top-level field scan can't see) and flags keys whose
# per-tick rate is either >=10x the baseline rate (with a floor to avoid
# noise on rare keys) or new vs baseline above a higher absolute floor.
# Defaults chosen so the 394 Wean regression (0 -> 2439 over ~125000 ticks
# -> 0.019/tick) flags clearly while small per-run variance doesn't.
PLAN_FAILURE_DICTS = (
    "plan_failures_by_reason",
    "planning_failures_by_reason",
    "interrupts_by_reason",
)
PLAN_FAILURE_RATIO_THRESHOLD = 10.0
PLAN_FAILURE_RATIO_FLOOR = 0.001
PLAN_FAILURE_NEW_KEY_FLOOR = 0.005


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


def plan_failure_canary(baseline: dict[str, Any], observed: dict[str, Any],
                        baseline_dur: int | None,
                        observed_dur: int | None) -> list[dict[str, Any]]:
    """Flag plan-failure-reason keys whose per-tick rate jumped sharply.

    Iterates `PLAN_FAILURE_DICTS` in both footers. For each reason:
    - New vs baseline (`baseline = 0`) above `PLAN_FAILURE_NEW_KEY_FLOOR`
      ticks per emission -> `band: new-high-rate`.
    - Ratio `rate_observed / rate_baseline >= PLAN_FAILURE_RATIO_THRESHOLD`
      AND `rate_observed >= PLAN_FAILURE_RATIO_FLOOR` -> `band: high-rate-ratio`.

    Skips when durations are unavailable (rate normalization required).
    Skips when observed count is 0 (a regression elsewhere drove the
    reason to zero - not a plan-failure regression).
    """
    rows: list[dict[str, Any]] = []
    if not baseline_dur or not observed_dur:
        return rows
    for dict_name in PLAN_FAILURE_DICTS:
        b_dict = baseline.get(dict_name) or {}
        o_dict = observed.get(dict_name) or {}
        if not isinstance(b_dict, dict) or not isinstance(o_dict, dict):
            continue
        for reason in set(b_dict.keys()) | set(o_dict.keys()):
            b = b_dict.get(reason, 0)
            o = o_dict.get(reason, 0)
            if not isinstance(b, (int, float)) or not isinstance(o, (int, float)):
                continue
            if o == 0:
                continue
            rate_b = b / baseline_dur
            rate_o = o / observed_dur
            if b == 0:
                if rate_o >= PLAN_FAILURE_NEW_KEY_FLOOR:
                    rows.append({
                        "dict": dict_name,
                        "reason": reason,
                        "baseline": b,
                        "observed": o,
                        "rate_baseline": 0.0,
                        "rate_observed": round(rate_o, 5),
                        "ratio": None,
                        "band": "new-high-rate",
                    })
            else:
                ratio = rate_o / rate_b
                if ratio >= PLAN_FAILURE_RATIO_THRESHOLD and rate_o >= PLAN_FAILURE_RATIO_FLOOR:
                    rows.append({
                        "dict": dict_name,
                        "reason": reason,
                        "baseline": b,
                        "observed": o,
                        "rate_baseline": round(rate_b, 5),
                        "rate_observed": round(rate_o, 5),
                        "ratio": round(ratio, 1),
                        "band": "high-rate-ratio",
                    })
    # Sort by severity: ratio rows by descending ratio, new-high-rate by
    # descending observed rate. Ratio rows first since they have a direct
    # comparison anchor; new keys are listed after.
    rows.sort(key=lambda r: (
        0 if r["band"] == "high-rate-ratio" else 1,
        -(r["ratio"] or 0.0),
        -r["rate_observed"],
    ))
    return rows


def derive_overall(canary_survival: str, canary_continuity: str,
                   constants: str, drift: list[dict[str, Any]],
                   colony_score: dict[str, dict[str, Any]] | None,
                   duration_drift_pct: float | None = None,
                   plan_failure_canary: list[dict[str, Any]] | None = None,
                   throughput: dict[str, Any] | None = None,
                   dispersion_low: list[dict[str, Any]] | None = None) -> str:
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
    # Ticket 396: any flagged plan-failure-rate row escalates to concern.
    # The gap this closes is "substrate ships a regression that absorbs
    # silently via plan_failures_by_reason churn" — the 364→394 Wean
    # 0→2439 jump (no deaths, welfare improved) would have caught here.
    if plan_failure_canary:
        return "concern"
    # Perf epic 480: throughput degradation escalates to concern at most,
    # never fail — single-run TPS can be faked by parallel-session CPU
    # contention. The gap this closes is "stairstep perf regression that
    # only surfaces weeks later in the LOESS chart."
    if throughput and throughput.get("band") in ("concern", "strong-concern"):
        return "concern"
    # Ticket 490: founders huddling below the dispersion floor is the
    # cuddle-puddle signature — invisible to every event-count gate.
    if dispersion_low:
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
    "kittens_born", "kittens_matured",
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


def select_colony_score_blocks(
        baseline: dict[str, Any],
        observed: dict[str, Any]) -> tuple[Any, Any, str]:
    """Pick the colony-score comparison surface.

    Prefers `colony_score_at_checkpoint` when BOTH footers carry a
    non-null block captured at the SAME `checkpoint_constant` — that
    surface is TPS-invariant (frozen at a fixed elapsed sim-tick), so it
    isn't confounded by binary throughput on wall-clock soaks. Falls
    back to the end-of-run `colony_score` block otherwise (legacy
    baselines, runs that died before the mark, or mismatched constants).
    """
    b_cp = baseline.get("colony_score_at_checkpoint")
    o_cp = observed.get("colony_score_at_checkpoint")
    if (isinstance(b_cp, dict) and isinstance(o_cp, dict)
            and b_cp.get("checkpoint_constant") is not None
            and b_cp.get("checkpoint_constant") == o_cp.get("checkpoint_constant")):
        return b_cp, o_cp, "checkpoint"
    return baseline.get("colony_score"), observed.get("colony_score"), "end_of_run"


def colony_score_drift(baseline: dict[str, Any],
                       observed: dict[str, Any]) -> dict[str, dict[str, Any]] | None:
    """Per-field numerical drift on the selected colony-score surface.

    Surface selection per `select_colony_score_blocks` (checkpoint
    preferred, end-of-run fallback). Returns `None` if either side lacks
    the selected block (older baseline, or a run that exited before
    first emission). Returns an empty dict only when both blocks exist
    but contain no comparable numeric fields.
    """
    b_block, o_block, _surface = select_colony_score_blocks(baseline, observed)
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


# Ticket 490: founder-dispersion absolute floor. Healthy early-game
# founder spread is ~24 tiles mean dist-to-centroid; the cuddle-puddle
# regression collapsed it to ~4.7. Window 0 (elapsed < 3000) contains
# the spawn clump (~1.3 tiles, expected) and is skipped.
FOUNDER_DISPERSION_FLOOR_TILES = 10.0
FOUNDER_DISPERSION_SKIP_BEFORE_ELAPSED = 3_000


def founder_dispersion_low(footer: dict[str, Any]) -> list[dict[str, Any]] | None:
    """Windows below the dispersion floor. None = field absent (old run)."""
    rows = footer.get("founder_dispersion")
    if not isinstance(rows, list):
        return None
    flagged: list[dict[str, Any]] = []
    for r in rows:
        if not isinstance(r, dict):
            continue
        start = r.get("window_start_elapsed", 0)
        if not isinstance(start, (int, float)) \
                or start < FOUNDER_DISPERSION_SKIP_BEFORE_ELAPSED:
            continue
        md = r.get("mean_dist")
        if isinstance(md, (int, float)) and md < FOUNDER_DISPERSION_FLOOR_TILES:
            flagged.append(r)
    return flagged


# Perf epic 480: throughput drift bands. Wide and degradation-only —
# a single run's wall-clock throughput is confounded by parallel-session
# CPU contention (the reason ticket 480 tracks p90 across runs, not
# single samples). A 15% single-run dip is plausible contention noise;
# a 40% dip is almost certainly real. Improvements never gate.
THROUGHPUT_CONCERN_PCT = 15.0
THROUGHPUT_STRONG_PCT = 40.0


def throughput_band(delta_pct: float) -> str:
    if delta_pct >= -THROUGHPUT_CONCERN_PCT:
        return "pass"
    if delta_pct >= -THROUGHPUT_STRONG_PCT:
        return "concern"
    return "strong-concern"


def throughput_drift(baseline: dict[str, Any], observed: dict[str, Any],
                     baseline_duration_secs: Any,
                     observed_duration_secs: Any) -> dict[str, Any] | None:
    """Throughput-vs-baseline comparison (perf epic 480).

    Metric selection, in preference order:
    1. `ticks_per_sec` when both footers carry it — the honest instrument
       (measured wall-clock, robust to wipeout-shortened runs).
    2. `elapsed_ticks` when both runs had the same `duration_secs` budget
       — ticks-done-in-a-fixed-wall-budget, works against every archive
       baseline written before `ticks_per_sec` landed.
    Returns `None` when neither applies (incomparable).
    """
    b_tps = baseline.get("ticks_per_sec")
    o_tps = observed.get("ticks_per_sec")
    if isinstance(b_tps, (int, float)) and isinstance(o_tps, (int, float)) \
            and b_tps > 0 and o_tps > 0:
        metric, b, o = "ticks_per_sec", float(b_tps), float(o_tps)
    else:
        b_et = baseline.get("elapsed_ticks")
        o_et = observed.get("elapsed_ticks")
        durations_match = (
            isinstance(baseline_duration_secs, (int, float))
            and isinstance(observed_duration_secs, (int, float))
            and baseline_duration_secs == observed_duration_secs
        )
        if durations_match and isinstance(b_et, int) and isinstance(o_et, int) \
                and b_et > 0 and o_et > 0:
            metric, b, o = "elapsed_ticks", float(b_et), float(o_et)
        else:
            return None
    delta = (o - b) / b * 100.0
    return {
        "metric": metric,
        "baseline": round(b, 1),
        "observed": round(o, 1),
        "delta_pct": round(delta, 1),
        "band": throughput_band(delta),
    }


def derive_next_steps(v: Verdict, run_dir: Path, footer: dict[str, Any]) -> list[str]:
    steps: list[str] = []
    # Ticket 196: unprovable means the run is structurally incapable of
    # evaluating the hypothesis the caller is asking about. Surface which
    # required Features fired 0× so the caller knows what to fix
    # (longer soak, different scenario, etc.).
    if v.features_fired:
        zero = [name for name, count in v.features_fired.items() if count == 0]
        if zero:
            steps.append(
                "required Features fired 0×: "
                + ", ".join(zero)
                + " — increase soak duration or pick a scenario that exercises them"
            )
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
    # Ticket 396: name plan-failure reasons whose rate jumped sharply.
    # Surfaces the substrate-regression class that survival + continuity
    # gates absorb silently (the 394 Wean 0->2439 case).
    if v.plan_failure_canary:
        top = v.plan_failure_canary[0]
        if top["band"] == "high-rate-ratio":
            shape = (f"{top['reason']}: rate {top['rate_observed']}/tick "
                     f"({top['ratio']}x baseline)")
        else:
            shape = (f"{top['reason']}: new at "
                     f"{top['rate_observed']}/tick (baseline 0)")
        rest = ""
        if len(v.plan_failure_canary) > 1:
            rest = f" + {len(v.plan_failure_canary) - 1} more"
        steps.append(
            f"plan-failure rate regression: {shape}{rest} - "
            f"`just q events {run_dir} _footer` to inspect the dict"
        )
    # Ticket 490: name the huddled windows so the caller can jump to the
    # spatial drill (position scan / trace) instead of re-deriving it.
    if v.founder_dispersion_low:
        worst = min(v.founder_dispersion_low,
                    key=lambda r: r.get("mean_dist", 0.0))
        steps.append(
            f"founder dispersion below {FOUNDER_DISPERSION_FLOOR_TILES:.0f}-tile floor in "
            f"{len(v.founder_dispersion_low)} window(s) (worst "
            f"{worst.get('mean_dist', 0):.1f} tiles at elapsed "
            f"{worst.get('window_start_elapsed', '?')}) — cuddle-puddle "
            f"signature; see ticket 490, check bond graph + social-warmth pull"
        )
    # Perf epic 480: name a throughput degradation and route the caller
    # to the contention-vs-real disambiguation step before they burn a
    # bisect on machine noise.
    if v.throughput_drift and v.throughput_drift.get("band") in ("concern", "strong-concern"):
        t = v.throughput_drift
        steps.append(
            f"throughput {t['delta_pct']:+.1f}% on {t['metric']} "
            f"({t['baseline']} → {t['observed']}) — re-run on an idle machine "
            f"or `just sweep-stats` across seeds to rule out contention; "
            f"if real, flamegraph + open a 480-child ticket"
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
        # The end-of-run surface is TPS-confounded (welfare × seasons and
        # the ledger both grow with elapsed sim-time on a wall-clock
        # budget). Flag the fallback so a score delta isn't mistaken for
        # behavior change when it might be binary speed.
        if v.colony_score_surface == "end_of_run":
            steps.append(
                "colony_score compared on the end-of-run surface "
                "(TPS-confounded) — promote a baseline that carries "
                "colony_score_at_checkpoint for a sim-time-invariant comparison"
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
    ap.add_argument("--require-feature", dest="require_feature", action="append",
                    default=[], metavar="NAME",
                    help="Require that Feature::<NAME> fired ≥ 1× in the run. "
                         "Repeatable. If any required Feature has count 0, the "
                         "verdict becomes `unprovable` (exit 3) — the run is "
                         "structurally incapable of evaluating the hypothesis "
                         "the caller is asking about. The footer-vs-baseline "
                         "drift readout still computes; it just isn't authoritative.")
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
    cs_surface: str | None = None
    tp_drift: dict[str, Any] | None = None
    plan_failure_rows: list[dict[str, Any]] = []
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
            if cs_drift is not None:
                _, _, cs_surface = select_colony_score_blocks(baseline_footer, footer)
            tp_drift = throughput_drift(
                baseline_footer, footer,
                read_header_field(baseline_path, ".duration_secs"),
                read_header_field(events_path, ".duration_secs"))
            plan_failure_rows = plan_failure_canary(
                baseline_footer, footer, baseline_dur, observed_dur)
        if baseline_dur and observed_dur:
            duration_drift_pct = round(
                abs(observed_dur - baseline_dur) / baseline_dur * 100.0, 1)

    dispersion_rows = founder_dispersion_low(footer)

    overall = derive_overall(surv_status, cont_status, constants_status,
                             drift_rows, cs_drift, duration_drift_pct,
                             plan_failure_rows, tp_drift, dispersion_rows)
    # Seed mismatch is a comparability failure: the drift table is bogus
    # because we're comparing different control worlds. Downgrade the
    # verdict (but never below the survival/continuity verdict) and let
    # `derive_next_steps` surface the re-run instruction.
    if seed_status == "mismatch" and overall == "pass":
        overall = "concern"

    features_fired: dict[str, int] | None = None
    if args.require_feature:
        activation = read_last_system_activation(events_path)
        if activation is None:
            features_fired = {name: 0 for name in args.require_feature}
        else:
            features_fired = feature_counts_for(activation, args.require_feature)
        # `unprovable` only displaces `pass` — fail / concern stay primary
        # because canary failures and drift are about the run, not about
        # the run's ability to evaluate a hypothesis.
        if overall == "pass" and any(c == 0 for c in features_fired.values()):
            overall = "unprovable"
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
        colony_score_surface=cs_surface,
        founder_dispersion_low=dispersion_rows,
        throughput_drift=tp_drift,
        plan_failure_canary=plan_failure_rows,
        baseline_duration_ticks=baseline_dur,
        observed_duration_ticks=observed_dur,
        duration_drift_pct=duration_drift_pct,
        features_fired=features_fired,
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

    exit_code = {"pass": 0, "concern": 1, "fail": 2, "unprovable": 3}[overall]
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
        surface = f" ({v.colony_score_surface})" if v.colony_score_surface else ""
        lines.append(f"  colony_score drift{surface}:")
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
    if v.founder_dispersion_low:
        worst = min(v.founder_dispersion_low, key=lambda r: r.get("mean_dist", 0.0))
        lines.append(
            f"  dispersion: LOW in {len(v.founder_dispersion_low)} window(s) "
            f"(worst {worst.get('mean_dist', 0):.1f} tiles, floor "
            f"{FOUNDER_DISPERSION_FLOOR_TILES:.0f})"
        )
    if v.throughput_drift:
        t = v.throughput_drift
        lines.append(
            f"  throughput: {t['band']:8s} {t['delta_pct']:+5.1f}%  "
            f"{t['metric']} ({t['baseline']} → {t['observed']})"
        )
    if v.plan_failure_canary:
        lines.append("  plan-failure regressions:")
        for r in v.plan_failure_canary[:5]:
            if r["band"] == "high-rate-ratio":
                tag = f"{r['ratio']:>5.1f}x"
            else:
                tag = "  new "
            lines.append(
                f"    {tag}  {r['reason']}  "
                f"({r['baseline']} -> {r['observed']}, "
                f"{r['rate_observed']}/tick)"
            )
    if v.features_fired:
        lines.append("  features required:")
        for name, count in v.features_fired.items():
            mark = "✓" if count > 0 else "✗"
            lines.append(f"    {mark} {name}: {count}")
    if v.next_steps:
        lines.append("  next:")
        for s in v.next_steps:
            lines.append(f"    $ {s}")
    return "\n".join(lines)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
