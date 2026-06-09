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
        "kittens_matured": 0,
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


def make_checkpoint_block(aggregate=500.0, constant=50_000, **overrides):
    """Build a colony_score_at_checkpoint footer block."""
    block = make_block(aggregate=aggregate, **overrides)
    block["captured_at_elapsed_tick"] = constant + 13
    block["checkpoint_constant"] = constant
    return block


class TestCheckpointSurfaceSelection(unittest.TestCase):
    """TPS-invariant checkpoint surface preferred over end-of-run when
    both runs carry it at the same checkpoint constant."""

    def test_prefers_checkpoint_when_both_carry_it(self):
        baseline = {
            "colony_score": make_block(aggregate=1000.0),
            "colony_score_at_checkpoint": make_checkpoint_block(aggregate=500.0),
        }
        observed = {
            "colony_score": make_block(aggregate=700.0),  # end-of-run dropped 30%
            "colony_score_at_checkpoint": make_checkpoint_block(aggregate=510.0),
        }
        _, _, surface = verdict.select_colony_score_blocks(baseline, observed)
        self.assertEqual(surface, "checkpoint")
        rows = verdict.colony_score_drift(baseline, observed)
        # Checkpoint delta is +2% (pass) — the -30% end-of-run delta is
        # the TPS-confounded reading the checkpoint exists to avoid.
        self.assertEqual(rows["aggregate"]["band"], "pass")
        self.assertEqual(rows["aggregate"]["delta_pct"], 2.0)

    def test_falls_back_when_baseline_is_legacy(self):
        baseline = {"colony_score": make_block(aggregate=1000.0)}
        observed = {
            "colony_score": make_block(aggregate=950.0),
            "colony_score_at_checkpoint": make_checkpoint_block(),
        }
        _, _, surface = verdict.select_colony_score_blocks(baseline, observed)
        self.assertEqual(surface, "end_of_run")
        rows = verdict.colony_score_drift(baseline, observed)
        self.assertEqual(rows["aggregate"]["delta_pct"], -5.0)

    def test_falls_back_when_observed_checkpoint_null(self):
        # Run died (or was too short) before the checkpoint mark.
        baseline = {
            "colony_score": make_block(),
            "colony_score_at_checkpoint": make_checkpoint_block(),
        }
        observed = {
            "colony_score": make_block(),
            "colony_score_at_checkpoint": None,
        }
        _, _, surface = verdict.select_colony_score_blocks(baseline, observed)
        self.assertEqual(surface, "end_of_run")

    def test_falls_back_on_checkpoint_constant_mismatch(self):
        baseline = {
            "colony_score": make_block(),
            "colony_score_at_checkpoint": make_checkpoint_block(constant=50_000),
        }
        observed = {
            "colony_score": make_block(),
            "colony_score_at_checkpoint": make_checkpoint_block(constant=60_000),
        }
        _, _, surface = verdict.select_colony_score_blocks(baseline, observed)
        self.assertEqual(surface, "end_of_run")

    def test_checkpoint_drift_none_when_fallback_also_missing(self):
        baseline = {"colony_score_at_checkpoint": make_checkpoint_block()}
        observed = {"colony_score_at_checkpoint": None}
        self.assertIsNone(verdict.colony_score_drift(baseline, observed))


if __name__ == "__main__":
    unittest.main()
