"""Tests for scripts/logq envelope + subtool behavior.

Runs against real log bundles in ./logs/ when present, otherwise falls
back to synthetic fixtures written into a tempdir.

Uses stdlib `unittest` because pytest isn't installed on the dev box.
Invoke with `just test-logq` (or `python -m unittest tests.logq.test_envelope`).
"""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

# Make scripts/logq importable.
REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "scripts" / "logq"))

from envelope import (  # noqa: E402
    Envelope, event_id, narrative_id, trace_id,
)
import logq as logq_mod  # noqa: E402


# ── synthetic-bundle helper ─────────────────────────────────────────────────

def write_bundle(dir_: Path, *, omit_final_tick: bool = False) -> Path:
    """Write a small synthetic events.jsonl + narrative.jsonl + trace file.

    `omit_final_tick=True` reproduces the real-soak schema where the
    footer doesn't carry final_tick — used to test the derived-from-
    max(.tick) fallback.
    """
    log_dir = dir_ / "tuned-synth"
    log_dir.mkdir(parents=True)

    header = {
        "_header": True,
        "commit_hash": "abc123def456",
        "commit_hash_short": "abc123d",
        "commit_dirty": False,
        "commit_time": "2026-04-24T00:00:00-00:00",
        "seed": 99,
        "duration_secs": 300,
    }
    # CatSnapshot rows for the actions subtool — Whiskers spends most
    # of its time in Patrol (the Thistle pattern), Mochi spreads across
    # Eat/Sleep, distinguishing per-cat aggregation from colony total.
    cat_snapshots = [
        {"tick": 100, "type": "CatSnapshot", "cat": "Whiskers",
         "current_action": "Patrol"},
        {"tick": 200, "type": "CatSnapshot", "cat": "Whiskers",
         "current_action": "Patrol"},
        {"tick": 300, "type": "CatSnapshot", "cat": "Whiskers",
         "current_action": "Patrol"},
        {"tick": 400, "type": "CatSnapshot", "cat": "Whiskers",
         "current_action": "Eat"},
        {"tick": 500, "type": "CatSnapshot", "cat": "Mochi",
         "current_action": "Eat"},
        {"tick": 600, "type": "CatSnapshot", "cat": "Mochi",
         "current_action": "Sleep"},
    ]
    # Plan-cadence triggers for cat-timeline summarize mode: Whiskers
    # creates plans every ~50 ticks, Mochi every ~200.
    plan_create_events = [
        {"tick": t, "type": "PlanCreated", "cat": "Whiskers"}
        for t in (50, 100, 150, 200, 250)
    ] + [
        {"tick": t, "type": "PlanCreated", "cat": "Mochi"}
        for t in (300, 500, 700)
    ]
    # HuntAttempt rows (ticket 149) — three attempts per cat with
    # mixed outcomes so the hunt-success subtool has a non-trivial
    # success-rate to compute. Whiskers: 2 kills (one direct, one
    # multi-kill replan) and 1 approach-loss = 2/3 = 66.7% success.
    # Mochi: 1 kill, 1 stalk-loss, 1 abandoned = 1/3 = 33.3% success.
    # Colony total: 6 attempts, 3 kills = 50% success.
    hunt_attempt_events = [
        {"tick": 110, "type": "HuntAttempt", "cat": "Whiskers",
         "prey_species": "mouse", "location": [5, 5],
         "outcome": "killed", "start_tick": 100, "end_tick": 110,
         "start_distance": 6, "failure_reason": None},
        {"tick": 220, "type": "HuntAttempt", "cat": "Whiskers",
         "prey_species": "mouse", "location": [6, 5],
         "outcome": "killed_and_replanned", "start_tick": 215, "end_tick": 220,
         "start_distance": 4, "failure_reason": None},
        {"tick": 330, "type": "HuntAttempt", "cat": "Whiskers",
         "prey_species": "bird", "location": [9, 9],
         "outcome": "lost_during_approach", "start_tick": 310, "end_tick": 330,
         "start_distance": 8, "failure_reason": "lost prey during approach"},
        {"tick": 540, "type": "HuntAttempt", "cat": "Mochi",
         "prey_species": "mouse", "location": [3, 3],
         "outcome": "killed", "start_tick": 530, "end_tick": 540,
         "start_distance": 5, "failure_reason": None},
        {"tick": 650, "type": "HuntAttempt", "cat": "Mochi",
         "prey_species": "bird", "location": [10, 4],
         "outcome": "lost_during_stalk", "start_tick": 640, "end_tick": 650,
         "start_distance": 7, "failure_reason": "anxiety spooked prey"},
        {"tick": 750, "type": "HuntAttempt", "cat": "Mochi",
         "prey_species": "fish", "location": [12, 6],
         "outcome": "abandoned", "start_tick": 745, "end_tick": 750,
         "start_distance": 9, "failure_reason": "prey despawned"},
    ]
    footer = {
        "_footer": True,
        "deaths_by_cause": {"Starvation": 1, "ShadowFoxAmbush": 1},
        "continuity_tallies": {"grooming": 2, "play": 0, "mentoring": 0,
                                "burial": 0, "courtship": 0,
                                "mythic-texture": 0},
        "never_fired_expected_positives": ["KittenBorn"],
        "wards_placed_total": 10,
        "interrupts_by_reason": {
            "urgency CriticalSafety (level 2) preempted level 4 plan": 425,
            "urgency CriticalHealth (level 1) preempted level 2 plan": 12,
            "urgency Starvation (level 1) preempted level 4 plan": 3,
        },
        "plan_failures_by_reason": {
            "GatherHerb: herb already taken": 2,
            "Construct: no target": 1,
        },
        "anxiety_interrupt_total": 15,
        "negative_events_total": 50,
        "positive_features_active": 13,
        "positive_features_total": 44,
    }
    if not omit_final_tick:
        footer["final_tick"] = 1500
    events = [
        header,
        {"tick": 100, "type": "ColonyScore", "aggregate": 0.9},
        {"tick": 500, "type": "Death", "cat": "Whiskers", "cause": "Starvation",
         "location": [3, 4], "injury_source": None},
        {"tick": 700, "type": "FeatureActivated", "feature": "BondFormed"},
        {"tick": 900, "type": "ColonyScore", "aggregate": 0.5},  # cliff vs 0.9
        {"tick": 1200, "type": "Death", "cat": "Mochi", "cause": "ShadowFoxAmbush",
         "location": [7, 2], "injury_source": "Fox"},
        *cat_snapshots,
        *plan_create_events,
        *hunt_attempt_events,
        footer,
    ]
    (log_dir / "events.jsonl").write_text(
        "\n".join(json.dumps(e) for e in events) + "\n"
    )

    narrative = [
        {"_header": True, "commit_hash": "abc123def456", "seed": 99},
        {"tick": 500, "day": 1, "phase": "Dawn",
         "text": "Whiskers starved in the wilds.", "tier": "Danger"},
        {"tick": 1200, "day": 2, "phase": "Dusk",
         "text": "A fox took Mochi.", "tier": "Legend"},
        {"tick": 1300, "day": 2, "phase": "Dusk",
         "text": "The birds fell silent.", "tier": "Nature"},
    ]
    (log_dir / "narrative.jsonl").write_text(
        "\n".join(json.dumps(e) for e in narrative) + "\n"
    )

    trace = [
        {"_header": True, "commit_hash": "abc123def456", "focal_cat": "Whiskers",
         "seed": 99},
        {"tick": 100, "cat": "Whiskers", "layer": "L3",
         "chosen": "Forage", "ranked": [["Forage", 0.8], ["Sleep", 0.3]],
         "softmax": {"temperature": 0.15}, "momentum": {}, "intention": {}},
        {"tick": 200, "cat": "Whiskers", "layer": "L3",
         "chosen": "Forage", "ranked": [["Forage", 0.7]],
         "softmax": {"temperature": 0.15}, "momentum": {}, "intention": {}},
        {"tick": 300, "cat": "Whiskers", "layer": "L3",
         "chosen": "Sleep", "ranked": [["Sleep", 0.9]],
         "softmax": {"temperature": 0.15}, "momentum": {}, "intention": {}},
        {"tick": 100, "cat": "Whiskers", "layer": "L2",
         "dse": "Forage", "final_score": 0.8,
         "eligibility": {"passed": True}},
        {"tick": 200, "cat": "Whiskers", "layer": "L2",
         "dse": "Hunt", "final_score": 0.1,
         "eligibility": {"passed": False, "markers_required": ["weapon"]}},
    ]
    (log_dir / "trace-Whiskers.jsonl").write_text(
        "\n".join(json.dumps(e) for e in trace) + "\n"
    )

    return log_dir


