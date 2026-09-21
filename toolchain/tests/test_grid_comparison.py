"""The metadata-only comparison must preserve the reviewed S1.1 experiment."""

import json
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))

from toolchain.build.compare_grids import compare_grids


class GridComparisonTests(unittest.TestCase):
    def test_frozen_calibration_metrics(self):
        fixture = json.loads((ROOT / "toolchain/fixtures/calibration.json").read_text())
        expected = json.loads((ROOT / "doc/experiments/data/grid-comparison.json").read_text())
        actual = compare_grids(fixture)
        # 2.1 removes optional 2.0 catalog fields; occupancy and payload metrics stay reviewed.
        for new, old in zip(actual, expected, strict=True):
            self.assertGreater(new.pop("catalog_bytes"), 0)
            old.pop("catalog_bytes")
        self.assertEqual(actual, expected)


if __name__ == "__main__":
    unittest.main()
