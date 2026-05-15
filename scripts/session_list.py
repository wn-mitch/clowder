#!/usr/bin/env python3
"""Dashboard for active parallel sessions.

Reads ~/clowder-sessions/* + each workspace's .session-info.json,
augments with jj bookmark heads + filesystem mtimes + sccache stats.

Used by /work skill via `--json`; humans run plain `just sessions` for
the table view.

Usage:
    session_list.py [--json] [--disk]
"""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
from pathlib import Path


SESSIONS_ROOT = Path.home() / "clowder-sessions"


def workspace_age(path: Path) -> str:
    try:
        delta = time.time() - path.stat().st_mtime
    except OSError:
        return "?"
    if delta < 60:
        return f"{int(delta)}s ago"
    if delta < 3600:
        return f"{int(delta // 60)}m ago"
    if delta < 86400:
        return f"{int(delta // 3600)}h ago"
    return f"{int(delta // 86400)}d ago"


def bookmark_head(slug: str) -> str:
    try:
        out = subprocess.run(
            ["jj", "log", "-r", f"bookmarks(\"session/{slug}\")",
             "--no-graph", "-T", "commit_id.short() ++ \" \" ++ description.first_line()"],
            capture_output=True, text=True, check=False,
        )
        return out.stdout.strip() or "(no head)"
    except FileNotFoundError:
        return "(jj missing)"


def workspace_disk(path: Path) -> int:
    """Return total bytes used by the workspace tree (best-effort)."""
    try:
        result = subprocess.run(
            ["du", "-sk", str(path)],
            capture_output=True, text=True, check=False,
        )
        if result.returncode == 0:
            return int(result.stdout.split()[0]) * 1024
    except (subprocess.SubprocessError, ValueError):
        pass
    return 0


def human_bytes(n: int) -> str:
    for unit in ("B", "K", "M", "G", "T"):
        if n < 1024:
            return f"{n:.1f}{unit}"
        n /= 1024
    return f"{n:.1f}P"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--disk", action="store_true", help="include workspace disk usage (slow)")
    args = parser.parse_args()

    if not SESSIONS_ROOT.is_dir():
        if args.json:
            print(json.dumps([]))
        else:
            print("sessions: no ~/clowder-sessions/ yet (no sessions in flight)")
        return 0

    sessions: list[dict] = []

    for path in sorted(SESSIONS_ROOT.iterdir()):
        if not path.is_dir():
            continue
        info_file = path / ".session-info.json"
        if info_file.exists():
            try:
                info = json.loads(info_file.read_text())
            except (OSError, json.JSONDecodeError):
                info = {}
        else:
            info = {"slug": path.name}

        slug = info.get("slug", path.name)
        rec = {
            "slug": slug,
            "path": str(path),
            "track": info.get("track", "—"),
            "tickets": info.get("tickets", []),
            "bookmark": info.get("bookmark", f"session/{slug}"),
            "head": bookmark_head(slug),
            "last_edit": workspace_age(path),
            "created_at": info.get("created_at", "?"),
        }
        if args.disk:
            rec["disk_bytes"] = workspace_disk(path)
            rec["disk_human"] = human_bytes(rec["disk_bytes"])
        sessions.append(rec)

    if args.json:
        print(json.dumps(sessions, indent=2))
        return 0

    if not sessions:
        print("sessions: none in flight")
        return 0

    print(f"{'SLUG':<24} {'TRACK':<22} {'TICKETS':<14} {'LAST-EDIT':<12} HEAD")
    for s in sessions:
        ticket_str = ",".join(str(t) for t in s["tickets"][:3]) or "—"
        if len(s["tickets"]) > 3:
            ticket_str += "…"
        print(f"{s['slug'][:24]:<24} {s['track']:<22} {ticket_str:<14} "
              f"{s['last_edit']:<12} {s['head'][:80]}")
        if args.disk:
            print(f"  {' ' * 60} disk: {s['disk_human']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
