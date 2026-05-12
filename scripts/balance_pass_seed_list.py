#!/usr/bin/env python3
"""Deterministic seed list for the pre-alpha balance pass.

Used as the matrix input for `.github/workflows/balance-pass.yml`'s sweep
phase. First five seeds are the canonical baseline-5b set (see
`scripts/run_baseline_dataset.sh:50`) so the new campaign *extends* rather
than diverges from the current `docs/balance/healthy-colony.md` baseline.
Remaining seeds come from a SHA-256-derived sequence so re-running the
workflow produces the same seed set every time.

Usage:
    python3 scripts/balance_pass_seed_list.py [N]
        Print a JSON list of N seeds to stdout (default 100). Suitable for
        `fromJson(...)` consumption in a GitHub Actions matrix.

    python3 scripts/balance_pass_seed_list.py 5 --as-csv
        Comma-separated list of seeds (handy for `just sweep` smoke tests).
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys

# Canonical baseline-5b seeds (kept first so any analysis that compares
# pre-alpha vs the existing healthy-colony.md band has a 5-seed overlap).
CANONICAL_SEEDS: list[int] = [42, 99, 7, 2025, 314]

# Sanity cap. 1024 seeds at 15 min/job = 256 runner-hours; even on public
# Linux this is more than a pre-alpha pass should ever need.
MAX_SEEDS = 1024


def seed_list(n: int) -> list[int]:
    """Return n deterministic seeds: canonical-5 first, then hash-derived."""
    seeds = list(CANONICAL_SEEDS)
    seen = set(seeds)
    counter = 0
    while len(seeds) < n:
        counter += 1
        digest = hashlib.sha256(f"balance-pass:{counter}".encode()).digest()
        # 31 bits keeps the value safely inside i32 for any downstream
        # parser that interprets seeds as signed integers.
        candidate = int.from_bytes(digest[:4], "big") & 0x7FFF_FFFF
        if candidate == 0 or candidate in seen:
            continue
        seeds.append(candidate)
        seen.add(candidate)
    return seeds[:n]


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("n", type=int, nargs="?", default=100, help="number of seeds (default 100)")
    parser.add_argument("--as-csv", action="store_true", help="emit comma-separated instead of JSON")
    args = parser.parse_args(argv)

    if args.n < 1 or args.n > MAX_SEEDS:
        print(f"error: n must be in [1, {MAX_SEEDS}]", file=sys.stderr)
        return 64

    seeds = seed_list(args.n)
    if args.as_csv:
        print(",".join(str(s) for s in seeds))
    else:
        print(json.dumps(seeds))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
