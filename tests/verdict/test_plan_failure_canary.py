"""Tests for `scripts/verdict.py`'s `plan_failure_canary` channel (ticket 396).

Stdlib unittest, mirrors the pattern in `tests/verdict/test_colony_score_drift.py`.
Invoke via `just test-verdict` or
`python3 tests/verdict/test_plan_failure_canary.py -v`.

The canary surfaces a class of regression the existing footer_drift channel
can't see: nested-dict counts under `plan_failures_by_reason` etc. The 394
Wean failure jump (0 -> 2439 over ~125000 ticks, no deaths, welfare actually
improved) was the seed case — survival + continuity + colony-score all
passed, but the substrate was visibly dirty in the trace.
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

import verdict  # noqa: E402


BASELINE_DUR = 125_000
OBSERVED_DUR = 125_000


def footer_with(**dicts: dict) -> dict:
    """Build a minimal footer carrying the requested by-reason dicts."""
    base: dict = {"_footer": True}
    base.update(dicts)
    return base


class TestPlanFailureCanaryNewKey(unittest.TestCase):
    """New-vs-baseline keys flagged when observed rate crosses the floor."""

    def test_394_wean_regression_flags(self):
        # The seed case: Wean: 2439 vs baseline 0 over ~125000 ticks
        # (= 0.0195/tick, well above the 0.005/tick new-key floor).
        baseline = footer_with(plan_failures_by_reason={})
        observed = footer_with(plan_failures_by_reason={
            "Wean: no dependent kitten in range/band": 2439,
        })
        rows = verdict.plan_failure_canary(
            baseline, observed, BASELINE_DUR, OBSERVED_DUR)
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["band"], "new-high-rate")
        self.assertEqual(rows[0]["reason"], "Wean: no dependent kitten in range/band")
        self.assertEqual(rows[0]["dict"], "plan_failures_by_reason")
        self.assertEqual(rows[0]["baseline"], 0)
        self.assertEqual(rows[0]["observed"], 2439)
        self.assertIsNone(rows[0]["ratio"])

    def test_new_key_below_floor_skipped(self):
        # 100 / 125000 = 0.0008/tick — below the 0.005/tick floor.
        baseline = footer_with(plan_failures_by_reason={})
        observed = footer_with(plan_failures_by_reason={
            "TravelTo: stuck": 100,
        })
        rows = verdict.plan_failure_canary(
            baseline, observed, BASELINE_DUR, OBSERVED_DUR)
        self.assertEqual(rows, [])

    def test_new_key_at_exactly_floor_flags(self):
        # 0.005/tick * 125000 = 625 observations.
        baseline = footer_with(plan_failures_by_reason={})
        observed = footer_with(plan_failures_by_reason={
            "X: edge case": 625,
        })
        rows = verdict.plan_failure_canary(
            baseline, observed, BASELINE_DUR, OBSERVED_DUR)
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["band"], "new-high-rate")


class TestPlanFailureCanaryRatio(unittest.TestCase):
    """Existing keys flagged when ratio crosses threshold AND observed rate
    crosses the absolute floor (avoids noise on rare keys)."""

    def test_10x_jump_with_floor_passed_flags(self):
        # Baseline 25 -> Observed 250 = 10x exact. Observed rate
        # 0.002/tick > 0.001/tick floor.
        baseline = footer_with(plan_failures_by_reason={
            "EngagePrey: lost prey": 25,
        })
        observed = footer_with(plan_failures_by_reason={
            "EngagePrey: lost prey": 250,
        })
        rows = verdict.plan_failure_canary(
            baseline, observed, BASELINE_DUR, OBSERVED_DUR)
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["band"], "high-rate-ratio")
        self.assertEqual(rows[0]["ratio"], 10.0)

    def test_below_threshold_skipped(self):
        # 5x jump — under the 10x threshold.
        baseline = footer_with(plan_failures_by_reason={
            "EngagePrey: lost prey": 100,
        })
        observed = footer_with(plan_failures_by_reason={
            "EngagePrey: lost prey": 500,
        })
        rows = verdict.plan_failure_canary(
            baseline, observed, BASELINE_DUR, OBSERVED_DUR)
        self.assertEqual(rows, [])

    def test_ratio_above_but_observed_below_floor_skipped(self):
        # 20x jump (1 -> 20), but 20/125000 = 0.00016/tick — under the
        # 0.001/tick floor. Avoids flagging when a baseline of 1 quirks up.
        baseline = footer_with(plan_failures_by_reason={
            "X: rare": 1,
        })
        observed = footer_with(plan_failures_by_reason={
            "X: rare": 20,
        })
        rows = verdict.plan_failure_canary(
            baseline, observed, BASELINE_DUR, OBSERVED_DUR)
        self.assertEqual(rows, [])

    def test_observed_zero_skipped(self):
        # Reason existed in baseline, fully resolved in observed - not a
        # plan-failure regression.
        baseline = footer_with(plan_failures_by_reason={"X": 500})
        observed = footer_with(plan_failures_by_reason={"X": 0})
        rows = verdict.plan_failure_canary(
            baseline, observed, BASELINE_DUR, OBSERVED_DUR)
        self.assertEqual(rows, [])


class TestPlanFailureCanaryMultiDict(unittest.TestCase):
    """Scans all three dicts in `PLAN_FAILURE_DICTS`."""

    def test_planning_failures_dict_also_scanned(self):
        baseline = footer_with(planning_failures_by_reason={})
        observed = footer_with(planning_failures_by_reason={
            "Foraging:GoalUnreachable": 1000,  # 0.008/tick, above floor
        })
        rows = verdict.plan_failure_canary(
            baseline, observed, BASELINE_DUR, OBSERVED_DUR)
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["dict"], "planning_failures_by_reason")

    def test_interrupts_dict_also_scanned(self):
        baseline = footer_with(interrupts_by_reason={})
        observed = footer_with(interrupts_by_reason={
            "ShadowFoxNearby": 1000,
        })
        rows = verdict.plan_failure_canary(
            baseline, observed, BASELINE_DUR, OBSERVED_DUR)
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["dict"], "interrupts_by_reason")


class TestPlanFailureCanaryOrdering(unittest.TestCase):
    """Ratio rows precede new-key rows; within ratio, descending; within
    new-key, descending by rate."""

    def test_ratio_row_before_new_key_row(self):
        baseline = footer_with(plan_failures_by_reason={
            "Old: jumped": 50,
        })
        observed = footer_with(plan_failures_by_reason={
            "Old: jumped": 1000,  # ratio 20x, rate 0.008/tick
            "New: appeared": 2000,  # rate 0.016/tick
        })
        rows = verdict.plan_failure_canary(
            baseline, observed, BASELINE_DUR, OBSERVED_DUR)
        self.assertEqual(len(rows), 2)
        self.assertEqual(rows[0]["band"], "high-rate-ratio")
        self.assertEqual(rows[1]["band"], "new-high-rate")

    def test_higher_ratio_first_within_band(self):
        baseline = footer_with(plan_failures_by_reason={
            "Small": 25,
            "Bigger": 25,
        })
        observed = footer_with(plan_failures_by_reason={
            "Small": 500,    # 20x
            "Bigger": 1500,  # 60x
        })
        rows = verdict.plan_failure_canary(
            baseline, observed, BASELINE_DUR, OBSERVED_DUR)
        self.assertEqual(len(rows), 2)
        self.assertEqual(rows[0]["reason"], "Bigger")
        self.assertEqual(rows[1]["reason"], "Small")


class TestPlanFailureCanaryEdgeCases(unittest.TestCase):
    def test_missing_durations_returns_empty(self):
        baseline = footer_with(plan_failures_by_reason={})
        observed = footer_with(plan_failures_by_reason={"X": 5000})
        self.assertEqual(verdict.plan_failure_canary(
            baseline, observed, None, OBSERVED_DUR), [])
        self.assertEqual(verdict.plan_failure_canary(
            baseline, observed, BASELINE_DUR, None), [])

    def test_missing_dicts_skipped_gracefully(self):
        baseline = footer_with()
        observed = footer_with()
        self.assertEqual(verdict.plan_failure_canary(
            baseline, observed, BASELINE_DUR, OBSERVED_DUR), [])

    def test_non_dict_field_skipped(self):
        baseline = footer_with(plan_failures_by_reason="not a dict")
        observed = footer_with(plan_failures_by_reason={"X": 5000})
        self.assertEqual(verdict.plan_failure_canary(
            baseline, observed, BASELINE_DUR, OBSERVED_DUR), [])


class TestDeriveOverallEscalation(unittest.TestCase):
    """A flagged plan-failure canary row escalates `pass` to `concern`;
    higher tiers stay primary."""

    def test_canary_row_escalates_pass_to_concern(self):
        rows = [{"dict": "plan_failures_by_reason", "reason": "X",
                 "baseline": 0, "observed": 2439,
                 "rate_baseline": 0.0, "rate_observed": 0.0195,
                 "ratio": None, "band": "new-high-rate"}]
        result = verdict.derive_overall(
            "pass", "pass", "clean", [], None, None, rows)
        self.assertEqual(result, "concern")

    def test_canary_empty_leaves_pass(self):
        result = verdict.derive_overall(
            "pass", "pass", "clean", [], None, None, [])
        self.assertEqual(result, "pass")

    def test_survival_fail_dominates_canary(self):
        rows = [{"band": "new-high-rate"}]  # truthy
        result = verdict.derive_overall(
            "fail", "pass", "clean", [], None, None, rows)
        self.assertEqual(result, "fail")

    def test_continuity_fail_stays_concern_with_canary(self):
        rows = [{"band": "new-high-rate"}]
        result = verdict.derive_overall(
            "pass", "fail", "clean", [], None, None, rows)
        self.assertEqual(result, "concern")

    def test_legacy_call_without_canary_arg_still_works(self):
        # Existing call sites pass six positional args (no canary).
        result = verdict.derive_overall(
            "pass", "pass", "clean", [], None, None)
        self.assertEqual(result, "pass")


if __name__ == "__main__":
    unittest.main()
