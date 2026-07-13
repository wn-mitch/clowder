#!/usr/bin/env python3
"""PostToolUse Bash hook — nudge `/diagnose-collapse` after `just soak` runs
that fail any survival gate.

Triggered for Bash tool calls. Reads the JSON payload from stdin, extracts the
command, matches `just soak` / `just soak-trace`, derives the resulting log
dir, parses its `_footer`, and emits a one-line stderr nudge if any survival
gate failed. Always exits 0 (advisory, never blocks).

Why: the diagnostic muscle memory for starvation collapses, predator waves,
and generational failures is encoded in `/diagnose-collapse`. Without a
nudge, the skill is under-invoked and the colony-health verdict ends at
"FAIL" without naming a cause. The hook closes the invocation gap.

Survival gates (any failure → fire nudge):
  - deaths_by_cause.Starvation > 0
  - deaths_by_cause.ShadowFoxAmbush > 10
  - kittens_born > 0 AND kittens_surviving == 0
  - "MatingOccurred" in never_fired_expected_positives
    AND continuity_tallies.courtship == 0
  - any continuity-canary at zero (grooming, mentoring, mythic-texture)

Rate-limit: a sentinel file `.collapse-checked` in the log dir prevents
duplicate nudges across multi-run sessions. The sentinel is removed if the
log dir's events.jsonl mtime is newer than the sentinel mtime, so a fresh
soak into the same dir re-arms the nudge.
"""
from __future__ import annotations

import json
import os
import re
import sys


def parse_soak_invocation(cmd: str) -> tuple[str, str] | None:
    """Return (recipe, log_dir) for `just soak[-trace]` invocations, or None."""
    m = re.search(r"\bjust\s+(soak|soak-trace)\b([^\n;&|]*)", cmd)
    if not m:
        return None
    recipe = m.group(1)
    args = m.group(2).strip().split()
    seed = args[0] if args else "42"
    return recipe, f"logs/tuned-{seed}"


def read_footer(events_path: str) -> dict | None:
    """Return the parsed `_footer` dict from the last line of events.jsonl,
    or None if the file is missing, empty, or has no footer."""
    if not os.path.isfile(events_path):
        return None
    try:
        size = os.path.getsize(events_path)
        if size == 0:
            return None
        # Read the tail efficiently — footers are tiny relative to events.jsonl.
        with open(events_path, "rb") as f:
            seek_back = min(size, 64 * 1024)
            f.seek(-seek_back, os.SEEK_END)
            tail = f.read().decode("utf-8", errors="replace")
        last_line = tail.strip().rsplit("\n", 1)[-1]
        if not last_line:
            return None
        record = json.loads(last_line)
    except (OSError, ValueError):
        return None
    if not isinstance(record, dict) or not record.get("_footer"):
        return None
    return record


def survival_gate_failures(footer: dict) -> list[str]:
    """Return a list of human-readable failure descriptions; empty list = pass."""
    failures: list[str] = []

    deaths = footer.get("deaths_by_cause") or {}
    if (n := int(deaths.get("Starvation", 0))) > 0:
        failures.append(f"Starvation deaths: {n}")
    if (n := int(deaths.get("ShadowFoxAmbush", 0))) > 10:
        failures.append(f"ShadowFoxAmbush deaths: {n} (gate: ≤10)")

    score = footer.get("colony_score") or {}
    born = int(score.get("kittens_born", 0))
    surviving = int(score.get("kittens_surviving", 0))
    if born > 0 and surviving == 0:
        failures.append(f"Generational collapse: {born} born, 0 surviving")

    never_fired = footer.get("never_fired_expected_positives") or []
    continuity = footer.get("continuity_tallies") or {}
    if "MatingOccurred" in never_fired and int(continuity.get("courtship", 0)) == 0:
        failures.append("Mating chain silent: MatingOccurred never fired AND courtship == 0")

    for canary in ("grooming", "mentoring", "mythic-texture"):
        if int(continuity.get(canary, 0)) == 0:
            failures.append(f"Continuity canary at zero: {canary}")

    return failures


def sentinel_is_fresh(log_dir: str, events_path: str) -> bool:
    """Return True iff the sentinel exists AND is newer than events.jsonl."""
    sentinel = os.path.join(log_dir, ".collapse-checked")
    if not os.path.isfile(sentinel):
        return False
    try:
        return os.path.getmtime(sentinel) >= os.path.getmtime(events_path)
    except OSError:
        return False


def write_sentinel(log_dir: str) -> None:
    sentinel = os.path.join(log_dir, ".collapse-checked")
    try:
        with open(sentinel, "w", encoding="utf-8") as f:
            f.write("checked\n")
    except OSError:
        pass


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

    parsed = parse_soak_invocation(cmd)
    if parsed is None:
        sys.exit(0)
    _recipe, log_dir = parsed

    repo_root = os.environ.get("CLAUDE_PROJECT_DIR") or os.getcwd()
    abs_log_dir = os.path.join(repo_root, log_dir)
    events_path = os.path.join(abs_log_dir, "events.jsonl")

    if sentinel_is_fresh(abs_log_dir, events_path):
        sys.exit(0)

    footer = read_footer(events_path)
    if footer is None:
        sys.exit(0)

    failures = survival_gate_failures(footer)
    write_sentinel(abs_log_dir)

    if not failures:
        sys.exit(0)

    bullets = "\n".join(f"  - {f}" for f in failures)
    print(
        f"\n[post-soak] Survival gates failed on {log_dir}/:\n"
        f"{bullets}\n"
        f"[post-soak] Recommend: /diagnose-collapse {log_dir} "
        f"— names cause + drafts structural-option menu.",
        file=sys.stderr,
    )
    sys.exit(0)


if __name__ == "__main__":
    main()
