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
  7. Plan-churn metrics (per disposition kind)
  8. Commitment-gate firings (per branch, per disposition)
  9. Fog/storm deltas vs. seed-42 baseline rep
 10. Deferred-balance baselines (the four blocked metrics)

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

    # Walk the file once for ColonySnapshot final + activation tallies + peak pop.
    population = []
    activations: dict[str, int] = {}
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
    plan_failures: dict[str, int] = field(default_factory=dict)              # reason → count


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
        elif layer == "L3PlanFailure":
            reason = rec.get("reason") or "?"
            agg.plan_failures[reason] = agg.plan_failures.get(reason, 0) + 1
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
    """Approximate need-cascade timeseries from CatSnapshot records."""
    out = ["\n## 5. Need-cascade timeseries\n"]
    sweep_runs = [r for r in runs if r.kind == "sweep" and r.footer_written]
    if not sweep_runs:
        out.append("_No sweep runs with footer._\n")
        return "".join(out)
    # Quartile ticks from any header's duration. Default 900s × 2000 ticks/s = 1_800_000.
    duration = None
    for r in sweep_runs:
        if r.header and r.header.get("duration_secs"):
            duration = r.header["duration_secs"]
            break
    if duration is None:
        duration = 900
    end_tick = duration * 2000
    quartiles = [int(end_tick * q) for q in (0.25, 0.5, 0.75, 1.0)]

    # Bucketed need samples per quartile across all sweep CatSnapshots.
    NEED_KEYS = ["hunger", "energy", "temperature", "safety", "social",
                 "acceptance", "mating", "respect", "mastery", "purpose"]
    buckets: dict[str, list[list[float]]] = {k: [[], [], [], []] for k in NEED_KEYS}
    for r in sweep_runs:
        for ev in read_jsonl_streaming(r.events_path):
            if ev.get("type") != "CatSnapshot":
                continue
            tick = ev.get("tick")
            if not isinstance(tick, int):
                continue
            # Pick the smallest quartile tick the sample is ≤.
            for q_idx, q_tick in enumerate(quartiles):
                if tick <= q_tick:
                    needs = ev.get("needs") or {}
                    for k in NEED_KEYS:
                        v = needs.get(k)
                        if isinstance(v, (int, float)):
                            buckets[k][q_idx].append(float(v))
                    break
    out.append(f"\nQuartile checkpoints (assuming 2000 ticks/s, duration={duration}s): T={duration*0.25:.0f}s, {duration*0.5:.0f}s, {duration*0.75:.0f}s, {duration:.0f}s.\n")
    out.append(f"\nMean (σ) per need bucket:\n\n")
    out.append("| need | T₁ | T₂ | T₃ | T₄ |\n")
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


def section_dse_landscape(traces: list[TraceAggregate]) -> str:
    out = ["\n## 6. DSE-score landscape\n"]
    if not traces:
        out.append("_No focal traces found._\n")
        return "".join(out)
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


def section_plan_churn(traces: list[TraceAggregate]) -> str:
    out = ["\n## 7. Plan-churn metrics\n"]
    if not traces:
        out.append("_No focal traces found._\n")
        return "".join(out)
    out.append("Plan-churn observable directly via L3PlanFailure / L3Commitment trace records (proxies for replan_count and drop branches).\n\n")
    out.append("| focal | total plan-failures | dominant failure reason | failure-reason breakdown |\n")
    out.append("|---|---:|---|---|\n")
    for tr in traces:
        total = sum(tr.plan_failures.values())
        if total == 0:
            out.append(f"| {tr.label} | 0 | — | — |\n")
            continue
        sorted_reasons = sorted(tr.plan_failures.items(), key=lambda kv: -kv[1])
        top = sorted_reasons[0][0]
        breakdown = ", ".join(f"{k}={v}" for k, v in sorted_reasons[:5])
        out.append(f"| {tr.label} | {total} | {top} | {breakdown} |\n")
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
        section_plan_churn(traces),
        section_commitment_gate(traces),
        section_conditional_deltas(runs),
        section_deferred_balance(runs),
    ]
    out_path.write_text("".join(sections))
    print(f"[report] wrote {out_path}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    import sys
    sys.exit(main())
