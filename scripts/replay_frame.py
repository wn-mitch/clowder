#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""
Focal-cat trace replay — reconstructs a full decision frame by pivoting
trace records on (tick, cat).

Reads `logs/trace-<focal>.jsonl` (or `--trace PATH`), joins L1/L2/L3
records for the requested (tick, cat), and prints the frame top-down:

    L1 samples → L2 per-DSE evaluations → L3 selection.

Primary acceptance gate for Phase 1 of the AI substrate refactor: the
ranked-DSE list emitted by `replay_frame.py` for a given (tick, cat)
must match that cat's `CatSnapshot.last_scores` in `events.jsonl` for
the same tick. A mismatch means the trace emitter and the snapshot
emitter have drifted — the trace format is broken for replay.

Usage:
    uv run scripts/replay_frame.py --tick N --cat NAME
    uv run scripts/replay_frame.py --tick N --cat NAME --trace logs/trace-Simba.jsonl
    uv run scripts/replay_frame.py --tick N --cat NAME --verify-against logs/events.jsonl

Per §11 of docs/systems/ai-substrate-refactor.md.
"""

import argparse
import json
import sys
from pathlib import Path


def read_header_and_records(path: Path):
    """Parse a JSONL trace file. Returns (header, records)."""
    header = None
    records = []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            obj = json.loads(line)
            if obj.get("_header"):
                header = obj
                continue
            records.append(obj)
    return header, records


def select_frame(records, tick, cat):
    """Filter records for a given (tick, cat), grouped by layer."""
    by_layer = {"L1": [], "L2": [], "L3": []}
    for r in records:
        if r.get("tick") != tick or r.get("cat") != cat:
            continue
        layer = r.get("layer")
        if layer in by_layer:
            by_layer[layer].append(r)
    return by_layer


def print_l1(records):
    if not records:
        print("  (no L1 samples this tick)")
        return
    for r in records:
        print(
            f"  {r['map']:>20}  faction={r['faction']}  channel={r['channel']}"
            f"  pos={tuple(r['pos'])}  base={r['base_sample']:.3f}"
            f"  perceived={r['perceived']:.3f}"
        )
        atten = r.get("attenuation", {})
        print(
            f"                        attenuation: "
            f"species×{atten.get('species_sens', 1.0):.2f} · "
            f"role×{atten.get('role_mod', 1.0):.2f} · "
            f"injury+{atten.get('injury_deficit', 0.0):.2f} · "
            f"env×{atten.get('env_mul', 1.0):.2f}"
        )
        contribs = r.get("top_contributors", [])
        if contribs:
            print(f"                        top contributors:")
            for c in contribs[:3]:
                print(
                    f"                          {c['emitter']} @ {tuple(c['pos'])} "
                    f"(d={c['distance']}, +{c['contribution']:.3f})"
                )


def print_l2(records):
    if not records:
        print("  (no L2 evaluations this tick)")
        return
    # Sort by final_score descending so the winning DSEs surface.
    ranked = sorted(records, key=lambda r: r.get("final_score", 0.0), reverse=True)
    for r in ranked:
        passed = "✓" if r["eligibility"]["passed"] else "✗"
        print(
            f"  {passed} {r['dse']:>16}  final={r['final_score']:+.3f}"
            f"  (composition={r['composition']['mode']}, raw={r['composition']['raw']:+.3f},"
            f" maslow={r['maslow_pregate']:.2f})"
        )
        cons = r.get("considerations", [])
        if cons:
            for c in cons:
                spatial = c.get("spatial")
                spatial_str = (
                    f"  [{spatial['map']}→{spatial.get('best_target', '?')}]"
                    if spatial
                    else ""
                )
                print(
                    f"      · {c['name']:>18}  input={c['input']:+.3f}"
                    f"  curve={c['curve']}  score={c['score']:+.3f}"
                    f"  w={c['weight']:.2f}{spatial_str}"
                )
        mods = r.get("modifiers", [])
        for m in mods:
            if m.get("delta") is not None:
                print(f"      ± modifier {m['name']}: {m['delta']:+.3f}")
            elif m.get("multiplier") is not None:
                print(f"      × modifier {m['name']}: ×{m['multiplier']:.2f}")
        losing = r.get("top_losing", [])
        if losing:
            print(f"      top-losing axes:")
            for l in losing:
                print(
                    f"        {l['axis']}: score={l['score']:+.3f} deficit={l['deficit']:+.3f}"
                )


def print_l3(records):
    if not records:
        print("  (no L3 selection this tick)")
        return
    # There should be exactly one L3 per (tick, cat).
    r = records[0]
    print(f"  chosen: {r['chosen']}")
    print(f"  intention: {json.dumps(r['intention'], separators=(', ', ': '))}")
    ranked = r.get("ranked", [])
    sm = r.get("softmax", {})
    probs = sm.get("probabilities", [])
    print(f"  softmax temperature: {sm.get('temperature', 'N/A')}")
    print(f"  ranked (top 5):")
    for i, (name, score) in enumerate(ranked[:5]):
        p = f"  p={probs[i]:.3f}" if i < len(probs) else ""
        print(f"    {i+1}. {name:>16}  score={score:+.3f}{p}")
    mom = r.get("momentum", {})
    print(
        f"  momentum: active={mom.get('active_intention', 'None')}"
        f"  commitment={mom.get('commitment_strength', 0.0):.3f}"
        f"  preempted={mom.get('preempted', False)}"
    )
    plan = r.get("goap_plan", [])
    if plan:
        print(f"  goap plan ({len(plan)} steps):")
        for step in plan[:8]:
            print(f"    → {step}")
    if len(records) > 1:
        print(f"  note: {len(records)} L3 records at this tick — expected 1")


def extract_last_scores_from_events(events_path, tick, cat):
    """Scan events.jsonl for the CatSnapshot at/near (tick, cat) and
    return its `last_scores` list. Returns None if no snapshot found.

    CatSnapshot only emits every `full_snapshot_interval` ticks (default
    100), so tick matching is "nearest snapshot at-or-before tick".
    """
    best = None
    best_tick = -1
    with open(events_path) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                e = json.loads(line)
            except json.JSONDecodeError:
                continue
            if e.get("_header") or e.get("_footer"):
                continue
            if e.get("type") != "CatSnapshot":
                continue
            if e.get("cat") != cat:
                continue
            t = e.get("tick", -1)
            if t <= tick and t > best_tick:
                best = e
                best_tick = t
    if best is None:
        return None, None
    return best_tick, best.get("last_scores", [])


def verify_ranked_matches_snapshot(trace_l3, events_path, tick, cat):
    """Phase 1 acceptance gate: the trace's L3 ranked list must match
    the same cat's CatSnapshot.last_scores at the nearest prior tick."""
    if not trace_l3:
        print("  verify: no L3 record in trace — cannot verify")
        return False
    snap_tick, last_scores = extract_last_scores_from_events(events_path, tick, cat)
    if last_scores is None:
        print(f"  verify: no CatSnapshot for '{cat}' at or before tick {tick}")
        return False

    trace_ranked = trace_l3[0].get("ranked", [])
    # last_scores is [[Action, f32]]; trace_ranked is [[String, f32]].
    # Action serializes to its variant name (unquoted in JSON array).
    print(f"  verify: comparing trace[tick={tick}] vs snapshot[tick={snap_tick}]")
    trace_names = [name for name, _ in trace_ranked]
    snap_names = [name for name, _ in last_scores]
    if trace_names == snap_names:
        print(f"  verify: ✓ ranked DSE order matches (n={len(trace_names)})")
        return True
    else:
        print("  verify: ✗ ranked DSE order diverges")
        for i, (t, s) in enumerate(zip(trace_names, snap_names)):
            marker = "  " if t == s else "!!"
            print(f"    {marker} [{i}] trace={t}  snapshot={s}")
        if len(trace_names) != len(snap_names):
            print(f"  verify:   length mismatch: trace={len(trace_names)} snapshot={len(snap_names)}")
        return False


