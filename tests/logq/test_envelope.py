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


if __name__ == "__main__":
    unittest.main()
