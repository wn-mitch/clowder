#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""
One-call run validation for a Clowder soak.

Composes existing primitives — `check_canaries.sh`, `check_continuity.sh`,
`diff-constants` (jq), and a footer-vs-baseline drift summary — into a
single structured JSON envelope so a Claude Code turn can decide
pass/concern/fail in one tool call.

Replaces `just autoloop`. Reads the active baseline from
`logs/baselines/current.json` (Tier 2.2) when present; falls back to
`logs/baseline-pre-substrate-refactor/events.jsonl` for backwards compat.

Usage:
    just verdict <run-dir>
    just verdict <run-dir> --baseline <path-to-events.jsonl>
    just verdict <run-dir> --no-history    # don't append to verdict-history.jsonl
    just verdict <run-dir> --text          # human-readable summary instead of JSON

Exit codes: 0 pass, 1 concern, 2 fail.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from dataclasses import dataclass, field, asdict
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parent.parent

LEGACY_BASELINE = REPO_ROOT / "logs" / "baseline-pre-substrate-refactor" / "events.jsonl"
BASELINES_DIR = REPO_ROOT / "logs" / "baselines"
HISTORY_PATH = REPO_ROOT / "logs" / "verdict-history.jsonl"

NOISE_PCT = 10.0
SIGNIFICANT_PCT = 30.0


@dataclass
class Verdict:
    run: str
    verdict: str  # pass | concern | fail
    canaries: dict[str, Any] = field(default_factory=dict)
    constants_drift_vs_baseline: str = "no-baseline"
    footer_drift: list[dict[str, Any]] = field(default_factory=list)
    baseline: str | None = None
    commit: str | None = None
    next_steps: list[str] = field(default_factory=list)


def find_events_log(run_dir: Path) -> Path:
    direct = run_dir / "events.jsonl"
    if direct.exists():
        return direct
    if run_dir.is_file() and run_dir.suffix == ".jsonl":
        return run_dir
    raise SystemExit(f"verdict: no events.jsonl found at {run_dir}")


def read_footer(events_path: Path) -> dict[str, Any]:
    proc = subprocess.run(
        ["jq", "-c", "select(._footer)", str(events_path)],
        capture_output=True, text=True,
    )
    if proc.returncode != 0:
        raise SystemExit(f"verdict: jq failed reading footer: {proc.stderr.strip()}")
    line = next((l for l in proc.stdout.splitlines() if l.strip()), "")
    if not line:
        return {}
    return json.loads(line)


def read_header_field(events_path: Path, jq_expr: str) -> Any:
    proc = subprocess.run(
        ["jq", "-c", f"select(._header) | {jq_expr}", str(events_path)],
        capture_output=True, text=True,
    )
    if proc.returncode != 0:
        return None
    line = next((l for l in proc.stdout.splitlines() if l.strip()), "")
    if not line or line == "null":
        return None
    try:
        return json.loads(line)
    except json.JSONDecodeError:
        return line


def run_canary_script(script: Path, events_path: Path) -> tuple[str, str]:
    """Returns (status, raw_output). status ∈ {pass, fail, error}."""
    if not script.exists():
        return ("error", f"missing script: {script}")
    proc = subprocess.run(
        ["bash", str(script), str(events_path)],
        capture_output=True, text=True,
    )
    out = (proc.stdout + proc.stderr).strip()
    if proc.returncode == 0:
        return ("pass", out)
    return ("fail", out)


def resolve_baseline(explicit: str | None) -> Path | None:
    if explicit:
        p = Path(explicit)
        return p if p.exists() else None
    current = BASELINES_DIR / "current.json"
    if current.exists():
        try:
            ref = json.loads(current.read_text())
            p = Path(ref.get("events_path") or ref.get("path", ""))
            if p.exists():
                return p
        except (json.JSONDecodeError, OSError):
            pass
    if LEGACY_BASELINE.exists():
        return LEGACY_BASELINE
    return None


def constants_drift(baseline: Path, observed: Path) -> str:
    proc = subprocess.run(
        ["bash", "-c",
         f"diff <(jq -c 'select(._header) | .constants' {baseline!s}) "
         f"<(jq -c 'select(._header) | .constants' {observed!s})"],
        capture_output=True, text=True,
    )
    return "clean" if proc.returncode == 0 else "drift"


_NUMERIC_FIELDS = (
    "wards_placed_total", "wards_despawned_total", "ward_count_final",
    "ward_avg_strength_final", "shadow_foxes_avoided_ward_total",
    "ward_siege_started_total", "shadow_fox_spawn_total",
    "anxiety_interrupt_total", "positive_features_active",
    "negative_events_total", "neutral_features_active",
)


def band(delta_pct: float) -> str:
    a = abs(delta_pct)
    if a < NOISE_PCT:
        return "noise"
    if a >= SIGNIFICANT_PCT:
        return "significant"
    return "drift"


def footer_drift(baseline: dict[str, Any], observed: dict[str, Any]) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for field_name in _NUMERIC_FIELDS:
        b = baseline.get(field_name)
        o = observed.get(field_name)
        if not isinstance(b, (int, float)) or not isinstance(o, (int, float)):
            continue
        if b == 0 and o == 0:
            continue
        if b == 0:
            rows.append({"field": field_name, "baseline": b, "observed": o,
                         "delta_pct": None, "band": "new-nonzero"})
            continue
        delta = (o - b) / b * 100.0
        rows.append({"field": field_name, "baseline": b, "observed": o,
                     "delta_pct": round(delta, 1), "band": band(delta)})

    for cause in set((baseline.get("deaths_by_cause") or {}).keys()
                     | (observed.get("deaths_by_cause") or {}).keys()):
        b = (baseline.get("deaths_by_cause") or {}).get(cause, 0)
        o = (observed.get("deaths_by_cause") or {}).get(cause, 0)
        if b == 0 and o == 0:
            continue
        delta = None if b == 0 else round((o - b) / b * 100.0, 1)
        rows.append({
            "field": f"deaths_by_cause.{cause}",
            "baseline": b, "observed": o,
            "delta_pct": delta,
            "band": "new-nonzero" if b == 0 else band(delta),
        })

    rows.sort(key=lambda r: -abs(r["delta_pct"]) if r["delta_pct"] is not None else -1e9)
    return rows[:20]


