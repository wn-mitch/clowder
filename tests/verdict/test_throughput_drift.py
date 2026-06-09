"""Tests for `scripts/verdict.py`'s `throughput_drift` channel (perf epic 480).

Stdlib unittest, mirrors `tests/verdict/test_colony_score_drift.py`.
Invoke with `just test-verdict` or
`python3 tests/verdict/test_throughput_drift.py -v`.
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

import verdict  # noqa: E402


class TestThroughputBand(unittest.TestCase):
    def test_pass_down_to_minus_15_inclusive(self):
        self.assertEqual(verdict.throughput_band(0.0), "pass")
        self.assertEqual(verdict.throughput_band(-15.0), "pass")

    def test_improvements_never_gate(self):
        self.assertEqual(verdict.throughput_band(50.0), "pass")
        self.assertEqual(verdict.throughput_band(200.0), "pass")

    def test_concern_between_15_and_40(self):
        self.assertEqual(verdict.throughput_band(-15.1), "concern")
        self.assertEqual(verdict.throughput_band(-40.0), "concern")

    def test_strong_concern_below_40(self):
        self.assertEqual(verdict.throughput_band(-40.1), "strong-concern")
        self.assertEqual(verdict.throughput_band(-63.0), "strong-concern")


class TestThroughputDrift(unittest.TestCase):
    def test_prefers_ticks_per_sec_when_both_present(self):
        baseline = {"ticks_per_sec": 150.0, "elapsed_ticks": 135_000}
        observed = {"ticks_per_sec": 75.0, "elapsed_ticks": 67_500}
        row = verdict.throughput_drift(baseline, observed, 900, 900)
        self.assertIsNotNone(row)
        self.assertEqual(row["metric"], "ticks_per_sec")
        self.assertEqual(row["delta_pct"], -50.0)
        self.assertEqual(row["band"], "strong-concern")

    def test_elapsed_ticks_fallback_for_legacy_baseline(self):
        baseline = {"elapsed_ticks": 100_000}  # pre-480 footer, no ticks_per_sec
        observed = {"ticks_per_sec": 80.0, "elapsed_ticks": 80_000}
        row = verdict.throughput_drift(baseline, observed, 900, 900)
        self.assertIsNotNone(row)
        self.assertEqual(row["metric"], "elapsed_ticks")
        self.assertEqual(row["delta_pct"], -20.0)
        self.assertEqual(row["band"], "concern")

    def test_incomparable_when_durations_differ_and_no_tps(self):
        baseline = {"elapsed_ticks": 100_000}
        observed = {"elapsed_ticks": 8_000}
        self.assertIsNone(verdict.throughput_drift(baseline, observed, 900, 60))

    def test_incomparable_when_duration_missing(self):
        baseline = {"elapsed_ticks": 100_000}
        observed = {"elapsed_ticks": 90_000}
        self.assertIsNone(verdict.throughput_drift(baseline, observed, None, 900))

    def test_tps_used_even_across_duration_mismatch(self):
        # ticks_per_sec is duration-invariant, so a 60s perf probe can
        # compare against a 900s baseline.
        baseline = {"ticks_per_sec": 140.0}
        observed = {"ticks_per_sec": 141.0}
        row = verdict.throughput_drift(baseline, observed, 900, 60)
        self.assertIsNotNone(row)
        self.assertEqual(row["metric"], "ticks_per_sec")
        self.assertEqual(row["band"], "pass")

    def test_zero_or_missing_values_are_incomparable(self):
        self.assertIsNone(verdict.throughput_drift({}, {}, 900, 900))
        self.assertIsNone(
            verdict.throughput_drift({"ticks_per_sec": 0.0},
                                     {"ticks_per_sec": 100.0}, 900, 900))

    def test_improvement_reports_pass(self):
        baseline = {"ticks_per_sec": 72.0}
        observed = {"ticks_per_sec": 150.0}
        row = verdict.throughput_drift(baseline, observed, 900, 900)
        self.assertEqual(row["band"], "pass")
        self.assertGreater(row["delta_pct"], 100.0)


class TestDeriveOverallEscalation(unittest.TestCase):
    def _overall(self, throughput):
        return verdict.derive_overall(
            "pass", "pass", "clean", [], None, None, [], throughput)

    def test_concern_band_escalates_pass_to_concern(self):
        self.assertEqual(
            self._overall({"band": "concern", "delta_pct": -20.0}), "concern")

    def test_strong_concern_band_escalates(self):
        self.assertEqual(
            self._overall({"band": "strong-concern", "delta_pct": -50.0}),
            "concern")

    def test_pass_band_does_not_escalate(self):
        self.assertEqual(
            self._overall({"band": "pass", "delta_pct": -3.0}), "pass")

    def test_none_does_not_escalate(self):
        self.assertEqual(self._overall(None), "pass")

    def test_never_escalates_past_concern(self):
        # Even a catastrophic single-run dip stays "concern" — survival
        # canaries are the only path to "fail".
        self.assertEqual(
            self._overall({"band": "strong-concern", "delta_pct": -90.0}),
            "concern")

    def test_survival_fail_dominates(self):
        self.assertEqual(
            verdict.derive_overall(
                "fail", "pass", "clean", [], None, None, [],
                {"band": "pass", "delta_pct": 0.0}),
            "fail")


if __name__ == "__main__":
    unittest.main()
