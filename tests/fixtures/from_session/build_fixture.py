#!/usr/bin/env python3
"""Build a synthetic failing-soak fixture for /ticket-from-session smoke tests.

The fixture is a minimal events.jsonl with:
  - A real header (copied from a recent tuned-* run, so constants schema stays current).
  - One Death{cause: Starvation} event.
  - A footer mutated to trigger `just verdict` exit code 2 (fail):
      * deaths_by_cause: {Starvation: 3}
      * never_fired_expected_positives: ["ItemDropped"]
      * continuity_tallies.play: 0  (continuity canary miss)

The fixture lives under logs/_test-from-session-fixture/ so it doesn't pollute
tuned-* and isn't picked up by the auto-detect logic in /ticket-from-session
unless the user explicitly passes the LOG_DIR.

Usage:
    python3 tests/fixtures/from_session/build_fixture.py
    python3 tests/fixtures/from_session/build_fixture.py --out logs/_my-fixture
    python3 tests/fixtures/from_session/build_fixture.py --source logs/tuned-42-2638f186
"""
import argparse
import json
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[3]
DEFAULT_OUT = REPO_ROOT / "logs" / "_test-from-session-fixture"


def find_source_run(repo_root: Path) -> Path:
    """Pick a recent tuned-* run as the schema donor."""
    logs = repo_root / "logs"
    candidates = sorted(
        (p for p in logs.glob("tuned-*") if (p / "events.jsonl").is_file()),
        key=lambda p: (p / "events.jsonl").stat().st_mtime,
        reverse=True,
    )
    if not candidates:
        sys.exit(
            "no tuned-* run with events.jsonl found under logs/. "
            "Run `just soak 42` once to seed a schema donor, or pass --source."
        )
    return candidates[0]


def read_header(events_jsonl: Path) -> dict:
    with events_jsonl.open() as f:
        line = f.readline()
    header = json.loads(line)
    if not header.get("_header"):
        sys.exit(f"{events_jsonl}: line 1 is not a header record")
    return header


def read_footer(events_jsonl: Path) -> dict:
    last = None
    with events_jsonl.open() as f:
        for line in f:
            last = line
    if last is None:
        sys.exit(f"{events_jsonl}: empty file")
    footer = json.loads(last)
    if not footer.get("_footer"):
        sys.exit(f"{events_jsonl}: last line is not a footer record")
    return footer


def mutate_footer_for_fail(footer: dict) -> dict:
    out = dict(footer)
    out["deaths_by_cause"] = {"Starvation": 3}
    out["never_fired_expected_positives"] = ["ItemDropped"]
    tallies = dict(out.get("continuity_tallies", {}))
    tallies["play"] = 0
    out["continuity_tallies"] = tallies
    return out


def build_fixture(source_dir: Path, out_dir: Path) -> Path:
    source = source_dir / "events.jsonl"
    if not source.is_file():
        sys.exit(f"source {source} not found")

    header = read_header(source)
    footer = read_footer(source)

    out_dir.mkdir(parents=True, exist_ok=True)
    out_file = out_dir / "events.jsonl"

    starvation_event = {
        "tick": header.get("constants", {}).get("ticks_per_season", 20000) * 60 + 100,
        "type": "Death",
        "cat": "Bramble",
        "cause": "Starvation",
        "location": [60, 45],
    }

    failing_footer = mutate_footer_for_fail(footer)

    with out_file.open("w") as f:
        f.write(json.dumps(header) + "\n")
        f.write(json.dumps(starvation_event) + "\n")
        f.write(json.dumps(failing_footer) + "\n")

    return out_file


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument(
        "--source",
        type=Path,
        default=None,
        help="Schema-donor run dir (default: most-recent logs/tuned-*).",
    )
    p.add_argument(
        "--out",
        type=Path,
        default=DEFAULT_OUT,
        help=f"Output dir (default: {DEFAULT_OUT.relative_to(REPO_ROOT)}).",
    )
    args = p.parse_args()

    source_dir = args.source if args.source else find_source_run(REPO_ROOT)
    out_file = build_fixture(source_dir, args.out)

    rel = out_file.relative_to(REPO_ROOT) if out_file.is_relative_to(REPO_ROOT) else out_file
    print(f"wrote {rel}")
    print("verify with:")
    print(f"  just verdict {out_file.parent} --no-history")
    print("expected: verdict=fail (Starvation>0, ItemDropped never-fired, play=0)")


if __name__ == "__main__":
    main()