def derive_overall(canary_survival: str, canary_continuity: str,
                   constants: str, drift: list[dict[str, Any]]) -> str:
    if canary_survival == "fail":
        return "fail"
    if canary_continuity == "fail" or constants == "drift":
        return "concern"
    if any(r["band"] == "significant" for r in drift):
        return "concern"
    return "pass"


def derive_next_steps(v: Verdict, run_dir: Path, footer: dict[str, Any]) -> list[str]:
    steps: list[str] = []
    if v.canaries.get("survival") == "fail":
        causes = list((footer.get("deaths_by_cause") or {}).keys())
        if causes:
            steps.append(f"just q deaths {run_dir} --cause={causes[0]}")
        else:
            steps.append(f"just q deaths {run_dir}")
    if v.canaries.get("continuity") == "fail":
        steps.append(f"just q anomalies {run_dir}")
    if v.constants_drift_vs_baseline == "drift":
        if v.baseline:
            steps.append(f"just diff-constants {v.baseline} {run_dir}/events.jsonl")
    sig = [r for r in v.footer_drift if r["band"] == "significant"]
    if sig:
        steps.append(f"just q events {run_dir} --type=Death")
    return steps


def append_history(v: Verdict) -> None:
    HISTORY_PATH.parent.mkdir(parents=True, exist_ok=True)
    with HISTORY_PATH.open("a") as f:
        f.write(json.dumps(asdict(v), default=str) + "\n")


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("run_dir", help="Path to a run directory (containing events.jsonl) or events.jsonl directly")
    ap.add_argument("--baseline", default=None, help="Override baseline events.jsonl path")
    ap.add_argument("--no-history", action="store_true", help="Don't append to logs/verdict-history.jsonl")
    ap.add_argument("--text", action="store_true", help="Human-readable output (JSON is default)")
    args = ap.parse_args(argv)

    run_dir = Path(args.run_dir)
    events_path = find_events_log(run_dir)
    footer = read_footer(events_path)
    if not footer:
        v = Verdict(
            run=str(run_dir), verdict="fail",
            canaries={"survival": "fail", "continuity": "skip", "detail": "no footer line"},
            next_steps=[f"check that {events_path} contains a `_footer` JSONL line"],
        )
        _emit(v, args.text)
        return 2

    surv_status, _ = run_canary_script(REPO_ROOT / "scripts" / "check_canaries.sh", events_path)
    cont_status, cont_out = run_canary_script(REPO_ROOT / "scripts" / "check_continuity.sh", events_path)

    cont_detail: list[str] = []
    if cont_status == "fail":
        for line in cont_out.splitlines():
            if "[FAIL]" in line:
                parts = line.split()
                if len(parts) >= 3:
                    cont_detail.append(f"{parts[1]}={parts[2]}")

    baseline_path = resolve_baseline(args.baseline)
    constants_status = "no-baseline"
    drift_rows: list[dict[str, Any]] = []
    if baseline_path:
        constants_status = constants_drift(baseline_path, events_path)
        baseline_footer = read_footer(baseline_path)
        if baseline_footer:
            drift_rows = footer_drift(baseline_footer, footer)

    overall = derive_overall(surv_status, cont_status, constants_status, drift_rows)
    commit = read_header_field(events_path, ".commit_hash_short")

    v = Verdict(
        run=str(run_dir),
        verdict=overall,
        canaries={
            "survival": surv_status,
            "continuity": cont_status if not cont_detail else f"fail:{','.join(cont_detail)}",
        },
        constants_drift_vs_baseline=constants_status,
        footer_drift=drift_rows,
        baseline=str(baseline_path) if baseline_path else None,
        commit=commit if isinstance(commit, str) else None,
    )
    v.next_steps = derive_next_steps(v, run_dir, footer)

    if not args.no_history:
        append_history(v)

    _emit(v, args.text)

    return {"pass": 0, "concern": 1, "fail": 2}[overall]


def _emit(v: Verdict, text_mode: bool) -> None:
    if text_mode:
        sys.stdout.write(_text(v) + "\n")
    else:
        sys.stdout.write(json.dumps(asdict(v), indent=2, default=str) + "\n")


def _text(v: Verdict) -> str:
    lines = [f"verdict: {v.verdict.upper()}  ({v.run})"]
    if v.commit:
        lines.append(f"  commit:    {v.commit}")
    lines.append(f"  survival:  {v.canaries.get('survival', '?')}")
    lines.append(f"  continuity: {v.canaries.get('continuity', '?')}")
    lines.append(f"  constants: {v.constants_drift_vs_baseline}"
                 + (f"  (baseline={v.baseline})" if v.baseline else ""))
    if v.footer_drift:
        lines.append("  footer drift (top):")
        for r in v.footer_drift[:5]:
            d = r["delta_pct"]
            d_s = "  new" if d is None else f"{d:+5.1f}%"
            lines.append(f"    {r['band']:11s} {d_s}  {r['field']} ({r['baseline']} → {r['observed']})")
    if v.next_steps:
        lines.append("  next:")
        for s in v.next_steps:
            lines.append(f"    $ {s}")
    return "\n".join(lines)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
