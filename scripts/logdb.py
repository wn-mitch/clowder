#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = [
#   "duckdb>=1.0",
#   "tqdm>=4.66",
#   "altair>=5.3",
#   "vl-convert-python>=1.6",
#   "pandas>=2.0",
# ]
# ///
"""Cross-run log database for Clowder simulation archives.

logdb ingests ``logs/<archive>/**/events.jsonl`` (and, when explicitly
requested, ``trace-<cat>.jsonl`` sidecars and per-cat DSE landscape
``cat_snapshot_scores``) into one DuckDB file at ``logs/runs.duckdb``.
Cross-run balance work then reaches for SQL instead of hand-rolled jq.

Subcommands:
    build              ingest archives, idempotent via mtime cache
    query SQL          one-shot read-only query, prints to stdout
    shell              exec into the duckdb CLI on logs/runs.duckdb
    chart RECIPE ...   render a chart recipe to logs/charts/<recipe>-<ts>.html

Build flags:
    --rebuild          drop and recreate the DB before ingest
    --with-scores      also ingest ``cat_snapshot_scores`` (~2.5M rows /
                       baseline-2026-04-25; the dominant insert cost — off
                       by default so the daily build stays under the gate)
    --with-traces      also ingest ``trace_l2`` / ``trace_l3`` / ``trace_l3_ranked``
                       from per-focal trace sidecars

Schema reference: docs/diagnostics/logdb.md.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import pkgutil
import re
import shutil
import subprocess
import sys
from contextlib import contextmanager
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterator

import duckdb  # type: ignore[import-not-found]
from tqdm import tqdm  # type: ignore[import-not-found]

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_DB = REPO_ROOT / "logs" / "runs.duckdb"
LOGS_DIR = REPO_ROOT / "logs"
CHARTS_DIR = REPO_ROOT / "logs" / "charts"
SCHEMA_VERSION = 1

DEFAULT_EXCLUDE_DIR_NAMES = {"charts", "baselines"}
HANDLED_EVENT_TYPES = {"CatSnapshot", "Death", "ColonyScore"}

BATCH_FLUSH_ROWS = 50_000

NEEDS_FIELDS = (
    "hunger", "energy", "temperature", "safety", "social",
    "acceptance", "mating", "respect", "mastery", "purpose",
)
PERSONALITY_FIELDS = (
    "boldness", "sociability", "curiosity", "diligence", "warmth",
    "spirituality", "ambition", "patience", "anxiety", "optimism",
    "temper", "stubbornness", "playfulness", "loyalty", "tradition",
    "compassion", "pride", "independence",
)
SKILLS_FIELDS = (
    "hunting", "foraging", "herbcraft", "building", "combat", "magic",
)


# ---------------------------------------------------------------------------
# Schema
# ---------------------------------------------------------------------------

def _struct_decl(fields: tuple[str, ...]) -> str:
    return "STRUCT(" + ", ".join(f"{name} DOUBLE" for name in fields) + ")"


SCHEMA_DDL = f"""
CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS ingested_files (
    file_path       VARCHAR PRIMARY KEY,
    mtime_ns        BIGINT,
    size_bytes      BIGINT,
    run_id          VARCHAR,
    role            VARCHAR,
    ingested_at     TIMESTAMP,
    schema_version  INTEGER
);

CREATE TABLE IF NOT EXISTS runs (
    run_id                  VARCHAR PRIMARY KEY,
    archive                 VARCHAR,
    kind                    VARCHAR,
    seed                    INTEGER,
    rep                     INTEGER,
    focal                   VARCHAR,
    forced_weather          VARCHAR,
    commit_hash             VARCHAR,
    commit_hash_short       VARCHAR,
    commit_dirty            BOOLEAN,
    commit_time             TIMESTAMP,
    duration_secs           DOUBLE,
    tick_start              BIGINT,
    tick_end                BIGINT,
    events_path             VARCHAR,
    narrative_path          VARCHAR,
    trace_path              VARCHAR,
    footer_written          BOOLEAN,
    constants               JSON,
    sensory_env_multipliers JSON
);

CREATE INDEX IF NOT EXISTS runs_commit_hash_short_idx ON runs(commit_hash_short);
CREATE INDEX IF NOT EXISTS runs_commit_time_idx ON runs(commit_time);
CREATE INDEX IF NOT EXISTS runs_archive_idx ON runs(archive);

