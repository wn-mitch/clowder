"""Envelope builder for `just q` subtools.

Every subtool returns the same JSON envelope so an LLM caller can learn
the surface once:

    {
      "query":       { ... echo of effective query, incl. applied defaults ... },
      "scan_stats":  { "scanned": N, "returned": K, "more_available": bool,
                       "narrow_by": [ ... ] },
      "results":     [ ... records, each with a stable `id` field ... ],
      "narrative":   "One-sentence gloss of what was found.",
      "next":        [ ... optional suggested follow-up `just q` commands ... ]
    }

Null results return nearest-match evidence in `narrative` rather than
leaving the caller with a wasted tool call on `[]`.
"""

from __future__ import annotations

import json
import subprocess
import sys
from dataclasses import dataclass, field, asdict
from pathlib import Path
from typing import Any


@dataclass
class Envelope:
    query: dict[str, Any]
    scan_stats: dict[str, Any]
    results: list[dict[str, Any]] = field(default_factory=list)
    narrative: str = ""
    next: list[str] = field(default_factory=list)

    def to_json(self, *, indent: int | None = None) -> str:
        return json.dumps(asdict(self), indent=indent, default=_json_default)

    def to_text(self) -> str:
        lines: list[str] = []
        lines.append(f"query: {json.dumps(self.query, default=_json_default)}")
        lines.append(
            f"scanned {self.scan_stats.get('scanned', 0)} / returned "
            f"{self.scan_stats.get('returned', len(self.results))}"
            + ("  (more available — narrow by "
               + ",".join(self.scan_stats.get("narrow_by", []))
               + ")" if self.scan_stats.get("more_available") else "")
        )
        if self.narrative:
            lines.append("")
            lines.append(self.narrative)
        if self.results:
            lines.append("")
            for r in self.results:
                lines.append(f"  - {r.get('id', '?')}  {_result_summary(r)}")
        if self.next:
            lines.append("")
            lines.append("next:")
            for n in self.next:
                lines.append(f"  $ {n}")
        return "\n".join(lines)


def _json_default(o: Any) -> Any:
    if isinstance(o, Path):
        return str(o)
    raise TypeError(f"not serializable: {type(o).__name__}")


def _result_summary(r: dict[str, Any]) -> str:
    """One-liner for text-mode result rendering. Prefers an explicit
    `summary` field; falls back to a compact repr of remaining fields."""
    if "summary" in r:
        return str(r["summary"])
    skip = {"id", "summary"}
    parts = [f"{k}={json.dumps(v, default=_json_default)}"
             for k, v in r.items() if k not in skip]
    return " ".join(parts)


# ── jq helpers ──────────────────────────────────────────────────────────────

def run_jq(jq_program: str, path: Path, *, slurp: bool = False) -> list[dict[str, Any]]:
    """Run jq against a file and parse each output line as JSON.

    Shells to the `jq` binary (already on PATH for all existing clowder
    diagnostic scripts). If `slurp=True`, jq sees the whole file as one
    array via `-s`."""
    if not path.exists():
        raise FileNotFoundError(f"log file not found: {path}")
    args = ["jq", "-c"]
    if slurp:
        args.append("-s")
    args += [jq_program, str(path)]
    proc = subprocess.run(args, capture_output=True, text=True)
    if proc.returncode != 0:
        raise RuntimeError(
            f"jq failed (code {proc.returncode}): {proc.stderr.strip()}\n"
            f"program: {jq_program}\npath: {path}"
        )
    out: list[dict[str, Any]] = []
    for line in proc.stdout.splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            out.append(json.loads(line))
        except json.JSONDecodeError:
            # Non-JSON line (jq -r string output) — wrap for uniformity.
            out.append({"_raw": line})
    return out


def run_jq_count(jq_program: str, path: Path) -> int:
    """Count records matching a jq selector without materializing them."""
    if not path.exists():
        raise FileNotFoundError(f"log file not found: {path}")
    proc = subprocess.run(
        ["jq", "-c", jq_program, str(path)],
        capture_output=True, text=True,
    )
    if proc.returncode != 0:
        raise RuntimeError(
            f"jq failed (code {proc.returncode}): {proc.stderr.strip()}"
        )
    return sum(1 for line in proc.stdout.splitlines() if line.strip())


# ── stable-ID helpers ───────────────────────────────────────────────────────

def event_id(record: dict[str, Any]) -> str:
    """Stable handle for an event record.

    Format: `tick:<N>:<type>[:<cat>]`. The type+tick+cat combination
    is deterministic across runs and lets the caller pivot via
    `just q events --tick-range=N..N+1` or `just q cat-timeline <cat>`."""
    tick = record.get("tick", "?")
    typ = record.get("type", "event")
    cat = record.get("cat") or record.get("name")
    return f"tick:{tick}:{typ}" + (f":{cat}" if cat else "")


def trace_id(record: dict[str, Any]) -> str:
    """Stable handle for a trace record (tick + cat + layer)."""
    return (
        f"tick:{record.get('tick', '?')}:"
        f"{record.get('cat', '?')}:"
        f"{record.get('layer', 'L?')}"
    )


def narrative_id(record: dict[str, Any]) -> str:
    """Stable handle for a narrative line (tick + tier + text-hash-shorthand)."""
    tick = record.get("tick", "?")
    tier = record.get("tier", "Nature")
    # Short text fingerprint — enough to disambiguate multiple lines at same tick.
    text = record.get("text", "")
    fingerprint = "".join(c for c in text[:32] if c.isalnum())[:16] or "empty"
    return f"tick:{tick}:{tier}:{fingerprint}"


# ── nearest-match helpers ───────────────────────────────────────────────────

def nearest_ticks(
    path: Path,
    jq_selector: str,
    target_range: tuple[int, int],
    *,
    max_return: int = 3,
) -> list[dict[str, Any]]:
    """Find records matching `jq_selector` near `target_range`.

    Used to give null queries useful evidence instead of `[]`. Returns
    up to `max_return` records nearest (by tick) to either edge of the
    target range. Cheap approximation — runs the selector once and
    filters in Python; relies on events.jsonl being tick-ordered."""
    try:
        records = run_jq(jq_selector, path)
    except (FileNotFoundError, RuntimeError):
        return []
    lo, hi = target_range
    candidates = [r for r in records
                  if isinstance(r.get("tick"), int)
                  and (r["tick"] < lo or r["tick"] > hi)]
    candidates.sort(key=lambda r: min(abs(r["tick"] - lo), abs(r["tick"] - hi)))
    return candidates[:max_return]


# ── top-level emit ──────────────────────────────────────────────────────────

def emit(envelope: Envelope, *, fmt: str = "json") -> None:
    """Write an envelope to stdout in the requested format."""
    if fmt == "text":
        sys.stdout.write(envelope.to_text() + "\n")
    else:
        sys.stdout.write(envelope.to_json(indent=2) + "\n")