def invoke(args: list[str]) -> dict:
    """Run a subtool in-process, capture JSON output."""
    import io, contextlib
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        rc = logq_mod.main(args)
    out = buf.getvalue().strip()
    assert rc in (0, 2), f"unexpected rc={rc}: {out}"
    return json.loads(out)


# ── tests ───────────────────────────────────────────────────────────────────

class EnvelopeShapeTests(unittest.TestCase):
    """Every subtool returns the standard envelope shape."""

    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.log_dir = write_bundle(Path(cls.tmp.name))

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def _assert_envelope(self, env: dict) -> None:
        for key in ("query", "scan_stats", "results", "narrative", "next"):
            self.assertIn(key, env, f"missing envelope key: {key}")
        self.assertIsInstance(env["query"], dict)
        self.assertIsInstance(env["scan_stats"], dict)
        self.assertIsInstance(env["results"], list)
        self.assertIsInstance(env["narrative"], str)
        self.assertIsInstance(env["next"], list)
        for key in ("scanned", "returned", "more_available", "narrow_by"):
            self.assertIn(key, env["scan_stats"])

    def test_run_summary(self):
        env = invoke(["run-summary", str(self.log_dir)])
        self._assert_envelope(env)
        self.assertTrue(env["results"], "run-summary should return header+footer")

    def test_events(self):
        env = invoke(["events", str(self.log_dir), "--kind=Death"])
        self._assert_envelope(env)
        self.assertEqual(len(env["results"]), 2)

    def test_deaths(self):
        env = invoke(["deaths", str(self.log_dir)])
        self._assert_envelope(env)
        self.assertEqual(env["scan_stats"]["returned"], 2)

    def test_narrative(self):
        env = invoke(["narrative", str(self.log_dir)])
        self._assert_envelope(env)
        # Default tiers exclude Nature; should return Danger+Legend only.
        tiers = {r["tier"] for r in env["results"]}
        self.assertEqual(tiers, {"Danger", "Legend"})

    def test_trace(self):
        env = invoke(["trace", str(self.log_dir), "Whiskers", "--layer=L3"])
        self._assert_envelope(env)
        # Aggregated chosen-counts: Forage=2, Sleep=1.
        self.assertEqual(len(env["results"]), 2)

    def test_cat_timeline(self):
        env = invoke(["cat-timeline", str(self.log_dir), "Whiskers"])
        self._assert_envelope(env)
        self.assertTrue(any(r["kind"] == "event" for r in env["results"]))
        self.assertTrue(any(r["kind"] == "narrative" for r in env["results"]))

    def test_anomalies(self):
        env = invoke(["anomalies", str(self.log_dir)])
        self._assert_envelope(env)
        names = {r.get("name") for r in env["results"]}
        # Synthetic bundle has: 1 starvation, 1 shadowfox (<=5 → no anomaly),
        # zeroed continuity canaries, 1 never-fired positive, and a
        # ColonyScore cliff from 0.9 → 0.5.
        self.assertIn("starvation_deaths", names)
        self.assertIn("never_fired_expected", names)
        self.assertIn("play", names)  # zero-tally continuity canary
        self.assertNotIn("shadowfox_ambush_deaths", names)  # 1 <= 5, not a fail


