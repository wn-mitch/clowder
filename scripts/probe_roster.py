#!/usr/bin/env python3
"""Probe a seed's founder roster and pick focal cats for the baseline dataset.

Runs a short headless soak (default 60s) per seed, parses CatSnapshot records
from events.jsonl, and emits a roster.json under the dataset root with a
focal-cat pick per seed:

- Slot A: an Adult generalist — covers Hunt / Eat / Socialize / Mate /
  Mentor / GroomOther / general DSE evaluation.
- Slot B: a marker-diverse cat — Priestess-leaning if any (highest
  ``skills.magic`` ceiling), else any Adult that is not Slot A; falls back
  to Slot A if the seed has only one Adult during the probe window.

Selection uses CatSnapshot records (no marker access from the JSONL today —
markers are inferred from the snapshot fields).

Usage:
    probe_roster.py --label baseline-2026-04-25 \\
        --seeds 42,99,7,2025,314 \\
        --duration 60 \\
        --binary target/release/clowder \\
        [--parallel 4]

Writes:
    logs/baseline-<label>/probe/<seed>/events.jsonl
    logs/baseline-<label>/probe/<seed>/narrative.jsonl
    logs/baseline-<label>/probe/<seed>/stderr.log
    logs/baseline-<label>/rosters.json

The rosters.json shape:
    {
      "label": "...",
      "duration_secs": 60,
      "seeds": {
        "42": {
          "cats": [
            {"name": "Simba", "life_stage": "Adult",
             "magic_skill_peak": 0.034, "actions_seen": ["Sleep","Hunt"]},
            ...
          ],
          "slot_a": "Simba",
          "slot_b": "<priestess>",
          "slot_b_reason": "highest_magic_skill"
        },
        ...
      }
    }
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path
from typing import Any


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--label", required=True, help="Dataset label (controls output path).")
    p.add_argument("--seeds", required=True, help="Comma-separated seed list.")
    p.add_argument("--duration", type=int, default=60, help="Probe soak duration in seconds.")
    p.add_argument("--binary", default="target/release/clowder", help="Path to clowder release binary.")
    p.add_argument("--parallel", type=int, default=4, help="Parallel probe runs.")
    p.add_argument("--root", default="logs", help="Logs root directory.")
    return p.parse_args()


def run_probe(binary: str, seed: int, out_dir: Path, duration: int) -> tuple[int, Path, str]:
    """Run a single probe soak. Returns (seed, events_path, stderr_text)."""
    out_dir.mkdir(parents=True, exist_ok=True)
    events = out_dir / "events.jsonl"
    narrative = out_dir / "narrative.jsonl"
    stderr_path = out_dir / "stderr.log"

    # Idempotency: if events.jsonl already has a _footer, skip the run.
    if events.exists() and _has_footer(events):
        return seed, events, "(skipped — _footer already present)"

    cmd = [
        binary, "--headless",
        "--seed", str(seed),
        "--duration", str(duration),
        "--log", str(narrative),
        "--event-log", str(events),
    ]
    with stderr_path.open("w") as err:
        rc = subprocess.run(cmd, stdout=err, stderr=subprocess.STDOUT).returncode
    if rc != 0:
        return seed, events, f"(probe rc={rc} — see {stderr_path})"
    return seed, events, "ok"


def _has_footer(path: Path) -> bool:
    """Cheap last-line check for the _footer sentinel."""
    if not path.exists() or path.stat().st_size == 0:
        return False
    # Tail-read the last few KB; the footer is the last non-empty line.
    with path.open("rb") as f:
        f.seek(0, 2)
        size = f.tell()
        f.seek(max(0, size - 16384))
        tail = f.read().decode("utf-8", errors="replace")
    for line in reversed([l for l in tail.splitlines() if l.strip()]):
        try:
            obj = json.loads(line)
        except ValueError:
            continue
        return bool(obj.get("_footer"))
    return False


def parse_roster(events_path: Path) -> list[dict[str, Any]]:
    """Walk events.jsonl, extract per-cat aggregate from CatSnapshot records."""
    cats: dict[str, dict[str, Any]] = {}
    with events_path.open() as f:
        for line in f:
            try:
                ev = json.loads(line)
            except ValueError:
                continue
            if ev.get("type") != "CatSnapshot":
                continue
            name = ev.get("cat")
            if not name:
                continue
            entry = cats.setdefault(name, {
                "name": name,
                "life_stage_history": set(),
                "magic_skill_peak": 0.0,
                "actions_seen": set(),
                "snapshot_count": 0,
                "personality": ev.get("personality", {}),
                "sex": ev.get("sex"),
            })
            entry["snapshot_count"] += 1
            ls = ev.get("life_stage")
            if ls:
                entry["life_stage_history"].add(ls)
            mag = (ev.get("skills") or {}).get("magic", 0.0)
            if isinstance(mag, (int, float)) and mag > entry["magic_skill_peak"]:
                entry["magic_skill_peak"] = float(mag)
            act = ev.get("current_action")
            if act:
                entry["actions_seen"].add(act)
    # Normalise sets → sorted lists for JSON.
    out = []
    for entry in cats.values():
        entry["life_stage_history"] = sorted(entry["life_stage_history"])
        entry["actions_seen"] = sorted(entry["actions_seen"])
        # The "current" stage: prefer the most-mature observed
        order = ["Kitten", "Young", "Adult", "Elder"]
        stages = entry["life_stage_history"]
        if stages:
            entry["life_stage"] = max(stages, key=lambda s: order.index(s) if s in order else -1)
        else:
            entry["life_stage"] = None
        out.append(entry)
    return sorted(out, key=lambda c: c["name"])


def pick_focals(cats: list[dict[str, Any]]) -> dict[str, Any]:
    """Pick Slot A (Adult generalist) and Slot B (Priestess-lean) per the heuristic."""
    if not cats:
        return {"slot_a": None, "slot_b": None, "slot_b_reason": "no_cats_observed"}

    # Slot A: Adults preferred, else Young, else any cat. Within the bucket,
    # take the first alphabetically for determinism.
    by_stage = {"Adult": [], "Young": [], "Kitten": [], "Elder": [], None: []}
    for c in cats:
        by_stage.setdefault(c.get("life_stage"), []).append(c)
    for stage in ("Adult", "Young", "Elder", "Kitten", None):
        if by_stage.get(stage):
            slot_a = by_stage[stage][0]["name"]
            break
    else:
        slot_a = cats[0]["name"]

    # Slot B: highest magic_skill_peak among cats != Slot A. Prefer Adults but
    # fall back to any. If everyone is at floor (no real magic divergence in
    # 60s), pick the next cat by name as a generalist B.
    candidates = [c for c in cats if c["name"] != slot_a]
    if not candidates:
        return {"slot_a": slot_a, "slot_b": slot_a, "slot_b_reason": "single_cat_seed"}
    candidates.sort(key=lambda c: (-c["magic_skill_peak"], c["name"]))
    top = candidates[0]
    if top["magic_skill_peak"] > 0.05:
        return {"slot_a": slot_a, "slot_b": top["name"], "slot_b_reason": f"highest_magic_skill={top['magic_skill_peak']:.4f}"}
    # Magic floor — fall back to first non-A Adult, else any non-A.
    adult_b = next((c for c in candidates if c.get("life_stage") == "Adult"), None)
    if adult_b is not None:
        return {"slot_a": slot_a, "slot_b": adult_b["name"], "slot_b_reason": "adult_generalist_b"}
    return {"slot_a": slot_a, "slot_b": top["name"], "slot_b_reason": "fallback_first_non_a"}


def main() -> int:
    args = parse_args()
    binary = Path(args.binary)
    if not binary.exists() or not binary.is_file():
        print(f"error: binary {binary} not found — run `cargo build --release` first", file=sys.stderr)
        return 2

    seeds = [int(s.strip()) for s in args.seeds.split(",") if s.strip()]
    base = Path(args.root) / f"baseline-{args.label}"
    probe_dir = base / "probe"
    base.mkdir(parents=True, exist_ok=True)

    print(f"[probe] running {len(seeds)} probes (duration={args.duration}s, parallel={args.parallel})", file=sys.stderr)
    results: dict[int, tuple[Path, str]] = {}
    with ThreadPoolExecutor(max_workers=args.parallel) as ex:
        futures = {
            ex.submit(run_probe, str(binary), seed, probe_dir / str(seed), args.duration): seed
            for seed in seeds
        }
        for fut in as_completed(futures):
            seed, events, msg = fut.result()
            results[seed] = (events, msg)
            print(f"[probe] seed {seed}: {msg}", file=sys.stderr)

    rosters: dict[str, Any] = {
        "label": args.label,
        "duration_secs": args.duration,
        "seeds": {},
    }
    for seed in seeds:
        events_path, msg = results[seed]
        if not events_path.exists() or not _has_footer(events_path):
            rosters["seeds"][str(seed)] = {"error": msg, "cats": []}
            continue
        cats = parse_roster(events_path)
        focals = pick_focals(cats)
        rosters["seeds"][str(seed)] = {
            "cats": [
                {
                    "name": c["name"],
                    "life_stage": c["life_stage"],
                    "sex": c["sex"],
                    "magic_skill_peak": round(c["magic_skill_peak"], 5),
                    "actions_seen": c["actions_seen"],
                    "snapshot_count": c["snapshot_count"],
                }
                for c in cats
            ],
            **focals,
        }
        print(
            f"[probe] seed {seed}: {len(cats)} cats; "
            f"slot_a={focals['slot_a']} slot_b={focals['slot_b']} ({focals['slot_b_reason']})",
            file=sys.stderr,
        )

    out_path = base / "rosters.json"
    out_path.write_text(json.dumps(rosters, indent=2) + "\n")
    print(f"[probe] wrote {out_path}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
