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
import difflib
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


def round_value(v: Any) -> Any:
    """Strip f32→f64 precision noise from numeric leaves.

    SimConstants stores floats as f32; serializing to JSON re-encodes via
    f64 and surfaces 7-digit noise (`0.001` → `0.0010000000474974513`).
    Round to 6 significant figures — generous enough to preserve every
    real tuning value, tight enough to hide the noise."""
    if isinstance(v, float):
        if v == 0.0:
            return 0.0
        from math import floor, log10
        digits = 6 - int(floor(log10(abs(v)))) - 1
        digits = max(0, min(digits, 12))
        return round(v, digits)
    return v


def nearest_paths(target: str, all_paths: list[str], n: int = 3) -> list[str]:
    """`difflib.get_close_matches` over the dotted-path catalog. Used to
    suggest fixes when the user passes `social.bond_proximity_social_rate`
    but the field actually lives at `needs.bond_proximity_social_rate`."""
    return difflib.get_close_matches(target, all_paths, n=n, cutoff=0.5)


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
    sub_paths: list[str] = []
    if isinstance(value, dict):
        # Caller pointed at a non-leaf (e.g. `magic` when meaning
        # `magic.thornward_decay_rate`) — surface the available leaves.
        sub_paths = sorted(p for p in list_paths(constants) if p.startswith(f"{args.path}."))
        value = None
    field_name = args.path.rsplit(".", 1)[-1]
    doc = find_doc_comment(field_name)
    sites = find_read_sites(field_name)
    sensitivity = load_sensitivity(args.path)
    rounded_value = round_value(value)

    envelope: dict[str, Any] = {
        "constant": args.path,
        "value": rounded_value,
        "doc": doc,
        "read_sites": sites,
        "sensitivity": sensitivity,
        "constants_source": str(events) if events else None,
    }

    if value is None and not constants:
        envelope["note"] = ("no events.jsonl with constants block was found — "
                            "value resolution skipped. Run a soak (`just soak`) "
                            "first or pass `--run <path>`.")
    elif value is None and constants:
        # Distinguish "method/non-field" (read sites exist for the leaf
        # name) from "no such path" (no read sites either). Both surface
        # nearest-match suggestions so a misspelled subtree is recoverable
        # in one turn.
        catalog = list_paths(constants)
        suggestions = nearest_paths(args.path, catalog)
        if sites and not sub_paths:
            envelope["note"] = ("path resolves to no constants leaf; the field "
                                "name has read sites in src/, suggesting it's a "
                                "method or fn rather than a struct field.")
        elif sub_paths:
            envelope["note"] = (f"path is a sub-tree, not a leaf — "
                                f"{len(sub_paths)} child paths available.")
            envelope["children"] = sub_paths
        else:
            envelope["note"] = "no such constant path"
        if suggestions and not sub_paths:
            envelope["nearest"] = suggestions

    if args.text:
        sys.stdout.write(f"explain: {args.path}\n")
        sys.stdout.write(f"  value:    {rounded_value}\n")
        if envelope.get("note"):
            sys.stdout.write(f"  note:     {envelope['note']}\n")
        if envelope.get("children"):
            sys.stdout.write("  children:\n")
            for c in envelope["children"][:10]:
                sys.stdout.write(f"    {c}\n")
            if len(envelope["children"]) > 10:
                sys.stdout.write(f"    ... ({len(envelope['children']) - 10} more)\n")
        if envelope.get("nearest"):
            sys.stdout.write("  did you mean:\n")
            for n in envelope["nearest"]:
                sys.stdout.write(f"    {n}\n")
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

    return 0 if (rounded_value is not None or sites or doc or envelope.get("nearest")
                 or envelope.get("children")) else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