class NullResultNearestMatchTests(unittest.TestCase):
    """Null results return nearest-match evidence, not []."""

    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.log_dir = write_bundle(Path(cls.tmp.name))

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def test_deaths_empty_range_cites_nearest(self):
        env = invoke(["deaths", str(self.log_dir), "--tick-range=600..650"])
        self.assertEqual(env["results"], [])
        # Narrative should name at least one of the deaths (tick 500 or 1200).
        self.assertTrue(
            "500" in env["narrative"] or "1200" in env["narrative"],
            env["narrative"],
        )

    def test_events_empty_kind_cites_nearest(self):
        env = invoke(["events", str(self.log_dir),
                      "--kind=Death", "--tick-range=600..650"])
        self.assertEqual(env["results"], [])
        self.assertIn("Nearest", env["narrative"])


class QueryEchoTests(unittest.TestCase):
    """The effective query (incl. defaults) is echoed back."""

    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.log_dir = write_bundle(Path(cls.tmp.name))

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def test_narrative_echoes_default_tiers(self):
        env = invoke(["narrative", str(self.log_dir)])
        self.assertEqual(set(env["query"]["tier"]),
                         {"Legend", "Danger", "Significant"})

    def test_events_echoes_none_defaults(self):
        env = invoke(["events", str(self.log_dir)])
        q = env["query"]
        self.assertEqual(q["subtool"], "events")
        # Defaults preserved as None so caller sees what wasn't filtered.
        self.assertIsNone(q["kind"])
        self.assertIsNone(q["tick_range"])


class StableIdTests(unittest.TestCase):
    """IDs are deterministic across runs on the same input."""

    def test_event_id_includes_cat(self):
        r = {"tick": 3812, "type": "Death", "cat": "Simba", "cause": "Starvation"}
        self.assertEqual(event_id(r), "tick:3812:Death:Simba")

    def test_event_id_without_cat(self):
        r = {"tick": 100, "type": "ColonyScore"}
        self.assertEqual(event_id(r), "tick:100:ColonyScore")

    def test_trace_id(self):
        r = {"tick": 42, "cat": "Simba", "layer": "L3"}
        self.assertEqual(trace_id(r), "tick:42:Simba:L3")

    def test_narrative_id_fingerprint_deterministic(self):
        r = {"tick": 500, "tier": "Legend", "text": "The spirits gathered."}
        self.assertEqual(narrative_id(r), narrative_id(dict(r)))
        self.assertTrue(narrative_id(r).startswith("tick:500:Legend:"))


