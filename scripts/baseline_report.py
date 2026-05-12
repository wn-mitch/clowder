#!/usr/bin/env python3
"""Aggregate a baseline-dataset directory into REPORT.md.

Walks the directory tree produced by ``scripts/run_baseline_dataset.sh`` and
emits a structured markdown summary across the 10 plan sections:

  1. Header parity (commit hash / dirty status across all runs)
  2. Survival-canary distribution (starvation, shadowfox-ambush, footer
     written, never-fired-expected)
  3. Continuity-tally envelope (grooming, play, mentoring, burial, courtship,
     mythic-texture)
  4. Population trajectory (peak, final, deaths breakdown)
  5. Need-cascade timeseries at quartile checkpoints
  6. DSE-score landscape (per focal trace: top DSEs by mean L3 final score
     and eligibility-rate)
  7. Plan-step failure reasons (from `PlanStepFailed` events in events.jsonl;
     the prior implementation read a `L3PlanFailure` trace layer that is no
     longer emitted)
  8. Commitment-gate firings (per branch, per disposition)
  9. Fog/storm deltas vs. seed-42 baseline rep
 10. Deferred-balance baselines (the four blocked metrics)
 11. Cascade signatures (181-iter-2 Patrol absorption detector; deltas
     against a prior sidecar via `--vs-sidecar`)

Designed to be tolerant: missing data emits a row note rather than crashing.
This is by design — the orchestrator runs collect-everything and may
produce partial datasets; the report has to summarise what's there.
"""

from __future__ import annotations

import argparse
import json
import math
import statistics
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

# Continuity classes the footer carries (mirrors scripts/check_continuity.sh).
CONTINUITY_CLASSES = ["grooming", "play", "mentoring", "burial", "courtship", "mythic_texture"]

DEFERRED_FEATURES = {
    "MatingOccurred": "mating cadence",
    "CleanseCompleted": "magic — cleanse",
    "CarcassHarvested": "magic — harvest",
    "SpiritCommunion": "magic — commune",
    "CropTended": "farming — tend",
    "CropHarvested": "farming — harvest",
    "WardPlaced": "ward placement",
}


# --- helpers ---------------------------------------------------------------


def fmt(x: float, places: int = 2) -> str:
    if x is None or (isinstance(x, float) and math.isnan(x)):
        return "n/a"
    return f"{x:.{places}f}"


