"""Tests for `scripts/verdict.py`'s founder-dispersion floor (ticket 490).

Stdlib unittest, mirrors `tests/verdict/test_colony_score_drift.py`.
Invoke with `just test-verdict` or
`python3 tests/verdict/test_founder_dispersion.py -v`.
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

import verdict  # noqa: E402


def window(start, mean_dist, samples=30):
    return {
        "window_start_elapsed": start,
        "window_end_elapsed": start + 3_000,
        "mean_dist": mean_dist,
        "samples": samples,
    }


class TestFounderDispersionLow(unittest.TestCase):
    def test_none_when_field_absent(self):
        self.assertIsNone(verdict.founder_dispersion_low({"_footer": True}))
        self.assertIsNone(verdict.founder_dispersion_low({"founder_dispersion": None}))

    def test_healthy_run_flags_nothing(self):
        footer = {"founder_dispersion": [
            window(0, 1.3),       # spawn clump — skipped
            window(3_000, 24.8),
            window(6_000, 24.1),
        ]}
        self.assertEqual(verdict.founder_dispersion_low(footer), [])

    def test_cuddle_puddle_flags_post_spawn_windows(self):
        # The 490 regression shape: 1.1 / 4.8 / 4.7 tiles.
        footer = {"founder_dispersion": [
            window(0, 1.1),
            window(3_000, 4.8),
            window(6_000, 4.7),
        ]}
        flagged = verdict.founder_dispersion_low(footer)
        self.assertEqual(len(flagged), 2)
        self.assertEqual(flagged[0]["window_start_elapsed"], 3_000)

    def test_spawn_window_low_is_expected_not_flagged(self):
        footer = {"founder_dispersion": [window(0, 1.3)]}
        self.assertEqual(verdict.founder_dispersion_low(footer), [])

    def test_floor_boundary_is_exclusive(self):
        footer = {"founder_dispersion": [
            window(3_000, verdict.FOUNDER_DISPERSION_FLOOR_TILES),
        ]}
        self.assertEqual(verdict.founder_dispersion_low(footer), [])

    def test_flagged_rows_escalate_overall_to_concern(self):
        flagged = [window(3_000, 4.8)]
        result = verdict.derive_overall(
            "pass", "pass", "clean", [], None, None, [], None, flagged)
        self.assertEqual(result, "concern")

    def test_empty_and_none_do_not_escalate(self):
        for rows in ([], None):
            result = verdict.derive_overall(
                "pass", "pass", "clean", [], None, None, [], None, rows)
            self.assertEqual(result, "pass")


if __name__ == "__main__":
    unittest.main()
