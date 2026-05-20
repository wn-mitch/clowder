#!/usr/bin/env python3
"""Aggregate samply profile samples by symbol name.

Reads a samply profile directory (containing `profile.json.gz` + the
`profile.json.syms.json` presymbolicate sidecar) and emits two views:

  1. Top-N SELF-time symbols (where samples land directly — the
     leaf frames). High self% with low inclusive% means a tight
     inner loop; high in both means cumulative hot work.

  2. Top-N INCLUSIVE-time symbols (samples passing through anywhere
     in the stack). Identifies which high-level systems own the
     CPU regardless of where the time bottoms out.

Plus a `--target` flag for §428-style attribution: pass a substring
and the script reports total inclusive % for any frame containing it.

Resolves symbols via the Firefox-Profiler `funcTable.func` →
`resourceTable.lib` → `libs[].debugName` chain, then binary-searches
each frame's address inside the matching lib's symbol_table from the
`.syms.json` sidecar.

Usage:
  python3 scripts/profiling/samply_top.py <profile-dir>
  python3 scripts/profiling/samply_top.py <profile-dir> --top 30
  python3 scripts/profiling/samply_top.py <profile-dir> --target resolve_goap_plans
"""

import argparse
import gzip
import json
import sys
from bisect import bisect_right
from collections import Counter, defaultdict
from pathlib import Path


def load_profile(profile_dir: Path):
    profile_path = profile_dir / "profile.json.gz"
    syms_path = profile_dir / "profile.json.syms.json"
    if not profile_path.exists():
        print(f"error: {profile_path} not found", file=sys.stderr)
        sys.exit(1)
    if not syms_path.exists():
        print(
            f"warning: {syms_path} not found — frames will show raw addresses",
            file=sys.stderr,
        )
    with gzip.open(profile_path, "rt") as f:
        profile = json.load(f)
    syms = {}
    if syms_path.exists():
        with open(syms_path) as f:
            syms = json.load(f)
    return profile, syms


def build_resolver(profile, syms):
    main = profile["threads"][0]
    frame_addr = main["frameTable"]["address"]
    frame_func = main["frameTable"]["func"]
    func_resource = main["funcTable"]["resource"]
    resource_lib = main["resourceTable"]["lib"]
    libs = profile["libs"]

    sym_by_debug_name = {}
    for entry in syms.get("data", []):
        table = sorted(entry["symbol_table"], key=lambda r: r["rva"])
        sym_by_debug_name[entry["debug_name"]] = (
            [r["rva"] for r in table],
            table,
        )
    syms_strings = syms.get("string_table", [])

    def frame_to_name(frame_idx: int) -> str:
        address = frame_addr[frame_idx]
        if address < 0:
            return f"<no-addr#{frame_idx}>"
        func_idx = frame_func[frame_idx]
        res_idx = func_resource[func_idx]
        lib_idx = resource_lib[res_idx]
        lib = libs[lib_idx]
        debug_name = lib.get("debugName") or lib.get("name", "")
        sym_pair = sym_by_debug_name.get(debug_name)
        if not sym_pair:
            return f"<{debug_name}>+0x{address:x}"
        rvas, rows = sym_pair
        idx = bisect_right(rvas, address) - 1
        if idx < 0:
            return f"<{debug_name}>+0x{address:x}"
        row = rows[idx]
        if address >= row["rva"] + row.get("size", 0):
            return f"<{debug_name}>+0x{address:x}"
        return syms_strings[row["symbol"]]

    return main, frame_to_name


def main():
    parser = argparse.ArgumentParser(
        description="Aggregate samply profile samples by symbol."
    )
    parser.add_argument(
        "profile_dir", help="Directory containing profile.json.gz + .syms.json"
    )
    parser.add_argument("--top", type=int, default=25, help="Top-N rows to print")
    parser.add_argument(
        "--target",
        action="append",
        default=[],
        help="Substring to attribute (repeatable). Reports inclusive percent "
        "for any frame containing this string.",
    )
    args = parser.parse_args()

    profile_dir = Path(args.profile_dir)
    profile, syms = load_profile(profile_dir)
    main_thread, frame_to_name = build_resolver(profile, syms)

    stack_prefix = main_thread["stackTable"]["prefix"]
    stack_frame = main_thread["stackTable"]["frame"]
    samples_stack = main_thread["samples"]["stack"]

    name_cache = {}

    def name(frame_idx: int) -> str:
        if frame_idx not in name_cache:
            name_cache[frame_idx] = frame_to_name(frame_idx)
        return name_cache[frame_idx]

    self_count = Counter()
    inclusive_count = Counter()
    parents_of = defaultdict(Counter)
    children_of = defaultdict(Counter)

    for stack_idx in samples_stack:
        if stack_idx is None:
            continue
        chain = []
        cur = stack_idx
        while cur is not None:
            chain.append(stack_frame[cur])
            cur = stack_prefix[cur]
        self_count[name(chain[0])] += 1
        seen = set()
        for i, f in enumerate(chain):
            nm = name(f)
            if nm not in seen:
                inclusive_count[nm] += 1
                seen.add(nm)
            if i + 1 < len(chain):
                parents_of[nm][name(chain[i + 1])] += 1
            if i > 0:
                children_of[nm][name(chain[i - 1])] += 1

    total = len(samples_stack)
    print(f"# Profile: {profile_dir}")
    print(f"# Samples: {total}\n")

    # Pull a generous pre-filter pool (200 frames or 5x requested),
    # then strip entry-point noise (frames present in >99% of samples
    # — typically main, exe entry, runtime bootstrap), then take top N.
    pre_filter_pool = max(args.top * 5, 200)
    candidates = [
        (nm, c)
        for nm, c in inclusive_count.most_common(pre_filter_pool)
        if 100.0 * c / total < 99.0
    ][: args.top]

    print(
        "| Rank | Symbol | Self % | Incl % | Top parent | Top child |\n"
        "|---:|---|---:|---:|---|---|"
    )
    for rank, (nm, incl) in enumerate(candidates, start=1):
        self_pct = 100.0 * self_count.get(nm, 0) / total
        incl_pct = 100.0 * incl / total
        parents = parents_of[nm].most_common(1)
        children = children_of[nm].most_common(1)
        top_parent = parents[0][0] if parents else "—"
        top_child = children[0][0] if children else "—"
        short = nm[:80] + ("…" if len(nm) > 80 else "")
        sp = top_parent[:50] + ("…" if len(top_parent) > 50 else "")
        sc = top_child[:50] + ("…" if len(top_child) > 50 else "")
        print(
            f"| {rank} | `{short}` | {self_pct:.2f}% | {incl_pct:.2f}% | `{sp}` | `{sc}` |"
        )

    if args.target:
        print("\n## Target attribution")
        for t in args.target:
            incl = sum(c for nm, c in inclusive_count.items() if t in nm)
            pct = 100.0 * incl / total if total else 0
            print(f"  {incl:6d}  {pct:5.2f}%  *{t}*")


if __name__ == "__main__":
    main()
