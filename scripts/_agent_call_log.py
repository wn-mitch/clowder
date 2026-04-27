#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""Shared append-only writer for `logs/agent-call-history.jsonl`.

Every agent-facing tool (`just verdict`, `just q`, `just hypothesize`) calls
``append_call_history`` after producing its result so the corpus of
*why a tool was called* accumulates over time.

The agent-design tooling intersection plan calls this Pillar 2 ("build
feedback loops"): patterns of intent — surfaced by grepping rationales —
become the seed data for new tools or refinements to existing ones.
"""

from __future__ import annotations

import datetime as dt
import json
import os
import subprocess
from dataclasses import is_dataclass, asdict
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parent.parent
HISTORY_PATH = REPO_ROOT / "logs" / "agent-call-history.jsonl"


def _commit_hash() -> str | None:
    try:
        proc = subprocess.run(
            ["git", "rev-parse", "--short", "HEAD"],
            capture_output=True, text=True, cwd=REPO_ROOT, timeout=5,
        )
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return None
    if proc.returncode != 0:
        return None
    out = proc.stdout.strip()
    return out or None


def _normalize_args(args: Any) -> dict[str, Any]:
    """Render an argparse.Namespace (or dict) into a JSON-safe dict.

    Drops `func` (set_defaults dispatch handle) and any non-serializable
    values; coerces Path / dataclass values to plain types so the line
    survives `json.dumps` without `default=str` truncation surprises.
    """
    if hasattr(args, "__dict__"):
        raw: dict[str, Any] = dict(vars(args))
    elif isinstance(args, dict):
        raw = dict(args)
    else:
        raw = {"value": repr(args)}

    out: dict[str, Any] = {}
    for k, v in raw.items():
        if k == "func":
            continue
        if isinstance(v, Path):
            out[k] = str(v)
        elif is_dataclass(v) and not isinstance(v, type):
            out[k] = asdict(v)
        elif isinstance(v, (str, int, float, bool, type(None), list, dict)):
            out[k] = v
        else:
            out[k] = repr(v)
    return out


def append_call_history(
    *,
    tool: str,
    subtool: str | None,
    args: Any,
    rationale: str | None,
    exit_code: int,
    commit: str | None = None,
) -> None:
    """Append one JSON line to logs/agent-call-history.jsonl.

    Failures here MUST NOT propagate — telemetry should never break the
    tool that's calling it. Writes are best-effort; if the disk is full
    or the parent directory can't be created, swallow the error and
    move on. Set ``CLOWDER_AGENT_LOG_DEBUG=1`` to surface failures.
    """
    record = {
        "timestamp": dt.datetime.now(dt.timezone.utc).isoformat(timespec="seconds"),
        "tool": tool,
        "subtool": subtool,
        "args": _normalize_args(args),
        "rationale": rationale,
        "commit": commit if commit is not None else _commit_hash(),
        "exit_code": int(exit_code),
    }
    try:
        HISTORY_PATH.parent.mkdir(parents=True, exist_ok=True)
        with HISTORY_PATH.open("a") as f:
            f.write(json.dumps(record, default=str) + "\n")
    except OSError as e:
        if os.environ.get("CLOWDER_AGENT_LOG_DEBUG"):
            import sys
            sys.stderr.write(f"_agent_call_log: append failed: {e}\n")
