#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""
Explain a sim_constants.rs field: doc comment, current value, read sites.

Resolves a dotted path (e.g. `magic.ward_decay_per_tick` or
`fulfillment.social_warmth_socialize_per_tick`) against the live
`SimConstants` struct as serialized in a recent `events.jsonl` header,
then greps the codebase for the field name to surface read sites.

If `logs/sensitivity-map.json` exists (Tier 4.2), per-knob → metric
Spearman rho is included in the output.

Usage:
    just explain magic.ward_decay_per_tick
    just explain fulfillment.social_warmth_socialize_per_tick --run logs/tuned-42
    just explain --list                                # list every dotted path

Exit codes: 0 if found, 1 if path not present, 2 on hard error.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parent.parent
SIM_CONSTANTS_RS = REPO_ROOT / "src" / "resources" / "sim_constants.rs"
SENSITIVITY_MAP = REPO_ROOT / "logs" / "sensitivity-map.json"


def find_default_run() -> Path | None:
    """Find a recent events.jsonl with a constants block to read values from."""
    candidates: list[Path] = []
    direct = REPO_ROOT / "logs" / "tuned-42" / "events.jsonl"
    if direct.exists():
        candidates.append(direct)
    legacy = REPO_ROOT / "logs" / "baseline-pre-substrate-refactor" / "events.jsonl"
    if legacy.exists():
        candidates.append(legacy)
    candidates += sorted(
        (REPO_ROOT / "logs").glob("tuned-*/events.jsonl"),
        key=lambda p: -p.stat().st_mtime,
    )
    for p in candidates:
        if p.exists() and p.stat().st_size > 0:
            return p
    return None


def read_constants(events_path: Path) -> dict[str, Any]:
    proc = subprocess.run(
        ["jq", "-c", "select(._header) | .constants", str(events_path)],
        capture_output=True, text=True,
    )
    if proc.returncode != 0 or not proc.stdout.strip():
        return {}
    line = next((l for l in proc.stdout.splitlines() if l.strip()), "")
    return json.loads(line) if line else {}


def lookup(d: dict[str, Any], dotted: str) -> Any:
    cur: Any = d
    for part in dotted.split("."):
        if not isinstance(cur, dict) or part not in cur:
            return None
        cur = cur[part]
    return cur


def list_paths(d: dict[str, Any], prefix: str = "") -> list[str]:
    out: list[str] = []
    for k, v in d.items():
        path = f"{prefix}{k}"
        if isinstance(v, dict):
            out.extend(list_paths(v, prefix=f"{path}."))
        else:
            out.append(path)
    return out


def find_doc_comment(field_name: str) -> str | None:
    """Walk sim_constants.rs to find the doc comment immediately preceding
    `pub <field_name>:`. Captures contiguous /// lines."""
    try:
        text = SIM_CONSTANTS_RS.read_text()
    except OSError:
        return None
    pattern = re.compile(
        r"((?:^[ \t]*///[^\n]*\n)+)\s*pub\s+" + re.escape(field_name) + r"\s*:",
        re.MULTILINE,
    )
    m = pattern.search(text)
    if not m:
        return None
    raw = m.group(1)
    return "\n".join(line.lstrip().lstrip("/").lstrip()
                     for line in raw.strip().splitlines())


def find_read_sites(field_name: str) -> list[str]:
    """Grep the codebase for `.<field_name>` references."""
    proc = subprocess.run(
        ["rg", "--no-heading", "-n", "-t", "rust",
         rf"\.{re.escape(field_name)}\b",
         str(REPO_ROOT / "src")],
        capture_output=True, text=True,
    )
    if proc.returncode not in (0, 1):
        return []
    sites: list[str] = []
    for line in proc.stdout.splitlines():
        m = re.match(r"^([^:]+):(\d+):", line)
        if m:
            try:
                rel = Path(m.group(1)).resolve().relative_to(REPO_ROOT)
            except ValueError:
                rel = Path(m.group(1))
            sites.append(f"{rel}:{m.group(2)}")
    return sites[:25]


def load_sensitivity(dotted_path: str) -> list[dict[str, Any]]:
    if not SENSITIVITY_MAP.exists():
        return []
    try:
        m = json.loads(SENSITIVITY_MAP.read_text())
        rows = m.get(dotted_path, [])
        return rows if isinstance(rows, list) else []
    except (json.JSONDecodeError, OSError):
        return []


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("path", nargs="?", help="Dotted constant path (e.g. magic.ward_decay_per_tick)")
    ap.add_argument("--run", default=None, help="events.jsonl path to read live values from")
    ap.add_argument("--list", action="store_true", help="List every dotted path and exit")
    ap.add_argument("--text", action="store_true", help="Human-readable output")
    args = ap.parse_args(argv)

    events = Path(args.run) if args.run else find_default_run()
    constants = read_constants(events) if events and events.exists() else {}

    if args.list:
        if not constants:
            sys.stderr.write("explain: no events.jsonl with constants block found; "
                             "run a soak first or pass --run.\n")
            return 2
        for p in sorted(list_paths(constants)):
            sys.stdout.write(p + "\n")
        return 0

    if not args.path:
        ap.print_help()
        return 2

    value = lookup(constants, args.path) if constants else None
    field_name = args.path.rsplit(".", 1)[-1]
    doc = find_doc_comment(field_name)
    sites = find_read_sites(field_name)
    sensitivity = load_sensitivity(args.path)

    envelope: dict[str, Any] = {
        "constant": args.path,
        "value": value,
        "doc": doc,
        "read_sites": sites,
        "sensitivity": sensitivity,
        "constants_source": str(events) if events else None,
    }

    if value is None and not constants:
        envelope["note"] = ("no events.jsonl with constants block was found — "
                            "value resolution skipped. Run a soak (`just soak`) "
                            "first or pass `--run <path>`.")

    if args.text:
        sys.stdout.write(f"explain: {args.path}\n")
        sys.stdout.write(f"  value:    {value}\n")
        if doc:
            sys.stdout.write("  doc:      " + doc.replace("\n", "\n            ") + "\n")
        else:
            sys.stdout.write("  doc:      (no /// comment immediately before field)\n")
        if sites:
            sys.stdout.write("  used in:\n")
            for s in sites[:10]:
                sys.stdout.write(f"    {s}\n")
        if sensitivity:
            sys.stdout.write("  sensitivity (top metrics):\n")
            for row in sensitivity[:5]:
                sys.stdout.write(f"    {row.get('rho', '?'):+.2f}  {row.get('metric', '?')}\n")
    else:
        sys.stdout.write(json.dumps(envelope, indent=2) + "\n")

    return 0 if (value is not None or sites or doc) else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