class ActionsSubtoolTests(unittest.TestCase):
    """`actions` aggregates current_action across CatSnapshot events."""

    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.log_dir = write_bundle(Path(cls.tmp.name))

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def test_colony_aggregate_ranks_actions(self):
        env = invoke(["actions", str(self.log_dir)])
        # Bundle has 3 Patrol (Whiskers) + 1 Eat (Whiskers) + 1 Eat
        # (Mochi) + 1 Sleep (Mochi) = 6 rows; top should be Patrol with
        # 3, then Eat with 2.
        actions = [r["action"] for r in env["results"]]
        counts = {r["action"]: r["count"] for r in env["results"]}
        self.assertEqual(actions[0], "Patrol")
        self.assertEqual(counts["Patrol"], 3)
        self.assertEqual(counts["Eat"], 2)
        self.assertEqual(counts["Sleep"], 1)
        # Percentages add to ~100.
        total_pct = sum(r["pct"] for r in env["results"])
        self.assertAlmostEqual(total_pct, 100.0, places=1)

    def test_per_cat_filter_isolates_one_cat(self):
        env = invoke(["actions", str(self.log_dir), "--cat=Whiskers"])
        counts = {r["action"]: r["count"] for r in env["results"]}
        self.assertEqual(counts["Patrol"], 3)
        self.assertEqual(counts["Eat"], 1)
        self.assertNotIn("Sleep", counts)  # Sleep was Mochi-only

    def test_null_result_cites_existing_cats(self):
        env = invoke(["actions", str(self.log_dir), "--cat=NotARealCat"])
        self.assertEqual(env["results"], [])
        # Narrative should mention that CatSnapshot rows DO exist for
        # other cats — that's the nearest-match evidence.
        self.assertIn("CatSnapshot", env["narrative"])
        self.assertTrue(
            "Whiskers" in env["narrative"] or "Mochi" in env["narrative"],
            env["narrative"],
        )

    def test_next_suggests_focal_drill_for_extreme_concentration(self):
        env = invoke(["actions", str(self.log_dir)])
        # Whiskers is 4/5 Patrol = 80% concentration; should be the
        # suggested focal drill.
        nexts = " ".join(env["next"])
        self.assertIn("Whiskers", nexts)


class HuntSuccessSubtoolTests(unittest.TestCase):
    """`hunt-success` aggregates HuntAttempt outcomes (ticket 149)."""

    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.log_dir = write_bundle(Path(cls.tmp.name))

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def _envelope_keys(self, env: dict) -> None:
        for key in ("query", "scan_stats", "results", "narrative", "next"):
            self.assertIn(key, env)
        for key in ("scanned", "returned", "more_available", "narrow_by"):
            self.assertIn(key, env["scan_stats"])

    def test_colony_aggregate_computes_success_rate(self):
        env = invoke(["hunt-success", str(self.log_dir)])
        self._envelope_keys(env)
        # Bundle has 6 HuntAttempts: 3 kill-flavored + 3 loss-flavored
        # = 50% success. Summary row is first; outcome rows follow.
        summary = env["results"][0]
        self.assertEqual(summary["kind"], "summary")
        self.assertEqual(summary["total_attempts"], 6)
        self.assertEqual(summary["kills"], 3)
        self.assertEqual(summary["success_rate_pct"], 50.0)
        outcomes = {r["outcome"]: r["count"] for r in env["results"]
                    if r["kind"] == "outcome"}
        self.assertEqual(outcomes["killed"], 2)
        self.assertEqual(outcomes["killed_and_replanned"], 1)
        self.assertEqual(outcomes["lost_during_approach"], 1)
        self.assertEqual(outcomes["lost_during_stalk"], 1)
        self.assertEqual(outcomes["abandoned"], 1)

    def test_per_cat_filter_isolates_one_cat(self):
        env = invoke(["hunt-success", str(self.log_dir), "--cat=Whiskers"])
        summary = env["results"][0]
        # Whiskers: 2 kills + 1 approach-loss = 66.7%.
        self.assertEqual(summary["total_attempts"], 3)
        self.assertEqual(summary["kills"], 2)
        self.assertAlmostEqual(summary["success_rate_pct"], 66.67, places=1)

    def test_per_species_filter_isolates_prey(self):
        env = invoke(["hunt-success", str(self.log_dir), "--species=mouse"])
        summary = env["results"][0]
        # Mice: 3 attempts, all killed (Whiskers x2, Mochi x1) = 100%.
        self.assertEqual(summary["total_attempts"], 3)
        self.assertEqual(summary["kills"], 3)
        self.assertEqual(summary["success_rate_pct"], 100.0)

    def test_narrative_names_top_failure_reason(self):
        env = invoke(["hunt-success", str(self.log_dir)])
        # Three distinct failure reasons each with 1 occurrence; one of
        # them is named in the narrative as the "top" failure.
        self.assertTrue(
            "lost prey during approach" in env["narrative"]
            or "anxiety spooked prey" in env["narrative"]
            or "prey despawned" in env["narrative"],
            env["narrative"],
        )

    def test_null_result_falls_back_to_prey_killed(self):
        # tick-range outside any HuntAttempt — should still narrate
        # how many HuntAttempts exist in the bundle.
        env = invoke([
            "hunt-success", str(self.log_dir), "--tick-range=2000..3000",
        ])
        self.assertEqual(env["results"], [])
        self.assertIn("HuntAttempt", env["narrative"])

    def test_query_echoes_filters(self):
        env = invoke(["hunt-success", str(self.log_dir),
                      "--cat=Whiskers", "--species=mouse"])
        q = env["query"]
        self.assertEqual(q["subtool"], "hunt-success")
        self.assertEqual(q["cat"], "Whiskers")
        self.assertEqual(q["species"], "mouse")


