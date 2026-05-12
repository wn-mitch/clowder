#!/usr/bin/env python3
"""Extract the `_footer` line from an events.jsonl into a standalone JSON file.

Used by the balance-pass workflow's matrix soak jobs to capture a
self-describing per-run footer artifact before compressing the events log.
Reads the tail of the file (last 64KB) so it stays fast even on long
soaks — Clowder's footer is always the last non-empty JSON line.

If no `_footer` is present, writes an empty JSON object and exits 0 (the
aggregator reports the run as `missing-footer` and the matrix cell does
not fail). Exits non-zero only on argument errors or unreadable inputs.

Usage:
    python3 scripts/balance_pass_extract_footer.py <events.jsonl> <footer.json>
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

TAIL_BYTES = 65536


def extract_footer(events_path: Path) -> dict | None:
    if not events_path.exists() or events_path.stat().st_size == 0:
        return None
    with events_path.open("rb") as f:
        f.seek(0, 2)
        size = f.tell()
        f.seek(max(0, size - TAIL_BYTES))
        tail = f.read().decode("utf-8", errors="replace")
    for line in reversed([ln for ln in tail.splitlines() if ln.strip()]):
        try:
            obj = json.loads(line)
        except ValueError:
            continue
        if obj.get("_footer"):
            return obj
    return None


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print("usage: balance_pass_extract_footer.py <events.jsonl> <footer.json>", file=sys.stderr)
        return 64
    events_path = Path(argv[0])
    out_path = Path(argv[1])
    footer = extract_footer(events_path)
    if footer is None:
        print(f"::warning::no _footer in {events_path}; writing empty footer.json", file=sys.stderr)
        footer = {}
    out_path.write_text(json.dumps(footer, indent=2) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
