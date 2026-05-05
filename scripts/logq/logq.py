#!/usr/bin/env python3
"""`just q` dispatcher — query tools over clowder sim logs.

Wraps the jq recipes in `docs/diagnostics/log-queries.md` as callable
subtools with a consistent output envelope. The jq recipe library
remains the source of truth; this file cites the recipe each subtool
implements (section + query name from that doc).

Intended consumers: Claude in a fresh session investigating a run
(primary), and Will at the CLI (secondary). Not a `diagnose-run`
replacement — that report path stays diff-stable and untouched.

Usage: `just q <subtool> <log_dir> [flags]` or
       `python scripts/logq/logq.py <subtool> <log_dir> [flags]`.

See `docs/diagnostics/log-queries.md` for the underlying jq recipes.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

# Support being run either as `python scripts/logq/logq.py` (direct)
# or as `python -m scripts.logq.logq` (module).
if __package__ in (None, ""):
    sys.path.insert(0, str(Path(__file__).parent))
    from envelope import (  # type: ignore[no-redef]
        Envelope, emit, event_id, narrative_id, nearest_ticks,
        run_jq, run_jq_count, trace_id,
    )
else:
    from .envelope import (
        Envelope, emit, event_id, narrative_id, nearest_ticks,
        run_jq, run_jq_count, trace_id,
    )

# Sibling-module import for the agent-call telemetry helper. logq lives
# in scripts/logq/ but the helper is at scripts/_agent_call_log.py.
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from _agent_call_log import append_call_history  # type: ignore[no-redef]  # noqa: E402


# ── shared helpers ──────────────────────────────────────────────────────────

def events_path(log_dir: Path) -> Path:
    return log_dir / "events.jsonl"


def narrative_path(log_dir: Path) -> Path:
    return log_dir / "narrative.jsonl"


def trace_path(log_dir: Path, cat: str) -> Path:
    return log_dir / f"trace-{cat}.jsonl"


def tick_range_jq(lo: int | None, hi: int | None) -> str:
    """jq predicate fragment for a closed tick range. Empty string if both None."""
    if lo is None and hi is None:
        return ""
    parts = []
    if lo is not None:
        parts.append(f".tick >= {lo}")
    if hi is not None:
        parts.append(f".tick <= {hi}")
    return " and " + " and ".join(parts)


def parse_tick_range(s: str | None) -> tuple[int | None, int | None]:
    if not s:
        return None, None
    if ".." not in s:
        raise ValueError(f"tick-range must look like A..B, got {s!r}")
    lo_s, hi_s = s.split("..", 1)
    lo = int(lo_s) if lo_s.strip() else None
    hi = int(hi_s) if hi_s.strip() else None
    return lo, hi


def paginate(records: list[dict[str, Any]], limit: int | None, offset: int) -> tuple[list[dict[str, Any]], bool]:
    """Slice records by offset/limit; return (page, more_available)."""
    sliced = records[offset:]
    more = False
    if limit is not None and len(sliced) > limit:
        sliced = sliced[:limit]
        more = True
    return sliced, more


# ── subtool: run-summary ────────────────────────────────────────────────────
# Wraps `log-queries.md` §1 (header, footer) + §2 (constants).
# Also folds in the joinability check from `diagnose-run.md` step 2.

def _top_n_dict(d: dict[str, Any] | None, n: int = 3) -> list[dict[str, Any]]:
    """Return the top-N entries of a count-valued dict, sorted desc by value."""
    if not d:
        return []
    items = [(k, v) for k, v in d.items() if isinstance(v, (int, float))]
    items.sort(key=lambda kv: -kv[1])
    return [{"key": k, "value": v} for k, v in items[:n]]


def _derive_final_tick(footer_field: Any, ev_path: Path) -> int | None:
    """Footer in current schema doesn't always carry `final_tick`. Derive
    it from `max(.tick)` over events when missing — that is the highest
    tick the sim emitted before exiting, which is what callers actually
    want from `final_tick`."""
    if isinstance(footer_field, int):
        return footer_field
    try:
        hits = run_jq(
            'select(.tick != null) | {tick}',
            ev_path,
        )
    except (FileNotFoundError, RuntimeError):
        return None
    max_tick = 0
    for r in hits:
        t = r.get("tick")
        if isinstance(t, int) and t > max_tick:
            max_tick = t
    return max_tick if max_tick > 0 else None


def cmd_run_summary(args: argparse.Namespace) -> Envelope:
    log_dir = Path(args.log_dir)
    ev_path = events_path(log_dir)
    tr_path = log_dir.glob("trace-*.jsonl")
    tr_paths = sorted(tr_path)

    query = {"subtool": "run-summary", "log_dir": str(log_dir)}

    header = run_jq('select(._header)', ev_path)
    footer = run_jq('select(._footer)', ev_path)

    results: list[dict[str, Any]] = []
    if header:
        h = header[0]
        results.append({
            "id": f"run:{h.get('commit_hash_short', '?')}:seed=?",
            "kind": "header",
            "commit_hash_short": h.get("commit_hash_short"),
            "commit_dirty": h.get("commit_dirty"),
            "commit_time": h.get("commit_time"),
            "summary": (
                f"commit={h.get('commit_hash_short')}"
                f"{'(dirty)' if h.get('commit_dirty') else ''} "
                f"time={h.get('commit_time')}"
            ),
        })
    if footer:
        f = footer[0]
        # `final_tick` isn't in the current footer schema; derive from
        # the highest event tick when it's absent so the field stops
        # reporting `null` / `?` to consumers.
        final_tick = _derive_final_tick(f.get("final_tick"), ev_path)
        deaths = f.get("deaths_by_cause") or {}
        nf = f.get("never_fired_expected_positives") or []
        results.append({
            "id": "run:footer",
            "kind": "footer",
            "final_tick": final_tick,
            "final_tick_source": "footer" if isinstance(f.get("final_tick"), int)
                                 else "derived_max_event_tick",
            "deaths_by_cause": deaths,
            "deaths_total": sum(deaths.values()) if deaths else 0,
            "never_fired_expected_positives": nf,
            "continuity_tallies": f.get("continuity_tallies", {}),
            "wards_placed_total": f.get("wards_placed_total"),
            "interrupts_by_reason_top": _top_n_dict(
                f.get("interrupts_by_reason"), 3
            ),
            "plan_failures_by_reason_top": _top_n_dict(
                f.get("plan_failures_by_reason"), 3
            ),
            "anxiety_interrupt_total": f.get("anxiety_interrupt_total"),
            "negative_events_total": f.get("negative_events_total"),
            "positive_features_active": f.get("positive_features_active"),
            "positive_features_total": f.get("positive_features_total"),
            "summary": (
                f"final_tick={final_tick if final_tick is not None else '?'} "
                f"deaths={sum(deaths.values()) if deaths else 0} "
                f"never_fired={len(nf)}"
            ),
        })

    # Joinability: compare commit_hash between events.jsonl and each trace-*.jsonl.
    join_status: list[dict[str, Any]] = []
    events_commit = header[0].get("commit_hash") if header else None
    for tp in tr_paths:
        trace_header = run_jq('select(._header)', tp)
        t_commit = trace_header[0].get("commit_hash") if trace_header else None
        focal = trace_header[0].get("focal_cat") if trace_header else None
        joinable = (events_commit == t_commit) and events_commit is not None
        join_status.append({
            "id": f"trace:{focal or tp.stem}",
            "kind": "trace",
            "path": str(tp),
            "focal_cat": focal,
            "joinable_with_events": joinable,
            "summary": (
                f"focal={focal or '?'} "
                f"joinable={'yes' if joinable else 'NO'}"
            ),
        })
    results.extend(join_status)

    narrative = _compose_run_summary_narrative(results)
    next_cmds = []
    if footer and (footer[0].get("deaths_by_cause") or {}):
        next_cmds.append(f"just q deaths {log_dir}")
    if footer and footer[0].get("never_fired_expected_positives"):
        next_cmds.append(f"just q anomalies {log_dir}")
    # Actions distribution is the central question for any DSE-balance
    # diagnosis — surface it in `next` whenever the run completed.
    if footer:
        next_cmds.append(f"just q actions {log_dir}")
    # Footer drill-down for callers who need fields beyond the curated
    # set in `run-summary` (e.g., `interrupts_by_reason` full breakdown).
    if footer:
        next_cmds.append(f"just q footer {log_dir}")
    if tr_paths:
        focals = [r["focal_cat"] for r in join_status if r.get("focal_cat")]
        for f in focals:
            next_cmds.append(f"just q cat-timeline {log_dir} {f}")

    return Envelope(
        query=query,
        scan_stats={
            "scanned": len(header) + len(footer) + len(tr_paths),
            "returned": len(results),
            "more_available": False,
            "narrow_by": [],
        },
        results=results,
        narrative=narrative,
        next=next_cmds,
    )


def _compose_run_summary_narrative(results: list[dict[str, Any]]) -> str:
    header = next((r for r in results if r["kind"] == "header"), None)
    footer = next((r for r in results if r["kind"] == "footer"), None)
    if not header and not footer:
        return "No header/footer found. Log bundle is malformed or empty."
    head_phrase = ""
    if header:
        head_phrase = (
            f"Run at commit {header.get('commit_hash_short')}"
            f"{' (dirty)' if header.get('commit_dirty') else ''}"
        )
    foot_phrase = ""
    if footer:
        total_deaths = footer.get("deaths_total") or 0
        nf = footer.get("never_fired_expected_positives", []) or []
        final_tick = footer.get("final_tick")
        source = footer.get("final_tick_source")
        # When the footer didn't carry final_tick we render the derived
        # value with a parenthetical so callers know it's max(.tick),
        # not an authoritative footer field. The source visibility is
        # load-bearing for diagnostics that diff "claimed final_tick"
        # across runs.
        if final_tick is not None:
            tick_phrase = (
                f"ended at tick {final_tick}"
                + (" (derived)" if source == "derived_max_event_tick" else "")
                + " "
            )
        else:
            tick_phrase = ""
        foot_phrase = (
            tick_phrase
            + f"with {total_deaths} deaths and {len(nf)} never-fired positives"
        )
    traces = [r for r in results if r["kind"] == "trace"]
    trace_phrase = ""
    if traces:
        joined = [t for t in traces if t.get("joinable_with_events")]
        trace_phrase = f"{len(joined)}/{len(traces)} trace sidecar(s) joinable"

    first = ", ".join(p for p in (head_phrase, foot_phrase) if p)
    sentences = [s for s in (first, trace_phrase) if s]
    return ". ".join(sentences) + "." if sentences else ""


# ── subtool: events ─────────────────────────────────────────────────────────
# Wraps `log-queries.md` §2–9 (generic event filtering).

def cmd_events(args: argparse.Namespace) -> Envelope:
    log_dir = Path(args.log_dir)
    ev_path = events_path(log_dir)

    kinds = [k for k in (args.kind.split(",") if args.kind else []) if k]
    lo, hi = parse_tick_range(args.tick_range)
    cat = args.cat
    limit = args.limit
    offset = args.offset

    query = {
        "subtool": "events", "log_dir": str(log_dir),
        "kind": kinds or None, "tick_range": args.tick_range,
        "cat": cat, "limit": limit, "offset": offset,
    }

    predicates = ["(.type != null)"]
    if kinds:
        kind_list = " or ".join(f'.type == "{k}"' for k in kinds)
        predicates.append(f"({kind_list})")
    if lo is not None:
        predicates.append(f"(.tick >= {lo})")
    if hi is not None:
        predicates.append(f"(.tick <= {hi})")
    if cat:
        predicates.append(f'(.cat == "{cat}" or .name == "{cat}")')

    jq_selector = f'select({" and ".join(predicates)})'
    records = run_jq(jq_selector, ev_path)

    scanned = len(records)
    page, more = paginate(records, limit, offset)

    results = [{
        "id": event_id(r),
        "tick": r.get("tick"),
        "type": r.get("type"),
        "cat": r.get("cat") or r.get("name"),
        "summary": _event_summary(r),
        "record": r,
    } for r in page]

    narrative = _events_narrative(records, kinds, (lo, hi), cat, ev_path)
    next_cmds = _events_next(log_dir, kinds, (lo, hi), cat, page)

    return Envelope(
        query=query,
        scan_stats={
            "scanned": scanned,
            "returned": len(results),
            "more_available": more,
            "narrow_by": _events_narrow_by(kinds, lo, hi, cat),
        },
        results=results,
        narrative=narrative,
        next=next_cmds,
    )


def _event_summary(r: dict[str, Any]) -> str:
    t = r.get("type", "?")
    bits = [f"type={t}"]
    for k in ("cat", "name", "cause", "feature", "ward_kind", "location", "reason"):
        if k in r and r[k] is not None:
            bits.append(f"{k}={r[k]}")
    return " ".join(bits)


def _events_narrative(records: list[dict[str, Any]], kinds: list[str],
                      tick_range: tuple[int | None, int | None],
                      cat: str | None, ev_path: Path) -> str:
    if records:
        kinds_str = ",".join(kinds) if kinds else "all kinds"
        tr_str = f" in ticks {tick_range[0]}..{tick_range[1]}" if any(tick_range) else ""
        cat_str = f" for {cat}" if cat else ""
        return f"Found {len(records)} events ({kinds_str}){tr_str}{cat_str}."
    # Null result: find nearest matches.
    preds = ["(.type != null)"]
    if kinds:
        preds.append("(" + " or ".join(f'.type == "{k}"' for k in kinds) + ")")
    if cat:
        preds.append(f'(.cat == "{cat}" or .name == "{cat}")')
    near = nearest_ticks(
        ev_path, f'select({" and ".join(preds)})',
        (tick_range[0] or 0, tick_range[1] or 0),
        max_return=3,
    )
    if not near:
        return "No matching events, and no similar events anywhere in the log."
    desc = ", ".join(f"tick {r['tick']} {r.get('type')}" + (f" ({r.get('cause') or r.get('feature') or ''})" if (r.get('cause') or r.get('feature')) else '') for r in near)
    return f"No matching events in range. Nearest matches by tick: {desc}."


def _events_narrow_by(kinds: list[str], lo: int | None, hi: int | None,
                      cat: str | None) -> list[str]:
    opts = []
    if not kinds:
        opts.append("kind")
    if lo is None or hi is None:
        opts.append("tick_range")
    if not cat:
        opts.append("cat")
    return opts


def _events_next(log_dir: Path, kinds: list[str],
                 tick_range: tuple[int | None, int | None],
                 cat: str | None, page: list[dict[str, Any]]) -> list[str]:
    next_cmds: list[str] = []
    # Offer deaths view if user queried generically and results include deaths.
    if any(r.get("type") == "Death" for r in page) and "Death" not in kinds:
        next_cmds.append(f"just q deaths {log_dir}")
    # Offer cat-timeline drill for the most-common cat on the page.
    cats = [r.get("cat") for r in page if r.get("cat")]
    if cats and not cat:
        most = max(set(cats), key=cats.count)
        next_cmds.append(f"just q cat-timeline {log_dir} {most}")
    return next_cmds


# ── subtool: deaths ─────────────────────────────────────────────────────────
# Wraps `log-queries.md` §4 (deaths-by-cause, deaths-by-cat, deaths-by-location).

def cmd_deaths(args: argparse.Namespace) -> Envelope:
    log_dir = Path(args.log_dir)
    ev_path = events_path(log_dir)

    lo, hi = parse_tick_range(args.tick_range)
    query = {
        "subtool": "deaths", "log_dir": str(log_dir),
        "cause": args.cause, "tick_range": args.tick_range, "cat": args.cat,
    }

    preds = ['.type == "Death"']
    if args.cause:
        preds.append(f'.cause == "{args.cause}"')
    if lo is not None:
        preds.append(f'.tick >= {lo}')
    if hi is not None:
        preds.append(f'.tick <= {hi}')
    if args.cat:
        preds.append(f'.cat == "{args.cat}"')
    selector = f'select({" and ".join(preds)}) | {{tick, cat, cause, injury_source, location}}'
    records = run_jq(selector, ev_path)

    results = [{
        "id": f"tick:{r.get('tick')}:Death:{r.get('cat', '?')}",
        "tick": r.get("tick"),
        "cat": r.get("cat"),
        "cause": r.get("cause"),
        "location": r.get("location"),
        "injury_source": r.get("injury_source"),
        "summary": f"tick {r.get('tick')} {r.get('cat')} ({r.get('cause')})",
    } for r in records]

    if results:
        cause_counts: dict[str, int] = {}
        for r in results:
            cause_counts[r["cause"] or "?"] = cause_counts.get(r["cause"] or "?", 0) + 1
        cause_summary = ", ".join(f"{v} {k}" for k, v in sorted(cause_counts.items(), key=lambda kv: -kv[1]))
        narrative = f"{len(results)} deaths: {cause_summary}."
    else:
        near = nearest_ticks(
            ev_path, 'select(.type=="Death") | {tick, cat, cause}',
            (lo or 0, hi or 0), max_return=3,
        )
        if near:
            desc = ", ".join(f"tick {r['tick']} {r.get('cat')} ({r.get('cause')})" for r in near)
            narrative = f"No deaths in range. Nearest: {desc}."
        else:
            narrative = "No deaths in range, and none anywhere in the log. Colony survived."

    next_cmds: list[str] = []
    cats = [r["cat"] for r in results if r.get("cat")]
    if cats:
        most = max(set(cats), key=cats.count)
        next_cmds.append(f"just q cat-timeline {log_dir} {most}")

    return Envelope(
        query=query,
        scan_stats={"scanned": len(records), "returned": len(results),
                    "more_available": False,
                    "narrow_by": ["cause", "tick_range", "cat"]},
        results=results, narrative=narrative, next=next_cmds,
    )


# ── subtool: narrative ──────────────────────────────────────────────────────
# Wraps `log-queries.md` §8 (legend-entries, narrative-tier-totals).

SIGNAL_TIERS = ("Legend", "Danger", "Significant")


def cmd_narrative(args: argparse.Namespace) -> Envelope:
    log_dir = Path(args.log_dir)
    nar_path = narrative_path(log_dir)

    if args.tier:
        tiers = [t.strip() for t in args.tier.split(",") if t.strip()]
    else:
        tiers = list(SIGNAL_TIERS)
    lo, hi = parse_tick_range(args.tick_range)

    query = {
        "subtool": "narrative", "log_dir": str(log_dir),
        "tier": tiers, "tick_range": args.tick_range,
    }

    preds = [".tier != null"]
    preds.append("(" + " or ".join(f'.tier == "{t}"' for t in tiers) + ")")
    if lo is not None:
        preds.append(f'.tick >= {lo}')
    if hi is not None:
        preds.append(f'.tick <= {hi}')
    selector = f'select({" and ".join(preds)})'
    records = run_jq(selector, nar_path)

    results = [{
        "id": narrative_id(r),
        "tick": r.get("tick"),
        "tier": r.get("tier"),
        "phase": r.get("phase"),
        "text": r.get("text"),
        "summary": f"[{r.get('tier')}] tick {r.get('tick')}: {r.get('text')}",
    } for r in records]

    if results:
        by_tier: dict[str, int] = {}
        for r in results:
            by_tier[r["tier"]] = by_tier.get(r["tier"], 0) + 1
        narrative = f"{len(results)} lines: " + ", ".join(
            f"{v} {k}" for k, v in sorted(by_tier.items(), key=lambda kv: -kv[1])
        ) + "."
    else:
        # Null: widen to all tiers and look for nearest.
        near = nearest_ticks(
            nar_path, "select(.tier != null)", (lo or 0, hi or 0), max_return=3,
        )
        if near:
            desc = ", ".join(f"tick {r['tick']} [{r.get('tier')}] {r.get('text', '')[:48]}" for r in near)
            narrative = f"No narrative lines in that tier/range. Nearest: {desc}."
        else:
            narrative = "No narrative lines at all — narrative.jsonl may be empty."

    return Envelope(
        query=query,
        scan_stats={"scanned": len(records), "returned": len(results),
                    "more_available": False,
                    "narrow_by": ["tier", "tick_range"]},
        results=results, narrative=narrative, next=[],
    )


# ── subtool: trace ──────────────────────────────────────────────────────────
# Wraps `log-queries.md` §11 (trace-l3-*, trace-l2-*, trace-l1-*).

def cmd_trace(args: argparse.Namespace) -> Envelope:
    log_dir = Path(args.log_dir)
    tr_path = trace_path(log_dir, args.cat)
    layer = args.layer or "L3"
    lo, hi = parse_tick_range(args.tick_range)

    query = {
        "subtool": "trace", "log_dir": str(log_dir), "cat": args.cat,
        "layer": layer, "tick_range": args.tick_range,
        "top_dses": args.top_dses,
    }

    if not tr_path.exists():
        return Envelope(
            query=query,
            scan_stats={"scanned": 0, "returned": 0, "more_available": False,
                        "narrow_by": []},
            results=[],
            narrative=f"No trace file for {args.cat} at {tr_path}. "
                      f"Re-run `just soak-trace <seed> {args.cat}` to produce one.",
            next=[],
        )

    preds = [f'.layer == "{layer}"']
    if lo is not None:
        preds.append(f'.tick >= {lo}')
    if hi is not None:
        preds.append(f'.tick <= {hi}')
    selector = f'select({" and ".join(preds)})'
    records = run_jq(selector, tr_path)

    # L3 summary: aggregate chosen-counts, top DSEs.
    results: list[dict[str, Any]] = []
    narrative: str
    next_cmds: list[str] = []

    if layer == "L3" and records:
        counts: dict[str, int] = {}
        for r in records:
            chosen = r.get("chosen") or "?"
            counts[chosen] = counts.get(chosen, 0) + 1
        top_n = args.top_dses or 5
        ranked = sorted(counts.items(), key=lambda kv: -kv[1])[:top_n]
        for dse, n in ranked:
            results.append({
                "id": f"trace:{args.cat}:L3:chosen={dse}",
                "dse": dse,
                "count": n,
                "pct": round(100.0 * n / len(records), 1),
                "summary": f"chosen={dse}  {n} ticks ({round(100.0*n/len(records),1)}%)",
            })
        narrative = (
            f"{args.cat} chose {len(counts)} distinct DSEs across "
            f"{len(records)} L3 ticks; top: "
            f"{ranked[0][0]} ({ranked[0][1]} ticks)."
        )
        # Drill suggestion: one L2 view, preserving user's tick range if set.
        range_flag = ""
        if lo is not None or hi is not None:
            range_flag = f" --tick-range={lo if lo is not None else ''}..{hi if hi is not None else ''}"
        next_cmds.append(
            f"just q trace {log_dir} {args.cat} --layer=L2{range_flag}"
        )
    elif layer == "L2" and records:
        # Summarize DSE evaluations: counts, avg scores.
        by_dse: dict[str, list[float]] = {}
        fails_by_dse: dict[str, int] = {}
        for r in records:
            dse = r.get("dse") or "?"
            if r.get("eligibility", {}).get("passed") is False:
                fails_by_dse[dse] = fails_by_dse.get(dse, 0) + 1
            sc = r.get("final_score")
            if isinstance(sc, (int, float)):
                by_dse.setdefault(dse, []).append(float(sc))
        top_n = args.top_dses or 8
        dses = sorted(by_dse.keys(), key=lambda d: -len(by_dse[d]))[:top_n]
        for d in dses:
            scores = by_dse[d]
            avg = sum(scores) / len(scores)
            results.append({
                "id": f"trace:{args.cat}:L2:dse={d}",
                "dse": d,
                "evals": len(scores),
                "avg_score": round(avg, 3),
                "eligibility_fails": fails_by_dse.get(d, 0),
                "summary": f"{d}  evals={len(scores)} avg={avg:.3f} "
                           f"elig_fails={fails_by_dse.get(d, 0)}",
            })
        narrative = (
            f"{args.cat} evaluated {len(by_dse)} distinct DSEs "
            f"across {len(records)} L2 rows."
        )
    elif layer == "L1" and records:
        # Summarize by map.
        by_map: dict[str, int] = {}
        for r in records:
            m = r.get("map") or "?"
            by_map[m] = by_map.get(m, 0) + 1
        top_n = args.top_dses or 8  # reuse knob for "top N groups"
        for m, n in sorted(by_map.items(), key=lambda kv: -kv[1])[:top_n]:
            results.append({
                "id": f"trace:{args.cat}:L1:map={m}",
                "map": m, "samples": n,
                "summary": f"map={m}  samples={n}",
            })
        narrative = (
            f"{args.cat} had {len(records)} L1 samples across "
            f"{len(by_map)} maps."
        )
    else:
        # Null: suggest widening.
        total = run_jq_count('select(.layer)', tr_path)
        if total == 0:
            narrative = f"Trace file {tr_path} has no layer records (corrupt?)."
        else:
            narrative = (
                f"No {layer} records in range. Trace has {total} total "
                f"non-header records. Try widening --tick-range or a "
                f"different --layer."
            )

    return Envelope(
        query=query,
        scan_stats={"scanned": len(records), "returned": len(results),
                    "more_available": False,
                    "narrow_by": ["tick_range", "layer"]},
        results=results, narrative=narrative, next=next_cmds,
    )


# ── subtool: cat-timeline ───────────────────────────────────────────────────
# Composite: events for cat + narrative mentions + trace presence.

CAT_TIMELINE_DEFAULT_LIMIT = 50


def cmd_cat_timeline(args: argparse.Namespace) -> Envelope:
    log_dir = Path(args.log_dir)
    ev_path = events_path(log_dir)
    nar_path = narrative_path(log_dir)
    tr_path = trace_path(log_dir, args.cat)
    lo, hi = parse_tick_range(args.tick_range)

    # Default limit so a focal cat with 97k events doesn't dump the whole
    # firehose into the envelope. Caller can pass `--limit=0` to disable
    # pagination explicitly when they want the full stream.
    limit = args.limit if args.limit is not None else CAT_TIMELINE_DEFAULT_LIMIT
    if limit == 0:
        limit = None
    offset = args.offset
    summarize = args.summarize

    query = {
        "subtool": "cat-timeline", "log_dir": str(log_dir),
        "cat": args.cat, "tick_range": args.tick_range,
        "limit": args.limit, "offset": offset, "summarize": summarize,
    }

    ev_preds = [
        f'(.cat == "{args.cat}" or .name == "{args.cat}" or '
        f'(.posse != null and (.posse | contains(["{args.cat}"]))))',
        ".type != null",
    ]
    if lo is not None:
        ev_preds.append(f".tick >= {lo}")
    if hi is not None:
        ev_preds.append(f".tick <= {hi}")
    ev_selector = f'select({" and ".join(ev_preds)}) | {{tick, type, cat, cause, feature, location}}'
    ev_records = run_jq(ev_selector, ev_path)

    # Narrative lines mentioning the cat name. Default to signal tiers
    # only — Action/Micro is a firehose ("Simba sets out to explore"
    # every few ticks). Drop through to `just q narrative --tier=Action,Micro`
    # if the user wants the full feed.
    nar_records = []
    if nar_path.exists():
        tiers_pred = "(" + " or ".join(f'.tier == "{t}"' for t in SIGNAL_TIERS) + ")"
        nar_preds = [f'.text != null and (.text | contains("{args.cat}")) and {tiers_pred}']
        if lo is not None:
            nar_preds.append(f'.tick >= {lo}')
        if hi is not None:
            nar_preds.append(f'.tick <= {hi}')
        nar_selector = f'select({" and ".join(nar_preds)})'
        nar_records = run_jq(nar_selector, nar_path)

    # Summarize mode: don't emit per-event rows; aggregate event-type
    # distribution + plan-create cadence + tick span. This is the right
    # surface for cats with tens of thousands of events where the
    # firehose is unreadable but the *pattern* is exactly what's
    # diagnostic ("Thistle creates a plan every 3 ticks").
    if summarize:
        return _cat_timeline_summary(
            args.cat, ev_records, nar_records, tr_path, query, log_dir,
        )

    # Merge by tick.
    merged: list[dict[str, Any]] = []
    for r in ev_records:
        merged.append({
            "id": event_id(r),
            "tick": r.get("tick"),
            "kind": "event",
            "type": r.get("type"),
            "summary": _event_summary(r),
        })
    for r in nar_records:
        merged.append({
            "id": narrative_id(r),
            "tick": r.get("tick"),
            "kind": "narrative",
            "tier": r.get("tier"),
            "text": r.get("text"),
            "summary": f"[{r.get('tier')}] {r.get('text')}",
        })
    merged.sort(key=lambda m: (m.get("tick") or 0))

    page, more = paginate(merged, limit, offset)

    narrative_parts = []
    range_phrase = (
        f" in ticks {lo}..{hi}" if lo is not None or hi is not None else ""
    )
    narrative_parts.append(
        f"{len(ev_records)} events + {len(nar_records)} narrative lines for "
        f"{args.cat}{range_phrase}"
    )
    if more:
        narrative_parts.append(
            f"showing {len(page)} (offset {offset}); pass --limit=0 for full "
            f"stream or --summarize for aggregates"
        )
    if tr_path.exists():
        narrative_parts.append(f"trace sidecar present at {tr_path}")
    narrative = ". ".join(narrative_parts) + "."

    next_cmds: list[str] = []
    if more:
        next_cmds.append(
            f"just q cat-timeline {log_dir} {args.cat} --summarize"
        )
    if tr_path.exists():
        next_cmds.append(f"just q trace {log_dir} {args.cat}")

    return Envelope(
        query=query,
        scan_stats={"scanned": len(merged),
                    "returned": len(page),
                    "more_available": more,
                    "narrow_by": ["tick_range", "limit"]},
        results=page, narrative=narrative, next=next_cmds,
    )


def _cat_timeline_summary(
    cat: str,
    ev_records: list[dict[str, Any]],
    nar_records: list[dict[str, Any]],
    tr_path: Path,
    query: dict[str, Any],
    log_dir: Path,
) -> Envelope:
    """Aggregate-mode response for cat-timeline.

    Surfaces the pattern that's lost in the per-event firehose:
    - Event-type distribution (top-N).
    - Plan-creation cadence (mean ticks between PlanCreated events).
    - Tick span covered by the events.
    - Narrative-line count.

    The plan-cadence number is the load-bearing summary — extreme low
    values (e.g. < 5 ticks) are the smoking gun for plan-churn loops.
    """
    by_type: dict[str, int] = {}
    plan_create_ticks: list[int] = []
    all_ticks: list[int] = []
    for r in ev_records:
        t = r.get("type") or "?"
        by_type[t] = by_type.get(t, 0) + 1
        tick = r.get("tick")
        if isinstance(tick, int):
            all_ticks.append(tick)
            if t == "PlanCreated":
                plan_create_ticks.append(tick)
    plan_create_ticks.sort()
    cadence_avg: float | None = None
    cadence_min: int | None = None
    cadence_max: int | None = None
    if len(plan_create_ticks) >= 2:
        gaps = [
            plan_create_ticks[i] - plan_create_ticks[i - 1]
            for i in range(1, len(plan_create_ticks))
        ]
        cadence_avg = round(sum(gaps) / len(gaps), 2)
        cadence_min = min(gaps)
        cadence_max = max(gaps)
    tick_span: list[int] | None = None
    if all_ticks:
        tick_span = [min(all_ticks), max(all_ticks)]

    results: list[dict[str, Any]] = []
    for typ, n in sorted(by_type.items(), key=lambda kv: -kv[1]):
        results.append({
            "id": f"timeline:{cat}:event_type:{typ}",
            "kind": "event_type_count",
            "type": typ,
            "count": n,
            "summary": f"{typ}: {n}",
        })
    if cadence_avg is not None:
        results.append({
            "id": f"timeline:{cat}:plan_create_cadence",
            "kind": "plan_create_cadence",
            "samples": len(plan_create_ticks),
            "avg_ticks_between": cadence_avg,
            "min_ticks_between": cadence_min,
            "max_ticks_between": cadence_max,
            "summary": (
                f"PlanCreated cadence: {cadence_avg} ticks avg "
                f"(min {cadence_min} / max {cadence_max}, "
                f"{len(plan_create_ticks)} samples)"
            ),
        })
    if tick_span:
        results.append({
            "id": f"timeline:{cat}:tick_span",
            "kind": "tick_span",
            "min": tick_span[0],
            "max": tick_span[1],
            "duration_ticks": tick_span[1] - tick_span[0],
            "summary": (
                f"tick span {tick_span[0]}..{tick_span[1]} "
                f"({tick_span[1] - tick_span[0]} ticks)"
            ),
        })
    if nar_records:
        results.append({
            "id": f"timeline:{cat}:narrative_lines",
            "kind": "narrative_count",
            "count": len(nar_records),
            "summary": f"{len(nar_records)} signal-tier narrative line(s)",
        })

    narrative_bits = [
        f"{len(ev_records)} events for {cat} across "
        f"{len(by_type)} type(s)"
    ]
    if cadence_avg is not None:
        churn_flag = ""
        if cadence_avg < 5:
            churn_flag = " — plan-churn pattern (cadence < 5 ticks)"
        narrative_bits.append(
            f"PlanCreated cadence avg {cadence_avg}{churn_flag}"
        )
    narrative = ". ".join(narrative_bits) + "."

    next_cmds: list[str] = []
    if tr_path.exists():
        next_cmds.append(f"just q trace {log_dir} {cat}")
    # Detail mode breadcrumb: if the caller wants individual events
    # after seeing the summary, the natural follow-up is the paginated
    # view at the head of the cat's tick span.
    if tick_span:
        next_cmds.append(
            f"just q cat-timeline {log_dir} {cat} "
            f"--tick-range={tick_span[0]}..{tick_span[0] + 5000}"
        )

    return Envelope(
        query=query,
        scan_stats={"scanned": len(ev_records) + len(nar_records),
                    "returned": len(results),
                    "more_available": False,
                    "narrow_by": []},
        results=results, narrative=narrative, next=next_cmds,
    )


# ── subtool: actions ────────────────────────────────────────────────────────
# Aggregates `current_action` from CatSnapshot events. Central question
# for any "did the DSE balance shift?" diagnosis. Wraps the recipe
# `grep '"type":"CatSnapshot"' | jq -r '.current_action' | sort | uniq -c`
# from docs/diagnostics/log-queries.md (and adds per-cat slicing).

def cmd_actions(args: argparse.Namespace) -> Envelope:
    log_dir = Path(args.log_dir)
    ev_path = events_path(log_dir)
    lo, hi = parse_tick_range(args.tick_range)

    query = {
        "subtool": "actions", "log_dir": str(log_dir),
        "cat": args.cat, "tick_range": args.tick_range,
        "limit": args.limit, "offset": args.offset,
    }

    preds = ['.type == "CatSnapshot"', '.current_action != null']
    if args.cat:
        preds.append(f'(.cat == "{args.cat}" or .name == "{args.cat}")')
    if lo is not None:
        preds.append(f".tick >= {lo}")
    if hi is not None:
        preds.append(f".tick <= {hi}")
    selector = (
        f'select({" and ".join(preds)}) | '
        f'{{tick, cat: (.cat // .name), action: .current_action}}'
    )
    records = run_jq(selector, ev_path)

    counts: dict[str, int] = {}
    by_cat: dict[str, dict[str, int]] = {}
    for r in records:
        a = r.get("action") or "?"
        counts[a] = counts.get(a, 0) + 1
        c = r.get("cat") or "?"
        by_cat.setdefault(c, {})
        by_cat[c][a] = by_cat[c].get(a, 0) + 1

    total = sum(counts.values()) or 1
    ranked = sorted(counts.items(), key=lambda kv: -kv[1])
    page, more = paginate(
        [{"action": a, "count": n} for a, n in ranked],
        args.limit, args.offset,
    )

    results: list[dict[str, Any]] = []
    for entry in page:
        a = entry["action"]
        n = entry["count"]
        results.append({
            "id": f"actions:{args.cat or 'colony'}:{a}",
            "kind": "action",
            "action": a,
            "count": n,
            "pct": round(100.0 * n / total, 2),
            "summary": f"{a}  {n} ({round(100.0*n/total,2)}%)",
        })

    if not records:
        # Null-result nearest-match: tell the caller whether CatSnapshot
        # exists at all in this bundle.
        snap_total = run_jq_count('select(.type=="CatSnapshot")', ev_path)
        if snap_total == 0:
            narrative = (
                "No CatSnapshot events at all — the snapshot system "
                "may be disabled or the log bundle is malformed."
            )
        else:
            cats = run_jq(
                'select(.type=="CatSnapshot") | {cat: (.cat // .name)} | '
                'select(.cat != null)',
                ev_path,
            )
            unique = sorted({c["cat"] for c in cats})[:10]
            narrative = (
                f"No CatSnapshot events match the filter. "
                f"{snap_total} CatSnapshot events exist; "
                f"first few cats by name: {', '.join(unique) or '(none)'}."
            )
    else:
        top = ranked[0]
        per_cat_phrase = ""
        if not args.cat and len(by_cat) > 0:
            per_cat_phrase = (
                f" across {len(by_cat)} distinct cat(s)"
            )
        narrative = (
            f"{total} CatSnapshot rows{per_cat_phrase}; "
            f"top action: {top[0]} ({top[1]}, "
            f"{round(100.0*top[1]/total,1)}%)."
        )

    next_cmds: list[str] = []
    if records and not args.cat:
        # Find the cat with the most extreme single-action concentration
        # — that's the cat most likely worth a focal trace look.
        if by_cat:
            extremes = []
            for c, dist in by_cat.items():
                c_total = sum(dist.values()) or 1
                top_pct = max(dist.values()) / c_total
                extremes.append((c, top_pct))
            extremes.sort(key=lambda kv: -kv[1])
            next_cat = extremes[0][0]
            next_cmds.append(f"just q actions {log_dir} --cat={next_cat}")
            next_cmds.append(f"just q cat-timeline {log_dir} {next_cat}")

    return Envelope(
        query=query,
        scan_stats={"scanned": len(records), "returned": len(results),
                    "more_available": more,
                    "narrow_by": ["cat", "tick_range"]},
        results=results, narrative=narrative, next=next_cmds,
    )


# ── subtool: hunt-success ───────────────────────────────────────────────────
# Per-discrete-attempt hunt outcomes (ticket 149). Each `HuntAttempt` event
# corresponds to one APPROACH→STALK→CHASE→POUNCE cycle on a single target,
# emitted at outcome resolution by `resolve_engage_prey`. Disambiguates the
# per-Hunt-action rate (which conflates within-Hunt retargeting) from the
# per-discrete-attempt rate that 30–50% real-cat-biology targets reference.

# Outcome variants that count as kills for success-rate computation. Mirror
# of `HuntOutcome::is_kill()` in src/resources/event_log.rs.
HUNT_KILL_OUTCOMES: set[str] = {"killed", "killed_and_replanned", "killed_and_consumed"}


def cmd_hunt_success(args: argparse.Namespace) -> Envelope:
    log_dir = Path(args.log_dir)
    ev_path = events_path(log_dir)
    lo, hi = parse_tick_range(args.tick_range)

    query = {
        "subtool": "hunt-success", "log_dir": str(log_dir),
        "cat": args.cat, "tick_range": args.tick_range,
        "species": args.species, "limit": args.limit, "offset": args.offset,
    }

    preds = ['.type == "HuntAttempt"']
    if args.cat:
        preds.append(f'.cat == "{args.cat}"')
    if args.species:
        preds.append(f'.prey_species == "{args.species}"')
    if lo is not None:
        preds.append(f".tick >= {lo}")
    if hi is not None:
        preds.append(f".tick <= {hi}")
    selector = (
        f'select({" and ".join(preds)}) | '
        f'{{tick, cat, species: .prey_species, outcome, '
        f'start_tick, end_tick, start_distance, '
        f'failure_reason}}'
    )
    records = run_jq(selector, ev_path)

    counts: dict[str, int] = {}
    by_cat: dict[str, dict[str, int]] = {}
    by_species: dict[str, dict[str, int]] = {}
    failure_reasons: dict[str, int] = {}
    duration_total = 0
    distance_total = 0
    for r in records:
        outcome = r.get("outcome") or "unknown"
        counts[outcome] = counts.get(outcome, 0) + 1
        c = r.get("cat") or "?"
        by_cat.setdefault(c, {})
        by_cat[c][outcome] = by_cat[c].get(outcome, 0) + 1
        sp = r.get("species") or "?"
        by_species.setdefault(sp, {})
        by_species[sp][outcome] = by_species[sp].get(outcome, 0) + 1
        fr = r.get("failure_reason")
        if fr:
            failure_reasons[fr] = failure_reasons.get(fr, 0) + 1
        st = r.get("start_tick")
        et = r.get("end_tick")
        if isinstance(st, int) and isinstance(et, int):
            duration_total += max(0, et - st)
        sd = r.get("start_distance")
        if isinstance(sd, int):
            distance_total += sd

    total = sum(counts.values())
    kills = sum(counts.get(o, 0) for o in HUNT_KILL_OUTCOMES)
    success_rate = (kills / total) if total > 0 else 0.0

    ranked = sorted(counts.items(), key=lambda kv: -kv[1])
    page, more = paginate(
        [{"outcome": o, "count": n} for o, n in ranked],
        args.limit, args.offset,
    )

    results: list[dict[str, Any]] = []
    if total > 0:
        results.append({
            "id": f"hunt-success:{args.cat or 'colony'}:summary",
            "kind": "summary",
            "total_attempts": total,
            "kills": kills,
            "success_rate_pct": round(100.0 * success_rate, 2),
            "mean_duration_ticks": round(duration_total / total, 2) if total else 0.0,
            "mean_start_distance": round(distance_total / total, 2) if total else 0.0,
            "summary": (
                f"{total} attempts; {round(100.0 * success_rate, 1)}% success "
                f"({kills} kills); mean duration "
                f"{round(duration_total / total, 1) if total else 0:.1f}t, "
                f"mean start_distance "
                f"{round(distance_total / total, 1) if total else 0:.1f}"
            ),
        })
    for entry in page:
        o = entry["outcome"]
        n = entry["count"]
        results.append({
            "id": f"hunt-success:{args.cat or 'colony'}:{o}",
            "kind": "outcome",
            "outcome": o,
            "count": n,
            "pct": round(100.0 * n / total, 2) if total else 0.0,
            "summary": (
                f"{o}  {n} ({round(100.0 * n / total, 2) if total else 0:.2f}%)"
            ),
        })

    if total == 0:
        # Null-result nearest-match: tell the caller whether HuntAttempt
        # exists at all in this bundle.
        ha_total = run_jq_count('select(.type=="HuntAttempt")', ev_path)
        if ha_total == 0:
            # Fall back on PreyKilled — older runs predate ticket 149's
            # instrumentation, so a non-zero PreyKilled with zero
            # HuntAttempt means "this run was generated before the
            # event was added", not "hunting is dead".
            pk_total = run_jq_count('select(.type=="PreyKilled")', ev_path)
            if pk_total > 0:
                narrative = (
                    f"No HuntAttempt events in this bundle, but {pk_total} "
                    f"PreyKilled events exist — log predates ticket 149 "
                    f"instrumentation. Re-run a fresh soak to populate "
                    f"per-discrete-attempt outcomes."
                )
            else:
                narrative = (
                    "No HuntAttempt events and no PreyKilled events. "
                    "Hunting pipeline is silent for this run."
                )
        else:
            narrative = (
                f"No HuntAttempt events match the filter. "
                f"{ha_total} HuntAttempt events exist in this bundle; "
                f"try widening --tick-range or removing --cat / --species."
            )
    else:
        # Top failure reason gives a one-glance lever for follow-on tuning.
        top_fr = ""
        if failure_reasons:
            fr_ranked = sorted(failure_reasons.items(), key=lambda kv: -kv[1])
            top_fr = (
                f" Top failure reason: {fr_ranked[0][0]} "
                f"({fr_ranked[0][1]}, "
                f"{round(100.0 * fr_ranked[0][1] / max(1, total - kills), 1)}% of losses)."
            )
        per_cat_phrase = ""
        if not args.cat and len(by_cat) > 0:
            per_cat_phrase = f" across {len(by_cat)} cat(s)"
        per_species_phrase = ""
        if not args.species and len(by_species) > 0:
            per_species_phrase = f" across {len(by_species)} prey species"
        narrative = (
            f"{total} discrete hunt attempts{per_cat_phrase}"
            f"{per_species_phrase}; "
            f"{round(100.0 * success_rate, 2)}% success rate "
            f"({kills} kills, {total - kills} losses)."
            f"{top_fr}"
        )

    next_cmds: list[str] = []
    if total > 0:
        if not args.cat and by_cat:
            # Suggest the cat with the worst success rate (≥ 5 attempts) for
            # focal-trace drill-down.
            worst_cat = None
            worst_rate = 1.1
            for c, dist in by_cat.items():
                c_total = sum(dist.values())
                if c_total < 5:
                    continue
                c_kills = sum(dist.get(o, 0) for o in HUNT_KILL_OUTCOMES)
                rate = c_kills / c_total
                if rate < worst_rate:
                    worst_rate = rate
                    worst_cat = c
            if worst_cat:
                next_cmds.append(
                    f"just q hunt-success {log_dir} --cat={worst_cat}"
                )
                next_cmds.append(
                    f"just q events {log_dir} --kind=HuntAttempt --cat={worst_cat}"
                )
        if not args.species and by_species:
            # Suggest the prey species with the worst rate for ecology
            # drill-down (e.g., birds vs mice often differ).
            worst_sp = None
            worst_sp_rate = 1.1
            for sp, dist in by_species.items():
                sp_total = sum(dist.values())
                if sp_total < 5:
                    continue
                sp_kills = sum(dist.get(o, 0) for o in HUNT_KILL_OUTCOMES)
                rate = sp_kills / sp_total
                if rate < worst_sp_rate:
                    worst_sp_rate = rate
                    worst_sp = sp
            if worst_sp:
                next_cmds.append(
                    f"just q hunt-success {log_dir} --species={worst_sp}"
                )

    return Envelope(
        query=query,
        scan_stats={"scanned": len(records), "returned": len(results),
                    "more_available": more,
                    "narrow_by": ["cat", "species", "tick_range"]},
        results=results, narrative=narrative,
        next=list(dict.fromkeys(next_cmds)),
    )


# ── subtool: footer ─────────────────────────────────────────────────────────
# Full footer drill-down. Complements `run-summary` which only exposes a
# curated subset. Use when you need a specific footer field that
# `run-summary` doesn't surface (e.g., `interrupts_by_reason`,
# `plan_failures_by_reason`, the full `continuity_tallies`).

def cmd_footer(args: argparse.Namespace) -> Envelope:
    log_dir = Path(args.log_dir)
    ev_path = events_path(log_dir)
    field = args.field
    top_keys = args.top_keys

    query = {
        "subtool": "footer", "log_dir": str(log_dir),
        "field": field, "top_keys": top_keys,
    }

    footer_records = run_jq("select(._footer)", ev_path)
    if not footer_records:
        return Envelope(
            query=query,
            scan_stats={"scanned": 0, "returned": 0, "more_available": False,
                        "narrow_by": []},
            results=[],
            narrative=(
                "No _footer line in events.jsonl. The sim may have aborted "
                "before footer emission. Check the last events with "
                f"`just q events {log_dir} --limit=20` and look for an "
                "abort/panic/stop signal."
            ),
            next=[f"just q events {log_dir} --limit=20"],
        )

    f = footer_records[0]

    # --field=<name> mode: drill into one specific field.
    if field:
        if field not in f:
            available = sorted(k for k in f.keys() if not k.startswith("_"))
            return Envelope(
                query=query,
                scan_stats={"scanned": 1, "returned": 0,
                            "more_available": False,
                            "narrow_by": []},
                results=[],
                narrative=(
                    f"Footer has no field '{field}'. "
                    f"Available: {', '.join(available)}."
                ),
                next=[f"just q footer {log_dir}"],
            )
        value = f[field]
        # Dict-valued: rank entries.
        if isinstance(value, dict):
            items = [(k, v) for k, v in value.items()
                     if isinstance(v, (int, float))]
            items.sort(key=lambda kv: -kv[1])
            if top_keys:
                items = items[:top_keys]
            results = [{
                "id": f"footer:{field}:{k}",
                "kind": "footer_field_entry",
                "field": field,
                "key": k,
                "value": v,
                "summary": f"{field}.{k} = {v}",
            } for k, v in items]
            total = sum(v for _, v in items) if items else 0
            narrative = (
                f"footer.{field}: {len(items)} entries"
                + (f" (top {top_keys})" if top_keys else "")
                + (f", total={total}" if total else "")
                + "."
            )
            return Envelope(
                query=query,
                scan_stats={"scanned": 1, "returned": len(results),
                            "more_available": False,
                            "narrow_by": ["top_keys"]},
                results=results, narrative=narrative, next=[],
            )
        # List-valued.
        if isinstance(value, list):
            results = [{
                "id": f"footer:{field}:{i}",
                "kind": "footer_field_entry",
                "field": field,
                "index": i,
                "value": v,
                "summary": str(v),
            } for i, v in enumerate(value)]
            return Envelope(
                query=query,
                scan_stats={"scanned": 1, "returned": len(results),
                            "more_available": False, "narrow_by": []},
                results=results,
                narrative=f"footer.{field}: list of {len(value)} entries.",
                next=[],
            )
        # Scalar.
        return Envelope(
            query=query,
            scan_stats={"scanned": 1, "returned": 1,
                        "more_available": False, "narrow_by": []},
            results=[{
                "id": f"footer:{field}",
                "kind": "footer_field",
                "field": field,
                "value": value,
                "summary": f"{field} = {value}",
            }],
            narrative=f"footer.{field} = {value}.",
            next=[],
        )

    # No --field: dump every top-level field as a result.
    results: list[dict[str, Any]] = []
    for k in sorted(f.keys()):
        if k.startswith("_"):
            continue
        v = f[k]
        if isinstance(v, dict):
            kind_summary = (
                f"{k}: {len(v)} entries"
                + (f" (top: {max(v.items(), key=lambda kv: kv[1])[0]})"
                   if v and all(isinstance(x, (int, float)) for x in v.values())
                   else "")
            )
        elif isinstance(v, list):
            kind_summary = f"{k}: list[{len(v)}]"
        else:
            kind_summary = f"{k} = {v}"
        results.append({
            "id": f"footer:{k}",
            "kind": "footer_field",
            "field": k,
            "value": v,
            "summary": kind_summary,
        })

    narrative = (
        f"Footer with {len(results)} fields. Drill into any with "
        f"`just q footer {log_dir} --field=<name>`."
    )
    next_cmds: list[str] = []
    if isinstance(f.get("interrupts_by_reason"), dict) and f["interrupts_by_reason"]:
        next_cmds.append(
            f"just q footer {log_dir} --field=interrupts_by_reason --top-keys=5"
        )
    if isinstance(f.get("plan_failures_by_reason"), dict) and f["plan_failures_by_reason"]:
        next_cmds.append(
            f"just q footer {log_dir} --field=plan_failures_by_reason --top-keys=5"
        )

    return Envelope(
        query=query,
        scan_stats={"scanned": 1, "returned": len(results),
                    "more_available": False, "narrow_by": ["field"]},
        results=results, narrative=narrative, next=next_cmds,
    )


# ── subtool: anomalies ──────────────────────────────────────────────────────
# Promotes `check-canaries` + `check-continuity` output into the envelope.

def cmd_anomalies(args: argparse.Namespace) -> Envelope:
    log_dir = Path(args.log_dir)
    ev_path = events_path(log_dir)
    query = {"subtool": "anomalies", "log_dir": str(log_dir)}

    footer = run_jq("select(._footer)", ev_path)
    if not footer:
        return Envelope(
            query=query,
            scan_stats={"scanned": 0, "returned": 0, "more_available": False,
                        "narrow_by": []},
            results=[],
            narrative="No footer found — run may have aborted. "
                      "Inspect the last 20 events with `just q events "
                      f"{log_dir} --limit=20 --offset=<scanned-20>`.",
            next=[],
        )
    f = footer[0]

    results: list[dict[str, Any]] = []
    next_cmds: list[str] = []

    # Canary 1: starvation == 0.
    starv = (f.get("deaths_by_cause") or {}).get("Starvation", 0)
    if starv > 0:
        results.append({
            "id": "anomaly:starvation",
            "kind": "canary",
            "name": "starvation_deaths",
            "severity": "fail",
            "value": starv,
            "target": "== 0",
            "summary": f"starvation_deaths={starv} (target == 0)",
        })
        next_cmds.append(f"just q deaths {log_dir} --cause=Starvation")

    # Canary 2: shadowfox ambush <= 5.
    sfx = (f.get("deaths_by_cause") or {}).get("ShadowFoxAmbush", 0)
    if sfx > 5:
        results.append({
            "id": "anomaly:shadowfox_ambush",
            "kind": "canary",
            "name": "shadowfox_ambush_deaths",
            "severity": "fail",
            "value": sfx,
            "target": "<= 5",
            "summary": f"shadowfox_ambush_deaths={sfx} (target <= 5)",
        })
        next_cmds.append(f"just q deaths {log_dir} --cause=ShadowFoxAmbush")

    # Canary 3: never_fired_expected_positives.
    nf = f.get("never_fired_expected_positives") or []
    for feature in nf:
        results.append({
            "id": f"anomaly:never_fired:{feature}",
            "kind": "canary",
            "name": "never_fired_expected",
            "severity": "fail",
            "feature": feature,
            "target": "any activation",
            "summary": f"expected positive '{feature}' never fired",
        })
    if nf:
        next_cmds.append(
            f'just q events {log_dir} --kind=FeatureActivated --limit=20'
        )

    # Continuity tallies: any zero tally is a fail.
    ct = f.get("continuity_tallies") or {}
    for canary in ("grooming", "play", "mentoring", "burial", "courtship", "mythic-texture"):
        v = ct.get(canary, 0)
        if v == 0:
            results.append({
                "id": f"anomaly:continuity:{canary}",
                "kind": "continuity",
                "name": canary,
                "severity": "fail",
                "value": 0,
                "target": "> 0",
                "summary": f"continuity/{canary}=0 (target > 0)",
            })

    # Feature-never-fired sweep (from feature-totals recipe): find features
    # at 0 in SystemActivation streams (approximate via footer if present).
    # ColonyScore cliff detection: sample a few ColonyScore events and flag
    # any drop > 0.3 between adjacent samples.
    scores = run_jq('select(.type=="ColonyScore") | {tick, aggregate}', ev_path)
    scores.sort(key=lambda r: r.get("tick") or 0)
    cliffs: list[tuple[int, float, float]] = []
    # A "cliff" = absolute drop > 0.3 AND relative drop > 20% of the prior
    # value. Relative guard keeps us from false-positiving on a 959.8→959.4
    # step when the aggregate has drifted into a large scale mid-run.
    for i in range(1, len(scores)):
        a = scores[i - 1].get("aggregate")
        b = scores[i].get("aggregate")
        if not (isinstance(a, (int, float)) and isinstance(b, (int, float))):
            continue
        drop = a - b
        if drop > 0.3 and a > 0 and (drop / a) > 0.2:
            cliffs.append((scores[i].get("tick") or 0, a, b))
    for tick, a, b in cliffs[:5]:
        results.append({
            "id": f"anomaly:colony_score_cliff:{tick}",
            "kind": "score_cliff",
            "name": "colony_score_cliff",
            "severity": "warn",
            "tick": tick,
            "from": round(a, 3),
            "to": round(b, 3),
            "summary": f"ColonyScore cliff at tick {tick}: {a:.3f} → {b:.3f}",
        })
    if cliffs:
        tick = cliffs[0][0]
        next_cmds.append(
            f"just q events {log_dir} --tick-range={max(0, tick-1000)}..{tick+1000}"
        )

    if not results:
        narrative = (
            "No anomalies. Canaries pass, continuity tallies non-zero, "
            "ColonyScore trajectory smooth."
        )
    else:
        fails = [r for r in results if r.get("severity") == "fail"]
        warns = [r for r in results if r.get("severity") == "warn"]
        narrative = f"{len(fails)} canary failures and {len(warns)} warnings."

    return Envelope(
        query=query,
        scan_stats={"scanned": len(scores) + len(nf) + len(ct),
                    "returned": len(results),
                    "more_available": False,
                    "narrow_by": []},
        results=results, narrative=narrative,
        next=list(dict.fromkeys(next_cmds)),  # de-dupe preserving order
    )


# ── argparse wiring ─────────────────────────────────────────────────────────

def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(
        prog="logq",
        description="Query tools over clowder sim logs. Wraps jq recipes "
                    "from docs/diagnostics/log-queries.md.",
    )
    p.add_argument("--format", choices=("json", "text"), default="json",
                   help="Output format. Default json (for tool callers). "
                        "Use text for human CLI reading.")
    p.add_argument("--rationale", default=None,
                   help="Why this query was issued (free text). Appended to "
                        "logs/agent-call-history.jsonl alongside the subtool "
                        "name and args; lets future review surface patterns of "
                        "what callers were trying to figure out. Always pass "
                        "when invoked by an agent.")
    sub = p.add_subparsers(dest="subtool", required=True)

    s = sub.add_parser("run-summary", help="Header + footer + joinability check.")
    s.add_argument("log_dir")
    s.set_defaults(func=cmd_run_summary)

    s = sub.add_parser("events", help="Generic event filter.")
    s.add_argument("log_dir")
    s.add_argument("--kind", help="Comma-separated event types "
                                   "(e.g. Death,FeatureActivated).")
    s.add_argument("--tick-range", help="Tick range A..B (inclusive).")
    s.add_argument("--cat", help="Filter by cat name (or posse membership).")
    s.add_argument("--limit", type=int, help="Page size.")
    s.add_argument("--offset", type=int, default=0, help="Page offset.")
    s.set_defaults(func=cmd_events)

    s = sub.add_parser("deaths", help="Death events with cause/cat/location.")
    s.add_argument("log_dir")
    s.add_argument("--cause", help="Filter by death cause "
                                    "(Starvation, ShadowFoxAmbush, ...).")
    s.add_argument("--tick-range", help="Tick range A..B.")
    s.add_argument("--cat", help="Filter by cat.")
    s.set_defaults(func=cmd_deaths)

    s = sub.add_parser("narrative",
                       help="Narrative lines (default tiers: Legend/Danger/Significant).")
    s.add_argument("log_dir")
    s.add_argument("--tier", help="Comma-separated tiers. Default Legend,Danger,Significant.")
    s.add_argument("--tick-range", help="Tick range A..B.")
    s.set_defaults(func=cmd_narrative)

    s = sub.add_parser("trace", help="Per-cat trace slicing (L1/L2/L3).")
    s.add_argument("log_dir")
    s.add_argument("cat")
    s.add_argument("--layer", choices=("L1", "L2", "L3"), default="L3")
    s.add_argument("--tick-range", help="Tick range A..B.")
    s.add_argument("--top-dses", type=int,
                   help="Top-N DSEs (L3 chosen or L2 evals). Default 5/8.")
    s.set_defaults(func=cmd_trace)

    s = sub.add_parser("cat-timeline",
                       help="Events + narrative lines for one cat, merged by tick.")
    s.add_argument("log_dir")
    s.add_argument("cat")
    s.add_argument("--tick-range", help="Tick range A..B.")
    s.add_argument("--limit", type=int,
                   help=f"Page size (default {CAT_TIMELINE_DEFAULT_LIMIT}; "
                        f"pass 0 for full stream).")
    s.add_argument("--offset", type=int, default=0, help="Page offset.")
    s.add_argument("--summarize", action="store_true",
                   help="Aggregate event-type distribution + plan-create "
                        "cadence instead of dumping individual events. "
                        "Right surface for cats with tens of thousands "
                        "of events.")
    s.set_defaults(func=cmd_cat_timeline)

    s = sub.add_parser("actions",
                       help="Aggregate `current_action` from CatSnapshot "
                            "events (DSE-balance diagnosis surface).")
    s.add_argument("log_dir")
    s.add_argument("--cat", help="Filter to a single cat.")
    s.add_argument("--tick-range", help="Tick range A..B.")
    s.add_argument("--limit", type=int,
                   help="Limit ranked results (default unlimited — there "
                        "are usually fewer than 20 distinct actions).")
    s.add_argument("--offset", type=int, default=0, help="Page offset.")
    s.set_defaults(func=cmd_actions)

    s = sub.add_parser("hunt-success",
                       help="Per-discrete-attempt hunt outcomes from "
                            "HuntAttempt events (ticket 149). Use to compute "
                            "the per-attempt success rate against the "
                            "30-50% real-cat-biology target.")
    s.add_argument("log_dir")
    s.add_argument("--cat", help="Filter to a single cat.")
    s.add_argument("--species", help="Filter to a single prey species "
                                      "(e.g. mouse, bird, fish, rabbit).")
    s.add_argument("--tick-range", help="Tick range A..B.")
    s.add_argument("--limit", type=int,
                   help="Limit ranked outcome rows (default unlimited; "
                        "there are only 7 outcome variants).")
    s.add_argument("--offset", type=int, default=0, help="Page offset.")
    s.set_defaults(func=cmd_hunt_success)

    s = sub.add_parser("footer",
                       help="Drill into _footer fields. Without --field, "
                            "lists every top-level field; with --field, "
                            "drills into that one (ranking entries for "
                            "dict-valued fields).")
    s.add_argument("log_dir")
    s.add_argument("--field",
                   help="Specific footer field name to drill into "
                        "(e.g. interrupts_by_reason, plan_failures_by_reason).")
    s.add_argument("--top-keys", type=int,
                   help="For dict-valued fields, return only the top-N "
                        "entries by value.")
    s.set_defaults(func=cmd_footer)

    s = sub.add_parser("anomalies",
                       help="Canaries + continuity + ColonyScore cliffs "
                            "in the standard envelope.")
    s.add_argument("log_dir")
    s.set_defaults(func=cmd_anomalies)

    return p


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        env = args.func(args)
    except FileNotFoundError as e:
        msg = Envelope(
            query={"subtool": args.subtool, "log_dir": str(args.log_dir)},
            scan_stats={"scanned": 0, "returned": 0,
                        "more_available": False, "narrow_by": []},
            results=[], narrative=f"missing file: {e}", next=[],
        )
        emit(msg, fmt=args.format)
        append_call_history(tool="q", subtool=args.subtool, args=args,
                            rationale=args.rationale, exit_code=2)
        return 2
    emit(env, fmt=args.format)
    append_call_history(tool="q", subtool=args.subtool, args=args,
                        rationale=args.rationale, exit_code=0)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