class FooterSubtoolTests(unittest.TestCase):
    """`footer` exposes every field, optionally drilling into one."""

    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.log_dir = write_bundle(Path(cls.tmp.name))

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def test_full_footer_lists_every_field(self):
        env = invoke(["footer", str(self.log_dir)])
        fields = {r["field"] for r in env["results"]}
        for expected in (
            "deaths_by_cause", "continuity_tallies",
            "interrupts_by_reason", "plan_failures_by_reason",
            "anxiety_interrupt_total", "wards_placed_total",
        ):
            self.assertIn(expected, fields,
                          f"footer-fields list missing {expected}")

    def test_drill_into_dict_field_ranks_entries(self):
        env = invoke([
            "footer", str(self.log_dir),
            "--field=interrupts_by_reason",
        ])
        # Synthetic bundle: top entry is CriticalSafety with 425.
        self.assertEqual(env["results"][0]["key"],
                         "urgency CriticalSafety (level 2) preempted level 4 plan")
        self.assertEqual(env["results"][0]["value"], 425)

    def test_drill_with_top_keys_truncates(self):
        env = invoke([
            "footer", str(self.log_dir),
            "--field=interrupts_by_reason", "--top-keys=2",
        ])
        # Bundle has 3 entries; top-keys=2 caps to 2.
        self.assertEqual(len(env["results"]), 2)

    def test_unknown_field_lists_available(self):
        env = invoke([
            "footer", str(self.log_dir), "--field=nonexistent",
        ])
        self.assertEqual(env["results"], [])
        self.assertIn("Available", env["narrative"])
        self.assertIn("deaths_by_cause", env["narrative"])

    def test_scalar_field_returns_value(self):
        env = invoke([
            "footer", str(self.log_dir),
            "--field=anxiety_interrupt_total",
        ])
        self.assertEqual(len(env["results"]), 1)
        self.assertEqual(env["results"][0]["value"], 15)


class FinalTickDerivationTests(unittest.TestCase):
    """When the footer doesn't carry `final_tick`, run-summary derives it
    from `max(.tick)` over events. The current real-soak footer schema
    omits `final_tick`, so this is the production path."""

    def test_explicit_final_tick_takes_precedence(self):
        with tempfile.TemporaryDirectory() as tmp:
            log_dir = write_bundle(Path(tmp), omit_final_tick=False)
            env = invoke(["run-summary", str(log_dir)])
            footer_row = next(r for r in env["results"] if r["kind"] == "footer")
            self.assertEqual(footer_row["final_tick"], 1500)
            self.assertEqual(footer_row["final_tick_source"], "footer")

    def test_derived_when_footer_omits_final_tick(self):
        with tempfile.TemporaryDirectory() as tmp:
            log_dir = write_bundle(Path(tmp), omit_final_tick=True)
            env = invoke(["run-summary", str(log_dir)])
            footer_row = next(r for r in env["results"] if r["kind"] == "footer")
            # Highest event tick in the bundle is 1200 (Mochi's death).
            # Plan-create events go up to 700, CatSnapshot to 600.
            self.assertEqual(footer_row["final_tick"], 1200)
            self.assertEqual(
                footer_row["final_tick_source"],
                "derived_max_event_tick",
            )
            # Narrative should annotate the derivation so consumers know.
            self.assertIn("derived", env["narrative"])

    def test_run_summary_exposes_interrupts_top(self):
        with tempfile.TemporaryDirectory() as tmp:
            log_dir = write_bundle(Path(tmp))
            env = invoke(["run-summary", str(log_dir)])
            footer_row = next(r for r in env["results"] if r["kind"] == "footer")
            # Top entry is the 425 CriticalSafety preempt.
            top = footer_row["interrupts_by_reason_top"]
            self.assertTrue(top, "interrupts_by_reason_top should be populated")
            self.assertEqual(top[0]["value"], 425)