def stats_or_na(values: list[float]) -> dict[str, float | None]:
    if not values:
        return {"n": 0, "mean": None, "stdev": None, "min": None, "max": None, "p50": None, "p95": None}
    s = sorted(values)
    return {
        "n": len(values),
        "mean": statistics.fmean(values),
        "stdev": statistics.pstdev(values) if len(values) > 1 else 0.0,
        "min": s[0],
        "max": s[-1],
        "p50": s[len(s) // 2],
        "p95": s[min(len(s) - 1, int(0.95 * len(s)))],
    }


def read_jsonl_streaming(path: Path):
    """Yield parsed lines from a JSONL file, skipping malformed records."""
    if not path.exists():
        return
    with path.open() as f:
        for line in f:
            try:
                yield json.loads(line)
            except ValueError:
                continue


def read_header_footer(events_path: Path) -> tuple[dict | None, dict | None]:
    """Return (header, footer) dicts from an events.jsonl, or (None, None)."""
    if not events_path.exists() or events_path.stat().st_size == 0:
        return None, None
    header = None
    with events_path.open() as f:
        first = f.readline()
        try:
            obj = json.loads(first)
            if obj.get("_header"):
                header = obj
        except ValueError:
            pass
    # Tail-read for footer.
    footer = None
    with events_path.open("rb") as f:
        f.seek(0, 2)
        sz = f.tell()
        f.seek(max(0, sz - 32768))
        tail = f.read().decode("utf-8", errors="replace")
    for line in reversed([l for l in tail.splitlines() if l.strip()]):
        try:
            obj = json.loads(line)
        except ValueError:
            continue
        if obj.get("_footer"):
            footer = obj
            break
    return header, footer


# --- per-run data extraction ----------------------------------------------


@dataclass
class RunSummary:
    label: str                # "sweep-42-1" / "trace-42-Simba" / "conditional-42-fog"
    kind: str                 # "sweep" | "trace" | "conditional"
    seed: int
    rep_or_focal: str         # "1" / "Simba" / "fog"
    events_path: Path
    trace_path: Path | None
    header: dict | None
    footer: dict | None
    starvation: int | None = None
    shadowfox_ambush: int | None = None
    footer_written: bool = False
    never_fired_expected: int | None = None
    continuity: dict[str, int] = field(default_factory=dict)
    population_peak: int | None = None
    population_final: int | None = None
    deaths_total: int = 0
    deaths_by_cause: dict[str, int] = field(default_factory=dict)
    activation_positive: dict[str, int] = field(default_factory=dict)
    # PlanStepFailed events: per-run total + per-reason counter. Populated
    # in summarize_run by walking events.jsonl. The L3PlanFailure trace
    # layer §7 originally read no longer exists in current trace emit;
    # this is the canonical source of plan-failure data.
    plan_step_failed_total: int = 0
    plan_step_failed_by_reason: dict[str, int] = field(default_factory=dict)


def summarize_run(label: str, kind: str, seed: int, rep_or_focal: str,
                  run_dir: Path) -> RunSummary | None:
    events = run_dir / "events.jsonl"
    if not events.exists():
        return None
    trace = None
    for cand in run_dir.glob("trace-*.jsonl"):
        trace = cand
        break

    header, footer = read_header_footer(events)
    summary = RunSummary(
        label=label, kind=kind, seed=seed, rep_or_focal=rep_or_focal,
        events_path=events, trace_path=trace,
        header=header, footer=footer,
        footer_written=bool(footer),
    )
    if footer:
        # deaths_by_cause may be nested under any of several keys depending on
        # event-log version — be permissive.
        deaths = footer.get("deaths_by_cause") or {}
        summary.deaths_by_cause = dict(deaths)
        summary.starvation = int(deaths.get("Starvation", 0))
        summary.shadowfox_ambush = int(deaths.get("ShadowFoxAmbush", 0))
        summary.deaths_total = sum(int(v) for v in deaths.values())
        nfe = footer.get("never_fired_expected_positives")
        if isinstance(nfe, list):
            summary.never_fired_expected = len(nfe)
        elif isinstance(nfe, int):
            summary.never_fired_expected = nfe
        ct = footer.get("continuity_tallies") or {}
        # Normalise key names: footer may use snake_case or PascalCase.
        for cls in CONTINUITY_CLASSES:
            for k in (cls, cls.replace("_", ""), cls.title().replace("_", ""), cls.replace("_", "-")):
                if k in ct:
                    summary.continuity[cls] = int(ct[k])
                    break
            else:
                summary.continuity[cls] = 0

    # Walk the file once for ColonySnapshot final + activation tallies + peak pop
    # + PlanStepFailed reason histogram.
    population = []
    activations: dict[str, int] = {}
    psf_total = 0
    psf_by_reason: dict[str, int] = {}
    for ev in read_jsonl_streaming(events):
        t = ev.get("type")
        if t == "ColonyScore":
            living = ev.get("living_cats")
            peak = ev.get("peak_population")
            if isinstance(living, int):
                population.append(living)
            if isinstance(peak, int):
                summary.population_peak = max(summary.population_peak or 0, peak)
        elif t == "SystemActivation":
            pos = ev.get("positive") or {}
            for k, v in pos.items():
                if isinstance(v, int):
                    activations[k] = max(activations.get(k, 0), v)
        elif t == "PlanStepFailed":
            psf_total += 1
            reason = ev.get("reason") or "?"
            psf_by_reason[reason] = psf_by_reason.get(reason, 0) + 1
    summary.plan_step_failed_total = psf_total
    summary.plan_step_failed_by_reason = psf_by_reason
    if population:
        summary.population_final = population[-1]
        if summary.population_peak is None:
            summary.population_peak = max(population)
    summary.activation_positive = activations
    return summary


# --- trace analysis (focal-cat L3 / L2) -----------------------------------


@dataclass
class TraceAggregate:
    label: str
    focal: str
    seed: int
    dse_scores: dict[str, list[float]] = field(default_factory=dict)         # DSE → list of final scores
    dse_eligible_ticks: dict[str, int] = field(default_factory=dict)         # DSE → eligibility count
    total_l2_ticks: int = 0
    chosen_counter: dict[str, int] = field(default_factory=dict)             # winning DSE → count
    commitment_branches: dict[str, dict[str, int]] = field(default_factory=dict)  # disposition → {branch: count}


def summarize_trace(trace_path: Path, label: str, focal: str, seed: int) -> TraceAggregate:
    agg = TraceAggregate(label=label, focal=focal, seed=seed)
    if not trace_path.exists():
        return agg
    last_l2_tick = None
    for rec in read_jsonl_streaming(trace_path):
        layer = rec.get("layer")
        if layer == "L2":
            dse = rec.get("dse")
            score = rec.get("final_score")
            if dse and isinstance(score, (int, float)):
                agg.dse_scores.setdefault(dse, []).append(float(score))
                agg.dse_eligible_ticks[dse] = agg.dse_eligible_ticks.get(dse, 0) + 1
            tick = rec.get("tick")
            if tick != last_l2_tick:
                agg.total_l2_ticks += 1
                last_l2_tick = tick
        elif layer == "L3":
            chosen = rec.get("chosen") or rec.get("chosen_dse") or rec.get("chosen_intention")
            if chosen:
                agg.chosen_counter[chosen] = agg.chosen_counter.get(chosen, 0) + 1
        elif layer == "L3Commitment":
            disp = rec.get("disposition") or "?"
            branch = rec.get("branch") or "?"
            agg.commitment_branches.setdefault(disp, {}).setdefault(branch, 0)
            agg.commitment_branches[disp][branch] += 1
    return agg


# --- discovery -------------------------------------------------------------


def discover_runs(base: Path) -> list[RunSummary]:
    runs: list[RunSummary] = []
    sweep = base / "sweep"
    if sweep.exists():
        for d in sorted(sweep.iterdir()):
            if not d.is_dir():
                continue
            try:
                seed_str, rep = d.name.rsplit("-", 1)
                seed = int(seed_str)
            except ValueError:
                continue
            r = summarize_run(f"sweep-{d.name}", "sweep", seed, rep, d)
            if r is not None:
                runs.append(r)
    trace = base / "trace"
    if trace.exists():
        for d in sorted(trace.iterdir()):
            if not d.is_dir():
                continue
            try:
                seed_str, focal = d.name.split("-", 1)
                seed = int(seed_str)
            except ValueError:
                continue
            r = summarize_run(f"trace-{d.name}", "trace", seed, focal, d)
            if r is not None:
                runs.append(r)
    cond = base / "conditional"
    if cond.exists():
        for d in sorted(cond.iterdir()):
            if not d.is_dir():
                continue
            try:
                seed_str, weather = d.name.split("-", 1)
                seed = int(seed_str)
            except ValueError:
                continue
            r = summarize_run(f"conditional-{d.name}", "conditional", seed, weather, d)
            if r is not None:
                runs.append(r)
    return runs


# --- markdown sections -----------------------------------------------------


def section_header_parity(runs: list[RunSummary]) -> str:
    out = ["## 1. Header parity\n"]
    by_commit: dict[tuple[str, bool], list[str]] = {}
    no_header: list[str] = []
    for r in runs:
        if r.header is None:
            no_header.append(r.label)
            continue
        key = (r.header.get("commit_hash_short") or "?", bool(r.header.get("commit_dirty")))
        by_commit.setdefault(key, []).append(r.label)
    if not by_commit:
        out.append("_No headers found in any run._\n")
        return "".join(out)
    out.append("| commit_hash_short | commit_dirty | runs | sample labels |\n")
    out.append("|---|---|---:|---|\n")
    for (sha, dirty), labels in sorted(by_commit.items(), key=lambda kv: -len(kv[1])):
        sample = ", ".join(labels[:5]) + (f", … (+{len(labels)-5})" if len(labels) > 5 else "")
        out.append(f"| `{sha}` | {dirty} | {len(labels)} | {sample} |\n")
    if no_header:
        out.append(f"\n**Runs missing header ({len(no_header)}):** {', '.join(no_header[:10])}\n")
    if len(by_commit) > 1:
        out.append("\n> **Tainted dataset:** more than one (commit, dirty) bucket present. Cross-run diffs may not be valid; subsequent sections still render against the surviving subset.\n")
    elif any(dirty for (_, dirty), _ in by_commit.items()):
        out.append("\n> **Note:** all runs share a commit but `commit_dirty=true`. Archive is internally consistent but cannot be diffed against a future or prior commit's logs.\n")
    return "".join(out)


def section_survival_canaries(runs: list[RunSummary]) -> str:
    out = ["\n## 2. Survival canaries\n"]
    sweep_runs = [r for r in runs if r.kind == "sweep"]
    if not sweep_runs:
        out.append("_No sweep runs found._\n")
        return "".join(out)
    starv = [r.starvation for r in sweep_runs if r.starvation is not None]
    sfa = [r.shadowfox_ambush for r in sweep_runs if r.shadowfox_ambush is not None]
    nfe = [r.never_fired_expected for r in sweep_runs if r.never_fired_expected is not None]
    written = sum(1 for r in sweep_runs if r.footer_written)
    out.append(f"\n**Sweep envelope** ({len(sweep_runs)} runs; {written} with footer):\n\n")
    out.append("| canary | min | p50 | mean | p95 | max | n |\n")
    out.append("|---|---:|---:|---:|---:|---:|---:|\n")
    for name, vals in [("Starvation deaths", starv), ("ShadowFox ambush deaths", sfa), ("Never-fired-expected count", nfe)]:
        s = stats_or_na([float(v) for v in vals])
        out.append(f"| {name} | {fmt(s['min'],1)} | {fmt(s['p50'],1)} | {fmt(s['mean'],2)} | {fmt(s['p95'],1)} | {fmt(s['max'],1)} | {s['n']} |\n")
    out.append("\n**Per-run table:**\n\n")
    out.append("| run | starv | shadow_fox_amb | nfe | footer | population_peak → final | total deaths |\n")
    out.append("|---|---:|---:|---:|:---:|---|---:|\n")
    for r in sweep_runs:
        peak = r.population_peak if r.population_peak is not None else "—"
        final = r.population_final if r.population_final is not None else "—"
        out.append(
            f"| {r.label} | "
            f"{r.starvation if r.starvation is not None else '—'} | "
            f"{r.shadowfox_ambush if r.shadowfox_ambush is not None else '—'} | "
            f"{r.never_fired_expected if r.never_fired_expected is not None else '—'} | "
            f"{'✓' if r.footer_written else '✗'} | "
            f"{peak} → {final} | "
            f"{r.deaths_total} |\n"
        )
    return "".join(out)


def section_continuity(runs: list[RunSummary]) -> str:
    out = ["\n## 3. Continuity-tallies envelope\n"]
    sweep_runs = [r for r in runs if r.kind == "sweep" and r.footer_written]
    if not sweep_runs:
        out.append("_No sweep runs with footer._\n")
        return "".join(out)
    out.append(f"\n{len(sweep_runs)} sweep runs contributing.\n\n")
    out.append("| class | mean | stdev | min | max | zero-runs |\n")
    out.append("|---|---:|---:|---:|---:|---:|\n")
    for cls in CONTINUITY_CLASSES:
        vals = [r.continuity.get(cls, 0) for r in sweep_runs]
        s = stats_or_na([float(v) for v in vals])
        zero = sum(1 for v in vals if v == 0)
        out.append(f"| {cls} | {fmt(s['mean'],2)} | {fmt(s['stdev'],2)} | {fmt(s['min'],0)} | {fmt(s['max'],0)} | {zero}/{len(vals)} |\n")
    return "".join(out)


def section_population(runs: list[RunSummary]) -> str:
    out = ["\n## 4. Population trajectory\n"]
    by_seed: dict[int, list[RunSummary]] = {}
    for r in runs:
        if r.kind == "sweep":
            by_seed.setdefault(r.seed, []).append(r)
    if not by_seed:
        out.append("_No sweep runs found._\n")
        return "".join(out)
    out.append("\n| seed | n | peak (mean) | final (mean) | total deaths (mean) | starvation share |\n")
    out.append("|---:|---:|---:|---:|---:|---:|\n")
    for seed in sorted(by_seed):
        rs = by_seed[seed]
        peaks = [r.population_peak for r in rs if r.population_peak is not None]
        finals = [r.population_final for r in rs if r.population_final is not None]
        deaths = [r.deaths_total for r in rs]
        starv = [r.starvation for r in rs if r.starvation is not None]
        starv_share = (sum(starv) / sum(deaths)) if deaths and sum(deaths) > 0 else 0
        out.append(
            f"| {seed} | {len(rs)} | "
            f"{fmt(statistics.fmean(peaks),1) if peaks else 'n/a'} | "
            f"{fmt(statistics.fmean(finals),1) if finals else 'n/a'} | "
            f"{fmt(statistics.fmean(deaths),1) if deaths else 'n/a'} | "
            f"{fmt(starv_share*100,1)}% |\n"
        )
    return "".join(out)


def section_needs(runs: list[RunSummary]) -> str:
    """Need-cascade timeseries: per-need mean/σ at quartile checkpoints.

    Quartile placement is data-driven — the sim's absolute tick counter is
    not anchored at zero (Bevy ``Time<Fixed>`` carries a ~1.2M-tick warmup
    offset on fresh runs), so percentile-of-tick-range is the only portable
    bucketing strategy.
    """
    out = ["\n## 5. Need-cascade timeseries\n"]
    sweep_runs = [r for r in runs if r.kind == "sweep" and r.footer_written]
    if not sweep_runs:
        out.append("_No sweep runs with footer._\n")
        return "".join(out)

    NEED_KEYS = ["hunger", "energy", "temperature", "safety", "social",
                 "acceptance", "mating", "respect", "mastery", "purpose"]

    # First pass per run: collect all (tick, needs) tuples; second pass: bucket
    # by min/max-relative quartile. Per-run bucketing keeps tick-range warmup
    # offsets from leaking between runs.
    buckets: dict[str, list[list[float]]] = {k: [[], [], [], []] for k in NEED_KEYS}
    for r in sweep_runs:
        per_run: list[tuple[int, dict[str, float]]] = []
        for ev in read_jsonl_streaming(r.events_path):
            if ev.get("type") != "CatSnapshot":
                continue
            tick = ev.get("tick")
            if not isinstance(tick, int):
                continue
            needs = ev.get("needs") or {}
            cleaned = {k: float(needs[k]) for k in NEED_KEYS if isinstance(needs.get(k), (int, float))}
            if cleaned:
                per_run.append((tick, cleaned))
        if not per_run:
            continue
        ticks = [t for t, _ in per_run]
        t_min, t_max = min(ticks), max(ticks)
        span = max(t_max - t_min, 1)
        for tick, needs in per_run:
            frac = (tick - t_min) / span
            # Quartile index 0..3 (last quartile inclusive of t_max).
            q_idx = min(3, int(frac * 4))
            for k, v in needs.items():
                buckets[k][q_idx].append(v)

    # Header info for the human-readable label.
    duration = None
    for r in sweep_runs:
        if r.header and r.header.get("duration_secs"):
            duration = r.header["duration_secs"]
            break

    label = f"duration={duration}s" if duration else "duration=unknown"
    out.append(f"\nData-driven quartile bucketing across {len(sweep_runs)} sweep runs ({label}). "
               "Q1=earliest 25% of ticks, Q4=latest 25%.\n\n")
    out.append("| need | Q1 mean (σ) | Q2 mean (σ) | Q3 mean (σ) | Q4 mean (σ) |\n")
    out.append("|---|---|---|---|---|\n")
    for k in NEED_KEYS:
        cells = []
        for q_idx in range(4):
            vals = buckets[k][q_idx]
            if not vals:
                cells.append("n/a")
                continue
            mean = statistics.fmean(vals)
            sd = statistics.pstdev(vals) if len(vals) > 1 else 0.0
            cells.append(f"{mean:.2f} ({sd:.2f})")
        out.append(f"| {k} | {' | '.join(cells)} |\n")
    return "".join(out)


def per_focal_meta(traces: list[TraceAggregate]) -> list[dict]:
    """One row per focal trace: enough state for cross-run cascade-signature
    comparison without re-reading the trace sidecars.

    The cascade detector (181 iter-2 anti-regression) wants two scalars per
    focal: (1) the fraction of L3-chosen ticks that picked the Guarding
    disposition — proxy for "Patrol share" since Patrol is the dominant
    Action in Guarding — and (2) the mean of the `patrol` DSE's L2
    final_score. Both can be absent: marker-gated DSEs and dispositions
    do not fire on every focal.
    """
    rows = []
    for tr in traces:
        total_l3 = sum(tr.chosen_counter.values())
        guarding_picks = tr.chosen_counter.get("Guarding", 0)
        patrol_scores = tr.dse_scores.get("patrol") or []
        rows.append({
            "seed": tr.seed,
            "focal": tr.focal,
            "total_l3_picks": total_l3,
            "guarding_picks": guarding_picks,
            "guarding_share_pct": (100.0 * guarding_picks / total_l3) if total_l3 else None,
            "patrol_n_samples": len(patrol_scores),
            "patrol_mean_l2": statistics.fmean(patrol_scores) if patrol_scores else None,
        })
    return rows


def compute_cascade_signatures(traces: list[TraceAggregate],
                               baseline_sidecar: dict | None) -> dict:
    """Detect the 181-iter-2 Patrol absorption cascade against a baseline.

    Signature shape (per-seed aggregate across all focals of that seed):
      - share_delta_pp ≥ +0.5pp on Guarding-disposition picks AND
      - mean_delta_pct ≤ −5% on `patrol` DSE L2 final_score
    Per the 211-coordinate-food-security thread: share-pp climbing while
    per-cat score falls is the softmax-mass-redistribution signature that
    181 iter-2 cited as the predator-exposure cascade root cause.

    Without a baseline sidecar, renders absolute values per seed and skips
    the flag column.
    """
    current_per_focal = per_focal_meta(traces)
    by_seed: dict[int, list[dict]] = {}
    for row in current_per_focal:
        by_seed.setdefault(row["seed"], []).append(row)

    baseline_by_seed: dict[int, list[dict]] = {}
    if baseline_sidecar:
        for row in baseline_sidecar.get("per_focal_meta") or []:
            baseline_by_seed.setdefault(row["seed"], []).append(row)

    per_seed: list[dict] = []
    flagged = 0
    for seed in sorted(by_seed):
        focals = by_seed[seed]
        # Pooled share across all focals for this seed (totals-weighted).
        total_l3 = sum(f["total_l3_picks"] for f in focals)
        guarding_picks = sum(f["guarding_picks"] for f in focals)
        guarding_share_pct = (100.0 * guarding_picks / total_l3) if total_l3 else None
        # Patrol mean pooled across all samples (sample-weighted).
        all_patrol = []
        for f in focals:
            if f["patrol_mean_l2"] is not None and f["patrol_n_samples"] > 0:
                all_patrol.extend([f["patrol_mean_l2"]] * f["patrol_n_samples"])
        patrol_mean_l2 = statistics.fmean(all_patrol) if all_patrol else None

        # Baseline lookup.
        base_focals = baseline_by_seed.get(seed) or []
        base_total = sum(f.get("total_l3_picks", 0) for f in base_focals)
        base_guarding = sum(f.get("guarding_picks", 0) for f in base_focals)
        base_share = (100.0 * base_guarding / base_total) if base_total else None
        base_patrol_samples = []
        for f in base_focals:
            if f.get("patrol_mean_l2") is not None and f.get("patrol_n_samples", 0) > 0:
                base_patrol_samples.extend([f["patrol_mean_l2"]] * f["patrol_n_samples"])
        base_patrol_mean = statistics.fmean(base_patrol_samples) if base_patrol_samples else None

        share_delta_pp = None
        if guarding_share_pct is not None and base_share is not None:
            share_delta_pp = guarding_share_pct - base_share
        mean_delta_pct = None
        if patrol_mean_l2 is not None and base_patrol_mean not in (None, 0):
            mean_delta_pct = 100.0 * (patrol_mean_l2 - base_patrol_mean) / base_patrol_mean
        flag = (
            share_delta_pp is not None and share_delta_pp >= 0.5
            and mean_delta_pct is not None and mean_delta_pct <= -5.0
        )
        if flag:
            flagged += 1
        per_seed.append({
            "seed": seed,
            "n_focals": len(focals),
            "guarding_share_pct": guarding_share_pct,
            "patrol_mean_l2": patrol_mean_l2,
            "baseline_guarding_share_pct": base_share,
            "baseline_patrol_mean_l2": base_patrol_mean,
            "share_delta_pp": share_delta_pp,
            "mean_delta_pct": mean_delta_pct,
            "flag": flag,
        })

    return {
        "patrol_181": {
            "per_seed": per_seed,
            "flagged_seeds": flagged,
            "total_seeds": len(per_seed),
            "has_baseline": baseline_sidecar is not None,
        }
    }


def cross_focal_dse_envelope(traces: list[TraceAggregate]) -> list[dict]:
    """Aggregate L2 final_score samples across every focal trace, per DSE.

    Returns one row per DSE, with mean / stdev / p50 / p95 over the pooled
    score sample. `n_focals` is the number of distinct focal traces that
    saw the DSE at least once — useful for spotting DSEs that fire on one
    focal and not others. Marker-gated DSEs naturally have low n_focals.
    """
    pooled: dict[str, list[float]] = {}
    seen_on: dict[str, set[tuple[int, str]]] = {}
    for tr in traces:
        for dse, scores in tr.dse_scores.items():
            pooled.setdefault(dse, []).extend(scores)
            seen_on.setdefault(dse, set()).add((tr.seed, tr.focal))
    rows = []
    for dse, scores in pooled.items():
        s = stats_or_na(scores)
        rows.append({
            "dse": dse,
            "mean": s["mean"],
            "stdev": s["stdev"],
            "p50": s["p50"],
            "p95": s["p95"],
            "n_samples": s["n"],
            "n_focals": len(seen_on[dse]),
        })
    rows.sort(key=lambda r: -(r["mean"] or 0.0))
    return rows


def section_dse_landscape(traces: list[TraceAggregate]) -> str:
    out = ["\n## 6. DSE-score landscape\n"]
    if not traces:
        out.append("_No focal traces found._\n")
        return "".join(out)

    # Cross-focal aggregate first — load-bearing for frame-diff style
    # readouts (per 210/211 food-security threads & 181 iter-2 post-mortem).
    envelope = cross_focal_dse_envelope(traces)
    if envelope:
        out.append("\n### Cross-focal envelope\n\n")
        out.append(f"Pooled L2 `final_score` samples across {len(traces)} focal trace(s); top 15 DSEs by mean.\n\n")
        out.append("| DSE | mean | stdev | p50 | p95 | samples | focals |\n")
        out.append("|---|---:|---:|---:|---:|---:|---:|\n")
        for row in envelope[:15]:
            out.append(
                f"| {row['dse']} | {fmt(row['mean'], 3)} | {fmt(row['stdev'], 3)} | "
                f"{fmt(row['p50'], 3)} | {fmt(row['p95'], 3)} | "
                f"{row['n_samples']} | {row['n_focals']} |\n"
            )

    # Per-focal tables (unchanged shape).
    for tr in traces:
        out.append(f"\n### Focal: seed {tr.seed} / {tr.focal}\n\n")
        if not tr.dse_scores:
            out.append("_No L2 records — focal cat may have been filtered out (eligibility, life-stage, name mismatch)._\n")
            continue
        rows = []
        for dse, scores in tr.dse_scores.items():
            elig_pct = 100.0 * tr.dse_eligible_ticks.get(dse, 0) / max(tr.total_l2_ticks, 1)
            rows.append((dse, statistics.fmean(scores), elig_pct, len(scores)))
        rows.sort(key=lambda x: -x[1])
        out.append("| DSE | mean L3 final score | eligibility-rate | samples |\n")
        out.append("|---|---:|---:|---:|\n")
        for dse, mean, elig, n in rows[:15]:
            out.append(f"| {dse} | {mean:.3f} | {elig:.1f}% | {n} |\n")
        if tr.chosen_counter:
            chosen_top = sorted(tr.chosen_counter.items(), key=lambda kv: -kv[1])[:5]
            chosen_str = ", ".join(f"{k}({v})" for k, v in chosen_top)
            out.append(f"\nTop chosen: {chosen_str}\n")
    return "".join(out)


def plan_failure_top10(runs: list[RunSummary]) -> list[dict]:
    """Cross-seed envelope of `PlanStepFailed.reason` counts.

    Pools every sweep run's reason histogram, then ranks the top 10 by
    mean count per run. Each row carries mean / stdev / min / max +
    zero-run count so balance work can spot reasons that fire only on a
    minority of seeds (high stdev relative to mean = seed-sensitive
    failure mode).
    """
    sweep_runs = [r for r in runs if r.kind == "sweep"]
    if not sweep_runs:
        return []
    all_reasons = set()
    for r in sweep_runs:
        all_reasons.update(r.plan_step_failed_by_reason)
    rows = []
    for reason in all_reasons:
        vals = [r.plan_step_failed_by_reason.get(reason, 0) for r in sweep_runs]
        s = stats_or_na([float(v) for v in vals])
        zero = sum(1 for v in vals if v == 0)
        rows.append({
            "reason": reason,
            "mean": s["mean"],
            "stdev": s["stdev"],
            "min": s["min"],
            "max": s["max"],
            "zero_runs": zero,
            "n_runs": len(sweep_runs),
        })
    rows.sort(key=lambda r: -(r["mean"] or 0.0))
    return rows[:10]


def plan_failure_reason_diff(current: list[dict],
                             baseline_sidecar: dict | None) -> dict:
    """When a sidecar is supplied, classify each current top-10 reason as
    `new` (absent from baseline), `dropped` (in baseline but not current),
    or `shared`. Returns {"new": [...], "dropped": [...], "shared": [...]}.
    """
    if not baseline_sidecar:
        return {}
    base_top = baseline_sidecar.get("plan_failure_top10") or []
    current_names = {r["reason"] for r in current}
    base_names = {r["reason"] for r in base_top}
    return {
        "new": sorted(current_names - base_names),
        "dropped": sorted(base_names - current_names),
        "shared": sorted(current_names & base_names),
    }


def section_plan_failures(runs: list[RunSummary],
                          top10: list[dict],
                          reason_diff: dict | None) -> str:
    out = ["\n## 7. Plan-step failure reasons\n"]
    sweep_runs = [r for r in runs if r.kind == "sweep"]
    if not sweep_runs:
        out.append("_No sweep runs found._\n")
        return "".join(out)
    # Sourced from `PlanStepFailed` events in events.jsonl (not the dead
    # `L3PlanFailure` trace layer the prior implementation read). Healthy-
    # colony.md:76-82 — drift in this distribution signals step-resolver
    # behavior changes.
    out.append("\nSourced from `PlanStepFailed` events.\n\n")
    out.append("### Per-run table\n\n")
    out.append("| run | total | dominant reason | top reasons |\n")
    out.append("|---|---:|---|---|\n")
    for r in sweep_runs:
        total = r.plan_step_failed_total
        if total == 0:
            out.append(f"| {r.label} | 0 | — | — |\n")
            continue
        sorted_reasons = sorted(r.plan_step_failed_by_reason.items(), key=lambda kv: -kv[1])
        top = sorted_reasons[0][0]
        breakdown = ", ".join(f"{k}={v}" for k, v in sorted_reasons[:5])
        out.append(f"| {r.label} | {total} | {top} | {breakdown} |\n")

    if top10:
        out.append("\n### Cross-seed envelope (top 10 reasons)\n\n")
        out.append("| reason | mean | stdev | min | max | zero-runs |\n")
        out.append("|---|---:|---:|---:|---:|---:|\n")
        for row in top10:
            out.append(
                f"| {row['reason']} | {fmt(row['mean'], 1)} | {fmt(row['stdev'], 1)} | "
                f"{fmt(row['min'], 0)} | {fmt(row['max'], 0)} | "
                f"{row['zero_runs']}/{row['n_runs']} |\n"
            )

    if reason_diff:
        new = reason_diff.get("new") or []
        dropped = reason_diff.get("dropped") or []
        if new or dropped:
            out.append("\n### Reason-set diff vs baseline\n\n")
            if new:
                out.append(f"**New reasons** ({len(new)}): {', '.join(f'`{r}`' for r in new)}\n\n")
            if dropped:
                out.append(f"**Dropped reasons** ({len(dropped)}): {', '.join(f'`{r}`' for r in dropped)}\n\n")
            out.append(
                "_Per healthy-colony.md: new failure reasons appearing or old ones disappearing "
                "entirely signal step-resolver behavior changes._\n"
            )
    return "".join(out)


def section_commitment_gate(traces: list[TraceAggregate]) -> str:
    out = ["\n## 8. Commitment-gate firings\n"]
    if not traces:
        out.append("_No focal traces found._\n")
        return "".join(out)
    out.append("Per-focal L3Commitment branch tally. Branch dispatch (§7.3): `Blind`→achieved-only; `SingleMinded`→achieved/unachievable; `OpenMinded`→achieved/dropped_goal.\n\n")
    for tr in traces:
        if not tr.commitment_branches:
            continue
        out.append(f"\n### {tr.label}\n\n")
        out.append("| disposition | achieved | unachievable | dropped_goal | retained | other |\n")
        out.append("|---|---:|---:|---:|---:|---:|\n")
        for disp, branches in sorted(tr.commitment_branches.items()):
            ach = branches.get("achieved", 0)
            una = branches.get("unachievable", 0)
            drp = branches.get("dropped_goal", 0)
            ret = branches.get("retained", 0)
            other = sum(v for k, v in branches.items() if k not in {"achieved", "unachievable", "dropped_goal", "retained"})
            out.append(f"| {disp} | {ach} | {una} | {drp} | {ret} | {other} |\n")
    return "".join(out)


def section_conditional_deltas(runs: list[RunSummary]) -> str:
    out = ["\n## 9. Fog/storm deltas vs. seed-42 baseline rep\n"]
    cond = [r for r in runs if r.kind == "conditional"]
    sweep_42 = [r for r in runs if r.kind == "sweep" and r.seed == 42 and r.footer_written]
    if not cond:
        out.append("_No conditional weather runs found (Phase 4 may have been skipped)._\n")
        return "".join(out)
    if not sweep_42:
        out.append("_No seed-42 sweep baseline rep — cannot diff._\n")
        return "".join(out)
    # Use median-rep (rep 1 if available).
    base_rep = sorted(sweep_42, key=lambda r: r.rep_or_focal)[0]
    out.append(f"Baseline: `{base_rep.label}` (seed 42 sweep, rep {base_rep.rep_or_focal}).\n\n")
    out.append("| metric | baseline | fog | storm | fog Δ | storm Δ |\n")
    out.append("|---|---:|---:|---:|---:|---:|\n")
    cond_by = {r.rep_or_focal: r for r in cond if r.seed == 42}
    fog = cond_by.get("fog")
    storm = cond_by.get("storm")
    metrics = [
        ("Starvation deaths", lambda r: r.starvation),
        ("ShadowFox ambush deaths", lambda r: r.shadowfox_ambush),
        ("Total deaths", lambda r: r.deaths_total),
        ("Population peak", lambda r: r.population_peak),
        ("Population final", lambda r: r.population_final),
    ]
    for name, fn in metrics:
        b = fn(base_rep)
        f = fn(fog) if fog else None
        s = fn(storm) if storm else None
        def _delta(x, base):
            if x is None or base is None:
                return "—"
            return f"{x - base:+d}" if isinstance(x, int) and isinstance(base, int) else f"{x - base:+.2f}"
        out.append(f"| {name} | {b if b is not None else '—'} | {f if f is not None else '—'} | {s if s is not None else '—'} | {_delta(f, b)} | {_delta(s, b)} |\n")
    return "".join(out)


def section_cascade_signatures(cascade: dict) -> str:
    out = ["\n## 11. Cascade signatures\n"]
    p181 = cascade.get("patrol_181") or {}
    rows = p181.get("per_seed") or []
    if not rows:
        out.append("_No trace data — cascade detector requires focal traces._\n")
        return "".join(out)
    has_baseline = bool(p181.get("has_baseline"))
    if has_baseline:
        out.append(
            "\n**181 iter-2 detector** — Patrol absorbs freed L3 bandwidth: "
            "Guarding-disposition share ↑ while `patrol` DSE per-cat L2 ↓. "
            "Flag fires when share Δ ≥ +0.5pp AND mean Δ ≤ −5%.\n\n"
        )
        out.append("| seed | focals | Guarding share | baseline | Δ pp | `patrol` mean | baseline | Δ % | flag |\n")
        out.append("|---:|---:|---:|---:|---:|---:|---:|---:|:---:|\n")
        for r in rows:
            out.append(
                f"| {r['seed']} | {r['n_focals']} | "
                f"{fmt(r['guarding_share_pct'], 2)}% | {fmt(r['baseline_guarding_share_pct'], 2)}% | "
                f"{fmt(r['share_delta_pp'], 2)} | "
                f"{fmt(r['patrol_mean_l2'], 3)} | {fmt(r['baseline_patrol_mean_l2'], 3)} | "
                f"{fmt(r['mean_delta_pct'], 1)}% | "
                f"{'⚠️' if r['flag'] else '·'} |\n"
            )
        out.append(f"\n**Cross-seed:** {p181['flagged_seeds']}/{p181['total_seeds']} seeds tripped the flag.\n")
    else:
        out.append(
            "\n_No baseline sidecar supplied — rendering absolute Guarding-share + `patrol` mean per seed. "
            "Cascade flags fire only when `--vs-sidecar` is passed._\n\n"
        )
        out.append("| seed | focals | Guarding share | `patrol` mean L2 |\n")
        out.append("|---:|---:|---:|---:|\n")
        for r in rows:
            out.append(
                f"| {r['seed']} | {r['n_focals']} | "
                f"{fmt(r['guarding_share_pct'], 2)}% | "
                f"{fmt(r['patrol_mean_l2'], 3)} |\n"
            )
    return "".join(out)


def section_deferred_balance(runs: list[RunSummary]) -> str:
    out = ["\n## 10. Deferred-balance baselines\n"]
    sweep_runs = [r for r in runs if r.kind == "sweep" and r.footer_written]
    if not sweep_runs:
        out.append("_No sweep runs with footer._\n")
        return "".join(out)
    out.append("\nCross-seed envelope of the four deferred-balance metrics (per ticket 014).\n\n")
    out.append("| feature | label | mean | stdev | min | max | zero-runs |\n")
    out.append("|---|---|---:|---:|---:|---:|---:|\n")
    for feat, label in DEFERRED_FEATURES.items():
        vals = [r.activation_positive.get(feat, 0) for r in sweep_runs]
        s = stats_or_na([float(v) for v in vals])
        zeros = sum(1 for v in vals if v == 0)
        out.append(f"| `{feat}` | {label} | {fmt(s['mean'],2)} | {fmt(s['stdev'],2)} | {fmt(s['min'],0)} | {fmt(s['max'],0)} | {zeros}/{len(vals)} |\n")
    out.append(
        "\n_Floor of zero across all sweeps means the DSE is either marker-gated invisible or "
        "softmax-buried — drill via `just q trace` and the focal-cat L2 records to disambiguate._\n"
    )
    return "".join(out)


# --- main ------------------------------------------------------------------


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--baseline-dir", required=True, help="logs/baseline-<LABEL> directory.")
    p.add_argument("--output", default=None, help="REPORT.md output path (default: <baseline-dir>/REPORT.md).")
    p.add_argument("--json-sidecar", default=None,
                   help="Machine-readable sidecar path (default: <output>.json). "
                        "Consumed by balance_pass_aggregate.sh's baseline-pack composer.")
    p.add_argument("--vs-sidecar", default=None,
                   help="Path to a previously-generated REPORT.json (the sidecar). "
                        "Enables cross-run delta computations such as the cascade-signature "
                        "detector in §11. Missing or empty file is treated as no baseline.")
    return p.parse_args()


def main() -> int:
    args = parse_args()
    base = Path(args.baseline_dir)
    if not base.is_dir():
        print(f"error: {base} is not a directory", file=sys.stderr)
        return 2
    out_path = Path(args.output) if args.output else base / "REPORT.md"

    print(f"[report] discovering runs under {base}", file=sys.stderr)
    runs = discover_runs(base)
    print(f"[report] found {len(runs)} runs", file=sys.stderr)

    # Trace aggregates from runs that have trace sidecars.
    traces: list[TraceAggregate] = []
    for r in runs:
        if r.trace_path and r.trace_path.exists():
            tr = summarize_trace(r.trace_path, r.label, r.rep_or_focal, r.seed)
            traces.append(tr)
    print(f"[report] processed {len(traces)} focal traces", file=sys.stderr)

    rosters_path = base / "rosters.json"
    rosters_block = ""
    if rosters_path.exists():
        try:
            rosters = json.loads(rosters_path.read_text())
            lines = ["\n### Rosters (Phase 1)\n", "\n| seed | slot A | slot B | reason | cats observed |\n", "|---:|---|---|---|---:|\n"]
            for seed in sorted(rosters.get("seeds", {}), key=lambda s: int(s)):
                info = rosters["seeds"][seed]
                lines.append(
                    f"| {seed} | {info.get('slot_a','—')} | {info.get('slot_b','—')} | "
                    f"{info.get('slot_b_reason','—')} | {len(info.get('cats',[]))} |\n"
                )
            rosters_block = "".join(lines)
        except (ValueError, OSError):
            rosters_block = "\n_rosters.json present but unreadable._\n"

    # Load the comparison sidecar once (used by §11 cascade signatures
    # and any future cross-run delta sections).
    baseline_sidecar: dict | None = None
    if args.vs_sidecar:
        vs_path = Path(args.vs_sidecar)
        if vs_path.exists() and vs_path.stat().st_size > 0:
            try:
                baseline_sidecar = json.loads(vs_path.read_text())
            except ValueError:
                print(f"[report] WARN: --vs-sidecar {vs_path} is not valid JSON; ignored", file=sys.stderr)
        else:
            print(f"[report] note: --vs-sidecar {vs_path} missing or empty; rendering absolute values", file=sys.stderr)

    cascade = compute_cascade_signatures(traces, baseline_sidecar)
    plan_top10 = plan_failure_top10(runs)
    reason_diff = plan_failure_reason_diff(plan_top10, baseline_sidecar)

    sections = [
        f"# Baseline dataset report — `{base.name}`\n",
        f"\nGenerated from {len(runs)} runs (sweep + trace + conditional). {len(traces)} focal traces.\n",
        rosters_block,
        section_header_parity(runs),
        section_survival_canaries(runs),
        section_continuity(runs),
        section_population(runs),
        section_needs(runs),
        section_dse_landscape(traces),
        section_plan_failures(runs, plan_top10, reason_diff),
        section_commitment_gate(traces),
        section_conditional_deltas(runs),
        section_deferred_balance(runs),
        section_cascade_signatures(cascade),
    ]
    out_path.write_text("".join(sections))
    print(f"[report] wrote {out_path}", file=sys.stderr)

    # JSON sidecar — machine-readable counterpart for the pack composer.
    # Later commits in this PR add more keys (plan_failure_top10,
    # death_timing, defense_cadence, …).
    sidecar_path = Path(args.json_sidecar) if args.json_sidecar else out_path.with_suffix(out_path.suffix + ".json")
    sidecar = {
        "label": base.name,
        "n_runs": len(runs),
        "n_focals": len(traces),
        "per_dse_l2": cross_focal_dse_envelope(traces),
        "per_focal_meta": per_focal_meta(traces),
        "cascade_signatures": cascade,
        "plan_failure_top10": plan_top10,
        "plan_failure_reason_diff": reason_diff,
    }
    sidecar_path.write_text(json.dumps(sidecar, indent=2) + "\n")
    print(f"[report] wrote {sidecar_path}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    import sys
    sys.exit(main())