def main():
    p = argparse.ArgumentParser(description=__doc__.strip().split("\n\n")[0])
    p.add_argument("--tick", type=int, required=True, help="Tick to replay")
    p.add_argument("--cat", type=str, required=True, help="Focal cat name")
    p.add_argument("--trace", type=Path, help="Trace sidecar path (default: logs/trace-<cat>.jsonl)")
    p.add_argument(
        "--verify-against",
        type=Path,
        help="Run the Phase-1 acceptance check: compare ranked DSE list against the events.jsonl CatSnapshot.",
    )
    args = p.parse_args()

    trace_path = args.trace or Path(f"logs/trace-{args.cat}.jsonl")
    if not trace_path.exists():
        print(f"error: trace file not found: {trace_path}", file=sys.stderr)
        sys.exit(2)

    header, records = read_header_and_records(trace_path)
    if header is None:
        print(f"warning: no header in {trace_path}", file=sys.stderr)

    frame = select_frame(records, args.tick, args.cat)
    total = sum(len(frame[l]) for l in frame)
    if total == 0:
        print(f"no records for (tick={args.tick}, cat={args.cat!r}) in {trace_path}")
        # Be helpful: show tick range and cats available.
        seen_cats = sorted({r.get("cat") for r in records if r.get("cat")})
        ticks = sorted({r.get("tick") for r in records if r.get("tick") is not None})
        if ticks:
            print(f"  available tick range: {ticks[0]} … {ticks[-1]}")
        if seen_cats:
            print(f"  cats seen: {', '.join(seen_cats)}")
        sys.exit(1)

    print(f"=== frame tick={args.tick} cat={args.cat} ===")
    if header:
        print(
            f"trace {trace_path.name}  commit={header.get('commit_hash_short', '?')}"
            f"  seed={header.get('seed', '?')}"
        )
    print()
    print("L1 (sensed):")
    print_l1(frame["L1"])
    print()
    print("L2 (DSE evaluations):")
    print_l2(frame["L2"])
    print()
    print("L3 (selection):")
    print_l3(frame["L3"])

    if args.verify_against:
        print()
        print("=== acceptance verification ===")
        ok = verify_ranked_matches_snapshot(
            frame["L3"], args.verify_against, args.tick, args.cat
        )
        sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