class CatTimelinePaginationTests(unittest.TestCase):
    """`cat-timeline` paginates by default (50) and supports --summarize
    for cats with too-many events to enumerate."""

    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.log_dir = write_bundle(Path(cls.tmp.name))

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def test_default_limit_caps_results(self):
        env = invoke(["cat-timeline", str(self.log_dir), "Whiskers",
                      "--limit=2"])
        self.assertEqual(env["scan_stats"]["returned"], 2)
        self.assertTrue(env["scan_stats"]["more_available"])
        # `next` should suggest the summarize alternative when more is
        # available — the central UX fix from the cat-timeline gap.
        self.assertTrue(any("--summarize" in n for n in env["next"]))

    def test_limit_zero_returns_all(self):
        env = invoke(["cat-timeline", str(self.log_dir), "Whiskers",
                      "--limit=0"])
        # Bundle has at least 5 PlanCreated + 4 CatSnapshot + 1 Death
        # for Whiskers = 10, plus narrative line(s).
        self.assertGreaterEqual(env["scan_stats"]["returned"], 10)
        self.assertFalse(env["scan_stats"]["more_available"])

    def test_summarize_returns_aggregates(self):
        env = invoke(["cat-timeline", str(self.log_dir), "Whiskers",
                      "--summarize"])
        kinds = {r["kind"] for r in env["results"]}
        self.assertIn("event_type_count", kinds)
        self.assertIn("plan_create_cadence", kinds)
        self.assertIn("tick_span", kinds)
        cadence = next(r for r in env["results"]
                       if r["kind"] == "plan_create_cadence")
        # Whiskers' plans at 50/100/150/200/250 → all gaps = 50.
        self.assertEqual(cadence["avg_ticks_between"], 50.0)
        self.assertEqual(cadence["min_ticks_between"], 50)
        self.assertEqual(cadence["max_ticks_between"], 50)

    def test_summarize_flags_plan_churn(self):
        # Build a bundle where PlanCreated cadence is < 5 ticks — that's
        # the smoking gun for plan churn that cat-timeline summary
        # should annotate explicitly.
        with tempfile.TemporaryDirectory() as tmp:
            log_dir = Path(tmp) / "churn"
            log_dir.mkdir(parents=True)
            churn_events = [
                {"_header": True, "commit_hash": "x"},
                *[{"tick": t, "type": "PlanCreated", "cat": "Loop"}
                  for t in range(1000, 1100, 3)],  # cadence = 3 ticks
                {"_footer": True, "deaths_by_cause": {}},
            ]
            (log_dir / "events.jsonl").write_text(
                "\n".join(json.dumps(e) for e in churn_events) + "\n"
            )
            env = invoke(["cat-timeline", str(log_dir), "Loop",
                          "--summarize"])
            self.assertIn("plan-churn", env["narrative"])


class EnvelopeToTextTests(unittest.TestCase):
    """text output is human-readable and mentions the key fields."""

    def test_to_text_contains_narrative_and_next(self):
        env = Envelope(
            query={"subtool": "events"},
            scan_stats={"scanned": 10, "returned": 2,
                        "more_available": True, "narrow_by": ["cat"]},
            results=[{"id": "tick:1:Death:Simba", "summary": "starved"}],
            narrative="Two deaths.",
            next=["just q cat-timeline logs/x Simba"],
        )
        out = env.to_text()
        self.assertIn("scanned 10", out)
        self.assertIn("Two deaths.", out)
        self.assertIn("narrow by cat", out)
        self.assertIn("just q cat-timeline", out)


# ── ticket 417: Haiku enrichment ────────────────────────────────────────────

