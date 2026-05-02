"""Tests for `scripts/verdict.py`'s `colony_score_drift` channel (ticket 125).

Stdlib unittest, mirrors the pattern in `tests/logq/test_envelope.py`.
Invoke with `just test-verdict` or
`python3 tests/verdict/test_colony_score_drift.py -v`.
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

import verdict  # noqa: E402


def make_block(aggregate=1000.0, welfare=0.5, **overrides):
    """Build a colony_score footer block with sensible defaults."""
    block = {
        "aggregate": aggregate,
        "welfare": welfare,
        "shelter": 0.6,
        "nourishment": 0.9,
        "health": 1.0,
        "happiness": 0.6,
        "fulfillment": 0.4,
        "seasons_survived": 5,
        "peak_population": 8,
        "kittens_born": 0,
        "kittens_surviving": 0,
        "structures_built": 8,
        "bonds_formed": 3,
        "deaths_starvation": 0,
        "deaths_old_age": 0,
        "deaths_injury": 0,
    }
    block.update(overrides)
    return block


class TestColonyScoreBand(unittest.TestCase):
    def test_pass_band_at_5_pct_inclusive(self):
        self.assertEqual(verdict.colony_score_band(0.0), "pass")
        self.assertEqual(verdict.colony_score_band(5.0), "pass")
        self.assertEqual(verdict.colony_score_band(-5.0), "pass")

    def test_concern_band_above_5_through_15(self):
        self.assertEqual(verdict.colony_score_band(5.1), "concern")
        self.assertEqual(verdict.colony_score_band(10.0), "concern")
        self.assertEqual(verdict.colony_score_band(15.0), "concern")
        self.assertEqual(verdict.colony_score_band(-15.0), "concern")

    def test_fail_band_above_15(self):
        self.assertEqual(verdict.colony_score_band(15.1), "fail")
        self.assertEqual(verdict.colony_score_band(30.0), "fail")
        self.assertEqual(verdict.colony_score_band(-50.0), "fail")


class TestColonyScoreDrift(unittest.TestCase):
    def test_returns_none_when_baseline_lacks_block(self):
        baseline = {"_footer": True}  # no colony_score
        observed = {"colony_score": make_block()}
        self.assertIsNone(verdict.colony_score_drift(baseline, observed))

    def test_returns_none_when_observed_lacks_block(self):
        baseline = {"colony_score": make_block()}
        observed = {"_footer": True, "colony_score": None}
        self.assertIsNone(verdict.colony_score_drift(baseline, observed))

    def test_pass_when_within_5_pct(self):
        baseline = {"colony_score": make_block(aggregate=1000.0)}
        observed = {"colony_score": make_block(aggregate=1040.0)}  # +4.0%
        rows = verdict.colony_score_drift(baseline, observed)
        self.assertIsNotNone(rows)
        self.assertEqual(rows["aggregate"]["band"], "pass")
        self.assertEqual(rows["aggregate"]["delta_pct"], 4.0)

    def test_concern_at_10_pct(self):
        baseline = {"colony_score": make_block(aggregate=1000.0)}
        observed = {"colony_score": make_block(aggregate=1100.0)}  # +10.0%
        rows = verdict.colony_score_drift(baseline, observed)
        self.assertEqual(rows["aggregate"]["band"], "concern")

    def test_fail_at_30_pct(self):
        baseline = {"colony_score": make_block(aggregate=1000.0)}
        observed = {"colony_score": make_block(aggregate=700.0)}  # -30.0%
        rows = verdict.colony_score_drift(baseline, observed)
        self.assertEqual(rows["aggregate"]["band"], "fail")
        self.assertEqual(rows["aggregate"]["delta_pct"], -30.0)

    def test_zero_baseline_marks_new_nonzero(self):
        baseline = {"colony_score": make_block(kittens_born=0)}
        observed = {"colony_score": make_block(kittens_born=3)}
        rows = verdict.colony_score_drift(baseline, observed)
        self.assertEqual(rows["kittens_born"]["band"], "new-nonzero")
        self.assertIsNone(rows["kittens_born"]["delta_pct"])

    def test_zero_both_sides_is_pass_with_zero_delta(self):
        baseline = {"colony_score": make_block(kittens_born=0)}
        observed = {"colony_score": make_block(kittens_born=0)}
        rows = verdict.colony_score_drift(baseline, observed)
        self.assertEqual(rows["kittens_born"]["band"], "pass")
        self.assertEqual(rows["kittens_born"]["delta_pct"], 0.0)

    def test_skips_non_numeric_fields(self):
        baseline = {"colony_score": {"aggregate": "not-a-number", "welfare": 0.5}}
        observed = {"colony_score": {"aggregate": "still-not", "welfare": 0.55}}
        rows = verdict.colony_score_drift(baseline, observed)
        self.assertNotIn("aggregate", rows)
        self.assertEqual(rows["welfare"]["band"], "concern")  # +10%

    def test_all_fields_walk_through(self):
        # Smoke check: every spec'd field is processed when both sides are numeric.
        baseline = {"colony_score": make_block()}
        observed = {"colony_score": make_block()}
        rows = verdict.colony_score_drift(baseline, observed)
        for f in verdict.COLONY_SCORE_FIELDS:
            self.assertIn(f, rows, f"missing colony_score field {f}")


class TestDeriveOverallEscalation(unittest.TestCase):
    """Aggregate moving 30% with all canaries green should land as `concern`,
    not `pass` (the gap ticket 125 closes). Hard canaries still gate."""

    def test_aggregate_fail_band_escalates_pass_to_concern(self):
        cs_drift = {
            "aggregate": {"baseline": 1000.0, "observed": 700.0,
                          "delta_pct": -30.0, "band": "fail"},
            "welfare": {"baseline": 0.5, "observed": 0.45,
                        "delta_pct": -10.0, "band": "concern"},
        }
        result = verdict.derive_overall("pass", "pass", "clean", [], cs_drift)
        self.assertEqual(result, "concern")

    def test_aggregate_pass_keeps_overall_pass(self):
        cs_drift = {
            "aggregate": {"baseline": 1000.0, "observed": 1020.0,
                          "delta_pct": 2.0, "band": "pass"},
            "welfare": {"baseline": 0.5, "observed": 0.51,
                        "delta_pct": 2.0, "band": "pass"},
        }
        result = verdict.derive_overall("pass", "pass", "clean", [], cs_drift)
        self.assertEqual(result, "pass")

    def test_canary_survival_fail_dominates_clean_aggregate(self):
        cs_drift = {
            "aggregate": {"baseline": 1000.0, "observed": 1010.0,
                          "delta_pct": 1.0, "band": "pass"},
        }
        result = verdict.derive_overall("fail", "pass", "clean", [], cs_drift)
        self.assertEqual(result, "fail")

    def test_no_cs_drift_falls_back_to_legacy_logic(self):
        result = verdict.derive_overall("pass", "pass", "clean", [], None)
        self.assertEqual(result, "pass")


if __name__ == "__main__":
    unittest.main()
