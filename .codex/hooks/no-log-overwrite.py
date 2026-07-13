#!/usr/bin/env python3
"""PreToolUse Bash hook — refuse commands that would overwrite logs/tuned-*/
or logs/baseline-*/ artifacts.

Triggered for Bash tool calls. Reads the JSON payload from stdin, scans the
command for *write targets* under those protected prefixes, and exits 2
(block) if any target already exists with content. Exits 0 (allow) for
read-only commands and for writes to fresh paths.

Why: soak / baseline-dataset runs cost minutes of wall time and produce the
ground-truth JSONL the project diffs against. An overwrite is unrecoverable.

Detected write patterns:
  - shell output redirect: `> path`, `>> path` (single command-level)
  - `tee path`
  - `mv … path` / `cp … path` (target arg)
  - cargo flags: `--event-log path`, `--log path`, `--trace-log path`
  - just recipes that write: soak, soak-trace, baseline-dataset

Read-only commands (`just check-canaries <path>`, `just q …`, `cat`, `grep`,
etc.) do not match any write pattern and are allowed even when the path is
under a protected prefix.
"""
from __future__ import annotations

import json
import os
import re
import sys


PROTECTED_DIR_PREFIXES = ("logs/tuned-", "logs/baseline-")
CANONICAL_LOGS = ("events.jsonl", "narrative.jsonl")


def collect_write_targets(cmd: str) -> set[str]:
    """Return paths the command would write to (best-effort static analysis)."""
    targets: set[str] = set()

    # 1. Shell output redirect: `> path`, `>> path`. Excludes `2>&1`,
    #    `&>`, etc. — we only want unambiguous file-target redirects.
    for m in re.finditer(r"(?<![\d&])>{1,2}\s*([^\s>&|;()`]+)", cmd):
        targets.add(m.group(1))

    # 2. tee target (with optional flags before the path).
    for m in re.finditer(r"\btee\b(?:\s+-[A-Za-z]+)*\s+([^\s>&|;()`]+)", cmd):
        targets.add(m.group(1))

    # 3. cp / mv last positional arg. Approximate: take the last bare token
    #    on the command before a separator. False positives possible; bias
    #    is to mark more rather than fewer.
    for m in re.finditer(
        r"\b(?:cp|mv)\b(?:\s+-[A-Za-z]+)*\s+\S+\s+([^\s>&|;()`]+)", cmd
    ):
        targets.add(m.group(1))

    # 4. cargo writers: --event-log / --log / --trace-log <path>  (or =path).
    for m in re.finditer(
        r"--(?:event-log|log|trace-log)(?:[=\s]+)([^\s>&|;()`]+)", cmd
    ):
        targets.add(m.group(1))

    # 5. just recipes that write to canonical paths.
    for m in re.finditer(r"\bjust\s+(soak|soak-trace)\b([^\n;&|]*)", cmd):
        recipe = m.group(1)
        args = m.group(2).strip().split()
        seed = args[0] if args else "42"
        targets.add(f"logs/tuned-{seed}/events.jsonl")
        targets.add(f"logs/tuned-{seed}/narrative.jsonl")
        if recipe == "soak-trace":
            focal = args[1] if len(args) >= 2 else "Simba"
            targets.add(f"logs/tuned-{seed}/trace-{focal}.jsonl")

    for m in re.finditer(r"\bjust\s+baseline-dataset\b([^\n;&|]*)", cmd):
        args = m.group(1).strip().split()
        if args:
            targets.add(f"logs/baseline-{args[0]}")

    return targets


def is_protected_overwrite(repo_root: str, target: str) -> str | None:
    """Return the relative protected path that would be overwritten, or None.

    A target is "protected" if it (or its containing dir) sits under
    logs/tuned-*/ or logs/baseline-*/. An overwrite is flagged if the
    target file already exists with size > 0, OR if the target's
    enclosing protected dir already contains any non-empty content.
    """
    rel = target.lstrip("./")
    if not any(rel.startswith(p) for p in PROTECTED_DIR_PREFIXES):
        return None

    full = os.path.join(repo_root, rel)

    # Specific file path inside a protected dir.
    if os.path.isfile(full) and os.path.getsize(full) > 0:
        return rel

    # Directory path that already contains content.
    if os.path.isdir(full):
        for root, _dirs, files in os.walk(full):
            for f in files:
                try:
                    if os.path.getsize(os.path.join(root, f)) > 0:
                        return rel
                except OSError:
                    continue

    # Path doesn't exist yet — but its enclosing protected dir might.
    # E.g. target = logs/tuned-42/events.jsonl, file doesn't exist,
    # but logs/tuned-42/ might already hold a footer-complete soak we
    # shouldn't clobber. (Caught above; this branch is only for
    # nested writes like logs/baseline-foo/sweep/seed-42-rep-0/...)
    return None


def main() -> None:
    try:
        payload = json.load(sys.stdin)
    except (json.JSONDecodeError, ValueError):
        sys.exit(0)

    if payload.get("tool_name") != "Bash":
        sys.exit(0)

    cmd = payload.get("tool_input", {}).get("command", "") or ""
    if not cmd:
        sys.exit(0)

    repo_root = os.environ.get("CLAUDE_PROJECT_DIR") or os.getcwd()

    targets = collect_write_targets(cmd)
    blocked = sorted({
        rel
        for t in targets
        for rel in [is_protected_overwrite(repo_root, t)]
        if rel
    })

    if blocked:
        msg_lines = [
            "REFUSED: command would overwrite protected log path(s):",
            *(f"  - {p}" for p in blocked),
            "",
            "Policy: never overwrite logs/tuned-*/ or logs/baseline-*/.",
            "These dirs hold ground-truth JSONL that the project diffs against;",
            "an overwrite is unrecoverable.",
            "",
            "To proceed: either",
            "  1. mv the existing dir to a versioned name first, e.g.",
            "       mv logs/tuned-42 logs/tuned-42-<suffix>",
            "     so logs/tuned-42 is free, OR",
            "  2. pass a versioned target path to the command directly",
            "     (e.g., --event-log logs/tuned-42-<suffix>/events.jsonl),",
            "     OR",
            "  3. if you really want to discard the existing data, rm it",
            "     explicitly first.",
        ]
        print("\n".join(msg_lines), file=sys.stderr)
        sys.exit(2)

    sys.exit(0)


if __name__ == "__main__":
    main()