class EnvelopeSerializationDeterminismTests(unittest.TestCase):
    """Strict-presenter contract (per ticket 010): the LLM never mutates
    sim state, and the enrichment fields are absent when not populated.

    The CI audit is the byte-identical invariant: stripping `hint` and
    `next_reasoned` from an enriched envelope's JSON yields the
    pre-417 JSON. Same shape as 010's
    `rm -rf logs/biographies && just soak 42 → byte-identical events.jsonl`.
    """

    def _base_envelope(self) -> Envelope:
        return Envelope(
            query={"subtool": "deaths", "log_dir": "logs/tuned-42"},
            scan_stats={"scanned": 100, "returned": 2,
                        "more_available": False, "narrow_by": []},
            results=[
                {"id": "tick:1200045:Death:Cedar",
                 "summary": "tick=1200045 cause=Starvation"},
                {"id": "tick:1200090:Death:Heron",
                 "summary": "tick=1200090 cause=Starvation"},
            ],
            narrative="2 starvation deaths in seed 42.",
            next=["just q cat-timeline logs/tuned-42 Cedar"],
        )

    def test_unenriched_envelope_omits_enrichment_keys(self):
        """An envelope with `hint=None, next_reasoned=None` must serialize
        to JSON that does NOT contain those keys at all — preserves
        byte-identical compatibility with pre-417 envelopes."""
        env = self._base_envelope()
        data = json.loads(env.to_json())
        self.assertNotIn("hint", data)
        self.assertNotIn("next_reasoned", data)
        self.assertEqual(
            sorted(data.keys()),
            ["narrative", "next", "query", "results", "scan_stats"],
        )

    def test_enriched_minus_enrichment_equals_unenriched(self):
        """The strict-presenter CI audit. Build the same envelope twice,
        fill enrichment on one, and assert the JSON differs only in the
        two enrichment keys."""
        plain = self._base_envelope()
        enriched = self._base_envelope()
        enriched.hint = "Both deaths within 45 ticks — clustered, not steady drip."
        enriched.next_reasoned = {
            "status": "ok",
            "suggestions": [
                {"cmd": "just q deaths logs/tuned-42 --cause=Starvation",
                 "why": "2 starvation deaths in a 45-tick window"},
            ],
            "elapsed_ms": 1234,
            "model": "claude-haiku-4-5",
        }
        plain_json = json.loads(plain.to_json())
        enriched_json = json.loads(enriched.to_json())
        # Strip enrichment keys from the enriched version → must equal plain.
        stripped = {k: v for k, v in enriched_json.items()
                    if k not in ("hint", "next_reasoned")}
        self.assertEqual(stripped, plain_json)

    def test_enriched_serialization_includes_enrichment_keys(self):
        env = self._base_envelope()
        env.hint = "demographic cluster"
        env.next_reasoned = {"status": "ok", "suggestions": [], "elapsed_ms": 50}
        data = json.loads(env.to_json())
        self.assertEqual(data["hint"], "demographic cluster")
        self.assertEqual(data["next_reasoned"]["status"], "ok")

    def test_existing_next_field_never_mutated(self):
        """Pillar of the strict-presenter contract: even when enrichment
        is filled, the deterministic `next` list stays exactly as the
        subtool built it."""
        env = self._base_envelope()
        original_next = list(env.next)
        env.hint = "x"
        env.next_reasoned = {
            "status": "ok",
            "suggestions": [{"cmd": "just q deaths logs/x", "why": "y"}],
            "elapsed_ms": 0,
        }
        self.assertEqual(env.next, original_next)


class EnvelopeToTextEnrichmentTests(unittest.TestCase):
    """Text rendering of the enrichment fields."""

    def _enriched(self) -> Envelope:
        return Envelope(
            query={"subtool": "anomalies", "log_dir": "logs/tuned-42"},
            scan_stats={"scanned": 1, "returned": 1,
                        "more_available": False, "narrow_by": []},
            results=[{"id": "anomaly:starvation",
                      "summary": "starvation_deaths=2"}],
            narrative="2 starvation deaths.",
            next=["just q deaths logs/tuned-42"],
            hint="Looks demographic — both kittens.",
            next_reasoned={
                "status": "ok",
                "suggestions": [
                    {"cmd": "just q deaths logs/tuned-42 --cause=Starvation",
                     "why": "2 starvation deaths in a 45-tick window"},
                ],
                "elapsed_ms": 1234,
            },
        )

    def test_to_text_includes_hint_when_set(self):
        out = self._enriched().to_text()
        self.assertIn("hint:", out)
        self.assertIn("Looks demographic", out)

    def test_to_text_includes_reasoned_suggestions(self):
        out = self._enriched().to_text()
        self.assertIn("next (reasoned", out)
        self.assertIn("--cause=Starvation", out)
        self.assertIn("45-tick window", out)

    def test_to_text_omits_hint_when_none(self):
        env = self._enriched()
        env.hint = None
        out = env.to_text()
        self.assertNotIn("hint:", out)

    def test_to_text_omits_reasoned_when_no_suggestions(self):
        env = self._enriched()
        env.next_reasoned = {"status": "timeout", "suggestions": [],
                             "elapsed_ms": 8000}
        out = env.to_text()
        self.assertNotIn("next (reasoned", out)


class TruncateForHaikuTests(unittest.TestCase):
    def test_small_envelope_passes_through(self):
        small = {
            "query": {"subtool": "deaths"},
            "results": [{"id": "tick:1:x", "summary": "x"}],
            "narrative": "small",
        }
        out = logq_mod._truncate_for_haiku(small, max_bytes=10_000)
        self.assertIs(out, small)
        self.assertNotIn("_truncated", out)

    def test_large_envelope_drops_trailing_results(self):
        # 200 results, each ~50 bytes of JSON — well over 1KB.
        big = {
            "query": {"subtool": "events"},
            "results": [{"id": f"tick:{i}:E", "summary": f"event_{i}"}
                        for i in range(200)],
            "narrative": "many events",
        }
        out = logq_mod._truncate_for_haiku(big, max_bytes=1024)
        self.assertTrue(out.get("_truncated"))
        self.assertIn("_truncated_note", out)
        self.assertLess(len(out["results"]), 200)
        self.assertLessEqual(len(json.dumps(out, default=str)), 1024)

    def test_truncation_preserves_other_fields(self):
        big = {
            "query": {"subtool": "events"},
            "results": [{"id": f"x{i}", "summary": "x" * 100}
                        for i in range(50)],
            "narrative": "test",
            "scan_stats": {"scanned": 50, "returned": 50},
        }
        out = logq_mod._truncate_for_haiku(big, max_bytes=500)
        self.assertEqual(out["query"], big["query"])
        self.assertEqual(out["narrative"], "test")
        self.assertEqual(out["scan_stats"], big["scan_stats"])