CREATE TABLE IF NOT EXISTS run_footers (
    run_id                          VARCHAR PRIMARY KEY,
    positive_features_active        INTEGER,
    positive_features_total         INTEGER,
    neutral_features_active         INTEGER,
    neutral_features_total          INTEGER,
    negative_events_total           BIGINT,
    anxiety_interrupt_total         BIGINT,
    shadow_fox_spawn_total          BIGINT,
    shadow_foxes_avoided_ward_total BIGINT,
    ward_count_final                INTEGER,
    ward_avg_strength_final         DOUBLE,
    wards_placed_total              BIGINT,
    wards_despawned_total           BIGINT,
    ward_siege_started_total        BIGINT,
    deaths_by_cause                 MAP(VARCHAR, INTEGER),
    continuity_tallies              MAP(VARCHAR, INTEGER),
    plan_failures_by_reason         MAP(VARCHAR, INTEGER),
    interrupts_by_reason            MAP(VARCHAR, INTEGER),
    never_fired_expected_positives  VARCHAR[],
    final_aggregate                 DOUBLE,
    final_welfare                   DOUBLE,
    final_shelter                   DOUBLE,
    final_nourishment               DOUBLE,
    final_health                    DOUBLE,
    final_happiness                 DOUBLE,
    final_fulfillment               DOUBLE,
    final_seasons_survived          BIGINT,
    final_peak_population           BIGINT,
    final_kittens_born              BIGINT,
    final_bonds_formed              BIGINT,
    final_living_cats               BIGINT
);

CREATE TABLE IF NOT EXISTS activations (
    run_id        VARCHAR,
    feature_name  VARCHAR,
    polarity      VARCHAR,
    final_count   BIGINT
);

CREATE TABLE IF NOT EXISTS colony_scores (
    run_id                       VARCHAR,
    tick                         BIGINT,
    shelter                      DOUBLE,
    nourishment                  DOUBLE,
    health                       DOUBLE,
    happiness                    DOUBLE,
    fulfillment                  DOUBLE,
    welfare                      DOUBLE,
    aggregate                    DOUBLE,
    positive_activation_score    DOUBLE,
    positive_features_active     INTEGER,
    positive_features_total      INTEGER,
    neutral_features_active      INTEGER,
    neutral_features_total       INTEGER,
    negative_events_total        BIGINT,
    seasons_survived             INTEGER,
    bonds_formed                 INTEGER,
    peak_population              INTEGER,
    deaths_starvation            INTEGER,
    deaths_old_age               INTEGER,
    deaths_injury                INTEGER,
    aspirations_completed        INTEGER,
    structures_built             INTEGER,
    kittens_born                 INTEGER,
    prey_dens_discovered         INTEGER,
    friends_count                INTEGER,
    partners_count               INTEGER,
    mates_count                  INTEGER,
    living_cats                  INTEGER
);
CREATE INDEX IF NOT EXISTS colony_scores_run_tick_idx ON colony_scores(run_id, tick);

CREATE TABLE IF NOT EXISTS cat_snapshots (
    run_id              VARCHAR,
    tick                BIGINT,
    cat                 VARCHAR,
    current_action      VARCHAR,
    position_x          INTEGER,
    position_y          INTEGER,
    mood_valence        DOUBLE,
    mood_modifier_count INTEGER,
    health              DOUBLE,
    corruption          DOUBLE,
    magic_affinity      DOUBLE,
    life_stage          VARCHAR,
    sex                 VARCHAR,
    orientation         VARCHAR,
    season              VARCHAR,
    social_warmth       DOUBLE,
    is_pregnant         BOOLEAN,
    needs               {_struct_decl(NEEDS_FIELDS)},
    personality         {_struct_decl(PERSONALITY_FIELDS)},
    skills              {_struct_decl(SKILLS_FIELDS)},
    relationships       JSON
);
CREATE INDEX IF NOT EXISTS cat_snapshots_run_tick_idx ON cat_snapshots(run_id, tick);
CREATE INDEX IF NOT EXISTS cat_snapshots_cat_idx ON cat_snapshots(cat);

-- cat_snapshot_scores is the long-format DSE landscape, one row per
-- (run, tick, cat, action). It is the only colony-wide DSE signal —
-- trace_l2 covers only focal cats. Off by default at ingest time
-- (~2.5M rows for the canonical 27-run baseline); enable with
-- ``logdb-build --with-scores``.
CREATE TABLE IF NOT EXISTS cat_snapshot_scores (
    run_id  VARCHAR,
    tick    BIGINT,
    cat     VARCHAR,
    action  VARCHAR,
    score   DOUBLE
);

CREATE TABLE IF NOT EXISTS deaths (
    run_id          VARCHAR,
    tick            BIGINT,
    cat             VARCHAR,
    cause           VARCHAR,
    injury_source   VARCHAR,
    killer_species  VARCHAR,
    position_x      INTEGER,
    position_y      INTEGER
);

CREATE TABLE IF NOT EXISTS trace_l2 (
    run_id                VARCHAR,
    tick                  BIGINT,
    cat                   VARCHAR,
    dse                   VARCHAR,
    eligibility_passed    BOOLEAN,
    markers_required      VARCHAR[],
    composition_mode      VARCHAR,
    composition_raw       DOUBLE,
    maslow_pregate        DOUBLE,
    final_score           DOUBLE,
    intention_kind        VARCHAR,
    intention_goal_state  VARCHAR,
    considerations        JSON,
    modifiers             JSON,
    top_losing            JSON
);
CREATE INDEX IF NOT EXISTS trace_l2_run_tick_idx ON trace_l2(run_id, tick);

CREATE TABLE IF NOT EXISTS trace_l3 (
    run_id                     VARCHAR,
    tick                       BIGINT,
    cat                        VARCHAR,
    chosen                     VARCHAR,
    softmax_temp               DOUBLE,
    momentum_active_intention  VARCHAR,
    momentum_strength          DOUBLE,
    momentum_preempted         BOOLEAN,
    intention_kind             VARCHAR,
    goap_plan                  JSON
);
CREATE INDEX IF NOT EXISTS trace_l3_run_tick_idx ON trace_l3(run_id, tick);

CREATE TABLE IF NOT EXISTS trace_l3_ranked (
    run_id        VARCHAR,
    tick          BIGINT,
    cat           VARCHAR,
    action        VARCHAR,
    raw_score     DOUBLE,
    softmax_prob  DOUBLE,
    rank          INTEGER
);
CREATE INDEX IF NOT EXISTS trace_l3_ranked_run_action_idx ON trace_l3_ranked(run_id, action);
"""


# ---------------------------------------------------------------------------
# Path classification
# ---------------------------------------------------------------------------

KIND_PATH_PATTERNS = {
    re.compile(r"/sweep/(?P<seed>\d+)-(?P<rep>\d+)/?"): "sweep",
    re.compile(r"/trace/(?P<seed>\d+)-(?P<focal>[A-Za-z][A-Za-z0-9]*)/?"): "trace",
    re.compile(r"/conditional/(?P<seed>\d+)-(?P<weather>[A-Za-z][A-Za-z0-9]*)/?"): "conditional",
    re.compile(r"/probe/(?P<seed>\d+)/?"): "probe",
    re.compile(r"/canaries/(?P<seed>\d+)-(?P<canary>[A-Za-z][A-Za-z0-9-]*)/?"): "canary",
}


@dataclass
class RunIdentity:
    archive: str
    kind: str
    seed: int | None
    rep: int | None = None
    focal: str | None = None
    forced_weather: str | None = None


def classify_path(events_path: Path, header_seed: int | None,
                  header_forced_weather: str | None) -> RunIdentity:
    abs_path = str(events_path.resolve())
    rel = abs_path[abs_path.find("/logs/") + len("/logs/"):]
    archive = rel.split("/", 1)[0] if "/" in rel else rel.replace("/events.jsonl", "")
    for pattern, kind in KIND_PATH_PATTERNS.items():
        m = pattern.search(abs_path)
        if not m:
            continue
        gd = m.groupdict()
        seed = int(gd["seed"]) if "seed" in gd else header_seed
        rep = int(gd["rep"]) if "rep" in gd else None
        focal = gd.get("focal")
        if "weather" in gd:
            forced_weather = gd["weather"]
        elif "canary" in gd:
            forced_weather = gd["canary"]
        else:
            forced_weather = header_forced_weather
        return RunIdentity(archive, kind, seed, rep, focal, forced_weather)
    return RunIdentity(archive, "flat", header_seed, None, None,
                       header_forced_weather)


# ---------------------------------------------------------------------------
# Walk
# ---------------------------------------------------------------------------

def walk_events_files(roots: list[Path]) -> Iterator[Path]:
    for root in roots:
        if not root.exists():
            continue
        for dirpath, dirnames, filenames in os.walk(root):
            dirnames[:] = [d for d in dirnames if d not in DEFAULT_EXCLUDE_DIR_NAMES]
            for fn in filenames:
                if fn == "events.jsonl":
                    yield Path(dirpath) / fn


def walk_trace_files(roots: list[Path]) -> Iterator[Path]:
    for root in roots:
        if not root.exists():
            continue
        for dirpath, dirnames, filenames in os.walk(root):
            dirnames[:] = [d for d in dirnames if d not in DEFAULT_EXCLUDE_DIR_NAMES]
            for fn in filenames:
                if fn.startswith("trace-") and fn.endswith(".jsonl"):
                    yield Path(dirpath) / fn


# ---------------------------------------------------------------------------
# Run id
# ---------------------------------------------------------------------------

def derive_run_id(file_path: Path, commit_hash: str, mtime_ns: int) -> str:
    """Deterministic 16-hex-char surrogate.

    Identical (path, commit, mtime) produces an identical run_id, so saved
    queries / external references survive a ``rm logs/runs.duckdb`` rebuild.
    """
    h = hashlib.sha256(f"{file_path.resolve()}|{commit_hash}|{mtime_ns}".encode())
    return h.hexdigest()[:16]


# ---------------------------------------------------------------------------
# Database helpers
# ---------------------------------------------------------------------------

def open_db(path: Path, *, read_only: bool = False) -> duckdb.DuckDBPyConnection:
    if read_only and not path.exists():
        sys.exit(f"logdb: {path} does not exist; run `just logdb-build` first.")
    path.parent.mkdir(parents=True, exist_ok=True)
    return duckdb.connect(str(path), read_only=read_only)


def ensure_schema(con: duckdb.DuckDBPyConnection, *, allow_rebuild: bool) -> None:
    con.execute(SCHEMA_DDL)
    row = con.execute("SELECT version FROM schema_version").fetchone()
    if row is None:
        con.execute("INSERT INTO schema_version VALUES (?)", [SCHEMA_VERSION])
        return
    if row[0] == SCHEMA_VERSION:
        return
    if not allow_rebuild:
        sys.exit(
            f"logdb: schema_version mismatch (db={row[0]}, code={SCHEMA_VERSION}). "
            f"Re-run with --rebuild to drop and recreate."
        )
    print(f"logdb: rebuilding (schema {row[0]} -> {SCHEMA_VERSION})", file=sys.stderr)


def drop_db(path: Path) -> None:
    if path.exists():
        path.unlink()
    wal = path.with_suffix(path.suffix + ".wal")
    if wal.exists():
        wal.unlink()


# ---------------------------------------------------------------------------
# Ingest helpers
# ---------------------------------------------------------------------------

@dataclass
class Buffers:
    activations: list[tuple] = field(default_factory=list)
    colony_scores: list[tuple] = field(default_factory=list)
    cat_snapshots: list[tuple] = field(default_factory=list)
    cat_snapshot_scores: list[tuple] = field(default_factory=list)
    deaths: list[tuple] = field(default_factory=list)
    trace_l2: list[tuple] = field(default_factory=list)
    trace_l3: list[tuple] = field(default_factory=list)
    trace_l3_ranked: list[tuple] = field(default_factory=list)


# Column order MUST match the table DDL.
INSERT_SQL = {
    "runs": (
        "INSERT INTO runs VALUES "
        "(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    ),
    "run_footers": (
        "INSERT INTO run_footers VALUES "
        "(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, "
        " MAP(?, ?), MAP(?, ?), MAP(?, ?), MAP(?, ?), ?, "
        " ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    ),
    "activations": "INSERT INTO activations VALUES (?, ?, ?, ?)",
    "colony_scores": (
        "INSERT INTO colony_scores VALUES "
        "(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, "
        " ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    ),
    "cat_snapshots": (
        "INSERT INTO cat_snapshots VALUES "
        "(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    ),
    "cat_snapshot_scores": "INSERT INTO cat_snapshot_scores VALUES (?, ?, ?, ?, ?)",
    "deaths": "INSERT INTO deaths VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    "trace_l2": (
        "INSERT INTO trace_l2 VALUES "
        "(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    ),
    "trace_l3": "INSERT INTO trace_l3 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    "trace_l3_ranked": "INSERT INTO trace_l3_ranked VALUES (?, ?, ?, ?, ?, ?, ?)",
}


def flush(con: duckdb.DuckDBPyConnection, buf: Buffers) -> None:
    for name, sql in INSERT_SQL.items():
        if name in {"runs", "run_footers"}:
            continue  # those are inserted as single-row execute() calls
        rows = getattr(buf, name)
        if rows:
            con.executemany(sql, rows)
            rows.clear()


# ---------------------------------------------------------------------------
# Parsing helpers
# ---------------------------------------------------------------------------

def _struct_from_dict(d: dict, fields: tuple[str, ...]) -> dict:
    """DuckDB STRUCTs bind via Python dicts (tuples become LIST<DOUBLE>)."""
    return {f: d.get(f, 0.0) for f in fields}


def _parse_commit_time(value: Any) -> datetime | None:
    if not isinstance(value, str):
        return None
    s = value.strip()
    if s.endswith("Z"):
        s = s[:-1] + "+00:00"
    try:
        return datetime.fromisoformat(s)
    except ValueError:
        return None


def _map_kv(d: dict | None) -> tuple[list[str], list[int]]:
    """Split ``{key: int}`` into parallel arrays for DuckDB ``MAP(keys, values)``."""
    if not d:
        return [], []
    keys: list[str] = []
    vals: list[int] = []
    for k, v in d.items():
        if isinstance(v, bool):
            continue
        if isinstance(v, (int, float)):
            keys.append(str(k))
            vals.append(int(v))
    return keys, vals


# ---------------------------------------------------------------------------
# Ingest: events.jsonl
# ---------------------------------------------------------------------------

def ingest_events_file(con: duckdb.DuckDBPyConnection, file_path: Path,
                       cache: dict[str, tuple[int, str]],
                       *, with_scores: bool) -> str | None:
    """Stream-parse one events.jsonl. Returns the run_id or None if skipped.

    ``with_scores`` controls whether ``cat_snapshot_scores`` rows are emitted.
    Off by default: ~2.5M extra rows on the 27-run baseline, the dominant
    insert cost. Opt in with ``logdb-build --with-scores``.
    """
    stat = file_path.stat()
    cache_hit = cache.get(str(file_path))
    if cache_hit and cache_hit[0] == stat.st_mtime_ns:
        return cache_hit[1]

    buf = Buffers()
    header: dict | None = None
    footer: dict | None = None
    last_colony: dict | None = None
    tick_start: int | None = None
    tick_end: int | None = None
    run_id: str | None = None
    pending = 0

    with file_path.open("r", encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if not line:
                continue
            try:
                rec = json.loads(line)
            except json.JSONDecodeError:
                continue

            if header is None:
                if not rec.get("_header"):
                    sys.stderr.write(
                        f"logdb: {file_path} line 1 is not _header; skipping run\n"
                    )
                    return None
                header = rec
                run_id = derive_run_id(file_path, rec.get("commit_hash", ""),
                                       stat.st_mtime_ns)
                ident = classify_path(file_path, rec.get("seed"),
                                      rec.get("forced_weather"))
                trace_dir = file_path.parent
                trace_files = sorted(p for p in trace_dir.glob("trace-*.jsonl"))
                trace_path = str(trace_files[0]) if trace_files else None
                narrative = trace_dir / "narrative.jsonl"
                narrative_path = str(narrative) if narrative.exists() else None
                con.execute(INSERT_SQL["runs"], [
                    run_id, ident.archive, ident.kind, ident.seed, ident.rep,
                    ident.focal, ident.forced_weather,
                    rec.get("commit_hash"), rec.get("commit_hash_short"),
                    bool(rec.get("commit_dirty", False)),
                    _parse_commit_time(rec.get("commit_time")),
                    rec.get("duration_secs"),
                    None, None,
                    str(file_path), narrative_path, trace_path, False,
                    json.dumps(rec.get("constants", {})),
                    json.dumps(rec.get("sensory_env_multipliers"))
                        if rec.get("sensory_env_multipliers") is not None else None,
                ])
                continue

            if rec.get("_footer"):
                footer = rec
                continue

            tick = rec.get("tick")
            if tick is not None:
                if tick_start is None:
                    tick_start = tick
                tick_end = tick

            rec_type = rec.get("type")
            if rec_type not in HANDLED_EVENT_TYPES:
                continue

            if rec_type == "CatSnapshot":
                pos = rec.get("position", [None, None]) or [None, None]
                buf.cat_snapshots.append((
                    run_id, tick, rec.get("cat"),
                    rec.get("current_action"),
                    pos[0] if len(pos) > 0 else None,
                    pos[1] if len(pos) > 1 else None,
                    rec.get("mood_valence"),
                    rec.get("mood_modifier_count"),
                    rec.get("health"),
                    rec.get("corruption"),
                    rec.get("magic_affinity"),
                    rec.get("life_stage"),
                    rec.get("sex"),
                    rec.get("orientation"),
                    rec.get("season"),
                    rec.get("social_warmth"),
                    rec.get("is_pregnant"),
                    _struct_from_dict(rec.get("needs", {}) or {}, NEEDS_FIELDS),
                    _struct_from_dict(rec.get("personality", {}) or {}, PERSONALITY_FIELDS),
                    _struct_from_dict(rec.get("skills", {}) or {}, SKILLS_FIELDS),
                    json.dumps(rec.get("relationships"))
                        if rec.get("relationships") is not None else None,
                ))
                pending += 1
                if with_scores:
                    cat = rec.get("cat")
                    for entry in rec.get("last_scores") or []:
                        if isinstance(entry, (list, tuple)) and len(entry) >= 2:
                            buf.cat_snapshot_scores.append(
                                (run_id, tick, cat, str(entry[0]), float(entry[1]))
                            )
                            pending += 1
            elif rec_type == "Death":
                pos = rec.get("location") or rec.get("position") or [None, None]
                buf.deaths.append((
                    run_id, tick, rec.get("cat"),
                    rec.get("cause"),
                    rec.get("injury_source"),
                    rec.get("killer_species"),
                    pos[0] if len(pos) > 0 else None,
                    pos[1] if len(pos) > 1 else None,
                ))
                pending += 1
            elif rec_type == "ColonyScore":
                last_colony = rec
                buf.colony_scores.append((
                    run_id, tick,
                    rec.get("shelter"), rec.get("nourishment"), rec.get("health"),
                    rec.get("happiness"), rec.get("fulfillment"), rec.get("welfare"),
                    rec.get("aggregate"),
                    rec.get("positive_activation_score"),
                    rec.get("positive_features_active"),
                    rec.get("positive_features_total"),
                    rec.get("neutral_features_active"),
                    rec.get("neutral_features_total"),
                    rec.get("negative_events_total"),
                    rec.get("seasons_survived"),
                    rec.get("bonds_formed"),
                    rec.get("peak_population"),
                    rec.get("deaths_starvation"),
                    rec.get("deaths_old_age"),
                    rec.get("deaths_injury"),
                    rec.get("aspirations_completed"),
                    rec.get("structures_built"),
                    rec.get("kittens_born"),
                    rec.get("prey_dens_discovered"),
                    rec.get("friends_count"),
                    rec.get("partners_count"),
                    rec.get("mates_count"),
                    rec.get("living_cats"),
                ))
                pending += 1

            if pending >= BATCH_FLUSH_ROWS:
                flush(con, buf)
                pending = 0

    if header is None or run_id is None:
        return None

    flush(con, buf)
    con.execute(
        "UPDATE runs SET tick_start = ?, tick_end = ?, footer_written = ? WHERE run_id = ?",
        [tick_start, tick_end, footer is not None, run_id],
    )

    if footer is not None:
        cs = last_colony or {}
        deaths_k, deaths_v = _map_kv(footer.get("deaths_by_cause"))
        cont_k, cont_v = _map_kv(footer.get("continuity_tallies"))
        plan_k, plan_v = _map_kv(footer.get("plan_failures_by_reason"))
        intr_k, intr_v = _map_kv(footer.get("interrupts_by_reason"))
        never_fired = footer.get("never_fired_expected_positives") or []
        con.execute(INSERT_SQL["run_footers"], [
            run_id,
            footer.get("positive_features_active"),
            footer.get("positive_features_total"),
            footer.get("neutral_features_active"),
            footer.get("neutral_features_total"),
            footer.get("negative_events_total"),
            footer.get("anxiety_interrupt_total"),
            footer.get("shadow_fox_spawn_total"),
            footer.get("shadow_foxes_avoided_ward_total"),
            footer.get("ward_count_final"),
            footer.get("ward_avg_strength_final"),
            footer.get("wards_placed_total"),
            footer.get("wards_despawned_total"),
            footer.get("ward_siege_started_total"),
            deaths_k, deaths_v, cont_k, cont_v,
            plan_k, plan_v, intr_k, intr_v,
            list(never_fired),
            cs.get("aggregate"), cs.get("welfare"),
            cs.get("shelter"), cs.get("nourishment"), cs.get("health"),
            cs.get("happiness"), cs.get("fulfillment"),
            cs.get("seasons_survived"), cs.get("peak_population"),
            cs.get("kittens_born"), cs.get("bonds_formed"),
            cs.get("living_cats"),
        ])

    cache[str(file_path)] = (stat.st_mtime_ns, run_id)
    con.execute(
        "INSERT OR REPLACE INTO ingested_files VALUES (?, ?, ?, ?, ?, ?, ?)",
        [str(file_path), stat.st_mtime_ns, stat.st_size, run_id, "events",
         datetime.now(timezone.utc), SCHEMA_VERSION],
    )
    return run_id


# ---------------------------------------------------------------------------
# Ingest: trace-*.jsonl
# ---------------------------------------------------------------------------

def ingest_trace_file(con: duckdb.DuckDBPyConnection, file_path: Path,
                      cache: dict[str, tuple[int, str]]) -> str | None:
    stat = file_path.stat()
    cache_hit = cache.get(str(file_path))
    if cache_hit and cache_hit[0] == stat.st_mtime_ns:
        return cache_hit[1]

    sibling_events = file_path.parent / "events.jsonl"
    run_id: str | None = None
    if sibling_events.exists():
        existing = con.execute(
            "SELECT run_id FROM runs WHERE events_path = ?", [str(sibling_events)]
        ).fetchone()
        if existing:
            run_id = existing[0]

    buf = Buffers()
    header_seen = False
    pending = 0

    with file_path.open("r", encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if not line:
                continue
            # Drop L1 records before the json.loads round-trip — they are
            # ~92% of trace volume and we never store them.
            if header_seen and ('"layer":"L1"' in line or '"layer": "L1"' in line):
                continue
            try:
                rec = json.loads(line)
            except json.JSONDecodeError:
                continue

            if not header_seen:
                if rec.get("_header"):
                    if run_id is None:
                        rid = derive_run_id(file_path, rec.get("commit_hash", ""),
                                            stat.st_mtime_ns)
                        focal = file_path.stem.split("-", 1)[1] if "-" in file_path.stem else None
                        ident = classify_path(file_path, rec.get("seed"),
                                              rec.get("forced_weather"))
                        if focal and not ident.focal:
                            ident.focal = focal
                            if ident.kind == "flat":
                                ident.kind = "trace"
                        con.execute(INSERT_SQL["runs"], [
                            rid, ident.archive, ident.kind, ident.seed, ident.rep,
                            ident.focal, ident.forced_weather,
                            rec.get("commit_hash"), rec.get("commit_hash_short"),
                            bool(rec.get("commit_dirty", False)),
                            _parse_commit_time(rec.get("commit_time")),
                            rec.get("duration_secs"),
                            None, None,
                            None, None, str(file_path), False,
                            json.dumps(rec.get("constants", {})),
                            json.dumps(rec.get("sensory_env_multipliers"))
                                if rec.get("sensory_env_multipliers") is not None else None,
                        ])
                        run_id = rid
                    header_seen = True
                    continue
                if run_id is None:
                    return None
                header_seen = True

            tick = rec.get("tick")
            layer = rec.get("layer")
            cat = rec.get("cat")

            if layer == "L2":
                eligibility = rec.get("eligibility") or {}
                composition = rec.get("composition") or {}
                intention = rec.get("intention") or {}
                buf.trace_l2.append((
                    run_id, tick, cat, rec.get("dse"),
                    bool(eligibility.get("passed", False)),
                    list(eligibility.get("markers_required") or []),
                    composition.get("mode"), composition.get("raw"),
                    rec.get("maslow_pregate"), rec.get("final_score"),
                    intention.get("kind"), intention.get("goal_state"),
                    json.dumps(rec.get("considerations"))
                        if rec.get("considerations") is not None else None,
                    json.dumps(rec.get("modifiers"))
                        if rec.get("modifiers") is not None else None,
                    json.dumps(rec.get("top_losing"))
                        if rec.get("top_losing") is not None else None,
                ))
                pending += 1
            elif layer == "L3":
                softmax = rec.get("softmax") or {}
                momentum = rec.get("momentum") or {}
                intention = rec.get("intention") or {}
                buf.trace_l3.append((
                    run_id, tick, cat,
                    rec.get("chosen"), softmax.get("temperature"),
                    momentum.get("active_intention"),
                    momentum.get("commitment_strength", momentum.get("strength")),
                    bool(momentum.get("preempted", False)),
                    intention.get("kind"),
                    json.dumps(rec.get("goap_plan"))
                        if rec.get("goap_plan") is not None else None,
                ))
                pending += 1
                ranked = rec.get("ranked") or []
                probs = softmax.get("probabilities") or []
                for idx, item in enumerate(ranked):
                    if not isinstance(item, (list, tuple)) or len(item) < 2:
                        continue
                    prob = probs[idx] if idx < len(probs) else None
                    buf.trace_l3_ranked.append((
                        run_id, tick, cat, str(item[0]),
                        float(item[1]),
                        float(prob) if prob is not None else None,
                        idx,
                    ))
                    pending += 1
            # L1 dropped (see comment above).

            if pending >= BATCH_FLUSH_ROWS:
                flush(con, buf)
                pending = 0

    flush(con, buf)
    cache[str(file_path)] = (stat.st_mtime_ns, run_id or "")
    con.execute(
        "INSERT OR REPLACE INTO ingested_files VALUES (?, ?, ?, ?, ?, ?, ?)",
        [str(file_path), stat.st_mtime_ns, stat.st_size, run_id, "trace",
         datetime.now(timezone.utc), SCHEMA_VERSION],
    )
    return run_id


# ---------------------------------------------------------------------------
# Subcommand: build
# ---------------------------------------------------------------------------

def cmd_build(args: argparse.Namespace) -> int:
    if args.rebuild:
        drop_db(args.db)
    con = open_db(args.db, read_only=False)
    ensure_schema(con, allow_rebuild=args.rebuild)

    if args.archive:
        candidate = Path(args.archive)
        if candidate.is_absolute() or candidate.exists():
            roots = [candidate]
        else:
            roots = [LOGS_DIR / args.archive]
    else:
        roots = [
            p for p in LOGS_DIR.iterdir()
            if p.is_dir() and p.name not in DEFAULT_EXCLUDE_DIR_NAMES
        ]

    existing = con.execute(
        "SELECT file_path, mtime_ns, run_id FROM ingested_files"
    ).fetchall()
    cache: dict[str, tuple[int, str]] = {row[0]: (row[1], row[2] or "") for row in existing}

    events_files = sorted(set(walk_events_files(roots)))
    trace_files = sorted(set(walk_trace_files(roots))) if args.with_traces else []

    flags = []
    if args.with_scores:
        flags.append("scores")
    if args.with_traces:
        flags.append("traces")
    flag_str = f" [+{','.join(flags)}]" if flags else ""
    print(
        f"logdb: scanning {len(events_files)} events.jsonl + "
        f"{len(trace_files)} trace files under {[str(r) for r in roots]}{flag_str}",
        file=sys.stderr,
    )

    for path in tqdm(events_files, desc="events", unit="file"):
        try:
            ingest_events_file(con, path, cache, with_scores=args.with_scores)
        except Exception as exc:  # noqa: BLE001 — surface the bad file
            print(f"\nlogdb: failed on {path}: {exc}", file=sys.stderr)
            raise

    for path in tqdm(trace_files, desc="traces", unit="file"):
        try:
            ingest_trace_file(con, path, cache)
        except Exception as exc:  # noqa: BLE001
            print(f"\nlogdb: failed on {path}: {exc}", file=sys.stderr)
            raise

    con.execute("ANALYZE")

    n_runs = con.execute("SELECT COUNT(*) FROM runs").fetchone()[0]
    n_footers = con.execute("SELECT COUNT(*) FROM run_footers").fetchone()[0]
    n_colony = con.execute("SELECT COUNT(*) FROM colony_scores").fetchone()[0]
    print(
        f"logdb: {n_runs} runs ({n_footers} with footer); "
        f"{n_colony} colony_score rows.",
        file=sys.stderr,
    )
    return 0


# ---------------------------------------------------------------------------
# Subcommand: query
# ---------------------------------------------------------------------------

@contextmanager
def _wide_pandas() -> Iterator[None]:
    import pandas as pd  # type: ignore[import-not-found]
    saved = {
        "display.max_rows": pd.get_option("display.max_rows"),
        "display.max_columns": pd.get_option("display.max_columns"),
        "display.width": pd.get_option("display.width"),
    }
    pd.set_option("display.max_rows", 200)
    pd.set_option("display.max_columns", 50)
    pd.set_option("display.width", 200)
    try:
        yield
    finally:
        for key, val in saved.items():
            pd.set_option(key, val)


def cmd_query(args: argparse.Namespace) -> int:
    con = open_db(args.db, read_only=True)
    df = con.execute(args.sql).fetchdf()
    if args.format == "csv":
        print(df.to_csv(index=False))
    elif args.format == "json":
        print(df.to_json(orient="records", indent=2))
    else:
        with _wide_pandas():
            print(df.to_string(index=False))
    return 0


# ---------------------------------------------------------------------------
# Subcommand: shell
# ---------------------------------------------------------------------------

def cmd_shell(args: argparse.Namespace) -> int:
    if not args.db.exists():
        sys.exit(f"logdb: {args.db} does not exist; run `just logdb-build` first.")
    duckdb_bin = shutil.which("duckdb")
    if duckdb_bin is None:
        sys.exit(
            "logdb: duckdb CLI not found. Install via `brew install duckdb` "
            "(or `uv tool install duckdb`) for the interactive shell."
        )
    return subprocess.call([duckdb_bin, str(args.db)])


# ---------------------------------------------------------------------------
# Subcommand: chart
# ---------------------------------------------------------------------------

def _discover_recipes() -> dict[str, Any]:
    sys.path.insert(0, str(REPO_ROOT / "scripts"))
    import logdb_charts  # type: ignore[import-not-found]

    recipes: dict[str, Any] = {}
    for info in pkgutil.iter_modules(logdb_charts.__path__):
        mod = __import__(f"logdb_charts.{info.name}", fromlist=["build", "register"])
        recipes[info.name.replace("_", "-")] = mod
    return recipes


def cmd_chart(args: argparse.Namespace) -> int:
    recipes = _discover_recipes()
    if args.recipe not in recipes:
        sys.exit(
            f"logdb: unknown chart recipe {args.recipe!r}. "
            f"Available: {sorted(recipes)}"
        )
    mod = recipes[args.recipe]
    con = open_db(args.db, read_only=True)
    chart = mod.build(con, args)
    CHARTS_DIR.mkdir(parents=True, exist_ok=True)
    ts = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    out = CHARTS_DIR / f"{args.recipe}-{ts}.html"
    chart.save(str(out))
    print(str(out))
    return 0


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(prog="logdb", description=__doc__)
    p.add_argument("--db", type=Path, default=DEFAULT_DB,
                   help=f"DuckDB file (default: {DEFAULT_DB})")
    sub = p.add_subparsers(dest="cmd", required=True)

    pb = sub.add_parser("build", help="ingest archives")
    pb.add_argument("archive", nargs="?", default=None,
                    help="single archive name or absolute path; default: all logs/<dir>")
    pb.add_argument("--rebuild", action="store_true",
                    help="drop and recreate the DB before ingest")
    pb.add_argument("--with-scores", action="store_true",
                    help="also ingest cat_snapshot_scores (~2.5M rows; ~5x slower)")
    pb.add_argument("--with-traces", action="store_true",
                    help="also ingest trace_l2/l3/l3_ranked from trace-*.jsonl sidecars")
    pb.set_defaults(func=cmd_build)

    pq = sub.add_parser("query", help="run one SQL statement")
    pq.add_argument("sql")
    pq.add_argument("--format", choices=("table", "csv", "json"), default="table")
    pq.set_defaults(func=cmd_query)

    ps = sub.add_parser("shell", help="open the duckdb interactive shell")
    ps.set_defaults(func=cmd_shell)

    pc = sub.add_parser("chart", help="render a chart recipe to logs/charts/")
    pc.add_argument("recipe", help="recipe name (e.g. colony-score-over-time)")
    pc.set_defaults(func=cmd_chart)

    try:
        recipes = _discover_recipes()
    except Exception:
        recipes = {}
    if recipes:
        pc.epilog = "Available recipes: " + ", ".join(sorted(recipes))
        for mod in recipes.values():
            if hasattr(mod, "register"):
                mod.register(pc)
    return p


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