class EnrichmentHookTests(unittest.TestCase):
    """`_enrich_envelope` mutates `env.hint` / `env.next_reasoned` only,
    never `env.next`. Excluded subtools and disabled flags short-circuit
    without calling the client."""

    def _args(self, subtool: str, *, enrich=False, no_enrich=False,
              enrich_timeout=8.0):
        import argparse
        ns = argparse.Namespace(
            subtool=subtool,
            enrich=enrich,
            no_enrich=no_enrich,
            enrich_timeout=enrich_timeout,
        )
        return ns

    def _envelope(self) -> Envelope:
        return Envelope(
            query={"subtool": "deaths", "log_dir": "logs/x"},
            scan_stats={"scanned": 1, "returned": 1,
                        "more_available": False, "narrow_by": []},
            results=[{"id": "a", "summary": "b"}],
            narrative="x",
            next=["just q cat-timeline logs/x A"],
        )

    def test_skipped_for_excluded_subtool(self):
        env = self._envelope()
        args = self._args("footer", enrich=True)
        status, elapsed = logq_mod._enrich_envelope(env, args)
        self.assertIsNone(status)
        self.assertIsNone(elapsed)
        self.assertIsNone(env.hint)
        self.assertIsNone(env.next_reasoned)

    def test_skipped_when_no_enrich_flag(self):
        env = self._envelope()
        args = self._args("deaths", enrich=True, no_enrich=True)
        status, _ = logq_mod._enrich_envelope(env, args)
        self.assertIsNone(status)
        self.assertIsNone(env.next_reasoned)

    def test_skipped_when_disabled(self):
        env = self._envelope()
        # Neither flag, no env var set → skipped.
        args = self._args("deaths")
        # Clear env to make sure LOGQ_ENRICH isn't leaking from the
        # test runner's shell.
        import unittest.mock as umock, os
        with umock.patch.dict(os.environ, {}, clear=True):
            status, _ = logq_mod._enrich_envelope(env, args)
        self.assertIsNone(status)

    def test_fills_envelope_on_successful_call(self):
        env = self._envelope()
        args = self._args("deaths", enrich=True)
        fake_meta = {"status": "ok", "elapsed_ms": 1500,
                     "stderr_tail": "", "model": "claude-haiku-4-5"}
        fake_parsed = {
            "hint": "cluster detected",
            "suggestions": [
                {"cmd": "just q deaths logs/x --cause=Starvation",
                 "why": "narrowing"},
            ],
        }
        import unittest.mock as umock
        with umock.patch.object(logq_mod, "call_haiku_json",
                                 return_value=(fake_parsed, fake_meta)):
            status, elapsed = logq_mod._enrich_envelope(env, args)
        self.assertEqual(status, "ok")
        self.assertEqual(elapsed, 1500)
        self.assertEqual(env.hint, "cluster detected")
        self.assertEqual(env.next_reasoned["status"], "ok")
        self.assertEqual(len(env.next_reasoned["suggestions"]), 1)
        # Existing `next` field is untouched.
        self.assertEqual(env.next, ["just q cat-timeline logs/x A"])

    def test_records_failure_status_without_filling_hint(self):
        env = self._envelope()
        args = self._args("anomalies", enrich=True)
        fake_meta = {"status": "timeout", "elapsed_ms": 8000,
                     "stderr_tail": "", "model": "claude-haiku-4-5"}
        import unittest.mock as umock
        with umock.patch.object(logq_mod, "call_haiku_json",
                                 return_value=(None, fake_meta)):
            status, elapsed = logq_mod._enrich_envelope(env, args)
        self.assertEqual(status, "timeout")
        self.assertEqual(elapsed, 8000)
        # Hint stays None when call failed.
        self.assertIsNone(env.hint)
        # next_reasoned carries the failure status discriminator.
        self.assertEqual(env.next_reasoned["status"], "timeout")
        self.assertEqual(env.next_reasoned["suggestions"], [])
        # Existing `next` field is still untouched.
        self.assertEqual(env.next, ["just q cat-timeline logs/x A"])


if __name__ == "__main__":
    unittest.main()
